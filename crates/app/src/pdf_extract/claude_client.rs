//! Claude LlmClient adapter for PDF extraction (L4e).
//!
//! Routes through the shared remote::orchestrator::remote_chat (skill=pdf_extract)
//! so redaction, budget, and audit logging apply. Captures the orchestrator's
//! RemoteCallLog row id so the pipeline can stamp it on inserted proposals.

use async_trait::async_trait;
use manor_core::recipe::import::LlmClient;
use std::sync::{Arc, Mutex};

pub struct ClaudeExtractClient {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub asset_name: String,
    pub remote_call_log_id_sink: Arc<Mutex<Option<i64>>>,
}

#[async_trait]
impl LlmClient for ClaudeExtractClient {
    async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        let reason = format!("Extract schedules from {} manual", self.asset_name);
        let req = crate::remote::orchestrator::RemoteChatRequest {
            skill: "pdf_extract",
            user_visible_reason: &reason,
            system_prompt: None,
            user_prompt: prompt,
            max_tokens: 2048,
        };
        let outcome = crate::remote::orchestrator::remote_chat(self.db.clone(), req)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        *self.remote_call_log_id_sink.lock().unwrap() = Some(outcome.log_id);
        Ok(outcome.text)
    }
}
