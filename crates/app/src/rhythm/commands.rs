//! Tauri commands for the Rhythm feature (chores + time blocks).

use crate::assistant::commands::Db;
use chrono::{Local, Utc};
use manor_core::assistant::{
    chore::{self, Chore, ChoreCompletion, FairnessNudge, RotationMember},
    time_block::{self, PatternSuggestion, TimeBlock},
};
use tauri::State;

fn end_of_today_ms() -> i64 {
    let now = Local::now();
    let end = now
        .date_naive()
        .and_hms_opt(23, 59, 59)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap();
    end.with_timezone(&Utc).timestamp_millis()
}

fn today_midnight_utc_ms() -> i64 {
    let now = Local::now();
    now.date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}

// ---------- Chore commands ----------

#[tauri::command]
pub fn list_chores_due_today(state: State<'_, Db>) -> Result<Vec<Chore>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::list_due_today(&conn, end_of_today_ms()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_all_chores(state: State<'_, Db>) -> Result<Vec<Chore>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::list_all(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct CreateChoreArgs {
    pub title: String,
    pub emoji: String,
    pub rrule: String,
    #[serde(rename = "firstDue")]
    pub first_due: i64,
    pub rotation: String,
}

#[tauri::command]
pub fn create_chore(state: State<'_, Db>, args: CreateChoreArgs) -> Result<Chore, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let id = chore::insert(
        &conn,
        &args.title,
        &args.emoji,
        &args.rrule,
        args.first_due,
        &args.rotation,
    )
    .map_err(|e| e.to_string())?;
    chore::get(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "failed to fetch new chore".to_string())
}

#[derive(serde::Deserialize)]
pub struct UpdateChoreArgs {
    pub id: i64,
    pub title: String,
    pub emoji: String,
    pub rrule: String,
    pub rotation: String,
}

#[tauri::command]
pub fn update_chore(state: State<'_, Db>, args: UpdateChoreArgs) -> Result<Chore, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::update(
        &conn,
        args.id,
        &args.title,
        &args.emoji,
        &args.rrule,
        &args.rotation,
    )
    .map_err(|e| e.to_string())?;
    chore::get(&conn, args.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "chore not found".to_string())
}

#[tauri::command]
pub fn delete_chore(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::soft_delete(&conn, id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct CompleteChoreArgs {
    pub id: i64,
    #[serde(rename = "completedBy")]
    pub completed_by: Option<i64>,
}

#[tauri::command]
pub fn complete_chore(state: State<'_, Db>, args: CompleteChoreArgs) -> Result<Chore, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::complete(&conn, args.id, args.completed_by).map_err(|e| e.to_string())?;
    chore::get(&conn, args.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "chore not found".to_string())
}

#[tauri::command]
pub fn skip_chore(state: State<'_, Db>, id: i64) -> Result<Chore, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::skip(&conn, id).map_err(|e| e.to_string())?;
    chore::get(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "chore not found".to_string())
}

#[tauri::command]
pub fn list_chore_completions(
    state: State<'_, Db>,
    chore_id: i64,
    limit: u32,
) -> Result<Vec<ChoreCompletion>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::list_completions(&conn, chore_id, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_chore_rotation(
    state: State<'_, Db>,
    chore_id: i64,
) -> Result<Vec<RotationMember>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::list_rotation(&conn, chore_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_chore_fairness(state: State<'_, Db>) -> Result<Vec<FairnessNudge>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().timestamp_millis();
    chore::check_fairness(&conn, now).map_err(|e| e.to_string())
}

// ---------- Time block commands ----------

#[tauri::command]
pub fn list_blocks_today(state: State<'_, Db>) -> Result<Vec<TimeBlock>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::list_for_date(&conn, today_midnight_utc_ms()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_blocks_for_week(
    state: State<'_, Db>,
    week_start_ms: i64,
) -> Result<Vec<TimeBlock>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::list_for_week(&conn, week_start_ms).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_recurring_blocks(state: State<'_, Db>) -> Result<Vec<TimeBlock>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::list_recurring(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct CreateBlockArgs {
    pub title: String,
    pub kind: String,
    #[serde(rename = "dateMs")]
    pub date_ms: i64,
    #[serde(rename = "startTime")]
    pub start_time: String,
    #[serde(rename = "endTime")]
    pub end_time: String,
}

#[derive(serde::Serialize)]
pub struct CreateBlockResult {
    pub block: TimeBlock,
    pub suggestion: Option<PatternSuggestion>,
}

#[tauri::command]
pub fn create_time_block(
    state: State<'_, Db>,
    args: CreateBlockArgs,
) -> Result<CreateBlockResult, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let id = time_block::insert(
        &conn,
        &args.title,
        &args.kind,
        args.date_ms,
        &args.start_time,
        &args.end_time,
    )
    .map_err(|e| e.to_string())?;
    let block = time_block::get(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "failed to fetch new block".to_string())?;
    let now = Utc::now().timestamp_millis();
    let suggestion = time_block::check_pattern(&conn, id, now).map_err(|e| e.to_string())?;
    Ok(CreateBlockResult { block, suggestion })
}

#[derive(serde::Deserialize)]
pub struct UpdateBlockArgs {
    pub id: i64,
    pub title: String,
    pub kind: String,
    #[serde(rename = "dateMs")]
    pub date_ms: i64,
    #[serde(rename = "startTime")]
    pub start_time: String,
    #[serde(rename = "endTime")]
    pub end_time: String,
}

#[tauri::command]
pub fn update_time_block(
    state: State<'_, Db>,
    args: UpdateBlockArgs,
) -> Result<TimeBlock, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::update(
        &conn,
        args.id,
        &args.title,
        &args.kind,
        args.date_ms,
        &args.start_time,
        &args.end_time,
    )
    .map_err(|e| e.to_string())?;
    time_block::get(&conn, args.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "block not found".to_string())
}

#[tauri::command]
pub fn delete_time_block(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::soft_delete(&conn, id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct PromoteArgs {
    pub id: i64,
    pub rrule: String,
}

#[tauri::command]
pub fn promote_to_pattern(state: State<'_, Db>, args: PromoteArgs) -> Result<TimeBlock, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::promote_to_pattern(&conn, args.id, &args.rrule).map_err(|e| e.to_string())?;
    time_block::get(&conn, args.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "block not found".to_string())
}

#[tauri::command]
pub fn dismiss_pattern_nudge(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::dismiss_pattern_nudge(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_time_block_pattern(
    state: State<'_, Db>,
    id: i64,
) -> Result<Option<PatternSuggestion>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().timestamp_millis();
    time_block::check_pattern(&conn, id, now).map_err(|e| e.to_string())
}

// ---------- Person commands ----------

#[tauri::command]
pub fn add_person(state: State<'_, Db>, name: String) -> Result<i64, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::insert_person(&conn, &name).map_err(|e| e.to_string())
}
