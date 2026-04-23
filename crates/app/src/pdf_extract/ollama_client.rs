//! Ollama LlmClient adapter for PDF extraction (L4e).

use async_trait::async_trait;
use manor_core::recipe::import::LlmClient;

pub struct OllamaExtractClient {
    model: String,
}

impl OllamaExtractClient {
    /// Caller must resolve the model from `ai.default_model` via
    /// `crate::assistant::ollama::resolve_model` before constructing this.
    pub fn new(model: String) -> Self {
        Self { model }
    }
}

#[async_trait]
impl LlmClient for OllamaExtractClient {
    async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        use crate::assistant::ollama::{
            ChatMessage, ChatRole, OllamaClient, DEFAULT_ENDPOINT,
        };
        let client = OllamaClient::new(DEFAULT_ENDPOINT, &self.model);
        client
            .chat_collect(&[ChatMessage {
                role: ChatRole::User,
                content: prompt.to_string(),
            }])
            .await
    }
}
