//! Fetch + parse + LLM-fallback orchestrator for recipe URL import.

use anyhow::{Context, Result};
use manor_core::recipe::import::{extract_via_llm, parse_jsonld, ImportedRecipe, LlmClient};
use manor_core::recipe::{ImportMethod, RecipeDraft};
use reqwest::header::CONTENT_TYPE;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_BODY_BYTES: u64 = 2 * 1024 * 1024;
const FETCH_TIMEOUT_SECS: u64 = 10;
const USER_AGENT_STRING: &str = "Manor/0.4 (+https://manor.app)";

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportPreview {
    pub recipe_draft: RecipeDraft,
    pub import_method: ImportMethod,
    pub parse_notes: Vec<String>,
    pub hero_image_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("URL is not a valid web address")]
    BadUrl,
    #[error("couldn't reach that URL")]
    FetchFailed,
    #[error("that URL isn't a web page (not html, content-type: {0})")]
    NotHtml(String),
    #[error("page too large to import")]
    TooLarge,
    #[error("couldn't extract a recipe from this page")]
    ExtractionFailed,
}

/// Fetch + parse a recipe URL. Returns a preview the frontend shows the user.
///
/// If `llm_client` is None, the LLM fallback is skipped — callers that only
/// exercise the JSON-LD path (or that haven't wired a live model) pass `None`.
pub async fn preview(
    url: &str,
    llm_client: Option<&dyn LlmClient>,
) -> Result<ImportPreview> {
    let vetted = manor_core::net::ssrf::vet_url(url).map_err(|_| ImportError::BadUrl)?;
    preview_inner(vetted, llm_client).await
}

/// Run a preview against a pre-vetted URL. Test-only entry point: wiremock binds
/// to 127.0.0.1 which the SSRF guard correctly blocks, so integration tests that
/// exercise content-type, JSON-LD, or LLM-fallback behaviour (not SSRF) bypass
/// the guard by calling this directly.
#[doc(hidden)]
pub async fn preview_inner(
    parsed_url: url::Url,
    llm_client: Option<&dyn LlmClient>,
) -> Result<ImportPreview> {
    let url = parsed_url.as_str().to_string();
    let host = parsed_url.host_str().unwrap_or("").to_string();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .user_agent(USER_AGENT_STRING)
        .build()
        .context("building http client")?;

    let resp = client
        .get(parsed_url.clone())
        .send()
        .await
        .map_err(|_| ImportError::FetchFailed)?;

    let ctype = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !ctype.contains("text/html") {
        return Err(ImportError::NotHtml(ctype).into());
    }
    if let Some(len) = resp.content_length() {
        if len > MAX_BODY_BYTES {
            return Err(ImportError::TooLarge.into());
        }
    }

    let body = resp
        .text()
        .await
        .map_err(|_| ImportError::FetchFailed)?;
    if body.len() as u64 > MAX_BODY_BYTES {
        return Err(ImportError::TooLarge.into());
    }

    // Try JSON-LD first — zero network cost, most recipe sites embed it.
    if let Some(mut imp) = parse_jsonld(&body) {
        imp.source_url = url.clone();
        imp.source_host = host.clone();
        return Ok(to_preview(imp));
    }

    // LLM fallback — only if a client was provided.
    let Some(client) = llm_client else {
        return Err(ImportError::ExtractionFailed.into());
    };
    let text = strip_html(&body);
    let mut imp = extract_via_llm(&text, client, /*via_remote=*/ false)
        .await
        .map_err(|_| ImportError::ExtractionFailed)?;
    imp.source_url = url;
    imp.source_host = host;
    Ok(to_preview(imp))
}

/// Strip HTML tags and collapse whitespace to produce plain text for the LLM.
fn strip_html(html: &str) -> String {
    let doc = scraper::Html::parse_document(html);
    let body_sel = scraper::Selector::parse("body").unwrap();
    let mut text = String::new();
    for el in doc.select(&body_sel) {
        for node in el.text() {
            text.push_str(node);
            text.push(' ');
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Hero image staging ────────────────────────────────────────────────────────

const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

struct HeroImageData {
    bytes: Vec<u8>,
    mime_type: String,
    original_name: String,
}

/// Fetch hero image bytes from a URL. Pure async HTTP — no DB, no lock.
async fn download_hero_image(url: &str) -> Result<HeroImageData> {
    let vetted = manor_core::net::ssrf::vet_url(url)
        .context("hero image URL rejected by SSRF guard")?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .user_agent(USER_AGENT_STRING)
        .build()
        .context("building http client for image")?;

    let resp = client.get(vetted).send().await.context("fetching hero image")?;

    let ctype = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let ext = if ctype.contains("jpeg") || ctype.contains("jpg") {
        "jpg"
    } else if ctype.contains("png") {
        "png"
    } else if ctype.contains("webp") {
        "webp"
    } else {
        return Err(anyhow::anyhow!("unsupported image type: {}", ctype));
    };

    if let Some(len) = resp.content_length() {
        if len > MAX_IMAGE_BYTES {
            return Err(anyhow::anyhow!("hero image too large ({} bytes)", len));
        }
    }

    let raw = resp.bytes().await.context("reading image bytes")?;
    if raw.len() as u64 > MAX_IMAGE_BYTES {
        return Err(anyhow::anyhow!("hero image too large ({} bytes)", raw.len()));
    }

    Ok(HeroImageData {
        bytes: raw.to_vec(),
        mime_type: ctype,
        original_name: format!("hero.{}", ext),
    })
}

/// Write a pre-fetched image to disk + DB as a staged attachment
/// (`entity_type='recipe'`, `entity_id=NULL`). Synchronous — call with the
/// lock held only for this duration.
///
/// Returns the attachment's `uuid` (TEXT), not the integer row id, because the
/// recipe table links via `hero_attachment_uuid TEXT` to avoid the
/// INTEGER/TEXT mismatch that existed in the old `attachment.entity_id` column.
fn store_hero_attachment(
    conn: &Connection,
    data: &HeroImageData,
    attachments_dir: &Path,
) -> Result<String> {
    let att = manor_core::attachment::store(
        conn,
        attachments_dir,
        &data.bytes,
        &data.original_name,
        &data.mime_type,
        Some("recipe"),
        None, // entity_id stays NULL; linkage is via recipe.hero_attachment_uuid
    )?;
    Ok(att.uuid)
}

/// Download + stage a hero image, returning the new attachment's `uuid`.
/// Splits async HTTP (no lock) from sync DB write (brief lock scope).
pub async fn stage_hero_image(
    db: &Arc<Mutex<Connection>>,
    url: &str,
    attachments_dir: &Path,
) -> Result<String> {
    // Phase 1: async HTTP fetch — no lock held.
    let data = download_hero_image(url).await?;

    // Phase 2: sync DB write — acquire lock, store, release.
    let conn = db.lock().map_err(|e| anyhow::anyhow!("db lock: {e}"))?;
    store_hero_attachment(&conn, &data, attachments_dir)
}

/// Called from `recipe_import_commit`: download + stage the hero image, then
/// set `recipe.hero_attachment_uuid` to point at the staged attachment's uuid.
/// Soft-fails: a missing image never blocks a successful recipe save.
///
/// Design: we do NOT call `attachment::link_to_entity` here. That helper would
/// write a TEXT recipe UUID into `attachment.entity_id INTEGER`, causing a type
/// mismatch. Instead, `recipe.hero_attachment_uuid TEXT` is the only linkage,
/// and the orphan sweep excludes attachments whose uuid appears in that column.
///
/// The lock is never held across `.await` — HTTP runs lock-free; DB writes
/// happen in two brief sync bursts (store then update recipe).
pub async fn fetch_and_link_hero_arc(
    db: Arc<Mutex<Connection>>,
    recipe_id: &str,
    image_url: Option<&str>,
    attachments_dir: &Path,
) -> Result<()> {
    let Some(url) = image_url else {
        return Ok(());
    };

    let att_uuid = match stage_hero_image(&db, url, attachments_dir).await {
        Ok(uuid) => uuid,
        Err(_) => return Ok(()), // soft-fail: recipe already saved
    };

    // Set recipe.hero_attachment_uuid — brief sync lock.
    let conn = db.lock().map_err(|e| anyhow::anyhow!("db lock: {e}"))?;
    let _ = manor_core::recipe::dal::set_hero_attachment(&conn, recipe_id, &att_uuid);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn preview_rejects_loopback_url() {
        let err = preview("http://127.0.0.1:8080/recipe", None).await.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("URL") || msg.contains("private") || msg.contains("scheme") || msg.contains("rejected"),
            "got error message: {msg}",
        );
    }

    #[tokio::test]
    async fn preview_rejects_file_scheme() {
        let err = preview("file:///etc/passwd", None).await.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("URL") || msg.contains("scheme"),
            "got error message: {msg}",
        );
    }
}

fn to_preview(imp: ImportedRecipe) -> ImportPreview {
    let hero = imp.hero_image_url.clone();
    let notes = imp.parse_notes.clone();
    let method = imp.import_method.clone();
    let draft = RecipeDraft {
        title: imp.title,
        servings: imp.servings,
        prep_time_mins: imp.prep_time_mins,
        cook_time_mins: imp.cook_time_mins,
        instructions: imp.instructions,
        source_url: Some(imp.source_url),
        source_host: Some(imp.source_host),
        import_method: imp.import_method,
        ingredients: imp.ingredients,
        // hero_attachment_uuid is set after commit once the image is staged;
        // it is not known at preview time.
        hero_attachment_uuid: None,
    };
    ImportPreview {
        recipe_draft: draft,
        import_method: method,
        parse_notes: notes,
        hero_image_url: hero,
    }
}
