//! Tauri commands exposed to the frontend Assistant + Today view.

use crate::assistant::ollama::{
    ChatMessage, ChatRole, OllamaClient, StreamChunk, DEFAULT_ENDPOINT, DEFAULT_MODEL,
};
use crate::assistant::prompts::SYSTEM_PROMPT;
use crate::assistant::tools;
use chrono::Local;
use manor_core::assistant::{
    conversation, db, message,
    message::Role,
    proposal::{self, AddTaskArgs, NewProposal, Proposal},
    task::{self, Task},
};
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

fn today_local_iso() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, Db>,
    content: String,
    on_event: Channel<StreamChunk>,
) -> Result<(), String> {
    // 1. Persist user message + placeholder assistant row, build chat history.
    let (assistant_row_id, history) = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        let conv = conversation::get_or_create_default(&conn).map_err(|e| e.to_string())?;
        message::insert(&conn, conv.id, Role::User, &content).map_err(|e| e.to_string())?;
        let assistant_row_id =
            message::insert(&conn, conv.id, Role::Assistant, "").map_err(|e| e.to_string())?;
        let recent = message::list(&conn, conv.id, CONTEXT_WINDOW, 0).map_err(|e| e.to_string())?;
        (assistant_row_id, recent)
    };

    // 2. Tell the frontend the real assistant row id (Phase 2 contract).
    on_event
        .send(StreamChunk::Started(assistant_row_id))
        .map_err(|e| e.to_string())?;

    // 3. Build chat-message history (system prompt + recent turns).
    let mut chat_msgs: Vec<ChatMessage> = vec![ChatMessage {
        role: ChatRole::System,
        content: SYSTEM_PROMPT.into(),
    }];
    for m in history {
        if m.content.is_empty() {
            continue;
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

    // 4. Run the Ollama stream with tools declared.
    let client = OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL);
    let tools_vec = tools::all_tools();
    let (tx, mut rx) = mpsc::channel::<StreamChunk>(64);

    // Spawned receiver collects chunks so we can persist + replay them in order.
    let recv_handle = tokio::spawn(async move {
        let mut chunks_to_persist = Vec::<String>::new();
        let mut events = Vec::<StreamChunk>::new();
        while let Some(chunk) = rx.recv().await {
            if let StreamChunk::Token(frag) = &chunk {
                chunks_to_persist.push(frag.clone());
            }
            events.push(chunk);
        }
        (chunks_to_persist, events)
    });

    let outcome = client.chat(&chat_msgs, &tools_vec, &tx).await;
    drop(tx); // close the channel so recv_handle finishes
    let (chunks_to_persist, events) = recv_handle.await.map_err(|e| e.to_string())?;

    // 5. Persist all token chunks and capture the rationale.
    let rationale = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        for frag in &chunks_to_persist {
            message::append_content(&conn, assistant_row_id, frag).map_err(|e| e.to_string())?;
        }
        let msgs = message::list(&conn, 1, 1, 0).map_err(|e| e.to_string())?;
        msgs.first()
            .filter(|m| m.id == assistant_row_id)
            .map(|m| m.content.clone())
            .unwrap_or_default()
    };

    // 6. Replay events to the frontend Channel.
    for event in events {
        on_event.send(event).map_err(|e| e.to_string())?;
    }

    // 7. Process collected tool calls into proposals.
    for tool_call in outcome.tool_calls {
        match tool_call.function.name.as_str() {
            "add_task" => {
                let args: AddTaskArgs = serde_json::from_value(tool_call.function.arguments)
                    .map_err(|e| format!("bad add_task args: {e}"))?;
                let diff_json = serde_json::to_string(&args).map_err(|e| e.to_string())?;
                let proposal_id = {
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    proposal::insert(
                        &conn,
                        NewProposal {
                            kind: "add_task",
                            rationale: &rationale,
                            diff_json: &diff_json,
                            skill: "tasks",
                        },
                    )
                    .map_err(|e| e.to_string())?
                };
                on_event
                    .send(StreamChunk::Proposal(proposal_id))
                    .map_err(|e| e.to_string())?;
            }
            unknown => {
                tracing::warn!("ignoring unknown tool call: {unknown}");
            }
        }
    }

    // 8. Emit Done.
    on_event
        .send(StreamChunk::Done)
        .map_err(|e| e.to_string())?;
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

// === Tasks ===

#[tauri::command]
pub fn list_tasks(state: State<'_, Db>) -> Result<Vec<Task>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    task::list_today_open(&conn, &today_local_iso()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_task(
    state: State<'_, Db>,
    title: String,
    due_date: Option<String>,
) -> Result<Task, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let due = due_date.unwrap_or_else(today_local_iso);
    let id = task::insert(&conn, &title, Some(&due), None).map_err(|e| e.to_string())?;
    let row = conn
        .query_row(
            "SELECT id, title, due_date, completed_at, created_at, proposal_id \
             FROM task WHERE id = ?1",
            [id],
            |r| {
                Ok(Task {
                    id: r.get("id")?,
                    title: r.get("title")?,
                    due_date: r.get("due_date")?,
                    completed_at: r.get("completed_at")?,
                    created_at: r.get("created_at")?,
                    proposal_id: r.get("proposal_id")?,
                })
            },
        )
        .map_err(|e| e.to_string())?;
    Ok(row)
}

#[tauri::command]
pub fn complete_task(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    task::complete(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn undo_complete_task(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    task::undo_complete(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_task(state: State<'_, Db>, id: i64, title: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    task::update_title(&conn, id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_task(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    task::delete(&conn, id).map_err(|e| e.to_string())
}

// === Proposals ===

#[tauri::command]
pub fn list_proposals(
    state: State<'_, Db>,
    status: Option<String>,
) -> Result<Vec<Proposal>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    proposal::list(&conn, status.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn approve_proposal(state: State<'_, Db>, id: i64) -> Result<Vec<Task>, String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    proposal::approve_add_task(&mut conn, id, &today_local_iso()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reject_proposal(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    proposal::reject(&conn, id).map_err(|e| e.to_string())
}
