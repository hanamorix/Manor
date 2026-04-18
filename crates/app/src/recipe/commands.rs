//! Tauri commands for the Recipe library — CRUD over `manor_core::recipe`.

use crate::assistant::commands::Db;
use crate::assistant::ollama::{OllamaClient, DEFAULT_ENDPOINT, DEFAULT_MODEL};
use crate::recipe::importer::{self, ImportPreview};
use crate::recipe::llm_adapter::OllamaLlmAdapter;
use manor_core::recipe::{
    dal::{self, ListFilter},
    Recipe, RecipeDraft,
};
use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

#[derive(Deserialize)]
pub struct ListRecipesArgs {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
}

#[tauri::command]
pub fn recipe_list(
    args: ListRecipesArgs,
    state: State<'_, Db>,
) -> Result<Vec<Recipe>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let filter = ListFilter {
        search: args.search,
        tag_ids: args.tag_ids,
        include_trashed: false,
    };
    dal::list_recipes(&conn, &filter).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn recipe_get(id: String, state: State<'_, Db>) -> Result<Option<Recipe>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::get_recipe(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn recipe_create(draft: RecipeDraft, state: State<'_, Db>) -> Result<String, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::insert_recipe(&conn, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn recipe_update(
    id: String,
    draft: RecipeDraft,
    state: State<'_, Db>,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::update_recipe(&conn, &id, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn recipe_delete(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::soft_delete_recipe(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn recipe_restore(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::restore_recipe(&conn, &id).map_err(|e| e.to_string())
}

/// Fetch, parse and preview a recipe from a URL.
/// Uses JSON-LD first; falls back to Ollama LLM extraction when JSON-LD is absent.
#[tauri::command]
pub async fn recipe_import_preview(url: String) -> Result<ImportPreview, String> {
    let adapter = OllamaLlmAdapter(OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL));
    importer::preview(&url, Some(&adapter))
        .await
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct ImportCommitArgs {
    pub draft: RecipeDraft,
    pub hero_image_url: Option<String>,
}

/// Commit an import preview to the database as a new recipe.
/// If `hero_image_url` is present, the image is downloaded and stored as an
/// attachment linked to the new recipe. Image errors are soft-failed: the
/// recipe is already saved and a missing image is acceptable.
#[tauri::command]
pub async fn recipe_import_commit(
    app: AppHandle,
    args: ImportCommitArgs,
    state: State<'_, Db>,
) -> Result<String, String> {
    // Step 1: insert recipe — sync, brief lock scope.
    let id = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        dal::insert_recipe(&conn, &args.draft).map_err(|e| e.to_string())?
    }; // lock released before any .await

    // Step 2: optionally download + store hero image. Soft-fail on any error.
    if let Some(url) = args.hero_image_url.as_deref() {
        let attachments_dir = app
            .path()
            .app_data_dir()
            .map(|d| d.join("attachments"))
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/manor-attachments"));

        // Lock never crosses .await — fetch_and_link_hero_arc acquires/releases
        // internally around each sync burst (HTTP runs without any lock held).
        let db_arc = state.0.clone();
        let _ = importer::fetch_and_link_hero_arc(
            db_arc,
            &id,
            Some(url),
            &attachments_dir,
        )
        .await;
    }

    Ok(id)
}
