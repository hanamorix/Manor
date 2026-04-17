//! Ollama HTTP streaming client.

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
    tools: &'a [serde_json::Value],
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
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OllamaToolCall {
    pub function: OllamaToolFunction,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OllamaToolFunction {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value")]
pub enum StreamChunk {
    /// Emitted once before any tokens; carries the new assistant row id so the
    /// frontend can mark-seen the right DB row when the bubble fades.
    Started(i64),
    Token(String),
    Proposal(i64),
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

/// Outcome of a `chat()` invocation: the collected tool calls (if any) for the caller
/// to act on. Tokens / errors / Done were emitted to the channel as they arrived.
#[derive(Debug, Default)]
pub struct ChatOutcome {
    pub tool_calls: Vec<OllamaToolCall>,
}

impl OllamaClient {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            http: reqwest::Client::new(),
        }
    }

    /// Send `messages` to Ollama (with `tools` declared) and stream tokens into `out`.
    /// Returns a `ChatOutcome` containing any tool calls the model emitted at end of stream.
    /// The caller is responsible for emitting the final `StreamChunk::Done` after handling
    /// any tool calls — this function does NOT emit Done itself, only Token + Error chunks.
    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[serde_json::Value],
        out: &mpsc::Sender<StreamChunk>,
    ) -> ChatOutcome {
        let url = format!("{}/api/chat", self.endpoint);
        let body = ChatRequest {
            model: &self.model,
            messages,
            stream: true,
            tools,
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
                return ChatOutcome::default();
            }
        };

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            let _ = out.send(StreamChunk::Error(ErrorCode::ModelMissing)).await;
            return ChatOutcome::default();
        }
        if !resp.status().is_success() {
            let _ = out.send(StreamChunk::Error(ErrorCode::Unknown)).await;
            return ChatOutcome::default();
        }

        let mut stream = resp.bytes_stream();
        let mut buf = Vec::<u8>::new();
        let mut collected_tool_calls = Vec::<OllamaToolCall>::new();

        while let Some(piece) = stream.next().await {
            let bytes = match piece {
                Ok(b) => b,
                Err(_) => {
                    let _ = out.send(StreamChunk::Error(ErrorCode::Interrupted)).await;
                    return ChatOutcome {
                        tool_calls: collected_tool_calls,
                    };
                }
            };
            buf.extend_from_slice(&bytes);

            while let Some(nl) = buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = buf.drain(..=nl).collect();
                let line = &line[..line.len().saturating_sub(1)];
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_slice::<OllamaChunk>(line) {
                    Ok(chunk) => {
                        if let Some(msg) = chunk.message.as_ref() {
                            if let Some(c) = msg.content.as_ref() {
                                if !c.is_empty() {
                                    let _ = out.send(StreamChunk::Token(c.clone())).await;
                                }
                            }
                            if !msg.tool_calls.is_empty() {
                                collected_tool_calls.extend(msg.tool_calls.iter().cloned());
                            }
                        }
                        if chunk.done {
                            // Don't emit Done here — caller handles it after acting on tool_calls.
                            return ChatOutcome {
                                tool_calls: collected_tool_calls,
                            };
                        }
                    }
                    Err(_) => {
                        let _ = out.send(StreamChunk::Error(ErrorCode::Unknown)).await;
                        return ChatOutcome {
                            tool_calls: collected_tool_calls,
                        };
                    }
                }
            }
        }

        // Stream ended without a `done:true` line — treat as interrupted.
        let _ = out.send(StreamChunk::Error(ErrorCode::Interrupted)).await;
        ChatOutcome {
            tool_calls: collected_tool_calls,
        }
    }

    /// Request embeddings from Ollama's /api/embeddings endpoint.
    /// Note: uses `self.model` — caller must configure an embedding model
    /// (e.g., nomic-embed-text) rather than a chat model.
    pub async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            prompt: &'a str,
        }
        #[derive(Deserialize)]
        struct Resp {
            embedding: Vec<f32>,
        }
        let url = format!("{}/api/embeddings", self.endpoint);
        let body = Req {
            model: &self.model,
            prompt: text,
        };
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("embed http: {e}"))?;
        anyhow::ensure!(
            resp.status().is_success(),
            "ollama /api/embeddings returned {}",
            resp.status()
        );
        let parsed: Resp = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("embed parse: {e}"))?;
        anyhow::ensure!(!parsed.embedding.is_empty(), "empty embedding returned");
        Ok(parsed.embedding)
    }

    /// Non-streaming completion — POST /api/generate with stream:false.
    /// Returns the full response string in one shot. Use for short batch
    /// calls that don't benefit from streaming (autocat, categorisation,
    /// single-turn classification). The model is the one configured on
    /// this client (typically a chat model).
    pub async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            prompt: &'a str,
            stream: bool,
        }
        #[derive(Deserialize)]
        struct Resp {
            response: String,
        }
        let url = format!("{}/api/generate", self.endpoint);
        let body = Req {
            model: &self.model,
            prompt,
            stream: false,
        };
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("complete http: {e}"))?;
        anyhow::ensure!(
            resp.status().is_success(),
            "ollama /api/generate returned {}",
            resp.status()
        );
        let parsed: Resp = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("complete parse: {e}"))?;
        Ok(parsed.response)
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
    async fn streams_tokens_then_returns_no_tool_calls_on_done() {
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
        let outcome = client.chat(&messages, &[], &tx).await;

        let mut received = Vec::new();
        while let Ok(c) = rx.try_recv() {
            received.push(c);
        }
        assert_eq!(
            received,
            vec![
                StreamChunk::Token("Hel".into()),
                StreamChunk::Token("lo.".into()),
            ]
        );
        assert!(outcome.tool_calls.is_empty());
    }

    #[tokio::test]
    async fn streams_content_then_tool_call() {
        let server = MockServer::start().await;
        let body = ndjson(&[
            r#"{"message":{"role":"assistant","content":"I'd like to add that."},"done":false}"#,
            r#"{"message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"add_task","arguments":{"title":"Pick up prescription"}}}]},"done":true}"#,
        ]);
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "test-model");
        let (tx, mut rx) = mpsc::channel(32);
        let outcome = client
            .chat(
                &[ChatMessage {
                    role: ChatRole::User,
                    content: "remind me to pick up prescription".into(),
                }],
                &[],
                &tx,
            )
            .await;

        let mut received = Vec::new();
        while let Ok(c) = rx.try_recv() {
            received.push(c);
        }
        assert_eq!(
            received,
            vec![StreamChunk::Token("I'd like to add that.".into())]
        );

        assert_eq!(outcome.tool_calls.len(), 1);
        assert_eq!(outcome.tool_calls[0].function.name, "add_task");
        assert_eq!(
            outcome.tool_calls[0].function.arguments["title"],
            "Pick up prescription"
        );
    }

    #[tokio::test]
    async fn unreachable_emits_ollama_unreachable() {
        let client = OllamaClient::new("http://127.0.0.1:1", "test-model");
        let (tx, mut rx) = mpsc::channel(4);
        let _ = client
            .chat(
                &[ChatMessage {
                    role: ChatRole::User,
                    content: "hi".into(),
                }],
                &[],
                &tx,
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
        let _ = client
            .chat(
                &[ChatMessage {
                    role: ChatRole::User,
                    content: "hi".into(),
                }],
                &[],
                &tx,
            )
            .await;
        let first = rx.recv().await.unwrap();
        assert_eq!(first, StreamChunk::Error(ErrorCode::ModelMissing));
    }

    #[tokio::test]
    async fn embed_returns_vector_from_valid_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embedding": [0.1, 0.2, 0.3]
            })))
            .mount(&server)
            .await;
        let client = OllamaClient::new(server.uri(), "nomic-embed-text");
        let v = client.embed("hello").await.unwrap();
        assert_eq!(v, vec![0.1, 0.2, 0.3]);
    }

    #[tokio::test]
    async fn embed_errors_on_empty_vector() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embedding": []
            })))
            .mount(&server)
            .await;
        let client = OllamaClient::new(server.uri(), "nomic-embed-text");
        let err = client.embed("hello").await.unwrap_err();
        assert!(err.to_string().to_lowercase().contains("empty"));
    }
}
