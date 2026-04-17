//! RemoteProvider trait + shared types + cost table.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub text: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

/// Static per-model cost table, pence per million tokens.
/// Updated manually when providers publish price changes.
pub struct ModelCost {
    pub input_per_m: i64,
    pub output_per_m: i64,
}

pub fn cost_for(provider: &str, model: &str) -> ModelCost {
    // Pence per million tokens, rounded up from provider's list prices.
    match (provider, model) {
        ("claude", "claude-opus-4-7") => ModelCost {
            input_per_m: 240,
            output_per_m: 1200,
        }
        .mul(10),
        ("claude", "claude-sonnet-4-6") => ModelCost {
            input_per_m: 80,
            output_per_m: 400,
        }
        .mul(10),
        ("claude", "claude-haiku-4-5-20251001") => ModelCost {
            input_per_m: 12,
            output_per_m: 60,
        }
        .mul(10),
        // Unknown model: charge nothing rather than guess. Logs but never rejects.
        _ => ModelCost {
            input_per_m: 0,
            output_per_m: 0,
        },
    }
}

impl ModelCost {
    // Internal helper to keep the table readable in penny-per-10K-tokens, then scale.
    fn mul(self, n: i64) -> Self {
        ModelCost {
            input_per_m: self.input_per_m * n,
            output_per_m: self.output_per_m * n,
        }
    }
    pub fn pence_for(&self, input_tokens: i64, output_tokens: i64) -> i64 {
        // Round UP — user sees accurate or over-estimated spend, never under.
        let input_pence = (input_tokens * self.input_per_m + 999_999) / 1_000_000;
        let output_pence = (output_tokens * self.output_per_m + 999_999) / 1_000_000;
        input_pence + output_pence
    }
}

#[async_trait]
pub trait RemoteProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn default_model(&self) -> &'static str;

    async fn chat(
        &self,
        api_key: &str,
        model: &str,
        messages: &[ChatMessage],
        system: Option<&str>,
        max_tokens: i64,
    ) -> Result<ChatResponse>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_sonnet_cost_basic() {
        let c = cost_for("claude", "claude-sonnet-4-6");
        // 1000 input + 500 output tokens → tiny pence
        let p = c.pence_for(1000, 500);
        // Sonnet 4.6: 80*10 = 800 pence/M input, 400*10 = 4000 pence/M output
        // => (1000 * 800 + 999_999) / 1_000_000 = 1 pence (rounded up from 0.8)
        //    (500 * 4000 + 999_999) / 1_000_000 = 2 pence (rounded up from 2.0)
        assert_eq!(p, 3);
    }

    #[test]
    fn unknown_model_costs_zero() {
        let c = cost_for("claude", "bogus-model");
        assert_eq!(c.pence_for(1_000_000, 1_000_000), 0);
    }
}
