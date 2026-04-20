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

#[cfg(test)]
mod integration_tests {
    use manor_core::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use manor_core::assistant::db;
    use manor_core::maintenance::dal as sched_dal;
    use manor_core::maintenance::event::MaintenanceEventDraft;
    use manor_core::maintenance::event_dal;
    use manor_core::maintenance::MaintenanceScheduleDraft;
    use rusqlite::Connection;
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Connection, String) {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let asset = AssetDraft {
            name: "Boiler".into(),
            category: AssetCategory::Appliance,
            make: None,
            model: None,
            serial_number: None,
            purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        };
        let asset_id = asset_dal::insert_asset(&conn, &asset).unwrap();
        (dir, conn, asset_id)
    }

    fn simple_schedule(conn: &Connection, asset_id: &str) -> String {
        let draft = MaintenanceScheduleDraft {
            asset_id: asset_id.into(),
            task: "Annual service".into(),
            interval_months: 12,
            last_done_date: None,
            notes: String::new(),
        };
        sched_dal::insert_schedule(conn, &draft).unwrap()
    }

    #[test]
    fn silent_mark_done_writes_event_discoverable_via_list_for_asset() {
        let (_d, conn, asset_id) = fresh();
        let sched_id = simple_schedule(&conn, &asset_id);
        sched_dal::mark_done(&conn, &sched_id, "2026-04-20", None).unwrap();
        let events = event_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.title, "Annual service");
        assert_eq!(events[0].event.cost_pence, None);
    }

    #[test]
    fn log_completion_links_transaction_and_updates_rollup() {
        let (_d, conn, asset_id) = fresh();
        let sched_id = simple_schedule(&conn, &asset_id);
        conn.execute(
            "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
             VALUES (-14500, 'GBP', 'British Gas', 1776513600, 'manual')",
            [],
        )
        .unwrap();
        let tx_id = conn.last_insert_rowid();

        let draft = MaintenanceEventDraft {
            asset_id: asset_id.clone(),
            schedule_id: Some(sched_id.clone()),
            title: "Annual service".into(),
            completed_date: "2026-04-20".into(),
            cost_pence: Some(14500),
            currency: "GBP".into(),
            notes: "".into(),
            transaction_id: Some(tx_id),
        };
        sched_dal::mark_done(&conn, &sched_id, "2026-04-20", Some(&draft)).unwrap();

        let total = event_dal::asset_spend_for_asset(&conn, &asset_id, "2026-04-20").unwrap();
        assert_eq!(total.total_last_12m_pence, 14500);
        assert_eq!(total.event_count_last_12m, 1);
    }
}
