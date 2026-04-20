//! Tauri commands for maintenance events (L4c).

use crate::assistant::commands::Db;
use manor_core::ledger::transaction::Transaction;
use manor_core::maintenance::dal;
use manor_core::maintenance::event::{
    AssetSpendTotal, CategorySpendTotal, EventWithContext, MaintenanceEvent, MaintenanceEventDraft,
};
use manor_core::maintenance::event_dal;
use tauri::State;

fn today_local_string() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

#[tauri::command]
pub fn maintenance_event_list_for_asset(
    asset_id: String,
    state: State<'_, Db>,
) -> Result<Vec<EventWithContext>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    event_dal::list_for_asset(&conn, &asset_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_event_get(
    id: String,
    state: State<'_, Db>,
) -> Result<Option<MaintenanceEvent>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    event_dal::get_event(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_event_create_oneoff(
    draft: MaintenanceEventDraft,
    state: State<'_, Db>,
) -> Result<String, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    event_dal::insert_event(&conn, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_event_log_completion(
    schedule_id: String,
    draft: MaintenanceEventDraft,
    state: State<'_, Db>,
) -> Result<String, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let today = today_local_string();
    dal::mark_done(&conn, &schedule_id, &today, Some(&draft)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_event_update(
    id: String,
    draft: MaintenanceEventDraft,
    state: State<'_, Db>,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    event_dal::update_event(&conn, &id, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_spend_asset_totals(
    state: State<'_, Db>,
) -> Result<Vec<AssetSpendTotal>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    event_dal::asset_spend_totals(&conn, &today_local_string()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_spend_for_asset(
    asset_id: String,
    state: State<'_, Db>,
) -> Result<AssetSpendTotal, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    event_dal::asset_spend_for_asset(&conn, &asset_id, &today_local_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_spend_category_totals(
    state: State<'_, Db>,
) -> Result<Vec<CategorySpendTotal>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    event_dal::category_spend_totals(&conn, &today_local_string()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_suggest_transactions(
    completed_date: String,
    cost_pence: Option<i64>,
    exclude_event_id: Option<String>,
    state: State<'_, Db>,
) -> Result<Vec<Transaction>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let limit = if cost_pence.is_some() { 3 } else { 5 };
    event_dal::suggest_transactions(
        &conn,
        &completed_date,
        cost_pence,
        exclude_event_id.as_deref(),
        limit,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_search_transactions(
    query: String,
    state: State<'_, Db>,
) -> Result<Vec<Transaction>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    event_dal::search_transactions(&conn, &query, 20).map_err(|e| e.to_string())
}
