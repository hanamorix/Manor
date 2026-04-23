//! Repair-note synthesis (L4d).

use crate::assistant::ollama::{
    ChatMessage, ChatRole, OllamaClient, DEFAULT_ENDPOINT,
};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

pub struct PageExcerpt {
    pub url: String,
    pub title: String,
    pub trimmed_text: String,
}

pub struct SynthInput<'a> {
    pub asset_name: &'a str,
    pub asset_make: Option<&'a str>,
    pub asset_model: Option<&'a str>,
    pub asset_category: &'a str,
    pub symptom: &'a str,
    pub augmented_query: &'a str,
    pub pages: &'a [PageExcerpt],
}

const SYSTEM_PROMPT: &str = "You are a concise home-repair troubleshooter. You help a homeowner diagnose appliance, vehicle, and fixture problems using search-result excerpts.";

pub fn build_user_prompt(input: &SynthInput<'_>) -> String {
    let make = input.asset_make.unwrap_or("unknown");
    let model = input.asset_model.unwrap_or("unknown");
    let mut out = String::new();
    out.push_str("You are helping a homeowner troubleshoot an appliance or fixture problem.\n\n");
    out.push_str("## About the item\n");
    out.push_str(&format!("- Name: {}\n", input.asset_name));
    out.push_str(&format!("- Make: {}\n", make));
    out.push_str(&format!("- Model: {}\n", model));
    out.push_str(&format!("- Category: {}\n\n", input.asset_category));
    out.push_str("## Reported symptom\n");
    out.push_str(input.symptom);
    out.push_str("\n\n");
    out.push_str(&format!(
        "## Search results (trimmed excerpts from the top {} pages)\n",
        input.pages.len()
    ));
    for (i, page) in input.pages.iter().enumerate() {
        out.push_str(&format!("[Source {} — {}]\n", i + 1, page.url));
        out.push_str(&page.trimmed_text);
        out.push_str("\n\n");
    }
    out.push_str(
        "## Your task\n\
         Synthesise a concise troubleshooting summary (150–300 words).\n\n\
         Requirements:\n\
         - Start with the most likely cause in plain language.\n\
         - List 2–4 specific things the user can check or try, in order.\n\
         - Flag any \"call a professional\" cases (gas, high voltage, sealed systems).\n\
         - At the end, list the source URLs as a Markdown bulleted list under \"## Sources\".\n\
         - Do NOT invent model-specific steps that aren't in the excerpts.\n\
         - If the excerpts are thin or off-topic, say so and suggest a more specific search.\n",
    );
    out
}

#[async_trait]
pub trait SynthBackend: Send + Sync {
    async fn synth(&self, input: &SynthInput<'_>) -> Result<String>;
}

pub struct OllamaSynth {
    model: String,
}

impl OllamaSynth {
    /// Caller must resolve the model from `ai.default_model` via
    /// `crate::assistant::ollama::resolve_model` before constructing this.
    pub fn new(model: String) -> Self {
        Self { model }
    }
}

#[async_trait]
impl SynthBackend for OllamaSynth {
    async fn synth(&self, input: &SynthInput<'_>) -> Result<String> {
        let client = OllamaClient::new(DEFAULT_ENDPOINT, &self.model);
        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: SYSTEM_PROMPT.to_string(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: build_user_prompt(input),
            },
        ];
        client.chat_collect(&messages).await
    }
}

pub struct ClaudeSynth {
    pub db: Arc<Mutex<rusqlite::Connection>>,
}

#[async_trait]
impl SynthBackend for ClaudeSynth {
    async fn synth(&self, input: &SynthInput<'_>) -> Result<String> {
        let user_prompt = build_user_prompt(input);
        let reason = format!("Troubleshooting {}", input.asset_name);
        let req = crate::remote::orchestrator::RemoteChatRequest {
            skill: "right_to_repair",
            user_visible_reason: &reason,
            system_prompt: Some(SYSTEM_PROMPT),
            user_prompt: &user_prompt,
            max_tokens: 1024,
        };
        let outcome = crate::remote::orchestrator::remote_chat(self.db.clone(), req)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(outcome.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(url: &str, text: &str) -> PageExcerpt {
        PageExcerpt {
            url: url.into(),
            title: "t".into(),
            trimmed_text: text.into(),
        }
    }

    #[test]
    fn build_user_prompt_includes_all_fields() {
        let pages = vec![
            page("https://example.com/a", "excerpt A text"),
            page("https://example.com/b", "excerpt B text"),
        ];
        let input = SynthInput {
            asset_name: "Worcester Boiler",
            asset_make: Some("Worcester"),
            asset_model: Some("Bosch 8000"),
            asset_category: "appliance",
            symptom: "won't fire up",
            augmented_query: "Worcester Bosch 8000 won't fire up",
            pages: &pages,
        };
        let p = build_user_prompt(&input);
        assert!(p.contains("Name: Worcester Boiler"));
        assert!(p.contains("Make: Worcester"));
        assert!(p.contains("Model: Bosch 8000"));
        assert!(p.contains("Category: appliance"));
        assert!(p.contains("won't fire up"));
        assert!(p.contains("top 2 pages"));
        assert!(p.contains("[Source 1 — https://example.com/a]"));
        assert!(p.contains("excerpt A text"));
        assert!(p.contains("[Source 2 — https://example.com/b]"));
    }

    #[test]
    fn build_user_prompt_handles_missing_make_model() {
        let pages = vec![page("https://example.com/a", "text")];
        let input = SynthInput {
            asset_name: "Something",
            asset_make: None,
            asset_model: None,
            asset_category: "other",
            symptom: "broken",
            augmented_query: "broken",
            pages: &pages,
        };
        let p = build_user_prompt(&input);
        assert!(p.contains("Make: unknown"));
        assert!(p.contains("Model: unknown"));
    }

    #[test]
    fn build_user_prompt_handles_partial_page_count() {
        let pages = vec![page("https://a", "only one")];
        let input = SynthInput {
            asset_name: "X",
            asset_make: None,
            asset_model: None,
            asset_category: "other",
            symptom: "s",
            augmented_query: "s",
            pages: &pages,
        };
        let p = build_user_prompt(&input);
        assert!(p.contains("top 1 pages"));
        assert!(p.contains("[Source 1 —"));
        assert!(!p.contains("[Source 2 —"));
    }
}
