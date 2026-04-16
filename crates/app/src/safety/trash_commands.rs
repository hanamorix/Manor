//! Tauri commands for the Trash aggregator.

use crate::assistant::commands::Db;
use manor_core::trash::{self, TrashEntry};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn trash_list(state: State<'_, Db>) -> Result<Vec<TrashEntry>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    trash::list_all(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn trash_restore(
    state: State<'_, Db>,
    entity_type: String,
    entity_id: i64,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    trash::restore(&conn, &entity_type, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn trash_permanent_delete(
    app: AppHandle,
    state: State<'_, Db>,
    entity_type: String,
    entity_id: i64,
) -> Result<(), String> {
    // Attachments need filesystem cleanup too — route via the attachment DAL.
    if entity_type == "attachment" {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("attachments");
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        manor_core::attachment::permanent_delete(&conn, &dir, entity_id)
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    trash::permanent_delete(&conn, &entity_type, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn trash_empty_all(
    app: AppHandle,
    state: State<'_, Db>,
) -> Result<Vec<(String, usize)>, String> {
    // Gather soft-deleted attachment IDs first so we can clean their files.
    let att_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("attachments");
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let att_ids: Vec<(i64, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, uuid FROM attachment WHERE deleted_at IS NOT NULL")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        rows
    };
    for (id, _uuid) in &att_ids {
        manor_core::attachment::permanent_delete(&conn, &att_dir, *id)
            .map_err(|e| e.to_string())?;
    }
    // Now nuke everything else.
    trash::empty_all(&conn).map_err(|e| e.to_string())
}
