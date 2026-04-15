//! Tauri commands exposed to the frontend Assistant + Today view.

use crate::assistant::ollama::{
    ChatMessage, ChatRole, OllamaClient, StreamChunk, DEFAULT_ENDPOINT, DEFAULT_MODEL,
};
use crate::assistant::prompts::SYSTEM_PROMPT;
use crate::assistant::tools;
use crate::sync::engine::{SyncResult, SyncState};
use crate::sync::keychain;
use chrono::Local;
use manor_core::assistant::{
    calendar_account::{self, CalendarAccount},
    conversation, db,
    event::{self, Event},
    message,
    message::Role,
    proposal::{self, AddTaskArgs, NewProposal, Proposal},
    task::{self, Task},
};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::mpsc;

/// DB state — wrapped in `Arc<Mutex<_>>` so it can be cloned into spawned tasks
/// without holding the guard across `.await` points.
pub struct Db(pub Arc<Mutex<Connection>>);

impl Db {
    pub fn open(path: PathBuf) -> anyhow::Result<Self> {
        let conn = db::init(&path)?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    /// Clone the inner `Arc` so a spawned task can hold its own handle to the mutex.
    pub fn clone_arc(&self) -> Arc<Mutex<Connection>> {
        self.0.clone()
    }
}

const CONTEXT_WINDOW: u32 = 20;

fn today_local_iso() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

fn today_utc_bounds() -> (i64, i64) {
    let now = Local::now();
    let start_local = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap();
    let end_local = start_local + chrono::Duration::days(1);
    (
        start_local.with_timezone(&chrono::Utc).timestamp(),
        end_local.with_timezone(&chrono::Utc).timestamp(),
    )
}

fn display_name_for_url(url: &str) -> String {
    if url.contains("caldav.icloud.com") {
        "iCloud".into()
    } else if url.contains("fastmail") {
        "Fastmail".into()
    } else {
        reqwest::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_else(|| url.to_string())
    }
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

// === Calendar Accounts ===

#[tauri::command]
pub fn list_calendar_accounts(state: State<'_, Db>) -> Result<Vec<CalendarAccount>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    calendar_account::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_calendar_account(
    db: State<'_, Db>,
    sync_state: State<'_, Arc<SyncState>>,
    server_url: String,
    username: String,
    password: String,
) -> Result<CalendarAccount, String> {
    let display_name = display_name_for_url(&server_url);
    let account = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let id = calendar_account::insert(&conn, &display_name, &server_url, &username)
            .map_err(|e| e.to_string())?;
        keychain::set_password(id, &password).map_err(|e| format!("keychain: {e}"))?;
        calendar_account::get(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "just-inserted row not found".to_string())?
    };

    // Kick off first sync in a blocking thread — lock is acquired inside the sync thread,
    // never held across an await boundary.
    let account_id = account.id;
    let db_arc = db.clone_arc();
    let sync_state_arc = sync_state.inner().clone();
    let handle = tokio::runtime::Handle::current();
    tauri::async_runtime::spawn_blocking(move || {
        let mut conn = db_arc.lock().unwrap();
        let result = handle.block_on(crate::sync::engine::sync_account(
            &mut conn,
            &sync_state_arc,
            account_id,
            &password,
            chrono_tz::UTC,
        ));
        drop(conn);
        result
    });

    Ok(account)
}

#[tauri::command]
pub fn remove_calendar_account(db: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    calendar_account::delete(&conn, id).map_err(|e| e.to_string())?;
    let _ = keychain::delete_password(id);
    Ok(())
}

#[tauri::command]
pub async fn sync_account(
    db: State<'_, Db>,
    sync_state: State<'_, Arc<SyncState>>,
    id: i64,
) -> Result<SyncResult, String> {
    let password = keychain::get_password(id).map_err(|e| format!("keychain: {e}"))?;
    let db_arc = db.clone_arc();
    let sync_state_arc = sync_state.inner().clone();

    // Run sync inside spawn_blocking: lock is held in the sync thread, never across .await.
    let handle = tokio::runtime::Handle::current();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut conn = db_arc.lock().unwrap();
        handle.block_on(crate::sync::engine::sync_account(
            &mut conn,
            &sync_state_arc,
            id,
            &password,
            chrono_tz::UTC,
        ))
    })
    .await
    .map_err(|e| e.to_string())?;

    Ok(result)
}

#[tauri::command]
pub async fn sync_all_accounts(
    db: State<'_, Db>,
    sync_state: State<'_, Arc<SyncState>>,
) -> Result<Vec<SyncResult>, String> {
    // Collect account ids under a brief lock — no awaiting while holding the guard.
    let ids: Vec<i64> = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        calendar_account::list(&conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|a| a.id)
            .collect()
    };

    let mut results = Vec::with_capacity(ids.len());
    for id in ids {
        let Ok(password) = keychain::get_password(id) else {
            continue;
        };
        let db_arc = db.clone_arc();
        let sync_state_arc = sync_state.inner().clone();

        // Each account synced via spawn_blocking: lock held in sync thread, not across .await.
        let handle = tokio::runtime::Handle::current();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let mut conn = db_arc.lock().unwrap();
            handle.block_on(crate::sync::engine::sync_account(
                &mut conn,
                &sync_state_arc,
                id,
                &password,
                chrono_tz::UTC,
            ))
        })
        .await
        .map_err(|e| e.to_string())?;

        results.push(result);
    }

    Ok(results)
}

// === Events ===

#[tauri::command]
pub fn list_events_today(state: State<'_, Db>) -> Result<Vec<Event>, String> {
    let (start, end) = today_utc_bounds();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    event::list_today(&conn, start, end).map_err(|e| e.to_string())
}
