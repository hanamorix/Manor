//! Tauri command glue for Manor.

pub mod assistant;

use serde::Serialize;
use tauri::{Builder, Manager, Wry};

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            assistant::commands::send_message,
            assistant::commands::list_messages,
            assistant::commands::get_unread_count,
            assistant::commands::mark_seen,
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
