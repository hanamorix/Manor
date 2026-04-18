//! Tauri commands exposing the foundation DALs to the frontend.
//! Phase A surfaces only CRUD — UI wiring arrives in Phases B–E.

use crate::assistant::commands::Db;
use manor_core::{attachment, household, note, person, setting, tag};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

fn attachments_root(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("attachments"))
}

// ── Settings ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn setting_get(state: State<'_, Db>, key: String) -> Result<Option<String>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    setting::get(&conn, &key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn setting_set(state: State<'_, Db>, key: String, value: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    setting::set(&conn, &key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn setting_delete(state: State<'_, Db>, key: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    setting::delete(&conn, &key).map_err(|e| e.to_string())
}

// ── People ───────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct PersonArgs {
    pub name: String,
    pub kind: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub note: Option<String>,
}

#[tauri::command]
pub fn person_list(state: State<'_, Db>) -> Result<Vec<person::Person>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    person::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn person_add(state: State<'_, Db>, args: PersonArgs) -> Result<person::Person, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    person::insert(
        &conn,
        &args.name,
        &args.kind,
        args.email.as_deref(),
        args.phone.as_deref(),
        args.note.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct UpdatePersonArgs {
    pub id: i64,
    #[serde(flatten)]
    pub fields: PersonArgs,
}

#[tauri::command]
pub fn person_update(
    state: State<'_, Db>,
    args: UpdatePersonArgs,
) -> Result<person::Person, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    person::update(
        &conn,
        args.id,
        &args.fields.name,
        &args.fields.kind,
        args.fields.email.as_deref(),
        args.fields.phone.as_deref(),
        args.fields.note.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn person_delete(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    person::delete(&conn, id).map_err(|e| e.to_string())
}

// ── Household ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn household_get(state: State<'_, Db>) -> Result<household::Household, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    household::get(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn household_set_owner(
    state: State<'_, Db>,
    owner_person_id: Option<i64>,
) -> Result<household::Household, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    household::set_owner(&conn, owner_person_id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct WorkingHoursArgs {
    pub hours: household::WorkingHours,
}

#[tauri::command]
pub fn household_set_working_hours(
    state: State<'_, Db>,
    args: WorkingHoursArgs,
) -> Result<household::Household, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    household::set_working_hours(&conn, &args.hours).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct DndWindowsArgs {
    pub windows: Vec<household::DndWindow>,
}

#[tauri::command]
pub fn household_set_dnd(
    state: State<'_, Db>,
    args: DndWindowsArgs,
) -> Result<household::Household, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    household::set_dnd_windows(&conn, &args.windows).map_err(|e| e.to_string())
}

// ── Tags ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn tag_list(state: State<'_, Db>) -> Result<Vec<tag::Tag>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    tag::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tag_upsert(state: State<'_, Db>, name: String, color: String) -> Result<tag::Tag, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    tag::upsert(&conn, &name, &color).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tag_delete(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    tag::delete_tag(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tag_link(
    state: State<'_, Db>,
    tag_id: i64,
    entity_type: String,
    entity_id: i64,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    tag::link(&conn, tag_id, &entity_type, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tag_unlink(
    state: State<'_, Db>,
    tag_id: i64,
    entity_type: String,
    entity_id: i64,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    tag::unlink(&conn, tag_id, &entity_type, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tag_for_entity(
    state: State<'_, Db>,
    entity_type: String,
    entity_id: i64,
) -> Result<Vec<tag::Tag>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    tag::tags_for(&conn, &entity_type, entity_id).map_err(|e| e.to_string())
}

// ── Notes ────────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct NoteInsertArgs {
    pub body_md: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
}

#[tauri::command]
pub fn note_insert(state: State<'_, Db>, args: NoteInsertArgs) -> Result<note::Note, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    note::insert(
        &conn,
        &args.body_md,
        args.entity_type.as_deref(),
        args.entity_id,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn note_update(state: State<'_, Db>, id: i64, body_md: String) -> Result<note::Note, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    note::update(&conn, id, &body_md).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn note_delete(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    note::delete(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn note_list_for(
    state: State<'_, Db>,
    entity_type: String,
    entity_id: i64,
) -> Result<Vec<note::Note>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    note::list_for(&conn, &entity_type, entity_id).map_err(|e| e.to_string())
}

// ── Attachments ──────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct AttachmentStoreArgs {
    pub bytes: Vec<u8>,
    pub original_name: String,
    pub mime_type: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
}

#[tauri::command]
pub fn attachment_store(
    app: AppHandle,
    state: State<'_, Db>,
    args: AttachmentStoreArgs,
) -> Result<attachment::Attachment, String> {
    let root = attachments_root(&app)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    attachment::store(
        &conn,
        &root,
        &args.bytes,
        &args.original_name,
        &args.mime_type,
        args.entity_type.as_deref(),
        args.entity_id,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn attachment_get_bytes(
    app: AppHandle,
    state: State<'_, Db>,
    id: i64,
) -> Result<Vec<u8>, String> {
    let root = attachments_root(&app)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    attachment::get_bytes(&conn, &root, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn attachment_list_for(
    state: State<'_, Db>,
    entity_type: String,
    entity_id: i64,
) -> Result<Vec<attachment::Attachment>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    attachment::list_for(&conn, &entity_type, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn attachment_delete(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    attachment::delete(&conn, id).map_err(|e| e.to_string())
}

// ── Settings (listing) ────────────────────────────────────────────────────────

#[tauri::command]
pub fn setting_list_prefixed(
    state: State<'_, Db>,
    prefix: String,
) -> Result<Vec<(String, String)>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    setting::list_prefixed(&conn, &prefix).map_err(|e| e.to_string())
}

// ── Notes (orphans + restore) ─────────────────────────────────────────────────

#[tauri::command]
pub fn note_list_orphans(state: State<'_, Db>) -> Result<Vec<note::Note>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    note::list_orphans(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn note_restore(state: State<'_, Db>, id: i64) -> Result<note::Note, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    note::restore(&conn, id).map_err(|e| e.to_string())
}

// ── Person (restore) ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn person_restore(state: State<'_, Db>, id: i64) -> Result<person::Person, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    person::restore(&conn, id).map_err(|e| e.to_string())
}

// ── Attachment (restore + permanent delete) ───────────────────────────────────

#[tauri::command]
pub fn attachment_restore(state: State<'_, Db>, id: i64) -> Result<attachment::Attachment, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    attachment::restore(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn attachment_permanent_delete(
    app: AppHandle,
    state: State<'_, Db>,
    id: i64,
) -> Result<(), String> {
    let root = attachments_root(&app)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    attachment::permanent_delete(&conn, &root, id).map_err(|e| e.to_string())
}

/// Return the absolute filesystem path for an attachment looked up by its `uuid`
/// (TEXT). Used by the recipe hero-image read path: `recipe.hero_attachment_uuid`
/// is a TEXT uuid, not an integer id, so we look up directly by uuid here.
/// The frontend calls `convertFileSrc(path)` from `@tauri-apps/api/core` to
/// convert the absolute path to a webview-safe `asset://` URL for rendering.
#[tauri::command]
pub fn attachment_get_path_by_uuid(
    app: AppHandle,
    state: State<'_, Db>,
    uuid: String,
) -> Result<String, String> {
    let root = attachments_root(&app)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    // Verify the attachment exists and is not deleted.
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM attachment WHERE uuid = ?1 AND deleted_at IS NULL",
            rusqlite::params![uuid],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .map_err(|e| e.to_string())?;
    if !exists {
        return Err(format!("attachment not found: {uuid}"));
    }
    let path = attachment::file_path(&root, &uuid);
    Ok(path.to_string_lossy().into_owned())
}

// ── App metadata ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn data_dir_path(app: AppHandle) -> Result<String, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().into_owned())
}

// ── Tags (reverse lookup) ─────────────────────────────────────────────────────

#[tauri::command]
pub fn entities_with_tag(state: State<'_, Db>, tag_id: i64) -> Result<Vec<(String, i64)>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    tag::entities_with_tag(&conn, tag_id).map_err(|e| e.to_string())
}

// ── Ollama status ─────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct OllamaStatus {
    pub reachable: bool,
    pub models: Vec<String>,
}

#[tauri::command]
pub async fn ollama_status() -> Result<OllamaStatus, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = match client.get("http://127.0.0.1:11434/api/tags").send().await {
        Ok(r) => r,
        Err(_) => {
            return Ok(OllamaStatus {
                reachable: false,
                models: vec![],
            })
        }
    };
    if !resp.status().is_success() {
        return Ok(OllamaStatus {
            reachable: false,
            models: vec![],
        });
    }
    #[derive(serde::Deserialize)]
    struct TagsResp {
        models: Vec<TagEntry>,
    }
    #[derive(serde::Deserialize)]
    struct TagEntry {
        name: String,
    }
    let body: TagsResp = resp.json().await.map_err(|e| e.to_string())?;
    Ok(OllamaStatus {
        reachable: true,
        models: body.models.into_iter().map(|m| m.name).collect(),
    })
}
