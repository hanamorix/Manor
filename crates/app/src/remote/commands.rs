//! Tauri commands for remote LLM support.

use super::{
    budget_setting_key, keychain, orchestrator, DEFAULT_BUDGET_PENCE, PROVIDER_CLAUDE,
    REMOTE_ENABLED_FOR_REVIEW_KEY,
};
use crate::assistant::commands::Db;
use manor_core::remote_call_log::{self, CallLogEntry};
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct RemoteProviderStatus {
    pub provider: String,
    pub has_key: bool,
    pub budget_pence: i64,
    pub spent_month_pence: i64,
    pub enabled_for_review: bool,
}

#[tauri::command]
pub fn remote_provider_status(state: State<'_, Db>) -> Result<RemoteProviderStatus, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let has_key = keychain::has_key(PROVIDER_CLAUDE);
    let budget_pence = manor_core::setting::get_or_default(
        &conn,
        &budget_setting_key(PROVIDER_CLAUDE),
        &DEFAULT_BUDGET_PENCE.to_string(),
    )
    .ok()
    .and_then(|s| s.parse().ok())
    .unwrap_or(DEFAULT_BUDGET_PENCE);
    let spent = remote_call_log::sum_month_pence(&conn, PROVIDER_CLAUDE, chrono::Utc::now())
        .map_err(|e| e.to_string())?;
    let enabled = manor_core::setting::get(&conn, REMOTE_ENABLED_FOR_REVIEW_KEY)
        .ok()
        .flatten()
        .as_deref()
        == Some("1");
    Ok(RemoteProviderStatus {
        provider: PROVIDER_CLAUDE.to_string(),
        has_key,
        budget_pence,
        spent_month_pence: spent,
        enabled_for_review: enabled,
    })
}

#[tauri::command]
pub fn remote_set_key(provider: String, key: String) -> Result<(), String> {
    keychain::set_key(&provider, &key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remote_remove_key(provider: String) -> Result<bool, String> {
    keychain::remove_key(&provider).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remote_set_budget(state: State<'_, Db>, provider: String, pence: i64) -> Result<(), String> {
    if pence < 0 {
        return Err("budget cannot be negative".into());
    }
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::setting::set(&conn, &budget_setting_key(&provider), &pence.to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remote_set_enabled_for_review(state: State<'_, Db>, enabled: bool) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::setting::set(
        &conn,
        REMOTE_ENABLED_FOR_REVIEW_KEY,
        if enabled { "1" } else { "0" },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remote_call_log_list(
    state: State<'_, Db>,
    limit: usize,
) -> Result<Vec<CallLogEntry>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    remote_call_log::list_recent(&conn, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remote_call_log_clear(state: State<'_, Db>) -> Result<usize, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    remote_call_log::clear_all(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_test(state: State<'_, Db>) -> Result<String, String> {
    let db_arc = state.inner().clone_arc();
    let outcome = orchestrator::remote_chat(
        db_arc,
        orchestrator::RemoteChatRequest {
            skill: "test",
            user_visible_reason: "User-initiated test call from Settings",
            system_prompt: Some("You are a test responder. Reply with exactly 'pong'."),
            user_prompt: "ping",
            max_tokens: 10,
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(outcome.text)
}
