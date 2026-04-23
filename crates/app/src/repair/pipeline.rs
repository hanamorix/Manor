//! Repair-lookup pipeline orchestrator (L4d).

use super::fetch::fetch_and_trim;
use super::search::{duckduckgo_top_n, youtube_top_n};
use super::synth::{ClaudeSynth, OllamaSynth, PageExcerpt, SynthBackend, SynthInput};
use anyhow::{anyhow, Result};
use manor_core::repair::{LlmTier, RepairNote, RepairNoteDraft, RepairSource};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub const USER_AGENT: &str = "Manor/0.4 (+https://manor.app)";
pub const HTTP_TIMEOUT_SECS: u64 = 10;
pub const DDG_TOP_N: usize = 3;
pub const YOUTUBE_TOP_N: usize = 2;
pub const MIN_SYNTH_BODY_CHARS: usize = 50;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierRequest {
    Ollama,
    Claude,
}

impl TierRequest {
    fn as_persist_tier(self) -> LlmTier {
        match self {
            TierRequest::Ollama => LlmTier::Ollama,
            TierRequest::Claude => LlmTier::Claude,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineOutcome {
    pub note: Option<RepairNote>,
    pub sources: Vec<RepairSource>,
    pub video_sources: Vec<RepairSource>,
    pub empty_or_failed: bool,
}

/// Pure helper — builds the query string sent to DDG/YouTube.
pub fn build_augmented_query(make: Option<&str>, model: Option<&str>, symptom: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(m) = make {
        if !m.trim().is_empty() {
            parts.push(m.trim());
        }
    }
    if let Some(m) = model {
        if !m.trim().is_empty() {
            parts.push(m.trim());
        }
    }
    parts.push(symptom.trim());
    parts.join(" ")
}

pub async fn run_repair_search(
    db: Arc<Mutex<rusqlite::Connection>>,
    asset_id: String,
    symptom: String,
    tier: TierRequest,
) -> Result<PipelineOutcome> {
    let backend: Box<dyn SynthBackend> = match tier {
        TierRequest::Ollama => {
            let model = {
                let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
                crate::assistant::ollama::resolve_model(&conn)
            };
            Box::new(OllamaSynth::new(model))
        }
        TierRequest::Claude => Box::new(ClaudeSynth { db: db.clone() }),
    };
    run_pipeline_with_backend(db, asset_id, symptom, tier, backend.as_ref()).await
}

pub(crate) async fn run_pipeline_with_backend(
    db: Arc<Mutex<rusqlite::Connection>>,
    asset_id: String,
    symptom: String,
    tier: TierRequest,
    backend: &dyn SynthBackend,
) -> Result<PipelineOutcome> {
    // 1. Load asset (synchronous lock scope).
    let (asset_name, asset_make, asset_model, asset_category) = {
        let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
        let asset = manor_core::asset::dal::get_asset(&conn, &asset_id)?
            .ok_or_else(|| anyhow!("Asset not found"))?;
        (
            asset.name,
            asset.make,
            asset.model,
            asset.category.as_str().to_string(),
        )
    };
    let query = build_augmented_query(asset_make.as_deref(), asset_model.as_deref(), &symptom);

    // 2. Build shared HTTP client.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| anyhow!("http client build: {e}"))?;

    // 3. Fire DDG + YouTube concurrently.
    let (ddg_res, yt_res) = tokio::join!(
        duckduckgo_top_n(&client, &query, DDG_TOP_N),
        youtube_top_n(&client, &query, YOUTUBE_TOP_N),
    );
    let ddg = ddg_res.unwrap_or_default();
    let yt = yt_res.unwrap_or_default();

    // 4. Fetch + trim each DDG URL concurrently.
    let fetches = ddg
        .iter()
        .map(|src| {
            let client = client.clone();
            let url = src.url.clone();
            let title = src.title.clone();
            async move {
                let txt = fetch_and_trim(&client, &url).await?;
                Ok::<PageExcerpt, anyhow::Error>(PageExcerpt {
                    url,
                    title,
                    trimmed_text: txt,
                })
            }
        })
        .collect::<Vec<_>>();
    let results = futures_util::future::join_all(fetches).await;
    let pages: Vec<PageExcerpt> = results.into_iter().filter_map(|r| r.ok()).collect();

    if pages.is_empty() {
        return Ok(PipelineOutcome {
            note: None,
            sources: ddg,
            video_sources: yt,
            empty_or_failed: true,
        });
    }

    // 5. Synthesise.
    let input = SynthInput {
        asset_name: &asset_name,
        asset_make: asset_make.as_deref(),
        asset_model: asset_model.as_deref(),
        asset_category: &asset_category,
        symptom: &symptom,
        augmented_query: &query,
        pages: &pages,
    };
    let body_text = match backend.synth(&input).await {
        Ok(t) => t,
        Err(_) => {
            return Ok(PipelineOutcome {
                note: None,
                sources: ddg,
                video_sources: yt,
                empty_or_failed: true,
            });
        }
    };
    if body_text.trim().len() < MIN_SYNTH_BODY_CHARS {
        return Ok(PipelineOutcome {
            note: None,
            sources: ddg,
            video_sources: yt,
            empty_or_failed: true,
        });
    }

    // 6. Persist.
    let video_sources = if yt.is_empty() {
        None
    } else {
        Some(yt.clone())
    };
    let draft = RepairNoteDraft {
        asset_id: asset_id.clone(),
        symptom: symptom.clone(),
        body_md: body_text,
        sources: ddg.clone(),
        video_sources,
        tier: tier.as_persist_tier(),
    };
    let note = {
        let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
        let id = manor_core::repair::dal::insert_repair_note(&conn, &draft)?;
        manor_core::repair::dal::get_repair_note(&conn, &id)?
            .ok_or_else(|| anyhow!("inserted row missing"))?
    };

    Ok(PipelineOutcome {
        note: Some(note),
        sources: ddg,
        video_sources: yt,
        empty_or_failed: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use manor_core::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use manor_core::assistant::db;

    struct StubSynth {
        response: std::result::Result<String, String>,
    }

    #[async_trait]
    impl SynthBackend for StubSynth {
        async fn synth(&self, _input: &SynthInput<'_>) -> Result<String> {
            match &self.response {
                Ok(s) => Ok(s.clone()),
                Err(e) => Err(anyhow!(e.clone())),
            }
        }
    }

    fn fresh_db_with_asset() -> (tempfile::TempDir, Arc<Mutex<rusqlite::Connection>>, String) {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let asset = AssetDraft {
            name: "Test Boiler".into(),
            category: AssetCategory::Appliance,
            make: Some("Worcester".into()),
            model: Some("B8000".into()),
            serial_number: None,
            purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        };
        let asset_id = asset_dal::insert_asset(&conn, &asset).unwrap();
        (dir, Arc::new(Mutex::new(conn)), asset_id)
    }

    #[test]
    fn build_query_make_plus_model_plus_symptom() {
        let q = build_augmented_query(Some("Worcester"), Some("B8000"), "won't fire up");
        assert_eq!(q, "Worcester B8000 won't fire up");
    }

    #[test]
    fn build_query_missing_make_uses_model_and_symptom() {
        let q = build_augmented_query(None, Some("B8000"), "won't fire up");
        assert_eq!(q, "B8000 won't fire up");
    }

    #[test]
    fn build_query_missing_both_returns_raw_symptom() {
        let q = build_augmented_query(None, None, "won't fire up");
        assert_eq!(q, "won't fire up");
    }

    #[test]
    fn build_query_empty_strings_treated_as_missing() {
        let q = build_augmented_query(Some("   "), Some(""), "symptom");
        assert_eq!(q, "symptom");
    }

    #[tokio::test]
    async fn pipeline_returns_error_when_asset_missing() {
        let (_d, db, _real_asset) = fresh_db_with_asset();
        let stub = StubSynth {
            response: Ok("body text long enough to not be empty".into()),
        };
        let err = run_pipeline_with_backend(
            db,
            "no-such-asset".into(),
            "symptom".into(),
            TierRequest::Ollama,
            &stub,
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(err.contains("Asset not found"), "got: {}", err);
    }

    #[test]
    fn direct_persist_round_trip_exercises_draft_conversion() {
        // Exercises the persist branch mechanics without running the full pipeline.
        let (_d, db, asset_id) = fresh_db_with_asset();
        let draft = manor_core::repair::RepairNoteDraft {
            asset_id: asset_id.clone(),
            symptom: "test".into(),
            body_md: "A".repeat(100),
            sources: vec![manor_core::repair::RepairSource {
                url: "https://example.com/a".into(),
                title: "A".into(),
            }],
            video_sources: None,
            tier: TierRequest::Ollama.as_persist_tier(),
        };
        let id = {
            let conn = db.lock().unwrap();
            manor_core::repair::dal::insert_repair_note(&conn, &draft).unwrap()
        };
        let conn = db.lock().unwrap();
        let note = manor_core::repair::dal::get_repair_note(&conn, &id)
            .unwrap()
            .unwrap();
        assert_eq!(note.symptom, "test");
        assert_eq!(note.tier, manor_core::repair::LlmTier::Ollama);
    }
}
