//! Tauri commands for repair-note + search (L4d).

use super::pipeline::{run_repair_search, PipelineOutcome, TierRequest};
use crate::assistant::commands::Db;
use manor_core::repair::RepairNote;
use tauri::State;

#[tauri::command]
pub async fn repair_search_ollama(
    asset_id: String,
    symptom: String,
    state: State<'_, Db>,
) -> Result<PipelineOutcome, String> {
    let db = state.0.clone();
    run_repair_search(db, asset_id, symptom, TierRequest::Ollama)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn repair_search_claude(
    asset_id: String,
    symptom: String,
    state: State<'_, Db>,
) -> Result<PipelineOutcome, String> {
    let db = state.0.clone();
    run_repair_search(db, asset_id, symptom, TierRequest::Claude)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn repair_note_list_for_asset(
    asset_id: String,
    state: State<'_, Db>,
) -> Result<Vec<RepairNote>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::repair::dal::list_for_asset(&conn, &asset_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn repair_note_get(id: String, state: State<'_, Db>) -> Result<Option<RepairNote>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::repair::dal::get_repair_note(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn repair_note_delete(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::repair::dal::soft_delete_repair_note(&conn, &id).map_err(|e| e.to_string())
}
