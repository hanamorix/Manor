//! Fetch + parse + LLM-fallback orchestrator for recipe URL import.

use anyhow::{Context, Result};
use manor_core::recipe::import::{extract_via_llm, parse_jsonld, ImportedRecipe, LlmClient};
use manor_core::recipe::{ImportMethod, RecipeDraft};
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
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
    let parsed_url = reqwest::Url::parse(url).map_err(|_| ImportError::BadUrl)?;
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
        imp.source_url = url.to_string();
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
    imp.source_url = url.to_string();
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
    };
    ImportPreview {
        recipe_draft: draft,
        import_method: method,
        parse_notes: notes,
        hero_image_url: hero,
    }
}
