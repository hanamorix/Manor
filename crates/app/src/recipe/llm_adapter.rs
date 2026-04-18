//! Thin adapter: wraps OllamaClient so it satisfies manor_core's LlmClient trait.

use crate::assistant::ollama::OllamaClient;
use manor_core::recipe::import::LlmClient;

pub struct OllamaLlmAdapter(pub OllamaClient);

#[async_trait::async_trait]
impl LlmClient for OllamaLlmAdapter {
    async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        self.0.complete(prompt).await
    }
}
