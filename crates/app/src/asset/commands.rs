use crate::assistant::commands::Db;
use manor_core::asset::{
    dal::{self, AssetListFilter},
    Asset, AssetCategory, AssetDraft,
};
use serde::Deserialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

#[derive(Deserialize)]
pub struct AssetListArgs {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub category: Option<AssetCategory>,
}

#[tauri::command]
pub fn asset_list(args: AssetListArgs, state: State<'_, Db>) -> Result<Vec<Asset>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let filter = AssetListFilter {
        search: args.search,
        category: args.category,
        include_trashed: false,
    };
    dal::list_assets(&conn, &filter).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_get(id: String, state: State<'_, Db>) -> Result<Option<Asset>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::get_asset(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_create(draft: AssetDraft, state: State<'_, Db>) -> Result<String, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::insert_asset(&conn, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_update(id: String, draft: AssetDraft, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::update_asset(&conn, &id, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_delete(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::soft_delete_asset(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_restore(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::restore_asset(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_list_documents(
    id: String,
    state: State<'_, Db>,
) -> Result<Vec<manor_core::attachment::Attachment>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::attachment::list_for_text_entity(&conn, "asset", &id)
        .map_err(|e| e.to_string())
}

fn resolve_attachments_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("attachments"))
        .map_err(|e| e.to_string())
}

/// Reject any source path that canonicalizes to something outside the user's
/// home directory. Prevents an XSS/devtools-reachable IPC caller from using
/// `asset_attach_*` to exfiltrate `/etc/*`, other users' homes, or system files.
fn validate_source_path(p: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let canonical = p.canonicalize().map_err(|e| format!("cannot resolve path: {e}"))?;
    let home = dirs::home_dir().ok_or("no home directory available")?;
    let home_canonical = home.canonicalize().unwrap_or(home);
    if !canonical.starts_with(&home_canonical) {
        return Err(format!(
            "source path is outside the user home directory: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

#[tauri::command]
pub async fn asset_attach_hero_from_path(
    id: String,
    source_path: String,
    state: State<'_, Db>,
    app: AppHandle,
) -> Result<String, String> {
    let dir = resolve_attachments_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let src = validate_source_path(std::path::Path::new(&source_path))?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let uuid = crate::asset::importer::attach_file(&tx, &dir, &src, &id)
        .map_err(|e| e.to_string())?;
    manor_core::asset::dal::set_hero_attachment(&tx, &id, Some(&uuid))
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    Ok(uuid)
}

#[tauri::command]
pub async fn asset_attach_document_from_path(
    id: String,
    source_path: String,
    state: State<'_, Db>,
    app: AppHandle,
) -> Result<String, String> {
    let dir = resolve_attachments_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let src = validate_source_path(std::path::Path::new(&source_path))?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;

    crate::asset::importer::attach_file(&conn, &dir, &src, &id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_source_rejects_system_paths() {
        // Any file under /etc exists on every Unix — /etc/hosts is universal.
        let err = validate_source_path(std::path::Path::new("/etc/hosts")).unwrap_err();
        assert!(
            err.contains("outside") || err.contains("not allowed"),
            "got error: {err}",
        );
    }

    #[test]
    fn canonicalize_source_accepts_home_path() {
        let home = dirs::home_dir().expect("home dir");
        // Use a temp file inside home to avoid polluting Downloads/.
        let tmp = home.join(".manor-test-asset-sandbox.txt");
        std::fs::write(&tmp, b"hi").unwrap();
        let result = validate_source_path(&tmp);
        std::fs::remove_file(&tmp).ok();
        assert!(result.is_ok(), "got: {result:?}");
    }

    #[test]
    fn canonicalize_source_rejects_missing_file() {
        // Canonicalize fails for nonexistent paths — that's the right behavior
        // because read() would fail anyway, but we want a clear error message
        // before we even try to open.
        let err = validate_source_path(std::path::Path::new(
            "/nonexistent-directory-xyz/file.txt",
        ))
        .unwrap_err();
        assert!(!err.is_empty());
    }
}
