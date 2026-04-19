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
    entity_id: String,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    // Attempt to parse as an integer first (covers all INTEGER-PK tables).
    if let Ok(int_id) = entity_id.parse::<i64>() {
        return trash::restore(&conn, &entity_type, int_id).map_err(|e| e.to_string());
    }
    // TEXT-PK entity — route by type.
    match entity_type.as_str() {
        "recipe" => manor_core::recipe::dal::restore_recipe(&conn, &entity_id)
            .map_err(|e| e.to_string()),
        "staple_item" => manor_core::meal_plan::staples::restore_staple(&conn, &entity_id)
            .map_err(|e| e.to_string()),
        "asset" => manor_core::asset::dal::restore_asset(&conn, &entity_id)
            .map_err(|e| e.to_string()),
        "maintenance_schedule" => manor_core::maintenance::dal::restore_schedule(&conn, &entity_id)
            .map_err(|e| e.to_string()),
        _ => Err(format!(
            "trash_restore: unknown TEXT-keyed entity type '{entity_type}'"
        )),
    }
}

#[tauri::command]
pub fn trash_permanent_delete(
    app: AppHandle,
    state: State<'_, Db>,
    entity_type: String,
    entity_id: String,
) -> Result<(), String> {
    // Attachments need filesystem cleanup too — route via the attachment DAL.
    if entity_type == "attachment" {
        let int_id = entity_id
            .parse::<i64>()
            .map_err(|e| format!("attachment id must be integer: {e}"))?;
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("attachments");
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        manor_core::attachment::permanent_delete(&conn, &dir, int_id)
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    // Attempt integer parse for INTEGER-PK tables.
    if let Ok(int_id) = entity_id.parse::<i64>() {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        return trash::permanent_delete(&conn, &entity_type, int_id)
            .map_err(|e| e.to_string());
    }
    // TEXT-PK entity — route by type.
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    match entity_type.as_str() {
        "recipe" => manor_core::recipe::dal::permanent_delete_recipe(&conn, &entity_id)
            .map_err(|e| e.to_string()),
        "staple_item" => manor_core::meal_plan::staples::permanent_delete_staple(&conn, &entity_id)
            .map_err(|e| e.to_string()),
        "asset" => manor_core::asset::dal::permanent_delete_asset(&conn, &entity_id)
            .map_err(|e| e.to_string()),
        "maintenance_schedule" => manor_core::maintenance::dal::permanent_delete_schedule(&conn, &entity_id)
            .map_err(|e| e.to_string()),
        _ => Err(format!(
            "trash_permanent_delete: unknown TEXT-keyed entity type '{entity_type}'"
        )),
    }
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
