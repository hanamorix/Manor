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

#[tauri::command]
pub async fn asset_attach_hero_from_path(
    id: String,
    source_path: String,
    state: State<'_, Db>,
    app: AppHandle,
) -> Result<String, String> {
    let dir = resolve_attachments_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let src = std::path::PathBuf::from(source_path);
    let conn = state.0.lock().map_err(|e| e.to_string())?;

    let uuid = crate::asset::importer::attach_file(&conn, &dir, &src, &id)
        .map_err(|e| e.to_string())?;
    manor_core::asset::dal::set_hero_attachment(&conn, &id, Some(&uuid))
        .map_err(|e| e.to_string())?;
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
    let src = std::path::PathBuf::from(source_path);
    let conn = state.0.lock().map_err(|e| e.to_string())?;

    crate::asset::importer::attach_file(&conn, &dir, &src, &id).map_err(|e| e.to_string())
}
