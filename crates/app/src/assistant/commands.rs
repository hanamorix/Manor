//! Tauri commands exposed to the frontend Assistant.

use crate::assistant::ollama::{
    ChatMessage, ChatRole, OllamaClient, StreamChunk, DEFAULT_ENDPOINT, DEFAULT_MODEL,
};
use crate::assistant::prompts::SYSTEM_PROMPT;
use manor_core::assistant::{conversation, db, message, message::Role};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::mpsc;

pub struct Db(pub Mutex<Connection>);

impl Db {
    pub fn open(path: PathBuf) -> anyhow::Result<Self> {
        let conn = db::init(&path)?;
        Ok(Self(Mutex::new(conn)))
    }
}

const CONTEXT_WINDOW: u32 = 20;

#[tauri::command]
pub async fn send_message(
    state: State<'_, Db>,
    content: String,
    on_event: Channel<StreamChunk>,
) -> Result<(), String> {
    // 1. Persist the user message and insert a placeholder assistant row.
    let (assistant_row_id, history) = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        let conv = conversation::get_or_create_default(&conn).map_err(|e| e.to_string())?;
        message::insert(&conn, conv.id, Role::User, &content).map_err(|e| e.to_string())?;
        let assistant_row_id =
            message::insert(&conn, conv.id, Role::Assistant, "").map_err(|e| e.to_string())?;
        let recent = message::list(&conn, conv.id, CONTEXT_WINDOW, 0).map_err(|e| e.to_string())?;
        (assistant_row_id, recent)
    };

    // 2. Build the chat-message history (including the system prompt) to send to Ollama.
    let mut chat_msgs: Vec<ChatMessage> = vec![ChatMessage {
        role: ChatRole::System,
        content: SYSTEM_PROMPT.into(),
    }];
    for m in history {
        if m.content.is_empty() {
            continue; // skip the empty placeholder we just inserted
        }
        let role = match m.role {
            Role::User => ChatRole::User,
            Role::Assistant => ChatRole::Assistant,
            Role::System => ChatRole::System,
        };
        chat_msgs.push(ChatMessage {
            role,
            content: m.content,
        });
    }

    // 3. Tell the frontend the real assistant row id so it can mark-seen the right
    //    DB row when the bubble fades.
    on_event
        .send(StreamChunk::Started(assistant_row_id))
        .map_err(|e| e.to_string())?;

    // 4. Run the Ollama stream, forwarding each chunk to both the DB and the frontend.
    let client = OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL);
    let (tx, mut rx) = mpsc::channel::<StreamChunk>(64);

    // Spawn the HTTP call on a background task.
    let chat_task = tokio::spawn(async move {
        client.chat(&chat_msgs, tx).await;
    });

    while let Some(chunk) = rx.recv().await {
        match &chunk {
            StreamChunk::Token(frag) => {
                // Persist incrementally.
                let conn = state.0.lock().map_err(|e| e.to_string())?;
                message::append_content(&conn, assistant_row_id, frag)
                    .map_err(|e| e.to_string())?;
            }
            StreamChunk::Started(_) | StreamChunk::Done | StreamChunk::Error(_) => {}
        }
        on_event.send(chunk).map_err(|e| e.to_string())?;
    }

    let _ = chat_task.await;
    Ok(())
}

#[tauri::command]
pub fn list_messages(
    state: State<'_, Db>,
    limit: u32,
    offset: u32,
) -> Result<Vec<message::Message>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let conv = conversation::get_or_create_default(&conn).map_err(|e| e.to_string())?;
    message::list(&conn, conv.id, limit, offset).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_unread_count(state: State<'_, Db>) -> Result<u32, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let conv = conversation::get_or_create_default(&conn).map_err(|e| e.to_string())?;
    message::unread_count(&conn, conv.id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mark_seen(state: State<'_, Db>, message_ids: Vec<i64>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    message::mark_seen(&conn, &message_ids).map_err(|e| e.to_string())
}
