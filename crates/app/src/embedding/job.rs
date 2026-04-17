//! Background embed-on-write job. Walks stale rows, calls Ollama's embed
//! endpoint, stores vectors via `manor_core::embedding`.
//!
//! Runs once at app start in a Tauri async task. Silently no-ops if Ollama is
//! unreachable — user gets a visible "unavailable" banner in Settings/AI.

use super::{EMBED_BATCH_SIZE, EMBED_MODEL_DEFAULT, EMBED_MODEL_SETTING_KEY};
use crate::assistant::ollama::{OllamaClient, DEFAULT_ENDPOINT};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

/// Run one batch of embed-on-write. Returns (attempted, succeeded).
pub async fn run_embed_job(db: Arc<Mutex<Connection>>) -> (usize, usize) {
    // Step 1: read model + stale rows (lock held only for the DB read).
    let (model, stale) = {
        let conn = match db.lock() {
            Ok(c) => c,
            Err(_) => return (0, 0),
        };
        let m = manor_core::setting::get_or_default(
            &conn,
            EMBED_MODEL_SETTING_KEY,
            EMBED_MODEL_DEFAULT,
        )
        .unwrap_or_else(|_| EMBED_MODEL_DEFAULT.to_string());
        let stale = match manor_core::embedding::list_stale(&conn, &m, EMBED_BATCH_SIZE) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("embed: list_stale failed: {e}");
                return (0, 0);
            }
        };
        (m, stale)
    };

    if stale.is_empty() {
        return (0, 0);
    }

    // Step 2: embed each stale row via Ollama. Lock is released before each HTTP call.
    let client = OllamaClient::new(DEFAULT_ENDPOINT, &model);
    let attempted = stale.len();
    let mut succeeded = 0usize;

    for row in stale {
        let vec_result = client.embed(&row.text).await;
        let vector = match vec_result {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("embed: ollama call failed: {e}");
                break; // Most likely Ollama is down — stop the batch.
            }
        };

        let conn = match db.lock() {
            Ok(c) => c,
            Err(_) => break,
        };
        if let Err(e) = manor_core::embedding::upsert(
            &conn,
            &row.entity_type,
            row.entity_id,
            &model,
            &vector,
            row.updated_at,
        ) {
            tracing::warn!(
                "embed: upsert failed for {}/{}: {e}",
                row.entity_type,
                row.entity_id
            );
            continue;
        }
        succeeded += 1;
    }

    (attempted, succeeded)
}
