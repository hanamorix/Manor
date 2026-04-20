//! Ollama LlmClient adapter for PDF extraction (L4e).

use async_trait::async_trait;
use manor_core::recipe::import::LlmClient;

pub struct OllamaExtractClient;

#[async_trait]
impl LlmClient for OllamaExtractClient {
    async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        use crate::assistant::ollama::{
            ChatMessage, ChatRole, OllamaClient, DEFAULT_ENDPOINT, DEFAULT_MODEL,
        };
        let client = OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL);
        client
            .chat_collect(&[ChatMessage {
                role: ChatRole::User,
                content: prompt.to_string(),
            }])
            .await
    }
}
