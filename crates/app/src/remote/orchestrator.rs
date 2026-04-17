//! The remote chat orchestrator — the single entry point that glues together
//! redaction, keychain, budget check, provider call, and audit logging.
//!
//! Every outbound remote call MUST go through `remote_chat`. Skills that bypass
//! this wiring are a design failure.

use super::claude::Claude;
use super::provider::{cost_for, ChatMessage, ChatResponse, RemoteProvider};
use super::{
    budget_setting_key, keychain, DEFAULT_BUDGET_PENCE, DEFAULT_MODEL_CLAUDE, PROVIDER_CLAUDE,
    WARN_THRESHOLD_NUM,
};
use anyhow::Result;
use chrono::Utc;
use manor_core::{redact, remote_call_log};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct RemoteChatRequest<'a> {
    pub skill: &'a str,                 // e.g., "ledger_review"
    pub user_visible_reason: &'a str,   // shown in call log
    pub system_prompt: Option<&'a str>, // optional system prompt (static, not redacted)
    pub user_prompt: &'a str,           // the bit that gets redacted
    pub max_tokens: i64,
}

pub struct RemoteChatOutcome {
    pub text: String,
    pub warn: bool, // hit the 75% cap but proceeded
    pub cost_pence: i64,
    pub log_id: i64,
    pub redaction_count: usize,
}

/// Errors the orchestrator surfaces. Distinct from generic `anyhow` so the UI
/// can pick friendly messaging per case.
#[derive(Debug, thiserror::Error)]
pub enum RemoteChatError {
    #[error("no api key stored for provider '{0}'")]
    NoKey(String),
    #[error("budget exceeded for provider '{0}' (spent {1}p of {2}p)")]
    BudgetExceeded(String, i64, i64),
    #[error("provider error: {0}")]
    Provider(#[from] anyhow::Error),
    #[error("db error: {0}")]
    Db(String),
}

pub async fn remote_chat(
    db: Arc<Mutex<Connection>>,
    req: RemoteChatRequest<'_>,
) -> std::result::Result<RemoteChatOutcome, RemoteChatError> {
    let provider_name = PROVIDER_CLAUDE;
    let model = DEFAULT_MODEL_CLAUDE;

    // Step 1: redact.
    let redacted = redact::redact(req.user_prompt);
    let redaction_count = redacted.count();

    // Step 2: read budget + key.
    let (api_key, cap_pence, spent_pence) = {
        let conn = db.lock().map_err(|e| RemoteChatError::Db(e.to_string()))?;
        let cap = manor_core::setting::get_or_default(
            &conn,
            &budget_setting_key(provider_name),
            &DEFAULT_BUDGET_PENCE.to_string(),
        )
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(DEFAULT_BUDGET_PENCE);
        let spent = remote_call_log::sum_month_pence(&conn, provider_name, Utc::now())
            .map_err(|e| RemoteChatError::Db(e.to_string()))?;
        let key = keychain::get_key(provider_name)
            .map_err(|e| RemoteChatError::Db(e.to_string()))?
            .ok_or_else(|| RemoteChatError::NoKey(provider_name.to_string()))?;
        (key, cap, spent)
    };

    if spent_pence >= cap_pence {
        // Log the refusal for auditability.
        let conn = db.lock().map_err(|e| RemoteChatError::Db(e.to_string()))?;
        let log_id = remote_call_log::insert_started(
            &conn,
            remote_call_log::NewCall {
                provider: provider_name,
                model,
                skill: req.skill,
                user_visible_reason: req.user_visible_reason,
                prompt_redacted: &redacted.text,
                redaction_count: redaction_count as i64,
            },
        )
        .map_err(|e| RemoteChatError::Db(e.to_string()))?;
        remote_call_log::mark_errored(&conn, log_id, "budget exceeded — refused before send")
            .map_err(|e| RemoteChatError::Db(e.to_string()))?;
        return Err(RemoteChatError::BudgetExceeded(
            provider_name.to_string(),
            spent_pence,
            cap_pence,
        ));
    }

    // Step 3: start log row.
    let log_id = {
        let conn = db.lock().map_err(|e| RemoteChatError::Db(e.to_string()))?;
        remote_call_log::insert_started(
            &conn,
            remote_call_log::NewCall {
                provider: provider_name,
                model,
                skill: req.skill,
                user_visible_reason: req.user_visible_reason,
                prompt_redacted: &redacted.text,
                redaction_count: redaction_count as i64,
            },
        )
        .map_err(|e| RemoteChatError::Db(e.to_string()))?
    };

    // Step 4: call provider.
    let client = Claude::new();
    let msgs = vec![ChatMessage {
        role: super::provider::ChatRole::User,
        content: redacted.text.clone(),
    }];
    let chat_result: Result<ChatResponse> = client
        .chat(&api_key, model, &msgs, req.system_prompt, req.max_tokens)
        .await;

    match chat_result {
        Ok(resp) => {
            let cost_pence =
                cost_for(provider_name, model).pence_for(resp.input_tokens, resp.output_tokens);
            let conn = db.lock().map_err(|e| RemoteChatError::Db(e.to_string()))?;
            remote_call_log::mark_completed(
                &conn,
                log_id,
                &resp.text,
                resp.input_tokens,
                resp.output_tokens,
                cost_pence,
            )
            .map_err(|e| RemoteChatError::Db(e.to_string()))?;

            let warn = (spent_pence + cost_pence) * 100 >= cap_pence * WARN_THRESHOLD_NUM;
            Ok(RemoteChatOutcome {
                text: resp.text,
                warn,
                cost_pence,
                log_id,
                redaction_count,
            })
        }
        Err(e) => {
            let msg = e.to_string();
            let conn = db.lock().map_err(|x| RemoteChatError::Db(x.to_string()))?;
            let _ = remote_call_log::mark_errored(&conn, log_id, &msg);
            Err(RemoteChatError::Provider(e))
        }
    }
}
