//! Tauri commands for embeddings: search_similar, embeddings_status, embeddings_rebuild.

use crate::assistant::commands::Db;
use crate::assistant::ollama::{OllamaClient, DEFAULT_ENDPOINT};
use manor_core::embedding::{self, SearchHit};
use serde::Serialize;
use tauri::State;

use super::{EMBED_MODEL_DEFAULT, EMBED_MODEL_SETTING_KEY};

#[derive(Serialize)]
pub struct EmbeddingsStatus {
    pub model: String,
    pub total: i64,
    pub by_entity_type: Vec<(String, i64)>,
}

#[tauri::command]
pub fn embeddings_status(state: State<'_, Db>) -> Result<EmbeddingsStatus, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let model =
        manor_core::setting::get_or_default(&conn, EMBED_MODEL_SETTING_KEY, EMBED_MODEL_DEFAULT)
            .unwrap_or_else(|_| EMBED_MODEL_DEFAULT.to_string());
    let counts = embedding::count_by_model(&conn).map_err(|e| e.to_string())?;
    let total: i64 = counts
        .iter()
        .filter(|(m, _)| *m == model)
        .map(|(_, n)| *n)
        .sum();
    let by_entity_type: Vec<(String, i64)> = {
        let mut stmt = conn
            .prepare(
                "SELECT entity_type, COUNT(*) FROM embedding
                 WHERE model = ?1 GROUP BY entity_type ORDER BY entity_type",
            )
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_map([&model], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        result
    };
    Ok(EmbeddingsStatus {
        model,
        total,
        by_entity_type,
    })
}

#[tauri::command]
pub async fn embeddings_search(
    state: State<'_, Db>,
    query: String,
    entity_types: Vec<String>,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    // Step 1: read model (drop lock before await).
    let model = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        manor_core::setting::get_or_default(&conn, EMBED_MODEL_SETTING_KEY, EMBED_MODEL_DEFAULT)
            .unwrap_or_else(|_| EMBED_MODEL_DEFAULT.to_string())
    };

    // Step 2: embed the query via Ollama.
    let client = OllamaClient::new(DEFAULT_ENDPOINT, &model);
    let query_vec = client.embed(&query).await.map_err(|e| e.to_string())?;

    // Step 3: cosine search.
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let filters: Vec<&str> = entity_types.iter().map(|s| s.as_str()).collect();
    embedding::search_similar(&conn, &query_vec, &model, &filters, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn embeddings_rebuild(state: State<'_, Db>) -> Result<usize, String> {
    // Step 1: clear existing vectors.
    let cleared = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        embedding::clear_all(&conn).map_err(|e| e.to_string())?
    };

    // Step 2: kick off one batch. Background job continues on next app start for more.
    let db_arc = state.inner().clone_arc();
    let (_attempted, _succeeded) = super::job::run_embed_job(db_arc).await;
    Ok(cleared)
}
