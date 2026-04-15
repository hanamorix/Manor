//! Ollama HTTP streaming client.
//!
//! Posts to Ollama's `/api/chat` with `stream=true` and yields `StreamChunk`s as
//! NDJSON lines arrive.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub const DEFAULT_ENDPOINT: &str = "http://localhost:11434";
pub const DEFAULT_MODEL: &str = "qwen2.5:7b-instruct";

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

#[derive(Debug, Clone, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaChunk {
    #[serde(default)]
    message: Option<OllamaChunkMessage>,
    #[serde(default)]
    done: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaChunkMessage {
    #[allow(dead_code)]
    role: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value")]
pub enum StreamChunk {
    /// Emitted once before any tokens; carries the new assistant row id so the
    /// frontend can mark-seen the right DB row when the bubble fades.
    Started(i64),
    Token(String),
    Done,
    Error(ErrorCode),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    OllamaUnreachable,
    ModelMissing,
    Interrupted,
    Unknown,
}

pub struct OllamaClient {
    endpoint: String,
    model: String,
    http: reqwest::Client,
}

impl OllamaClient {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            http: reqwest::Client::new(),
        }
    }

    /// Send `messages` to Ollama and stream tokens into the provided channel.
    /// The final message is either `StreamChunk::Done` or `StreamChunk::Error(_)`.
    pub async fn chat(&self, messages: &[ChatMessage], out: mpsc::Sender<StreamChunk>) {
        let url = format!("{}/api/chat", self.endpoint);
        let body = ChatRequest {
            model: &self.model,
            messages,
            stream: true,
        };

        let resp = match self.http.post(&url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                let code = if e.is_connect() {
                    ErrorCode::OllamaUnreachable
                } else {
                    ErrorCode::Unknown
                };
                let _ = out.send(StreamChunk::Error(code)).await;
                return;
            }
        };

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            let _ = out.send(StreamChunk::Error(ErrorCode::ModelMissing)).await;
            return;
        }
        if !resp.status().is_success() {
            let _ = out.send(StreamChunk::Error(ErrorCode::Unknown)).await;
            return;
        }

        let mut stream = resp.bytes_stream();
        let mut buf = Vec::<u8>::new();

        while let Some(piece) = stream.next().await {
            let bytes = match piece {
                Ok(b) => b,
                Err(_) => {
                    let _ = out.send(StreamChunk::Error(ErrorCode::Interrupted)).await;
                    return;
                }
            };
            buf.extend_from_slice(&bytes);

            while let Some(nl) = buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = buf.drain(..=nl).collect();
                let line = &line[..line.len().saturating_sub(1)]; // strip trailing \n
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_slice::<OllamaChunk>(line) {
                    Ok(chunk) => {
                        if let Some(c) = chunk.message.as_ref().and_then(|m| m.content.clone()) {
                            if !c.is_empty() {
                                let _ = out.send(StreamChunk::Token(c)).await;
                            }
                        }
                        if chunk.done {
                            let _ = out.send(StreamChunk::Done).await;
                            return;
                        }
                    }
                    Err(_) => {
                        let _ = out.send(StreamChunk::Error(ErrorCode::Unknown)).await;
                        return;
                    }
                }
            }
        }

        // Stream ended without a `done:true` line — treat as interrupted.
        let _ = out.send(StreamChunk::Error(ErrorCode::Interrupted)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn ndjson(lines: &[&str]) -> String {
        lines
            .iter()
            .map(|l| format!("{l}\n"))
            .collect::<Vec<_>>()
            .join("")
    }

    #[tokio::test]
    async fn streams_tokens_then_done() {
        let server = MockServer::start().await;

        let body = ndjson(&[
            r#"{"message":{"role":"assistant","content":"Hel"},"done":false}"#,
            r#"{"message":{"role":"assistant","content":"lo."},"done":false}"#,
            r#"{"message":{"role":"assistant","content":""},"done":true}"#,
        ]);

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "test-model");
        let (tx, mut rx) = mpsc::channel(32);

        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: "hi".into(),
        }];
        client.chat(&messages, tx).await;

        let mut received = Vec::new();
        while let Some(c) = rx.recv().await {
            received.push(c);
        }

        assert_eq!(
            received,
            vec![
                StreamChunk::Token("Hel".into()),
                StreamChunk::Token("lo.".into()),
                StreamChunk::Done,
            ]
        );
    }

    #[tokio::test]
    async fn unreachable_emits_ollama_unreachable() {
        // port 1 is essentially guaranteed closed
        let client = OllamaClient::new("http://127.0.0.1:1", "test-model");
        let (tx, mut rx) = mpsc::channel(4);

        client
            .chat(
                &[ChatMessage {
                    role: ChatRole::User,
                    content: "hi".into(),
                }],
                tx,
            )
            .await;

        let first = rx.recv().await.unwrap();
        assert_eq!(first, StreamChunk::Error(ErrorCode::OllamaUnreachable));
    }

    #[tokio::test]
    async fn not_found_emits_model_missing() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "nonexistent-model");
        let (tx, mut rx) = mpsc::channel(4);

        client
            .chat(
                &[ChatMessage {
                    role: ChatRole::User,
                    content: "hi".into(),
                }],
                tx,
            )
            .await;

        let first = rx.recv().await.unwrap();
        assert_eq!(first, StreamChunk::Error(ErrorCode::ModelMissing));
    }
}
