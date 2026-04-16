//! Panic button — erases all data and asks the user to restart the app.
//!
//! We do NOT force-relaunch the process automatically; the frontend asks the
//! user to quit + reopen. This avoids races with in-flight Tauri commands and
//! keeps logic testable on a real Mac.

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn panic_erase_everything(app: AppHandle, confirmation: String) -> Result<(), String> {
    // Require the literal string "DELETE" to avoid accidental triggers.
    if confirmation != "DELETE" {
        return Err("confirmation text must be exactly 'DELETE'".into());
    }
    let data: PathBuf = app.path().app_data_dir().map_err(|e| e.to_string())?;
    // Remove the whole app data directory. Next launch re-creates it (migrations run).
    if data.exists() {
        std::fs::remove_dir_all(&data)
            .map_err(|e| format!("failed to remove {}: {e}", data.display()))?;
    }
    // Re-create the directory so the next process start doesn't error on a missing dir.
    std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    // Also scrub the backup passphrase from Keychain; user is starting fresh.
    if let Ok(entry) = keyring::Entry::new("manor", "backup-passphrase") {
        let _ = entry.delete_credential();
    }
    Ok(())
}
