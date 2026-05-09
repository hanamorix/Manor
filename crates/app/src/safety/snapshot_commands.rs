//! Tauri commands for snapshot backup (create / list / restore).

use crate::assistant::commands::Db;
use keyring::Entry;
use manor_core::snapshot;
use std::path::{Path, PathBuf};
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

fn default_data_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|base| base.join("com.hanamorix.manor"))
        .ok_or_else(|| "could not resolve application data dir".to_string())
}

fn create_backup_from_data_dir(data: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    let pass = passphrase()?;
    let db = data.join("manor.db");
    let att = data.join("attachments");
    let out = out_dir.join(snapshot::default_filename(chrono::Utc::now()));
    snapshot::create(&db, &att, &out, &pass).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Entry point used by the launchd CLI mode. This deliberately does not need a
/// Tauri AppHandle, so scheduled backups can run while the UI is closed.
pub fn run_scheduled_backup(out_dir: PathBuf) -> Result<PathBuf, String> {
    let data = default_data_dir()?;
    create_backup_from_data_dir(&data, &out_dir)
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
    let data = data_dir(&app)?;
    let out_dir = PathBuf::from(&out_dir);
    let out = create_backup_from_data_dir(&data, &out_dir)?;
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

// ── launchd scheduling ────────────────────────────────────────────────────────

use crate::safety::launchd;

#[derive(serde::Deserialize)]
pub struct ScheduleArgs {
    pub out_dir: String,
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
}

#[tauri::command]
pub fn backup_schedule_install(args: ScheduleArgs) -> Result<(), String> {
    let program_path = std::env::current_exe().map_err(|e| e.to_string())?;
    launchd::install(
        &program_path,
        &PathBuf::from(&args.out_dir),
        args.weekday,
        args.hour,
        args.minute,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn backup_schedule_uninstall() -> Result<(), String> {
    launchd::uninstall().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn backup_schedule_is_installed() -> Result<bool, String> {
    launchd::is_installed().map_err(|e| e.to_string())
}
