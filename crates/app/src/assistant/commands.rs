//! Tauri commands exposed to the frontend Assistant + Today view.

use crate::assistant::ollama::{
    resolve_model, ChatMessage, ChatRole, OllamaClient, StreamChunk, DEFAULT_ENDPOINT,
};
use crate::assistant::prompts::build_system_prompt;
use crate::assistant::tools;
use crate::sync::engine::{SyncResult, SyncState};
use crate::sync::keychain;
use chrono::Local;
use manor_core::assistant::{
    calendar::{self, Calendar},
    calendar_account::{self, CalendarAccount},
    conversation, db,
    event::{self, Event},
    message,
    message::Role,
    proposal::{
        self, AddChoreArgs, AddEventArgs, AddEventItem, AddLedgerTransactionArgs,
        AddRecurringBlockArgs, AddRecurringPaymentArgs, AddTaskArgs, AddTimeBlockArgs,
        CompleteChoreArgs, CompleteTaskArgs, NewProposal, Proposal, SetBudgetArgs,
    },
    proposal_registry,
    task::{self, Task},
    Applied, ApplyError,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeSet;
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

fn new_uid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("manor-{ts:x}-{seq:x}")
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

/// Returns true if the stream produced an Error event AND zero token fragments were persisted.
/// Call sites use this to decide whether the empty assistant row should be deleted.
fn stream_ended_with_unusable_error(chunks_to_persist: &[String], events: &[StreamChunk]) -> bool {
    chunks_to_persist.is_empty() && events.iter().any(|e| matches!(e, StreamChunk::Error(_)))
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, Db>,
    content: String,
    on_event: Channel<StreamChunk>,
) -> Result<(), String> {
    // 1. Persist user message + placeholder assistant row, build chat history.
    let (assistant_row_id, conversation_id, history) = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        let conv = conversation::get_or_create_default(&conn).map_err(|e| e.to_string())?;
        message::insert(&conn, conv.id, Role::User, &content).map_err(|e| e.to_string())?;
        let assistant_row_id =
            message::insert(&conn, conv.id, Role::Assistant, "").map_err(|e| e.to_string())?;
        let recent = message::list(&conn, conv.id, CONTEXT_WINDOW, 0).map_err(|e| e.to_string())?;
        (assistant_row_id, conv.id, recent)
    };

    // 2. Tell the frontend the real assistant row id (Phase 2 contract).
    on_event
        .send(StreamChunk::Started(assistant_row_id))
        .map_err(|e| e.to_string())?;

    // 3. Build chat-message history (system prompt + dynamic per-room context + recent turns).
    //
    // The classifier reads the *user's latest message* (not full history) — multi-room
    // conversations re-classify each turn so context tracks the user's focus.
    let context_block = {
        let slices = crate::assistant::context::classify(&content);
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        crate::assistant::context::render(Local::now(), &conn, slices).map_err(|e| e.to_string())?
    };
    let mut chat_msgs: Vec<ChatMessage> = vec![ChatMessage {
        role: ChatRole::System,
        content: build_system_prompt(&context_block),
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
    let model = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        resolve_model(&conn)
    };
    let client = OllamaClient::new(DEFAULT_ENDPOINT, &model);
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

    // 5a. If the stream errored with zero tokens, delete the empty assistant row and
    //     replay the error event so the UI can show the toast, then bail out.
    if stream_ended_with_unusable_error(&chunks_to_persist, &events) {
        {
            let conn = state.0.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "DELETE FROM message WHERE id = ?1 AND content = ''",
                [assistant_row_id],
            )
            .map_err(|e| e.to_string())?;
        }
        // Still replay the Error event so the UI shows the error toast.
        for event in events {
            on_event.send(event).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    // 5. Persist all token chunks and capture the rationale.
    let rationale = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        for frag in &chunks_to_persist {
            message::append_content(&conn, assistant_row_id, frag).map_err(|e| e.to_string())?;
        }
        let msgs = message::list(&conn, conversation_id, 1, 0).map_err(|e| e.to_string())?;
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
            "add_chore" => {
                let args: AddChoreArgs = serde_json::from_value(tool_call.function.arguments)
                    .map_err(|e| format!("bad add_chore args: {e}"))?;
                let diff_json = serde_json::to_string(&args).map_err(|e| e.to_string())?;
                let proposal_id = {
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    proposal::insert(
                        &conn,
                        NewProposal {
                            kind: "add_chore",
                            rationale: &rationale,
                            diff_json: &diff_json,
                            skill: "rhythm",
                        },
                    )
                    .map_err(|e| e.to_string())?
                };
                on_event
                    .send(StreamChunk::Proposal(proposal_id))
                    .map_err(|e| e.to_string())?;
            }
            "complete_chore" => {
                let args: CompleteChoreArgs = serde_json::from_value(tool_call.function.arguments)
                    .map_err(|e| format!("bad complete_chore args: {e}"))?;
                let diff_json = serde_json::to_string(&args).map_err(|e| e.to_string())?;
                let proposal_id = {
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    proposal::insert(
                        &conn,
                        NewProposal {
                            kind: "complete_chore",
                            rationale: &rationale,
                            diff_json: &diff_json,
                            skill: "rhythm",
                        },
                    )
                    .map_err(|e| e.to_string())?
                };
                on_event
                    .send(StreamChunk::Proposal(proposal_id))
                    .map_err(|e| e.to_string())?;
            }
            "complete_task" => {
                let args: CompleteTaskArgs = serde_json::from_value(tool_call.function.arguments)
                    .map_err(|e| format!("bad complete_task args: {e}"))?;
                let diff_json = serde_json::to_string(&args).map_err(|e| e.to_string())?;
                let proposal_id = {
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    proposal::insert(
                        &conn,
                        NewProposal {
                            kind: "complete_task",
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
            "add_event" => {
                let args: AddEventArgs = serde_json::from_value(tool_call.function.arguments)
                    .map_err(|e| format!("bad add_event args: {e}"))?;
                let diff_json = serde_json::to_string(&args).map_err(|e| e.to_string())?;
                let proposal_id = {
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    proposal::insert(
                        &conn,
                        NewProposal {
                            kind: "add_event",
                            rationale: &rationale,
                            diff_json: &diff_json,
                            skill: "today",
                        },
                    )
                    .map_err(|e| e.to_string())?
                };
                on_event
                    .send(StreamChunk::Proposal(proposal_id))
                    .map_err(|e| e.to_string())?;
            }
            "add_transaction" => {
                let args: AddLedgerTransactionArgs =
                    serde_json::from_value(tool_call.function.arguments)
                        .map_err(|e| format!("bad add_transaction args: {e}"))?;
                let diff_json = serde_json::to_string(&args).map_err(|e| e.to_string())?;
                let proposal_id = {
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    proposal::insert(
                        &conn,
                        NewProposal {
                            kind: "add_transaction",
                            rationale: &rationale,
                            diff_json: &diff_json,
                            skill: "ledger",
                        },
                    )
                    .map_err(|e| e.to_string())?
                };
                on_event
                    .send(StreamChunk::Proposal(proposal_id))
                    .map_err(|e| e.to_string())?;
            }
            "set_budget" => {
                let args: SetBudgetArgs = serde_json::from_value(tool_call.function.arguments)
                    .map_err(|e| format!("bad set_budget args: {e}"))?;
                let diff_json = serde_json::to_string(&args).map_err(|e| e.to_string())?;
                let proposal_id = {
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    proposal::insert(
                        &conn,
                        NewProposal {
                            kind: "set_budget",
                            rationale: &rationale,
                            diff_json: &diff_json,
                            skill: "ledger",
                        },
                    )
                    .map_err(|e| e.to_string())?
                };
                on_event
                    .send(StreamChunk::Proposal(proposal_id))
                    .map_err(|e| e.to_string())?;
            }
            "add_recurring_payment" => {
                let args: AddRecurringPaymentArgs =
                    serde_json::from_value(tool_call.function.arguments)
                        .map_err(|e| format!("bad add_recurring_payment args: {e}"))?;
                let diff_json = serde_json::to_string(&args).map_err(|e| e.to_string())?;
                let proposal_id = {
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    proposal::insert(
                        &conn,
                        NewProposal {
                            kind: "add_recurring_payment",
                            rationale: &rationale,
                            diff_json: &diff_json,
                            skill: "ledger",
                        },
                    )
                    .map_err(|e| e.to_string())?
                };
                on_event
                    .send(StreamChunk::Proposal(proposal_id))
                    .map_err(|e| e.to_string())?;
            }
            "add_time_block" => {
                let args: AddTimeBlockArgs =
                    serde_json::from_value(tool_call.function.arguments)
                        .map_err(|e| format!("bad add_time_block args: {e}"))?;
                let diff_json = serde_json::to_string(&args).map_err(|e| e.to_string())?;
                let proposal_id = {
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    proposal::insert(
                        &conn,
                        NewProposal {
                            kind: "add_time_block",
                            rationale: &rationale,
                            diff_json: &diff_json,
                            skill: "rhythm",
                        },
                    )
                    .map_err(|e| e.to_string())?
                };
                on_event
                    .send(StreamChunk::Proposal(proposal_id))
                    .map_err(|e| e.to_string())?;
            }
            "add_recurring_block" => {
                let args: AddRecurringBlockArgs =
                    serde_json::from_value(tool_call.function.arguments)
                        .map_err(|e| format!("bad add_recurring_block args: {e}"))?;
                let diff_json = serde_json::to_string(&args).map_err(|e| e.to_string())?;
                let proposal_id = {
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    proposal::insert(
                        &conn,
                        NewProposal {
                            kind: "add_recurring_block",
                            rationale: &rationale,
                            diff_json: &diff_json,
                            skill: "rhythm",
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
pub async fn approve_proposal(
    state: State<'_, Db>,
    sync_state: State<'_, Arc<SyncState>>,
    id: i64,
) -> Result<Applied, ApplyError> {
    let kind = {
        let conn = state
            .0
            .lock()
            .map_err(|e| ApplyError::Internal(format!("db lock: {e}")))?;
        proposal_registry::read_kind(&conn, id)?
    };

    if kind == "add_event" {
        return approve_add_event_proposal(state.clone_arc(), sync_state.inner().clone(), id).await;
    }

    let mut conn = state
        .0
        .lock()
        .map_err(|e| ApplyError::Internal(format!("db lock: {e}")))?;
    proposal_registry::approve(&mut conn, id)
}

#[tauri::command]
pub async fn approve_proposal_with_override(
    state: State<'_, Db>,
    sync_state: State<'_, Arc<SyncState>>,
    id: i64,
    edited_diff_json: String,
) -> Result<Applied, ApplyError> {
    let kind = {
        let conn = state
            .0
            .lock()
            .map_err(|e| ApplyError::Internal(format!("db lock: {e}")))?;
        proposal_registry::read_kind(&conn, id)?
    };

    if kind == "add_event" {
        {
            let conn = state
                .0
                .lock()
                .map_err(|e| ApplyError::Internal(format!("db lock: {e}")))?;
            proposal_registry::update_diff(&conn, id, &edited_diff_json)?;
        }
        return approve_add_event_proposal(state.clone_arc(), sync_state.inner().clone(), id).await;
    }

    let mut conn = state
        .0
        .lock()
        .map_err(|e| ApplyError::Internal(format!("db lock: {e}")))?;
    proposal_registry::approve_with_override(&mut conn, id, &edited_diff_json)
}

async fn approve_add_event_proposal(
    db: Arc<Mutex<Connection>>,
    sync_state: Arc<SyncState>,
    proposal_id: i64,
) -> Result<Applied, ApplyError> {
    let handle = tokio::runtime::Handle::current();
    tauri::async_runtime::spawn_blocking(move || {
        let items = load_add_event_items(&db, proposal_id)?;
        if items.is_empty() {
            return Err(ApplyError::InvalidArg {
                field: "items".into(),
                reason: "at least one event is required".into(),
            });
        }

        let mut applied = 0usize;
        let mut errors = Vec::<ApplyError>::new();
        let mut touched_accounts = BTreeSet::<i64>::new();

        for (idx, item) in items.into_iter().enumerate() {
            match put_event_item(&db, &handle, item) {
                Ok(account_id) => {
                    applied += 1;
                    touched_accounts.insert(account_id);
                }
                Err(err) => errors.push(indexed_event_error(idx, err)),
            }
        }

        for account_id in touched_accounts {
            let Ok(password) = keychain::get_password(account_id) else {
                continue;
            };
            let mut conn = db
                .lock()
                .map_err(|e| ApplyError::Internal(format!("db lock: {e}")))?;
            let _ = handle.block_on(crate::sync::engine::sync_account(
                &mut conn,
                &sync_state,
                account_id,
                &password,
                chrono_tz::UTC,
            ));
        }

        let failed = errors.len();
        let status = match (applied, failed) {
            (0, _) => proposal::Status::Rejected,
            (_, 0) => proposal::Status::Applied,
            _ => proposal::Status::PartiallyApplied,
        };

        {
            let conn = db
                .lock()
                .map_err(|e| ApplyError::Internal(format!("db lock: {e}")))?;
            persist_event_proposal_outcome(&conn, proposal_id, status, &errors)?;
        }

        Ok(Applied {
            proposal_id,
            status,
            items_applied: applied,
            items_failed: failed,
            errors,
        })
    })
    .await
    .map_err(|e| ApplyError::Internal(format!("event approval task failed: {e}")))?
}

fn load_add_event_items(
    db: &Arc<Mutex<Connection>>,
    proposal_id: i64,
) -> Result<Vec<AddEventItem>, ApplyError> {
    let conn = db
        .lock()
        .map_err(|e| ApplyError::Internal(format!("db lock: {e}")))?;
    let row: Option<(String, String, String)> = conn
        .query_row(
            "SELECT kind, status, diff FROM proposal WHERE id = ?1",
            params![proposal_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| ApplyError::Internal(format!("proposal lookup failed: {e}")))?;
    let (kind, status, diff) =
        row.ok_or_else(|| ApplyError::Internal(format!("proposal {proposal_id} not found")))?;
    if status != proposal::Status::Pending.as_str() {
        return Err(ApplyError::Conflict("proposal not pending".into()));
    }
    if kind != "add_event" {
        return Err(ApplyError::UnknownKind(kind));
    }
    let args: AddEventArgs = serde_json::from_str(&diff).map_err(|e| ApplyError::InvalidArg {
        field: "diff".into(),
        reason: e.to_string(),
    })?;
    Ok(args.into_items())
}

fn put_event_item(
    db: &Arc<Mutex<Connection>>,
    handle: &tokio::runtime::Handle,
    item: AddEventItem,
) -> Result<i64, ApplyError> {
    validate_event_item(&item)?;
    let (account, calendar_url) = {
        let conn = db
            .lock()
            .map_err(|e| ApplyError::Internal(format!("db lock: {e}")))?;
        resolve_event_target(&conn, &item)?
    };

    let password = keychain::get_password(account.id)
        .map_err(|e| ApplyError::Internal(format!("keychain: {e}")))?;
    let uid = new_uid();
    let ical = crate::sync::ical_write::generate_vcalendar(
        &uid,
        item.title.trim(),
        item.start_at,
        item.end_at,
        item.description.as_deref(),
        item.location.as_deref(),
        item.all_day,
    );
    let url = format!("{}/{}.ics", calendar_url.trim_end_matches('/'), uid);
    let client = crate::sync::caldav::CalDavClient::new(&account.username, &password);
    handle
        .block_on(client.put_event(&url, &ical, None))
        .map_err(|e| ApplyError::Network(e.to_string()))?;
    Ok(account.id)
}

fn validate_event_item(item: &AddEventItem) -> Result<(), ApplyError> {
    if item.title.trim().is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "title".into(),
            reason: "title cannot be empty".into(),
        });
    }
    if item.start_at >= item.end_at {
        return Err(ApplyError::InvalidArg {
            field: "end_at".into(),
            reason: "end_at must be after start_at".into(),
        });
    }
    Ok(())
}

fn resolve_event_target(
    conn: &Connection,
    item: &AddEventItem,
) -> Result<(CalendarAccount, String), ApplyError> {
    if let Some(account_id) = item.account_id {
        let account = calendar_account::get(conn, account_id)
            .map_err(|e| ApplyError::Internal(format!("calendar account lookup failed: {e}")))?
            .ok_or_else(|| ApplyError::StaleReference {
                entity: "calendar_account".into(),
                id: account_id.to_string(),
            })?;
        let calendar_url = item
            .calendar_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| account.default_calendar_url.clone())
            .ok_or_else(|| ApplyError::InvalidArg {
                field: "calendar_url".into(),
                reason: "calendar_url is required when the account has no default calendar".into(),
            })?;
        return Ok((account, calendar_url));
    }

    if let Some(calendar_url) = item
        .calendar_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        if let Some(account) = find_account_for_calendar_url(conn, calendar_url)? {
            return Ok((account, calendar_url.to_string()));
        }
        return Err(ApplyError::StaleReference {
            entity: "calendar".into(),
            id: calendar_url.to_string(),
        });
    }

    let accounts = calendar_account::list(conn)
        .map_err(|e| ApplyError::Internal(format!("calendar account list failed: {e}")))?;
    let with_default: Vec<CalendarAccount> = accounts
        .into_iter()
        .filter(|account| account.default_calendar_url.is_some())
        .collect();

    match with_default.as_slice() {
        [account] => Ok((
            account.clone(),
            account.default_calendar_url.clone().unwrap_or_default(),
        )),
        [] => Err(ApplyError::InvalidArg {
            field: "calendar_url".into(),
            reason: "no default calendar is configured".into(),
        }),
        _ => Err(ApplyError::Conflict(
            "multiple calendar accounts have defaults; specify account_id".into(),
        )),
    }
}

fn find_account_for_calendar_url(
    conn: &Connection,
    calendar_url: &str,
) -> Result<Option<CalendarAccount>, ApplyError> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT ca.id
             FROM calendar_account ca
             LEFT JOIN calendar c ON c.calendar_account_id = ca.id
             WHERE ca.default_calendar_url = ?1 OR c.url = ?1
             ORDER BY ca.id
             LIMIT 1",
            [calendar_url],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| ApplyError::Internal(format!("calendar lookup failed: {e}")))?;
    id.map(|account_id| {
        calendar_account::get(conn, account_id)
            .map_err(|e| ApplyError::Internal(format!("calendar account lookup failed: {e}")))?
            .ok_or_else(|| ApplyError::StaleReference {
                entity: "calendar_account".into(),
                id: account_id.to_string(),
            })
    })
    .transpose()
}

fn persist_event_proposal_outcome(
    conn: &Connection,
    proposal_id: i64,
    status: proposal::Status,
    errors: &[ApplyError],
) -> Result<(), ApplyError> {
    let now = chrono::Utc::now().timestamp();
    let applied_at = if matches!(status, proposal::Status::Rejected) {
        None
    } else {
        Some(now)
    };
    let errors_json = if errors.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(errors)
                .map_err(|e| ApplyError::Internal(format!("apply errors json: {e}")))?,
        )
    };

    conn.execute(
        "UPDATE proposal SET status = ?1, applied_at = ?2, apply_errors_json = ?3 WHERE id = ?4",
        params![status.as_str(), applied_at, errors_json, proposal_id],
    )
    .map_err(|e| ApplyError::Internal(format!("proposal update failed: {e}")))?;
    Ok(())
}

fn indexed_event_error(index: usize, err: ApplyError) -> ApplyError {
    match err {
        ApplyError::InvalidArg { field, reason } => ApplyError::InvalidArg {
            field: format!("items[{index}].{field}"),
            reason,
        },
        other => other,
    }
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

// === Calendar list + default ===

#[tauri::command]
pub fn list_calendars(db: State<'_, Db>, account_id: i64) -> Result<Vec<Calendar>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    calendar::list(&conn, account_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_default_calendar(db: State<'_, Db>, account_id: i64, url: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    calendar_account::set_default_calendar(&conn, account_id, &url).map_err(|e| e.to_string())
}

// === Event write commands ===

#[derive(serde::Deserialize)]
pub struct CreateEventArgs {
    pub account_id: i64,
    pub calendar_url: String,
    pub title: String,
    pub start_at: i64,
    pub end_at: i64,
    pub description: Option<String>,
    pub location: Option<String>,
    pub all_day: bool,
}

#[tauri::command]
pub async fn create_event(
    db: State<'_, Db>,
    sync_state: State<'_, Arc<SyncState>>,
    args: CreateEventArgs,
) -> Result<(), String> {
    let password = {
        crate::sync::keychain::get_password(args.account_id)
            .map_err(|e| format!("keychain: {e}"))?
    };

    let uid = new_uid();
    let ical = crate::sync::ical_write::generate_vcalendar(
        &uid,
        &args.title,
        args.start_at,
        args.end_at,
        args.description.as_deref(),
        args.location.as_deref(),
        args.all_day,
    );

    // Build event URL: calendar_url + uid + ".ics"
    let url = format!("{}/{}.ics", args.calendar_url.trim_end_matches('/'), uid);

    let account_id = args.account_id;
    let db_arc = db.clone_arc();
    let sync_state_arc = sync_state.inner().clone();

    let handle = tokio::runtime::Handle::current();
    tauri::async_runtime::spawn_blocking(move || {
        let account = {
            let conn = db_arc.lock().unwrap();
            calendar_account::get(&conn, account_id).ok().flatten()
        }
        .ok_or_else(|| "account not found".to_string())?;

        let client = crate::sync::caldav::CalDavClient::new(&account.username, &password);
        handle
            .block_on(client.put_event(&url, &ical, None))
            .map_err(|e| e.to_string())?;

        // Re-sync account to pick up the new event
        let mut conn = db_arc.lock().unwrap();
        handle.block_on(crate::sync::engine::sync_account(
            &mut conn,
            &sync_state_arc,
            account_id,
            &password,
            chrono_tz::UTC,
        ));
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Deserialize)]
pub struct UpdateEventArgs {
    pub event_id: i64,
    pub title: String,
    pub start_at: i64,
    pub end_at: i64,
    pub description: Option<String>,
    pub location: Option<String>,
    pub all_day: bool,
    /// For recurring occurrences only — edit this occurrence only.
    pub edit_occurrence_only: bool,
}

#[tauri::command]
pub async fn update_event(
    db: State<'_, Db>,
    sync_state: State<'_, Arc<SyncState>>,
    args: UpdateEventArgs,
) -> Result<(), String> {
    // Load the event and account under a brief lock.
    let (account_id, event_url, is_recurring, parent_url, occurrence_dtstart, password) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let events = event::list_today(&conn, 0, i64::MAX).map_err(|e| e.to_string())?;
        let ev = events
            .iter()
            .find(|e| e.id == args.event_id)
            .ok_or_else(|| "event not found".to_string())?
            .clone();
        let pw = crate::sync::keychain::get_password(ev.calendar_account_id)
            .map_err(|e| format!("keychain: {e}"))?;
        (
            ev.calendar_account_id,
            ev.event_url
                .clone()
                .ok_or("event has no URL (manual event?)")?,
            ev.is_recurring_occurrence,
            ev.parent_event_url.clone(),
            ev.occurrence_dtstart.clone(),
            pw,
        )
    };

    let db_arc = db.clone_arc();
    let sync_state_arc = sync_state.inner().clone();
    let handle = tokio::runtime::Handle::current();

    tauri::async_runtime::spawn_blocking(move || {
        let account = {
            let conn = db_arc.lock().unwrap();
            calendar_account::get(&conn, account_id).ok().flatten()
        }
        .ok_or_else(|| "account not found".to_string())?;

        let client = crate::sync::caldav::CalDavClient::new(&account.username, &password);

        if is_recurring && args.edit_occurrence_only {
            // Fetch parent .ics, add RECURRENCE-ID override, PUT back
            let parent = parent_url.ok_or("recurring event has no parent_event_url")?;
            let rec_id = occurrence_dtstart.ok_or("occurrence has no dtstart")?;
            let (parent_ical, parent_etag) = handle
                .block_on(client.fetch_ical(&parent))
                .map_err(|e| e.to_string())?;
            let patched = crate::sync::ical_write::add_recurrence_override(
                &parent_ical,
                &rec_id,
                &args.title,
                args.start_at,
                args.end_at,
                args.description.as_deref(),
                args.location.as_deref(),
            );
            handle
                .block_on(client.put_event(&parent, &patched, Some(&parent_etag)))
                .map_err(|e| e.to_string())?;
        } else {
            // Fetch the event's .ics, regenerate with new fields, PUT back
            let (old_ical, etag) = handle
                .block_on(client.fetch_ical(&event_url))
                .map_err(|e| e.to_string())?;
            // Extract UID from old ical
            let uid = old_ical
                .lines()
                .find(|l| l.trim_start_matches(' ').starts_with("UID:"))
                .map(|l| {
                    l.trim_start_matches(' ')
                        .trim_start_matches("UID:")
                        .trim()
                        .to_string()
                })
                .unwrap_or_else(new_uid);
            let new_ical = crate::sync::ical_write::generate_vcalendar(
                &uid,
                &args.title,
                args.start_at,
                args.end_at,
                args.description.as_deref(),
                args.location.as_deref(),
                args.all_day,
            );
            handle
                .block_on(client.put_event(&event_url, &new_ical, Some(&etag)))
                .map_err(|e| e.to_string())?;
        }

        // Re-sync
        let mut conn = db_arc.lock().unwrap();
        handle.block_on(crate::sync::engine::sync_account(
            &mut conn,
            &sync_state_arc,
            account_id,
            &password,
            chrono_tz::UTC,
        ));
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Deserialize)]
pub struct DeleteEventArgs {
    pub event_id: i64,
    /// For recurring occurrences — delete this occurrence only (adds EXDATE to parent).
    pub delete_occurrence_only: bool,
}

#[tauri::command]
pub async fn delete_event(
    db: State<'_, Db>,
    sync_state: State<'_, Arc<SyncState>>,
    args: DeleteEventArgs,
) -> Result<(), String> {
    let (account_id, event_url, is_recurring, parent_url, occurrence_dtstart, password) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let events = event::list_today(&conn, 0, i64::MAX).map_err(|e| e.to_string())?;
        let ev = events
            .iter()
            .find(|e| e.id == args.event_id)
            .ok_or_else(|| "event not found".to_string())?
            .clone();
        let pw = crate::sync::keychain::get_password(ev.calendar_account_id)
            .map_err(|e| format!("keychain: {e}"))?;
        (
            ev.calendar_account_id,
            ev.event_url
                .clone()
                .ok_or("event has no URL (manual event?)")?,
            ev.is_recurring_occurrence,
            ev.parent_event_url.clone(),
            ev.occurrence_dtstart.clone(),
            pw,
        )
    };

    let db_arc = db.clone_arc();
    let sync_state_arc = sync_state.inner().clone();
    let handle = tokio::runtime::Handle::current();
    let event_id = args.event_id;

    tauri::async_runtime::spawn_blocking(move || {
        let account = {
            let conn = db_arc.lock().unwrap();
            calendar_account::get(&conn, account_id).ok().flatten()
        }
        .ok_or_else(|| "account not found".to_string())?;

        // Optimistically soft-delete in DB so the UI updates immediately.
        {
            let conn = db_arc.lock().unwrap();
            event::soft_delete(&conn, event_id).map_err(|e| e.to_string())?;
        }

        let client = crate::sync::caldav::CalDavClient::new(&account.username, &password);

        if is_recurring && args.delete_occurrence_only {
            let parent = parent_url.ok_or("recurring event has no parent_event_url")?;
            let occ = occurrence_dtstart.ok_or("occurrence has no dtstart")?;
            let (parent_ical, etag) = handle
                .block_on(client.fetch_ical(&parent))
                .map_err(|e| e.to_string())?;
            let patched = crate::sync::ical_write::add_exdate(&parent_ical, &occ);
            handle
                .block_on(client.put_event(&parent, &patched, Some(&etag)))
                .map_err(|e| e.to_string())?;
        } else {
            let (_, etag) = handle
                .block_on(client.fetch_ical(&event_url))
                .map_err(|e| e.to_string())?;
            handle
                .block_on(client.delete_event(&event_url, &etag))
                .map_err(|e| e.to_string())?;
        }

        // Re-sync to reconcile
        let mut conn = db_arc.lock().unwrap();
        handle.block_on(crate::sync::engine::sync_account(
            &mut conn,
            &sync_state_arc,
            account_id,
            &password,
            chrono_tz::UTC,
        ));
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests_send_message {
    use super::*;
    use crate::assistant::ollama::ErrorCode;

    #[test]
    fn cleanup_triggers_when_error_and_no_tokens() {
        let events = vec![StreamChunk::Error(ErrorCode::OllamaUnreachable)];
        assert!(stream_ended_with_unusable_error(&[], &events));
    }

    #[test]
    fn cleanup_skipped_when_tokens_present() {
        let events = vec![
            StreamChunk::Token("hi".to_string()),
            StreamChunk::Error(ErrorCode::OllamaUnreachable),
        ];
        let chunks = vec!["hi".to_string()];
        assert!(!stream_ended_with_unusable_error(&chunks, &events));
    }

    #[test]
    fn cleanup_skipped_when_no_error() {
        let events = vec![StreamChunk::Done];
        assert!(!stream_ended_with_unusable_error(&[], &events));
    }
}

#[cfg(test)]
mod tests_add_event_proposal {
    use super::*;
    use tempfile::tempdir;

    fn fresh_db() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn resolve_event_target_uses_single_default_calendar() {
        let (_d, conn) = fresh_db();
        let account_id =
            calendar_account::insert(&conn, "iCloud", "https://cal.example", "hana").unwrap();
        calendar_account::set_default_calendar(&conn, account_id, "https://cal.example/home/")
            .unwrap();
        let item = AddEventItem {
            account_id: None,
            calendar_url: None,
            title: "Dentist".into(),
            start_at: 1_778_842_800,
            end_at: 1_778_846_400,
            description: None,
            location: None,
            all_day: false,
        };

        let (account, url) = resolve_event_target(&conn, &item).unwrap();
        assert_eq!(account.id, account_id);
        assert_eq!(url, "https://cal.example/home/");
    }

    #[test]
    fn resolve_event_target_rejects_ambiguous_defaults() {
        let (_d, conn) = fresh_db();
        let a = calendar_account::insert(&conn, "A", "https://a.example", "a").unwrap();
        let b = calendar_account::insert(&conn, "B", "https://b.example", "b").unwrap();
        calendar_account::set_default_calendar(&conn, a, "https://a.example/home/").unwrap();
        calendar_account::set_default_calendar(&conn, b, "https://b.example/home/").unwrap();
        let item = AddEventItem {
            account_id: None,
            calendar_url: None,
            title: "Dentist".into(),
            start_at: 1_778_842_800,
            end_at: 1_778_846_400,
            description: None,
            location: None,
            all_day: false,
        };

        let err = resolve_event_target(&conn, &item).unwrap_err();
        assert!(matches!(err, ApplyError::Conflict(_)));
    }

    #[test]
    fn add_event_args_accepts_bundle() {
        let diff = serde_json::json!([
            {
                "title": "Dentist",
                "start_at": 1_778_842_800i64,
                "end_at": 1_778_846_400i64
            },
            {
                "title": "Lunch",
                "start_at": 1_778_850_000i64,
                "end_at": 1_778_853_600i64
            }
        ])
        .to_string();

        let args: AddEventArgs = serde_json::from_str(&diff).unwrap();
        assert_eq!(args.into_items().len(), 2);
    }

    #[test]
    fn validate_event_item_rejects_empty_title_and_bad_range() {
        let mut item = AddEventItem {
            account_id: None,
            calendar_url: None,
            title: "".into(),
            start_at: 10,
            end_at: 20,
            description: None,
            location: None,
            all_day: false,
        };
        assert!(matches!(
            validate_event_item(&item),
            Err(ApplyError::InvalidArg { field, .. }) if field == "title"
        ));

        item.title = "Dentist".into();
        item.end_at = 10;
        assert!(matches!(
            validate_event_item(&item),
            Err(ApplyError::InvalidArg { field, .. }) if field == "end_at"
        ));
    }
}

/// Tests for the Tauri command surface exposed in 1.F. We exercise the same
/// `proposal_registry::approve(_with_override)` entry points the commands
/// call, plus assert the wire shape of `ApplyError` (the contract the
/// frontend pattern-matches against).
#[cfg(test)]
mod tests_approve_proposal_command {
    use super::*;
    use manor_core::assistant::proposal::{insert, NewProposal, Status};
    use tempfile::tempdir;

    fn fresh_db() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn approve_returns_applied_for_add_task() {
        let (_d, mut conn) = fresh_db();
        let diff = serde_json::json!({ "title": "Buy milk" }).to_string();
        let pid = insert(
            &conn,
            NewProposal {
                kind: "add_task",
                rationale: "command-test",
                diff_json: &diff,
                skill: "tasks",
            },
        )
        .unwrap();

        let applied = proposal_registry::approve(&mut conn, pid).unwrap();
        assert_eq!(applied.proposal_id, pid);
        assert_eq!(applied.status, Status::Applied);
        assert_eq!(applied.items_applied, 1);
        assert_eq!(applied.items_failed, 0);
        assert!(applied.errors.is_empty());

        // Wire-shape: serialise as the frontend will receive it.
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&applied).unwrap()).unwrap();
        assert_eq!(json["proposal_id"], pid);
        assert_eq!(json["status"], "applied");
        assert_eq!(json["items_applied"], 1);
        assert_eq!(json["items_failed"], 0);
        assert!(json["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn approve_unknown_kind_serialises_to_typed_apply_error() {
        let (_d, mut conn) = fresh_db();
        let pid = insert(
            &conn,
            NewProposal {
                kind: "weird_kind",
                rationale: "command-test",
                diff_json: "{}",
                skill: "test",
            },
        )
        .unwrap();

        let err = proposal_registry::approve(&mut conn, pid).unwrap_err();

        // Frontend-facing JSON shape: `{ "type": "UnknownKind", "value": "weird_kind" }`.
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&err).unwrap()).unwrap();
        assert_eq!(json["type"], "UnknownKind");
        assert_eq!(json["value"], "weird_kind");
    }

    #[test]
    fn approve_with_override_persists_edited_diff() {
        let (_d, mut conn) = fresh_db();
        let original = serde_json::json!({ "title": "original" }).to_string();
        let pid = insert(
            &conn,
            NewProposal {
                kind: "add_task",
                rationale: "command-test",
                diff_json: &original,
                skill: "tasks",
            },
        )
        .unwrap();

        let edited = serde_json::json!({ "title": "edited via drawer" }).to_string();
        let applied = proposal_registry::approve_with_override(&mut conn, pid, &edited).unwrap();
        assert_eq!(applied.status, Status::Applied);

        let title: String = conn
            .query_row(
                "SELECT title FROM task WHERE proposal_id = ?1",
                [pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title, "edited via drawer");
    }
}
