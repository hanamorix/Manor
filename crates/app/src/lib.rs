//! Tauri command glue for Manor.

pub mod assistant;
pub mod rhythm;
pub mod sync;

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
