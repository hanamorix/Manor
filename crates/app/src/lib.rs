//! Tauri command glue for Manor.

pub mod assistant;
pub mod embedding;
pub mod foundation;
pub mod ledger;
pub mod remote;
pub mod rhythm;
pub mod safety;
pub mod sync;
pub mod weather;

use serde::Serialize;
use std::sync::Arc;
use tauri::{Builder, Manager, Wry};

use crate::sync::engine::SyncState;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PingResponse {
    pub message: String,
    pub core_version: String,
}

mod commands {
    use super::PingResponse;

    /// Minimal smoke command that proves IPC works end-to-end.
    #[tauri::command]
    pub fn ping() -> PingResponse {
        PingResponse {
            message: "pong".to_string(),
            core_version: manor_core::version().to_string(),
        }
    }
}

pub use commands::ping;

/// Registers every Tauri command this crate exposes and wires the SQLite DB
/// into application state via Tauri's `setup()` closure.
pub fn register(builder: Builder<Wry>) -> Builder<Wry> {
    builder
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("could not resolve app data dir");
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("manor.db");
            let db = assistant::commands::Db::open(db_path)?;
            app.manage(db);

            // Register SyncState as Arc so it can be cheaply cloned into spawned tasks.
            let sync_state: Arc<SyncState> = Arc::new(SyncState::new());
            app.manage(sync_state.clone());

            // Kick off background app-start sync of all existing accounts.
            let sync_state_for_start = sync_state.clone();
            let db_arc = app.state::<assistant::commands::Db>().inner().clone_arc();
            tauri::async_runtime::spawn(async move {
                let ids: Vec<i64> = {
                    let conn = db_arc.lock().unwrap();
                    manor_core::assistant::calendar_account::list(&conn)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|a| a.id)
                        .collect()
                };
                for id in ids {
                    let Ok(password) = crate::sync::keychain::get_password(id) else {
                        continue;
                    };
                    let db_arc2 = db_arc.clone();
                    let sync_state2 = sync_state_for_start.clone();
                    // Lock is acquired inside the blocking thread, never held across .await.
                    let handle = tokio::runtime::Handle::current();
                    let result = tauri::async_runtime::spawn_blocking(move || {
                        let mut conn = db_arc2.lock().unwrap();
                        handle.block_on(crate::sync::engine::sync_account(
                            &mut conn,
                            &sync_state2,
                            id,
                            &password,
                            chrono_tz::UTC,
                        ))
                    })
                    .await;
                    if let Err(e) = result {
                        tracing::warn!("app-start sync for account {id} failed: {e}");
                    }
                }
            });

            // Trash: permanently delete rows that have been soft-deleted beyond
            // the user's configured retention period.
            let db_arc_trash = app.state::<assistant::commands::Db>().inner().clone_arc();
            tauri::async_runtime::spawn_blocking(move || {
                let conn = db_arc_trash.lock().unwrap();
                let now_ts = chrono::Utc::now().timestamp();
                let raw = manor_core::setting::get_or_default(&conn, "trash.auto_empty_days", "30")
                    .unwrap_or_else(|_| "30".to_string());
                if raw == "never" {
                    return;
                }
                let days: i64 = raw.parse().unwrap_or(30);
                if days <= 0 {
                    return;
                }
                let cutoff = now_ts - (days * 86400);
                match manor_core::trash::empty_older_than(&conn, cutoff) {
                    Ok(totals) => {
                        for (table, n) in totals {
                            tracing::info!("trash: permanently deleted {n} row(s) from {table}");
                        }
                    }
                    Err(e) => tracing::warn!("trash: empty_older_than failed: {e}"),
                }
            });

            // Ledger: auto-insert due recurring payments + log renewal alerts.
            let db_arc_ledger = app.state::<assistant::commands::Db>().inner().clone_arc();
            tauri::async_runtime::spawn_blocking(move || {
                let mut conn = db_arc_ledger.lock().unwrap();
                let now = chrono::Utc::now();

                match manor_core::ledger::recurring::auto_insert_due(&mut conn, now) {
                    Ok(n) if n > 0 => {
                        tracing::info!("ledger: auto-inserted {n} recurring transaction(s)")
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!("ledger: auto_insert_due failed: {e}"),
                }
                match manor_core::ledger::contract::check_renewals(&conn, now.timestamp()) {
                    Ok(alerts) if !alerts.is_empty() => {
                        tracing::info!("ledger: {} contract renewal alert(s) active", alerts.len());
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!("ledger: check_renewals failed: {e}"),
                }
            });

            // Embeddings: one batch of stale-row indexing on app start.
            // Runs async so it doesn't block setup; silently no-ops if Ollama is down.
            let db_arc_embed = app.state::<assistant::commands::Db>().inner().clone_arc();
            tauri::async_runtime::spawn(async move {
                let (attempted, succeeded) =
                    crate::embedding::job::run_embed_job(db_arc_embed).await;
                if attempted > 0 {
                    tracing::info!("embed: indexed {succeeded}/{attempted} stale row(s)");
                }
            });

            // Phase 5d: register pending OAuth callbacks map for bank connect flow.
            app.manage::<ledger::bank_commands::PendingCallbacks>(std::sync::Arc::new(
                tokio::sync::Mutex::new(std::collections::HashMap::new()),
            ));

            // Phase 5d: bank sync every 6 hours.
            {
                let db = app.state::<assistant::commands::Db>().inner().clone_arc();
                tauri::async_runtime::spawn(async move {
                    let client = ledger::gocardless::GoCardlessClient::default_prod();
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
                        let handle = tokio::runtime::Handle::current();
                        let db = db.clone();
                        let client = client.clone();
                        let _ = tauri::async_runtime::spawn_blocking(move || {
                            if let Ok(mut conn) = db.lock() {
                                let ctx = ledger::bank_sync::SyncContext {
                                    client: &client,
                                    allow_rate_limit_bypass: false,
                                };
                                if let Err(e) =
                                    handle.block_on(ledger::bank_sync::sync_all(&mut conn, &ctx))
                                {
                                    tracing::warn!("bank sync tick failed: {e}");
                                }
                            }
                        })
                        .await;
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            assistant::commands::send_message,
            assistant::commands::list_messages,
            assistant::commands::get_unread_count,
            assistant::commands::mark_seen,
            assistant::commands::list_tasks,
            assistant::commands::add_task,
            assistant::commands::complete_task,
            assistant::commands::undo_complete_task,
            assistant::commands::update_task,
            assistant::commands::delete_task,
            assistant::commands::list_proposals,
            assistant::commands::approve_proposal,
            assistant::commands::reject_proposal,
            assistant::commands::list_calendar_accounts,
            assistant::commands::add_calendar_account,
            assistant::commands::remove_calendar_account,
            assistant::commands::sync_account,
            assistant::commands::sync_all_accounts,
            assistant::commands::list_events_today,
            assistant::commands::list_calendars,
            assistant::commands::set_default_calendar,
            assistant::commands::create_event,
            assistant::commands::update_event,
            assistant::commands::delete_event,
            rhythm::commands::list_chores_due_today,
            rhythm::commands::list_all_chores,
            rhythm::commands::create_chore,
            rhythm::commands::update_chore,
            rhythm::commands::delete_chore,
            rhythm::commands::complete_chore,
            rhythm::commands::skip_chore,
            rhythm::commands::list_chore_completions,
            rhythm::commands::list_chore_rotation,
            rhythm::commands::check_chore_fairness,
            rhythm::commands::list_blocks_today,
            rhythm::commands::list_blocks_for_week,
            rhythm::commands::list_recurring_blocks,
            rhythm::commands::create_time_block,
            rhythm::commands::update_time_block,
            rhythm::commands::delete_time_block,
            rhythm::commands::promote_to_pattern,
            rhythm::commands::dismiss_pattern_nudge,
            rhythm::commands::check_time_block_pattern,
            rhythm::commands::add_person,
            ledger::commands::ledger_list_categories,
            ledger::commands::ledger_upsert_category,
            ledger::commands::ledger_delete_category,
            ledger::commands::ledger_add_transaction,
            ledger::commands::ledger_update_transaction,
            ledger::commands::ledger_delete_transaction,
            ledger::commands::ledger_list_transactions,
            ledger::commands::ledger_list_budgets,
            ledger::commands::ledger_upsert_budget,
            ledger::commands::ledger_delete_budget,
            ledger::commands::ledger_monthly_summary,
            ledger::commands::ledger_list_recurring,
            ledger::commands::ledger_add_recurring,
            ledger::commands::ledger_update_recurring,
            ledger::commands::ledger_delete_recurring,
            ledger::commands::ledger_list_contracts,
            ledger::commands::ledger_add_contract,
            ledger::commands::ledger_update_contract,
            ledger::commands::ledger_delete_contract,
            ledger::commands::ledger_get_renewal_alerts,
            ledger::commands::ledger_preview_csv,
            ledger::commands::ledger_import_csv,
            ledger::commands::ledger_ai_month_review,
            ledger::bank_commands::ledger_bank_credentials_status,
            ledger::bank_commands::ledger_bank_save_credentials,
            ledger::bank_commands::ledger_bank_list_institutions,
            ledger::bank_commands::ledger_bank_begin_connect,
            ledger::bank_commands::ledger_bank_complete_connect,
            ledger::bank_commands::ledger_bank_list_accounts,
            ledger::bank_commands::ledger_bank_sync_now,
            ledger::bank_commands::ledger_bank_disconnect,
            ledger::bank_commands::ledger_bank_reconnect,
            ledger::bank_commands::ledger_bank_cancel_connect,
            ledger::bank_commands::ledger_bank_autocat_pending,
            foundation::commands::setting_get,
            foundation::commands::setting_set,
            foundation::commands::setting_delete,
            foundation::commands::person_list,
            foundation::commands::person_add,
            foundation::commands::person_update,
            foundation::commands::person_delete,
            foundation::commands::household_get,
            foundation::commands::household_set_owner,
            foundation::commands::household_set_working_hours,
            foundation::commands::household_set_dnd,
            foundation::commands::tag_list,
            foundation::commands::tag_upsert,
            foundation::commands::tag_delete,
            foundation::commands::tag_link,
            foundation::commands::tag_unlink,
            foundation::commands::tag_for_entity,
            foundation::commands::entities_with_tag,
            foundation::commands::note_insert,
            foundation::commands::note_update,
            foundation::commands::note_delete,
            foundation::commands::note_list_for,
            foundation::commands::attachment_store,
            foundation::commands::attachment_get_bytes,
            foundation::commands::attachment_list_for,
            foundation::commands::attachment_delete,
            foundation::commands::setting_list_prefixed,
            foundation::commands::note_list_orphans,
            foundation::commands::note_restore,
            foundation::commands::person_restore,
            foundation::commands::attachment_restore,
            foundation::commands::attachment_permanent_delete,
            foundation::commands::app_version,
            foundation::commands::data_dir_path,
            foundation::commands::ollama_status,
            safety::trash_commands::trash_list,
            safety::trash_commands::trash_restore,
            safety::trash_commands::trash_permanent_delete,
            safety::trash_commands::trash_empty_all,
            safety::snapshot_commands::backup_set_passphrase,
            safety::snapshot_commands::backup_has_passphrase,
            safety::snapshot_commands::backup_create_now,
            safety::snapshot_commands::backup_list,
            safety::snapshot_commands::backup_restore,
            safety::panic_commands::panic_erase_everything,
            safety::snapshot_commands::backup_schedule_install,
            safety::snapshot_commands::backup_schedule_uninstall,
            safety::snapshot_commands::backup_schedule_is_installed,
            weather::commands::weather_current,
            embedding::commands::embeddings_status,
            embedding::commands::embeddings_search,
            embedding::commands::embeddings_rebuild,
            remote::commands::remote_provider_status,
            remote::commands::remote_set_key,
            remote::commands::remote_remove_key,
            remote::commands::remote_set_budget,
            remote::commands::remote_set_enabled_for_review,
            remote::commands::remote_call_log_list,
            remote::commands::remote_call_log_clear,
            remote::commands::remote_test,
        ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Builder;

    #[test]
    fn ping_returns_pong_with_core_version() {
        let resp = ping();
        assert_eq!(resp.message, "pong");
        assert_eq!(resp.core_version, manor_core::version());
    }

    #[test]
    fn register_returns_builder() {
        let _builder = register(Builder::default());
    }
}
