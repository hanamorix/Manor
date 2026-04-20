use crate::assistant::commands::Db;
use manor_core::maintenance::{dal, MaintenanceSchedule, MaintenanceScheduleDraft};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct ScheduleWithAsset {
    pub schedule: MaintenanceSchedule,
    pub asset_name: String,
    pub asset_category: String,
}

fn today_local_string() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

fn join_with_asset(
    conn: &rusqlite::Connection,
    schedules: Vec<MaintenanceSchedule>,
) -> Result<Vec<ScheduleWithAsset>, String> {
    let mut out = Vec::with_capacity(schedules.len());
    for s in schedules {
        let asset =
            manor_core::asset::dal::get_asset(conn, &s.asset_id).map_err(|e| e.to_string())?;
        let (name, category) = asset
            .map(|a| (a.name, a.category.as_str().to_string()))
            .unwrap_or_else(|| ("(deleted asset)".to_string(), "other".to_string()));
        out.push(ScheduleWithAsset {
            schedule: s,
            asset_name: name,
            asset_category: category,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn maintenance_schedule_list_for_asset(
    asset_id: String,
    state: State<'_, Db>,
) -> Result<Vec<MaintenanceSchedule>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::list_for_asset(&conn, &asset_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_schedule_get(
    id: String,
    state: State<'_, Db>,
) -> Result<Option<MaintenanceSchedule>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::get_schedule(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_schedule_create(
    draft: MaintenanceScheduleDraft,
    state: State<'_, Db>,
) -> Result<String, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::insert_schedule(&conn, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_schedule_update(
    id: String,
    draft: MaintenanceScheduleDraft,
    state: State<'_, Db>,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::update_schedule(&conn, &id, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_schedule_mark_done(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::mark_done(&conn, &id, &today_local_string(), None).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn maintenance_schedule_delete(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::soft_delete_schedule(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_schedule_restore(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::restore_schedule(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_due_soon(state: State<'_, Db>) -> Result<Vec<ScheduleWithAsset>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let today = chrono::Local::now().date_naive();
    let cutoff = today + chrono::Duration::days(30);
    let cutoff_str = cutoff.format("%Y-%m-%d").to_string();
    let schedules = dal::list_due_before(&conn, &cutoff_str).map_err(|e| e.to_string())?;
    join_with_asset(&conn, schedules)
}

#[tauri::command]
pub fn maintenance_due_today_and_overdue(
    state: State<'_, Db>,
) -> Result<Vec<ScheduleWithAsset>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let today = today_local_string();
    let schedules = dal::list_due_today_and_overdue(&conn, &today).map_err(|e| e.to_string())?;
    join_with_asset(&conn, schedules)
}

#[tauri::command]
pub fn maintenance_overdue_count(state: State<'_, Db>) -> Result<i64, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let today = today_local_string();
    dal::overdue_count(&conn, &today).map_err(|e| e.to_string())
}
