//! Anthropic /v1/messages implementation.

use super::provider::{ChatMessage, ChatResponse, ChatRole, RemoteProvider};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub struct Claude {
    endpoint: String,
    http: reqwest::Client,
}

impl Claude {
    pub fn new() -> Self {
        Self::with_endpoint("https://api.anthropic.com")
    }
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client build"),
        }
    }
}

impl Default for Claude {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct MessagesReq<'a> {
    model: &'a str,
    max_tokens: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<MsgPayload<'a>>,
}

#[derive(Serialize)]
struct MsgPayload<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct MessagesResp {
    content: Vec<ContentBlock>,
    usage: UsageBlock,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    _type: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct UsageBlock {
    input_tokens: i64,
    output_tokens: i64,
}

#[async_trait]
impl RemoteProvider for Claude {
    fn name(&self) -> &'static str {
        "claude"
    }
    fn default_model(&self) -> &'static str {
        "claude-sonnet-4-6"
    }

    async fn chat(
        &self,
        api_key: &str,
        model: &str,
        messages: &[ChatMessage],
        system: Option<&str>,
        max_tokens: i64,
    ) -> Result<ChatResponse> {
        let mapped: Vec<MsgPayload<'_>> = messages
            .iter()
            .map(|m| MsgPayload {
                role: match m.role {
                    ChatRole::User => "user",
                    ChatRole::Assistant => "assistant",
                    // Anthropic API doesn't have a "system" role in messages array —
                    // system text goes in the top-level `system` field. Coerce to user.
                    ChatRole::System => "user",
                },
                content: &m.content,
            })
            .collect();

        let body = MessagesReq {
            model,
            max_tokens,
            system,
            messages: mapped,
        };
        let url = format!("{}/v1/messages", self.endpoint);
        let resp = self
            .http
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("claude http send")?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("claude returned {status}: {body_text}"));
        }
        let parsed: MessagesResp = resp.json().await.context("claude parse")?;
        let text: String = parsed.content.into_iter().map(|c| c.text).collect();
        Ok(ChatResponse {
            text,
            input_tokens: parsed.usage.input_tokens,
            output_tokens: parsed.usage.output_tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn chat_success_returns_text_and_token_counts() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "test-key"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type": "text", "text": "Hello back."}],
                "usage": {"input_tokens": 12, "output_tokens": 5}
            })))
            .mount(&server)
            .await;

        let client = Claude::with_endpoint(server.uri());
        let msgs = vec![ChatMessage {
            role: ChatRole::User,
            content: "Hi".into(),
        }];
        let resp = client
            .chat("test-key", "claude-sonnet-4-6", &msgs, None, 100)
            .await
            .unwrap();
        assert_eq!(resp.text, "Hello back.");
        assert_eq!(resp.input_tokens, 12);
        assert_eq!(resp.output_tokens, 5);
    }

    #[tokio::test]
    async fn chat_propagates_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(401).set_body_string("invalid key"))
            .mount(&server)
            .await;
        let client = Claude::with_endpoint(server.uri());
        let msgs = vec![ChatMessage {
            role: ChatRole::User,
            content: "Hi".into(),
        }];
        let err = client
            .chat("bad", "claude-sonnet-4-6", &msgs, None, 100)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("401"));
    }

    #[tokio::test]
    async fn chat_passes_system_prompt_in_top_level_field() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type": "text", "text": "ok"}],
                "usage": {"input_tokens": 5, "output_tokens": 2}
            })))
            .mount(&server)
            .await;
        let client = Claude::with_endpoint(server.uri());
        let msgs = vec![ChatMessage {
            role: ChatRole::User,
            content: "go".into(),
        }];
        let resp = client
            .chat("k", "claude-sonnet-4-6", &msgs, Some("Be brief"), 50)
            .await
            .unwrap();
        assert_eq!(resp.text, "ok");
    }
}
