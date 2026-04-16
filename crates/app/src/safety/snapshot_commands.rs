//! Tauri commands for snapshot backup (create / list / restore).

use crate::assistant::commands::Db;
use keyring::Entry;
use manor_core::snapshot;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

const KEYRING_SERVICE: &str = "manor";
const KEYRING_ACCOUNT: &str = "backup-passphrase";

fn passphrase() -> Result<String, String> {
    Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|_| "backup passphrase not set in Keychain".to_string())
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct BackupEntry {
    pub path: String,
    pub mtime: i64,
    pub size_bytes: u64,
}

#[tauri::command]
pub fn backup_set_passphrase(passphrase: String) -> Result<(), String> {
    Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| e.to_string())?
        .set_password(&passphrase)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn backup_has_passphrase() -> bool {
    Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .ok()
        .and_then(|e| e.get_password().ok())
        .is_some()
}

#[tauri::command]
pub fn backup_create_now(
    app: AppHandle,
    _state: State<'_, Db>,
    out_dir: String,
) -> Result<String, String> {
    let pass = passphrase()?;
    let data = data_dir(&app)?;
    let db = data.join("manor.db");
    let att = data.join("attachments");
    let out = PathBuf::from(&out_dir).join(snapshot::default_filename(chrono::Utc::now()));
    snapshot::create(&db, &att, &out, &pass).map_err(|e| e.to_string())?;
    Ok(out.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn backup_list(dir: String) -> Result<Vec<BackupEntry>, String> {
    let items = snapshot::list(&PathBuf::from(dir)).map_err(|e| e.to_string())?;
    Ok(items
        .into_iter()
        .map(|(path, mtime)| {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            BackupEntry {
                path: path.to_string_lossy().into_owned(),
                mtime,
                size_bytes: size,
            }
        })
        .collect())
}

#[tauri::command]
pub fn backup_restore(
    app: AppHandle,
    backup_path: String,
    provided_passphrase: String,
) -> Result<String, String> {
    let data = data_dir(&app)?;
    let staging = data.join("restore-staging");
    // Clear staging first.
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    snapshot::restore_to_staging(&PathBuf::from(backup_path), &staging, &provided_passphrase)
        .map_err(|e| e.to_string())?;
    Ok(staging.to_string_lossy().into_owned())
}
