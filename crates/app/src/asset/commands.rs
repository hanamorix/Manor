use crate::assistant::commands::Db;
use manor_core::asset::{
    dal::{self, AssetListFilter},
    Asset, AssetCategory, AssetDraft,
};
use serde::Deserialize;
use tauri::State;

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
