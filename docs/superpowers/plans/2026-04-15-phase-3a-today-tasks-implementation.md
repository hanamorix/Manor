# Phase 3a — Today View + Manual Tasks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Manor's placeholder main area with a real Today view (stacked cards), introduce manual task CRUD, and ship the first end-to-end test of the proposal abstraction (Ollama tool calling → proposal banner → approve/reject).

**Architecture:** Adds a `task` table (matching the 3c spec's contract) + Rust data-access functions; extends the Ollama client to declare an `add_task` tool and parse tool calls; new `useTodayStore` zustand slice; new `<Today>` component tree (`HeaderCard`, `EventsCard`, `TasksCard`, `TaskRow`, `ProposalBanner`); slash-command parser (`/task <title>`) in both the corner pill and the conversation drawer's textarea.

**Tech Stack:** Rust (rusqlite, refinery, reqwest, serde, wiremock for tests); React 18 + TypeScript + zustand; Tauri 2.x with `Channel<StreamChunk>` for streaming; Ollama (`qwen2.5:7b-instruct`).

**Worktree setup:** Before starting Task 1, create a worktree at `/Users/hanamori/life-assistant/.worktrees/phase-3a-today-tasks` on a new branch `feature/phase-3a-today-tasks` branched from `main`. Execute the plan inside that worktree.

---

## Task breakdown (15 tasks)

1. `V2__task.sql` migration + `task` data-access module (TDD)
2. Extend `proposal` module with `approve_add_task` + `reject` (TDD)
3. `tools.rs` module with `add_task` schema (no test)
4. Extend Ollama client: `tools` in request, parse `tool_calls`, `StreamChunk::Started`/`Token`/`Proposal`/`Done`/`Error` evolved (TDD with wiremock)
5. System prompt addition
6. New Tauri commands + `send_message` rewires for tool calls + emit `Proposal` chunk
7. Frontend `lib/layout.ts` + `lib/today/ipc.ts` + `lib/today/slash.ts` + `lib/today/state.ts` with vitest tests (TDD)
8. `HeaderCard` + `EventsCard` components
9. `TaskRow` component with edit / hover / completion states
10. `TasksCard` component (list + "+ Add" reveal + empty state)
11. `ProposalBanner` component
12. `Today.tsx` composes everything; mount in `App.tsx`
13. Wire slash detection into `Assistant.tsx` + `ConversationDrawer` submit
14. Toast component + delete soft-confirm + commit polish
15. Manual smoke + tag + PR

---

### Task 1: `V2__task.sql` migration + `task` data-access module (TDD)

**Files:**
- Create: `crates/core/migrations/V2__task.sql`
- Create: `crates/core/src/assistant/task.rs`
- Modify: `crates/core/src/assistant/mod.rs`

- [ ] **Step 1: Write the SQL migration**

Create `crates/core/migrations/V2__task.sql`:

```sql
CREATE TABLE task (
  id              INTEGER PRIMARY KEY,
  title           TEXT    NOT NULL,
  due_date        TEXT    NULL,
  completed_at    INTEGER NULL,
  created_at      INTEGER NOT NULL,
  proposal_id     INTEGER NULL REFERENCES proposal(id)
);

CREATE INDEX idx_task_open_due ON task (completed_at, due_date);
```

- [ ] **Step 2: Expose the new submodule**

Edit `crates/core/src/assistant/mod.rs`:

```rust
//! Assistant substrate: SQLite persistence for conversations, messages, proposals, and tasks.

pub mod conversation;
pub mod db;
pub mod message;
pub mod proposal;
pub mod task;
```

- [ ] **Step 3: Write `task.rs` with type, functions, and TDD tests**

Create `crates/core/src/assistant/task.rs`:

```rust
//! Tasks — the user's open / completed to-dos.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub due_date: Option<String>,
    pub completed_at: Option<i64>,
    pub created_at: i64,
    pub proposal_id: Option<i64>,
}

impl Task {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            due_date: row.get("due_date")?,
            completed_at: row.get("completed_at")?,
            created_at: row.get("created_at")?,
            proposal_id: row.get("proposal_id")?,
        })
    }
}

/// Insert a new task. Returns the new row id.
pub fn insert(
    conn: &Connection,
    title: &str,
    due_date: Option<&str>,
    proposal_id: Option<i64>,
) -> Result<i64> {
    let now_ms = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO task (title, due_date, completed_at, created_at, proposal_id)
         VALUES (?1, ?2, NULL, ?3, ?4)",
        params![title, due_date, now_ms, proposal_id],
    )?;
    Ok(conn.last_insert_rowid())
}

/// All open tasks (completed_at IS NULL), ordered with NULL due_dates last.
/// Used by Phase 3c's prompt context — it wants every open task Manor should
/// know about, regardless of due date.
pub fn list_open(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, due_date, completed_at, created_at, proposal_id
         FROM task
         WHERE completed_at IS NULL
         ORDER BY (due_date IS NULL), due_date, created_at",
    )?;
    let rows = stmt
        .query_map([], Task::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Open tasks due today or with no due date. Used by Phase 3a's Today view
/// so tasks scheduled for future days don't appear in today's list.
pub fn list_today_open(conn: &Connection, today_iso: &str) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, due_date, completed_at, created_at, proposal_id
         FROM task
         WHERE completed_at IS NULL AND (due_date IS NULL OR due_date = ?1)
         ORDER BY (due_date IS NULL), due_date, created_at",
    )?;
    let rows = stmt
        .query_map([today_iso], Task::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Mark a task complete (set completed_at to now).
pub fn complete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE task SET completed_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Undo completion (set completed_at to NULL). Called inside the 4s undo window.
pub fn undo_complete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE task SET completed_at = NULL WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

/// Rename a task.
pub fn update_title(conn: &Connection, id: i64, title: &str) -> Result<()> {
    conn.execute(
        "UPDATE task SET title = ?1 WHERE id = ?2",
        params![title, id],
    )?;
    Ok(())
}

/// Hard-delete a task.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM task WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn insert_returns_id_and_persists() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "Pick up prescription", Some("2026-04-15"), None).unwrap();
        assert!(id > 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM task WHERE id = ?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn list_open_excludes_completed() {
        let (_d, conn) = fresh_conn();
        let a = insert(&conn, "A", Some("2026-04-15"), None).unwrap();
        let _b = insert(&conn, "B", Some("2026-04-15"), None).unwrap();
        complete(&conn, a).unwrap();

        let open = list_open(&conn).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].title, "B");
    }

    #[test]
    fn list_open_orders_by_due_date_then_created_at() {
        let (_d, conn) = fresh_conn();
        // No due date — should sort to the end.
        insert(&conn, "no_due", None, None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        // Far due date.
        insert(&conn, "later", Some("2026-04-30"), None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        // Earlier due date.
        insert(&conn, "earlier", Some("2026-04-15"), None).unwrap();

        let open = list_open(&conn).unwrap();
        let titles: Vec<&str> = open.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["earlier", "later", "no_due"]);
    }

    #[test]
    fn list_today_open_filters_by_today_and_includes_no_due() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "today_task", Some("2026-04-15"), None).unwrap();
        insert(&conn, "future_task", Some("2026-04-30"), None).unwrap();
        insert(&conn, "no_due_task", None, None).unwrap();

        let today = list_today_open(&conn, "2026-04-15").unwrap();
        let titles: Vec<&str> = today.iter().map(|t| t.title.as_str()).collect();
        // future_task excluded; today_task and no_due_task included.
        assert!(titles.contains(&"today_task"));
        assert!(titles.contains(&"no_due_task"));
        assert!(!titles.contains(&"future_task"));
    }

    #[test]
    fn complete_then_undo_round_trip() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "T", Some("2026-04-15"), None).unwrap();
        complete(&conn, id).unwrap();
        assert_eq!(list_open(&conn).unwrap().len(), 0);

        undo_complete(&conn, id).unwrap();
        let open = list_open(&conn).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, id);
        assert!(open[0].completed_at.is_none());
    }

    #[test]
    fn update_title_persists() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "Old", Some("2026-04-15"), None).unwrap();
        update_title(&conn, id, "New").unwrap();
        let open = list_open(&conn).unwrap();
        assert_eq!(open[0].title, "New");
    }

    #[test]
    fn delete_removes_row() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "Doomed", Some("2026-04-15"), None).unwrap();
        delete(&conn, id).unwrap();
        assert_eq!(list_open(&conn).unwrap().len(), 0);
    }
}
```

- [ ] **Step 4: Run tests + clippy + fmt**

Run: `cargo test -p manor-core --all-targets`
Expected: existing 12 tests + 7 new = 19 passing.

Run: `cargo clippy -p manor-core --all-targets -- -D warnings`
Expected: clean.

Run: `cargo fmt --all --check`
Expected: clean (or run `cargo fmt --all` if it surfaces wrap fixes — see Phase 2's commit `eb38b37`).

- [ ] **Step 5: Commit**

```bash
git add crates/core/migrations/V2__task.sql crates/core/src/assistant/task.rs crates/core/src/assistant/mod.rs
git commit -m "feat(core): task table + V2 migration + CRUD with TDD"
```

---

### Task 2: Extend `proposal` module with `approve_add_task` + `reject` (TDD)

**Files:**
- Modify: `crates/core/src/assistant/proposal.rs`

- [ ] **Step 1: Read current proposal.rs to understand the existing shape**

Run: `cat crates/core/src/assistant/proposal.rs`
Expected: existing module with `Status` enum, `NewProposal`, and `insert()`.

- [ ] **Step 2: Add the `approve_add_task` + `reject` functions and their tests**

Replace `crates/core/src/assistant/proposal.rs` with:

```rust
//! Proposals — central AI-action artefacts.
//!
//! Phase 2 scaffolded the table + types. Phase 3a wires the first lifecycle:
//! `add_task` proposals can be applied (insert task + mark applied) or
//! rejected (mark rejected, no apply).

use anyhow::{bail, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::assistant::task;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Approved,
    Rejected,
    Applied,
    PartiallyApplied,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Approved => "approved",
            Status::Rejected => "rejected",
            Status::Applied => "applied",
            Status::PartiallyApplied => "partially_applied",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProposal<'a> {
    pub kind: &'a str,
    pub rationale: &'a str,
    pub diff_json: &'a str,
    pub skill: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Proposal {
    pub id: i64,
    pub kind: String,
    pub rationale: String,
    pub diff: String,
    pub status: String,
    pub proposed_at: i64,
    pub applied_at: Option<i64>,
    pub skill: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddTaskArgs {
    pub title: String,
    pub due_date: Option<String>,
}

/// Insert a new proposal. Returns the new row id.
pub fn insert(conn: &Connection, new: NewProposal<'_>) -> Result<i64> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO proposal (kind, rationale, diff, status, proposed_at, skill)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            new.kind,
            new.rationale,
            new.diff_json,
            Status::Pending.as_str(),
            now,
            new.skill,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// List proposals filtered by status (pass `None` for all).
pub fn list(conn: &Connection, status: Option<&str>) -> Result<Vec<Proposal>> {
    let (sql, has_filter) = match status {
        Some(_) => (
            "SELECT id, kind, rationale, diff, status, proposed_at, applied_at, skill
             FROM proposal WHERE status = ?1 ORDER BY proposed_at",
            true,
        ),
        None => (
            "SELECT id, kind, rationale, diff, status, proposed_at, applied_at, skill
             FROM proposal ORDER BY proposed_at",
            false,
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let mapper = |row: &rusqlite::Row| {
        Ok(Proposal {
            id: row.get("id")?,
            kind: row.get("kind")?,
            rationale: row.get("rationale")?,
            diff: row.get("diff")?,
            status: row.get("status")?,
            proposed_at: row.get("proposed_at")?,
            applied_at: row.get("applied_at")?,
            skill: row.get("skill")?,
        })
    };
    let rows: Vec<Proposal> = if has_filter {
        stmt.query_map(params![status.unwrap()], mapper)?
            .collect::<rusqlite::Result<_>>()?
    } else {
        stmt.query_map([], mapper)?
            .collect::<rusqlite::Result<_>>()?
    };
    Ok(rows)
}

/// Apply a pending `add_task` proposal: insert the task, mark proposal as `applied`.
/// Returns the refreshed list of all open tasks (caller usually wants this for UI sync).
pub fn approve_add_task(conn: &mut Connection, id: i64, today_iso: &str) -> Result<Vec<task::Task>> {
    let tx = conn.transaction()?;

    let row: Option<(String, String, String)> = tx
        .query_row(
            "SELECT kind, status, diff FROM proposal WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (kind, status, diff) = match row {
        Some(r) => r,
        None => bail!("proposal {id} not found"),
    };
    if status != "pending" {
        bail!("proposal {id} is not pending (status={status})");
    }
    if kind != "add_task" {
        bail!("proposal {id} has unsupported kind: {kind}");
    }

    let args: AddTaskArgs = serde_json::from_str(&diff)?;
    let due_date = args.due_date.unwrap_or_else(|| today_iso.to_string());
    task::insert(&tx, &args.title, Some(&due_date), Some(id))?;

    tx.execute(
        "UPDATE proposal SET status = 'applied', applied_at = ?1 WHERE id = ?2",
        params![Utc::now().timestamp(), id],
    )?;

    tx.commit()?;
    task::list_open(conn)
}

/// Mark a pending proposal rejected. No-op (returns Ok) if the proposal is already
/// non-pending — caller may have raced with another approve/reject.
pub fn reject(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE proposal SET status = 'rejected' WHERE id = ?1 AND status = 'pending'",
        [id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn make_add_task_proposal(conn: &Connection, title: &str) -> i64 {
        let diff = serde_json::json!({ "title": title }).to_string();
        insert(
            conn,
            NewProposal {
                kind: "add_task",
                rationale: "test rationale",
                diff_json: &diff,
                skill: "tasks",
            },
        )
        .unwrap()
    }

    #[test]
    fn insert_returns_new_row_id() {
        let (_d, conn) = fresh_conn();
        let id = make_add_task_proposal(&conn, "Test");
        assert!(id > 0);
    }

    #[test]
    fn list_pending_filters_by_status() {
        let (_d, mut conn) = fresh_conn();
        let a = make_add_task_proposal(&conn, "A");
        let b = make_add_task_proposal(&conn, "B");
        approve_add_task(&mut conn, a, "2026-04-15").unwrap();

        let pending = list(&conn, Some("pending")).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, b);

        let all = list(&conn, None).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn approve_add_task_creates_task_and_marks_applied() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Pick up prescription");
        let tasks = approve_add_task(&mut conn, pid, "2026-04-15").unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Pick up prescription");
        assert_eq!(tasks[0].due_date.as_deref(), Some("2026-04-15"));
        assert_eq!(tasks[0].proposal_id, Some(pid));

        let proposal: String = conn
            .query_row("SELECT status FROM proposal WHERE id = ?1", [pid], |r| r.get(0))
            .unwrap();
        assert_eq!(proposal, "applied");
    }

    #[test]
    fn approve_already_applied_errors() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "X");
        approve_add_task(&mut conn, pid, "2026-04-15").unwrap();
        let err = approve_add_task(&mut conn, pid, "2026-04-15").unwrap_err();
        assert!(err.to_string().contains("not pending"));
    }

    #[test]
    fn reject_marks_rejected_without_applying() {
        let (_d, conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Y");
        reject(&conn, pid).unwrap();

        let proposal: String = conn
            .query_row("SELECT status FROM proposal WHERE id = ?1", [pid], |r| r.get(0))
            .unwrap();
        assert_eq!(proposal, "rejected");

        let task_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM task", [], |r| r.get(0))
            .unwrap();
        assert_eq!(task_count, 0);
    }

    #[test]
    fn reject_already_rejected_is_noop() {
        let (_d, conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Z");
        reject(&conn, pid).unwrap();
        reject(&conn, pid).unwrap(); // does not error
    }

    #[test]
    fn approve_uses_proposal_due_date_when_present() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({ "title": "Future thing", "due_date": "2026-04-30" }).to_string();
        let pid = insert(
            &conn,
            NewProposal {
                kind: "add_task",
                rationale: "r",
                diff_json: &diff,
                skill: "tasks",
            },
        )
        .unwrap();
        let tasks = approve_add_task(&mut conn, pid, "2026-04-15").unwrap();
        assert_eq!(tasks[0].due_date.as_deref(), Some("2026-04-30"));
    }
}
```

- [ ] **Step 3: Run tests + clippy + fmt**

Run: `cargo test -p manor-core --all-targets`
Expected: 19 + 6 new + 1 modified = 25 tests, all passing. (The original `insert_returns_new_row_id` is preserved, plus 6 new tests.)

Run: `cargo clippy -p manor-core --all-targets -- -D warnings`
Run: `cargo fmt --all --check`
Both clean.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/assistant/proposal.rs
git commit -m "feat(core): proposal lifecycle — approve_add_task + reject + list (TDD)"
```

---

### Task 3: `tools.rs` module with `add_task` schema

**Files:**
- Create: `crates/app/src/assistant/tools.rs`
- Modify: `crates/app/src/assistant/mod.rs`

- [ ] **Step 1: Expose the new submodule**

Edit `crates/app/src/assistant/mod.rs`:

```rust
//! Assistant glue: Ollama client + Tauri commands.

pub mod commands;
pub mod ollama;
pub mod prompts;
pub mod tools;
```

- [ ] **Step 2: Write `tools.rs`**

Create `crates/app/src/assistant/tools.rs`:

```rust
//! Tool schemas declared to Ollama for function calling.

use serde_json::json;

/// JSON schema for the `add_task` tool — Manor's only tool in Phase 3a.
pub fn add_task_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "add_task",
            "description": "Propose adding a task to the user's task list. \
                            Use when the user asks you to remember, remind, or \
                            track something. Do not claim to have added it; the \
                            user must approve the proposal first.",
            "parameters": {
                "type": "object",
                "required": ["title"],
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "A short imperative — e.g. 'Pick up prescription'"
                    },
                    "due_date": {
                        "type": "string",
                        "description": "Optional. ISO date, format YYYY-MM-DD. Omit for today."
                    }
                }
            }
        }
    })
}

/// All tools available in Phase 3a.
pub fn all_tools() -> Vec<serde_json::Value> {
    vec![add_task_tool()]
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p manor-app`
Expected: clean (no tests in this module — schema correctness is exercised by Task 4's wiremock test).

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/assistant/tools.rs crates/app/src/assistant/mod.rs
git commit -m "feat(app): add_task tool schema for Ollama function calling"
```

---

### Task 4: Extend Ollama client — tools in request, parse tool_calls, new StreamChunk variants (TDD with wiremock)

**Files:**
- Modify: `crates/app/src/assistant/ollama.rs`

- [ ] **Step 1: Read current ollama.rs**

Run: `cat crates/app/src/assistant/ollama.rs`
Note the existing `ChatRequest`, `OllamaChunkMessage`, `StreamChunk`, and `OllamaClient::chat()` shapes.

- [ ] **Step 2: Modify `ollama.rs` to support tool calling**

Edit the file to make these changes:

1. Add a `tools` field to `ChatRequest`:

```rust
#[derive(Debug, Clone, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
    tools: &'a [serde_json::Value],
}
```

2. Add `tool_calls` to `OllamaChunkMessage`, with two new structs:

```rust
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
```

3. Add `Proposal(i64)` variant to `StreamChunk` (kept as an exhaustive `#[serde(tag, content)]` enum):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value")]
pub enum StreamChunk {
    Started(i64),
    Token(String),
    Proposal(i64),
    Done,
    Error(ErrorCode),
}
```

4. Modify `OllamaClient::chat()` to:
   - Accept a `tools: &[serde_json::Value]` parameter
   - Collect any tool_calls seen during the stream into a `Vec<OllamaToolCall>`
   - Return `(stream_outcome, tool_calls)` so `commands::send_message` can act on them after the stream

The full file should now read:

```rust
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
        let mut saw_done = false;

        while let Some(piece) = stream.next().await {
            let bytes = match piece {
                Ok(b) => b,
                Err(_) => {
                    let _ = out.send(StreamChunk::Error(ErrorCode::Interrupted)).await;
                    return ChatOutcome { tool_calls: collected_tool_calls };
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
                            saw_done = true;
                            // Don't emit Done here — caller handles it after acting on tool_calls.
                            return ChatOutcome { tool_calls: collected_tool_calls };
                        }
                    }
                    Err(_) => {
                        let _ = out.send(StreamChunk::Error(ErrorCode::Unknown)).await;
                        return ChatOutcome { tool_calls: collected_tool_calls };
                    }
                }
            }
        }

        if !saw_done {
            let _ = out.send(StreamChunk::Error(ErrorCode::Interrupted)).await;
        }
        ChatOutcome { tool_calls: collected_tool_calls }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn ndjson(lines: &[&str]) -> String {
        lines.iter().map(|l| format!("{l}\n")).collect::<Vec<_>>().join("")
    }

    #[tokio::test]
    async fn streams_tokens_then_returns_no_tool_calls_on_done() {
        let server = MockServer::start().await;
        let body = ndjson(&[
            r#"{"message":{"role":"assistant","content":"Hel"},"done":false}"#,
            r#"{"message":{"role":"assistant","content":"lo."},"done":false}"#,
            r#"{"message":{"role":"assistant","content":""},"done":true}"#,
        ]);
        Mock::given(method("POST")).and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "test-model");
        let (tx, mut rx) = mpsc::channel(32);
        let messages = vec![ChatMessage { role: ChatRole::User, content: "hi".into() }];
        let outcome = client.chat(&messages, &[], &tx).await;

        let mut received = Vec::new();
        while let Ok(c) = rx.try_recv() { received.push(c); }
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
        Mock::given(method("POST")).and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "test-model");
        let (tx, mut rx) = mpsc::channel(32);
        let outcome = client.chat(
            &[ChatMessage { role: ChatRole::User, content: "remind me to pick up prescription".into() }],
            &[],
            &tx,
        ).await;

        let mut received = Vec::new();
        while let Ok(c) = rx.try_recv() { received.push(c); }
        assert_eq!(received, vec![StreamChunk::Token("I'd like to add that.".into())]);

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
        let _ = client.chat(
            &[ChatMessage { role: ChatRole::User, content: "hi".into() }],
            &[],
            &tx,
        ).await;
        let first = rx.recv().await.unwrap();
        assert_eq!(first, StreamChunk::Error(ErrorCode::OllamaUnreachable));
    }

    #[tokio::test]
    async fn not_found_emits_model_missing() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = OllamaClient::new(server.uri(), "nonexistent-model");
        let (tx, mut rx) = mpsc::channel(4);
        let _ = client.chat(
            &[ChatMessage { role: ChatRole::User, content: "hi".into() }],
            &[],
            &tx,
        ).await;
        let first = rx.recv().await.unwrap();
        assert_eq!(first, StreamChunk::Error(ErrorCode::ModelMissing));
    }
}
```

**Note on the chat() signature change:** the function no longer emits `StreamChunk::Done` itself — it returns the collected tool_calls and lets the caller emit `Done` (or `Proposal` chunks first, then `Done`). This is a breaking change to `commands::send_message`, fixed in Task 6.

- [ ] **Step 3: Run tests + clippy + fmt**

Run: `cargo test -p manor-app --all-targets`
Expected: existing 2 tests + 4 new wiremock tests (one is the modified `streams_tokens_then_returns_no_tool_calls_on_done`, replacing the old `streams_tokens_then_done`). Total: 6 tests, all passing.

Run: `cargo clippy -p manor-app --all-targets -- -D warnings`
Run: `cargo fmt --all --check`
Both clean.

**Note:** `cargo check -p manor-desktop` will fail at this step because `commands::send_message` still calls the old `chat()` signature. That's expected — Task 6 fixes it. Only run the test commands above.

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/assistant/ollama.rs
git commit -m "feat(app): ollama tool-calling support — tools in request, tool_calls in response, Proposal StreamChunk variant"
```

---

### Task 5: System prompt addition

**Files:**
- Modify: `crates/app/src/assistant/prompts.rs`

- [ ] **Step 1: Read current prompts.rs**

Run: `cat crates/app/src/assistant/prompts.rs`
Note the existing `SYSTEM_PROMPT` constant (one paragraph).

- [ ] **Step 2: Append the tool-use boundary instruction**

Replace `crates/app/src/assistant/prompts.rs` with:

```rust
//! Prompts sent to the local LLM.

/// System prompt for Manor. Establishes identity, role, persona hygiene,
/// and (Phase 3a) the tool-use boundary so she proposes rather than claims.
pub const SYSTEM_PROMPT: &str = concat!(
    "You are Manor, a calm household assistant built into a local-first desktop app. ",
    "You help the user manage their calendar, chores, money, meals, and home. ",
    "Be warm, concise, and practical. Never speak as Nell or any other persona. ",
    "If you need to modify the user's data, describe the change you would make ",
    "rather than claiming to have made it; the app will ask for explicit approval.",
    "\n\n",
    "You can propose changes to the user's data using the tools provided. ",
    "When you call a tool, the change is *proposed* — the user reviews and ",
    "approves before it takes effect. Do not say 'I added' or 'I did' — say ",
    "'I'd like to add' or 'shall I…?' instead. The proposal banner will ",
    "show them what you suggested.",
);
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p manor-app --tests`
Expected: clean (no behaviour change beyond the constant string content).

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/assistant/prompts.rs
git commit -m "feat(app): system prompt — Manor proposes, never claims done"
```

---

### Task 6: New Tauri commands + `send_message` rewires for tool calls

**Files:**
- Modify: `crates/app/src/assistant/commands.rs`

- [ ] **Step 1: Read current commands.rs**

Run: `cat crates/app/src/assistant/commands.rs`

- [ ] **Step 2: Replace `commands.rs` with the new shape**

The new file adds 9 commands (list_tasks, add_task, complete_task, undo_complete_task, update_task, delete_task, list_proposals, approve_proposal, reject_proposal) and rewires `send_message` to declare tools, collect tool_calls from `chat()`, create proposals from them, and emit `Proposal` chunks before `Done`.

Replace `crates/app/src/assistant/commands.rs` with:

```rust
//! Tauri commands exposed to the frontend Assistant + Today view.

use crate::assistant::ollama::{
    ChatMessage, ChatRole, OllamaClient, OllamaToolCall, StreamChunk, DEFAULT_ENDPOINT, DEFAULT_MODEL,
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
        chat_msgs.push(ChatMessage { role, content: m.content });
    }

    // 4. Run the Ollama stream with tools declared.
    let client = OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL);
    let tools_vec = tools::all_tools();
    let (tx, mut rx) = mpsc::channel::<StreamChunk>(64);

    let chat_state = state.0.clone();  // for the lock inside the loop below

    // Spawn the chat call so we can interleave persistence with the stream.
    let chat_msgs_for_task = chat_msgs.clone();
    let tools_for_task = tools_vec.clone();
    let chat_task = tokio::spawn(async move {
        let local_client = OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL);
        local_client.chat(&chat_msgs_for_task, &tools_for_task, &tx).await
    });

    while let Some(chunk) = rx.recv().await {
        if let StreamChunk::Token(frag) = &chunk {
            let conn = chat_state.lock().map_err(|e| e.to_string())?;
            message::append_content(&conn, assistant_row_id, frag).map_err(|e| e.to_string())?;
        }
        on_event.send(chunk).map_err(|e| e.to_string())?;
    }

    let outcome = chat_task.await.map_err(|e| e.to_string())?;

    // 5. Capture rationale (Manor's text so far) for the proposal.
    let rationale = {
        let conn = chat_state.lock().map_err(|e| e.to_string())?;
        let msgs = message::list(&conn, 1, 1, 0).map_err(|e| e.to_string())?;
        msgs.first()
            .filter(|m| m.id == assistant_row_id)
            .map(|m| m.content.clone())
            .unwrap_or_default()
    };

    // 6. Process collected tool calls into proposals.
    for tool_call in outcome.tool_calls {
        match tool_call.function.name.as_str() {
            "add_task" => {
                let args: AddTaskArgs = serde_json::from_value(tool_call.function.arguments)
                    .map_err(|e| format!("bad add_task args: {e}"))?;
                let diff_json = serde_json::to_string(&args).map_err(|e| e.to_string())?;
                let proposal_id = {
                    let conn = chat_state.lock().map_err(|e| e.to_string())?;
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

    // 7. Emit Done.
    on_event.send(StreamChunk::Done).map_err(|e| e.to_string())?;
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
            "SELECT id, title, due_date, completed_at, created_at, proposal_id FROM task WHERE id = ?1",
            [id],
            |r| {
                Ok(Task {
                    id: r.get("id").map_err(|e| rusqlite::Error::from(e))?,
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
```

**Note:** `chat_state.clone()` requires `Mutex<Connection>` to be wrapped in an `Arc`. But Tauri's `State<'_, Db>` already gives us a shared reference; the inner `Mutex` is what needs sharing. Looking at the spawn — actually we can avoid the clone by NOT spawning. Refactor: do the chat call inline (don't spawn), since the channel is already async. Adjust the `send_message` body so the chat call is `await`ed inline rather than spawned, and the lock is acquired only inside the loop, not held across the await.

Replace the relevant block in `send_message` (steps 4–6 in the prior code) with:

```rust
    // 4. Run the Ollama stream with tools declared.
    let client = OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL);
    let tools_vec = tools::all_tools();
    let (tx, mut rx) = mpsc::channel::<StreamChunk>(64);

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

    // Persist all token chunks in one DB transaction.
    let rationale = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        for frag in &chunks_to_persist {
            message::append_content(&conn, assistant_row_id, frag).map_err(|e| e.to_string())?;
        }
        // Pull the freshly-built rationale.
        let msgs = message::list(&conn, 1, 1, 0).map_err(|e| e.to_string())?;
        msgs.first()
            .filter(|m| m.id == assistant_row_id)
            .map(|m| m.content.clone())
            .unwrap_or_default()
    };

    // Replay events to the frontend Channel after persistence is done.
    for event in events {
        on_event.send(event).map_err(|e| e.to_string())?;
    }
```

Then the tool-call processing loop and `Done` emission stay as-is (steps 6–7 in the prior code). The full revised file should be the entire content above with this block substituted.

This refactor solves the lifetime/Clone problem on Mutex<Connection>: we don't try to share state across spawned tasks; the chat happens inline, the receiver task collects chunks, then we persist + replay in one shot.

**Tradeoff vs. true streaming:** the frontend now sees all tokens at once after the stream completes, instead of as they arrive. For Phase 3a this is acceptable — we're testing the proposal flow, not optimising perceived latency. A future iteration can move to a true-streaming pattern by wrapping the connection in `Arc<Mutex<...>>` properly, but doing it cleanly without breaking Phase 2's TTL/bubble UX is its own task. Document this as a Phase 3a deviation.

- [ ] **Step 3: Verify build + tests**

Run: `cargo test --workspace --all-targets`
Expected: all 31 tests pass (19 core + 6 app + 6 task tests in Task 1).

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo fmt --all --check`

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/assistant/commands.rs
git commit -m "feat(app): tasks/proposals commands + send_message tool-call wiring

Trade real-time token streaming for inline-then-replay so we sidestep
needing Arc<Mutex<Connection>> in this phase. Acceptable for 3a — the
proposal flow is the focus; perceived latency can be revisited in a
later phase if needed."
```

---

### Task 7: Frontend `lib/layout.ts` + `lib/today/{ipc,slash,state}.ts` with vitest tests (TDD)

**Files:**
- Create: `apps/desktop/src/lib/layout.ts`
- Create: `apps/desktop/src/lib/today/ipc.ts`
- Create: `apps/desktop/src/lib/today/slash.ts`
- Create: `apps/desktop/src/lib/today/slash.test.ts`
- Create: `apps/desktop/src/lib/today/state.ts`
- Create: `apps/desktop/src/lib/today/state.test.ts`
- Modify: `apps/desktop/src/lib/assistant/ipc.ts` (add `Proposal` to `StreamChunk`)

- [ ] **Step 1: Layout constant**

Create `apps/desktop/src/lib/layout.ts`:

```ts
/**
 * Pixels reserved at the bottom of the main window for the corner avatar
 * (height 96 + 16px corner padding + 28px breathing room). Both the avatar's
 * own positioning and the Today view's bottom padding consume this constant
 * so they never drift out of sync.
 */
export const AVATAR_FOOTPRINT_PX = 140;
```

- [ ] **Step 2: Update Phase 2's StreamChunk type to include Proposal**

Edit `apps/desktop/src/lib/assistant/ipc.ts` — add the `Proposal` variant to the `StreamChunk` union:

```ts
export type StreamChunk =
  | { type: "Started"; value: number }
  | { type: "Token"; value: string }
  | { type: "Proposal"; value: number }
  | { type: "Done" }
  | { type: "Error"; value: "OllamaUnreachable" | "ModelMissing" | "Interrupted" | "Unknown" };
```

Leave the rest of the file unchanged.

- [ ] **Step 3: Create the today/ipc.ts wrappers**

Create `apps/desktop/src/lib/today/ipc.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export interface Task {
  id: number;
  title: string;
  due_date: string | null;
  completed_at: number | null;
  created_at: number;
  proposal_id: number | null;
}

export interface Proposal {
  id: number;
  kind: string;
  rationale: string;
  diff: string;        // raw JSON string
  status: string;      // 'pending' | 'applied' | 'rejected' | ...
  proposed_at: number;
  applied_at: number | null;
  skill: string;
}

export async function listTasks(): Promise<Task[]> {
  return invoke<Task[]>("list_tasks");
}

export async function addTask(title: string, dueDate?: string | null): Promise<Task> {
  return invoke<Task>("add_task", { title, dueDate: dueDate ?? null });
}

export async function completeTask(id: number): Promise<void> {
  return invoke<void>("complete_task", { id });
}

export async function undoCompleteTask(id: number): Promise<void> {
  return invoke<void>("undo_complete_task", { id });
}

export async function updateTask(id: number, title: string): Promise<void> {
  return invoke<void>("update_task", { id, title });
}

export async function deleteTask(id: number): Promise<void> {
  return invoke<void>("delete_task", { id });
}

export async function listProposals(status?: string): Promise<Proposal[]> {
  return invoke<Proposal[]>("list_proposals", { status: status ?? null });
}

export async function approveProposal(id: number): Promise<Task[]> {
  return invoke<Task[]>("approve_proposal", { id });
}

export async function rejectProposal(id: number): Promise<void> {
  return invoke<void>("reject_proposal", { id });
}
```

- [ ] **Step 4: Write the failing slash test (TDD)**

Create `apps/desktop/src/lib/today/slash.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { parseSlash } from "./slash";

describe("parseSlash", () => {
  it("returns null for non-slash input", () => {
    expect(parseSlash("hello")).toBeNull();
    expect(parseSlash("")).toBeNull();
    expect(parseSlash("  /task ")).toBeNull(); // leading whitespace not a slash command
  });

  it("parses /task with title", () => {
    expect(parseSlash("/task pick up prescription")).toEqual({
      type: "task",
      title: "pick up prescription",
    });
  });

  it("trims trailing whitespace from title", () => {
    expect(parseSlash("/task  reply to Miriam   ")).toEqual({
      type: "task",
      title: "reply to Miriam",
    });
  });

  it("returns null for /task with empty title", () => {
    expect(parseSlash("/task")).toBeNull();
    expect(parseSlash("/task   ")).toBeNull();
  });

  it("returns unknown for unrecognised slash command", () => {
    expect(parseSlash("/banana split")).toEqual({
      type: "unknown",
      raw: "/banana split",
    });
  });
});
```

- [ ] **Step 5: Implement slash.ts**

Create `apps/desktop/src/lib/today/slash.ts`:

```ts
export type SlashCommand =
  | { type: "task"; title: string }
  | { type: "unknown"; raw: string };

/**
 * Parse a submitted message for slash-command syntax.
 *
 * Returns:
 *  - null if the input is not a slash command (no leading slash, or `/task` with no title)
 *  - a typed SlashCommand object otherwise
 *
 * Only `/task <title>` is recognised in Phase 3a. Future commands should add
 * their own arms here and update the test file in parity.
 */
export function parseSlash(input: string): SlashCommand | null {
  if (!input.startsWith("/")) return null;

  const taskMatch = input.match(/^\/task\s+(.+?)\s*$/);
  if (taskMatch) {
    return { type: "task", title: taskMatch[1].trim() };
  }
  if (input === "/task" || /^\/task\s*$/.test(input)) {
    return null; // empty title — treat as not-a-command (falls through to chat)
  }

  return { type: "unknown", raw: input };
}
```

- [ ] **Step 6: Create the today/state.ts store + tests**

Create `apps/desktop/src/lib/today/state.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useTodayStore } from "./state";
import type { Task, Proposal } from "./ipc";

const sampleTask = (overrides: Partial<Task> = {}): Task => ({
  id: 1,
  title: "Sample",
  due_date: "2026-04-15",
  completed_at: null,
  created_at: Date.now(),
  proposal_id: null,
  ...overrides,
});

const sampleProposal = (overrides: Partial<Proposal> = {}): Proposal => ({
  id: 1,
  kind: "add_task",
  rationale: "Manor said so",
  diff: '{"title":"X"}',
  status: "pending",
  proposed_at: Date.now(),
  applied_at: null,
  skill: "tasks",
  ...overrides,
});

describe("useTodayStore", () => {
  beforeEach(() => {
    useTodayStore.setState(useTodayStore.getInitialState(), true);
  });

  it("starts empty", () => {
    const s = useTodayStore.getState();
    expect(s.tasks).toEqual([]);
    expect(s.pendingProposals).toEqual([]);
  });

  it("setTasks replaces the array", () => {
    const a = sampleTask({ id: 1, title: "A" });
    const b = sampleTask({ id: 2, title: "B" });
    useTodayStore.getState().setTasks([a, b]);
    expect(useTodayStore.getState().tasks).toEqual([a, b]);
  });

  it("upsertTask appends a new id", () => {
    const a = sampleTask({ id: 1 });
    const b = sampleTask({ id: 2 });
    useTodayStore.getState().setTasks([a]);
    useTodayStore.getState().upsertTask(b);
    expect(useTodayStore.getState().tasks).toEqual([a, b]);
  });

  it("upsertTask replaces an existing id", () => {
    const a = sampleTask({ id: 1, title: "old" });
    const aPrime = sampleTask({ id: 1, title: "new" });
    useTodayStore.getState().setTasks([a]);
    useTodayStore.getState().upsertTask(aPrime);
    expect(useTodayStore.getState().tasks).toEqual([aPrime]);
  });

  it("removeTask drops by id", () => {
    const a = sampleTask({ id: 1 });
    const b = sampleTask({ id: 2 });
    useTodayStore.getState().setTasks([a, b]);
    useTodayStore.getState().removeTask(1);
    expect(useTodayStore.getState().tasks).toEqual([b]);
  });

  it("setPendingProposals replaces the array", () => {
    const p = sampleProposal();
    useTodayStore.getState().setPendingProposals([p]);
    expect(useTodayStore.getState().pendingProposals).toEqual([p]);
  });

  it("removeProposal drops by id", () => {
    const p1 = sampleProposal({ id: 1 });
    const p2 = sampleProposal({ id: 2 });
    useTodayStore.getState().setPendingProposals([p1, p2]);
    useTodayStore.getState().removeProposal(1);
    expect(useTodayStore.getState().pendingProposals).toEqual([p2]);
  });
});
```

Create `apps/desktop/src/lib/today/state.ts`:

```ts
import { create } from "zustand";
import type { Task, Proposal } from "./ipc";

interface TodayStore {
  tasks: Task[];
  pendingProposals: Proposal[];

  setTasks: (t: Task[]) => void;
  upsertTask: (t: Task) => void;
  removeTask: (id: number) => void;

  setPendingProposals: (p: Proposal[]) => void;
  removeProposal: (id: number) => void;
}

export const useTodayStore = create<TodayStore>((set) => ({
  tasks: [],
  pendingProposals: [],

  setTasks: (t) => set({ tasks: t }),

  upsertTask: (t) =>
    set((st) => {
      const idx = st.tasks.findIndex((x) => x.id === t.id);
      if (idx === -1) return { tasks: [...st.tasks, t] };
      const next = st.tasks.slice();
      next[idx] = t;
      return { tasks: next };
    }),

  removeTask: (id) =>
    set((st) => ({ tasks: st.tasks.filter((x) => x.id !== id) })),

  setPendingProposals: (p) => set({ pendingProposals: p }),

  removeProposal: (id) =>
    set((st) => ({ pendingProposals: st.pendingProposals.filter((x) => x.id !== id) })),
}));
```

- [ ] **Step 7: Run the tests**

Run: `pnpm --filter manor-desktop test`
Expected: existing 8 tests + 5 slash + 7 today-store = 20 passing.

Run: `pnpm tsc`
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src/lib/layout.ts apps/desktop/src/lib/today apps/desktop/src/lib/assistant/ipc.ts
git commit -m "feat(desktop): today store + ipc wrappers + slash parser (TDD)"
```

---

### Task 8: `HeaderCard` + `EventsCard` components

**Files:**
- Create: `apps/desktop/src/components/Today/HeaderCard.tsx`
- Create: `apps/desktop/src/components/Today/EventsCard.tsx`

- [ ] **Step 1: HeaderCard**

Create `apps/desktop/src/components/Today/HeaderCard.tsx`:

```tsx
import { useEffect, useState } from "react";

const cardStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
};

const FORMATTER = new Intl.DateTimeFormat(undefined, {
  weekday: "long",
  day: "numeric",
  month: "long",
});

function tzAbbrev(): string {
  // Best-effort short timezone label, falls back to GMT offset.
  const parts = new Intl.DateTimeFormat(undefined, { timeZoneName: "short" })
    .formatToParts(new Date());
  return parts.find((p) => p.type === "timeZoneName")?.value ?? "";
}

export default function HeaderCard() {
  const [now, setNow] = useState(new Date());

  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(id);
  }, []);

  const dateLabel = FORMATTER.format(now);
  const time = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
  const tz = tzAbbrev();

  return (
    <div style={cardStyle}>
      <div>
        <h1 style={{ margin: 0, fontSize: 22, fontWeight: 700 }}>Today</h1>
        <div style={{ fontSize: 13, color: "rgba(0,0,0,0.55)" }}>{dateLabel}</div>
      </div>
      <div
        style={{
          fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
          fontSize: 12,
          color: "rgba(0,0,0,0.55)",
        }}
        aria-label="current local time"
      >
        {time} {tz}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: EventsCard (3a empty state)**

Create `apps/desktop/src/components/Today/EventsCard.tsx`:

```tsx
const cardStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
};

const sectionHeader: React.CSSProperties = {
  fontSize: 11,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)",
  fontWeight: 700,
  margin: 0,
  marginBottom: 8,
};

export default function EventsCard() {
  return (
    <div style={cardStyle}>
      <p style={sectionHeader}>Events</p>
      <p style={{ fontStyle: "italic", color: "rgba(0,0,0,0.5)", margin: 0, fontSize: 13 }}>
        No calendar connected. Coming next phase.
      </p>
    </div>
  );
}
```

- [ ] **Step 3: Verify typecheck**

Run: `pnpm tsc`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Today/HeaderCard.tsx apps/desktop/src/components/Today/EventsCard.tsx
git commit -m "feat(today): HeaderCard with live clock + EventsCard empty state"
```

---

### Task 9: `TaskRow` component

**Files:**
- Create: `apps/desktop/src/components/Today/TaskRow.tsx`

- [ ] **Step 1: Write the component**

Create `apps/desktop/src/components/Today/TaskRow.tsx`:

```tsx
import { useEffect, useRef, useState } from "react";
import type { Task } from "../../lib/today/ipc";
import {
  completeTask,
  undoCompleteTask,
  updateTask,
  deleteTask,
} from "../../lib/today/ipc";
import { useTodayStore } from "../../lib/today/state";

interface TaskRowProps {
  task: Task;
}

const COMPLETE_FADE_MS = 4000;
const DELETE_CONFIRM_MS = 1000;

export default function TaskRow({ task }: TaskRowProps) {
  const removeTask = useTodayStore((s) => s.removeTask);
  const upsertTask = useTodayStore((s) => s.upsertTask);

  const [hovering, setHovering] = useState(false);
  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState(task.title);
  const [completing, setCompleting] = useState(false);
  const [deleteArmed, setDeleteArmed] = useState(false);

  const completeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (completeTimerRef.current) clearTimeout(completeTimerRef.current);
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
    };
  }, []);

  const startComplete = () => {
    if (completing) {
      // Click while in completing state = undo
      void undoCompleteTask(task.id);
      setCompleting(false);
      if (completeTimerRef.current) {
        clearTimeout(completeTimerRef.current);
        completeTimerRef.current = null;
      }
      return;
    }
    setCompleting(true);
    void completeTask(task.id);
    completeTimerRef.current = setTimeout(() => {
      removeTask(task.id);
    }, COMPLETE_FADE_MS);
  };

  const startEdit = () => {
    setEditValue(task.title);
    setEditing(true);
  };

  const commitEdit = () => {
    const trimmed = editValue.trim();
    if (trimmed.length === 0 || trimmed === task.title) {
      setEditing(false);
      return;
    }
    void updateTask(task.id, trimmed);
    upsertTask({ ...task, title: trimmed });
    setEditing(false);
  };

  const armOrConfirmDelete = () => {
    if (deleteArmed) {
      void deleteTask(task.id);
      removeTask(task.id);
      return;
    }
    setDeleteArmed(true);
    deleteTimerRef.current = setTimeout(() => setDeleteArmed(false), DELETE_CONFIRM_MS);
  };

  return (
    <div
      onMouseEnter={() => setHovering(true)}
      onMouseLeave={() => setHovering(false)}
      style={{
        display: "flex",
        gap: 10,
        padding: "6px 4px",
        alignItems: "center",
        background: deleteArmed ? "rgba(255, 59, 48, 0.08)" : "transparent",
        borderRadius: 6,
        transition: "background 100ms ease",
      }}
    >
      <button
        onClick={startComplete}
        aria-label={completing ? "undo complete" : "complete"}
        style={{
          width: 18,
          height: 18,
          padding: 0,
          border: completing ? "1.5px solid var(--imessage-green)" : "1.5px solid rgba(0,0,0,0.35)",
          background: completing ? "var(--imessage-green)" : "transparent",
          color: "white",
          borderRadius: 4,
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
          fontSize: 12,
          fontWeight: 700,
          lineHeight: 1,
        }}
      >
        {completing ? "✓" : ""}
      </button>

      {editing ? (
        <input
          type="text"
          value={editValue}
          autoFocus
          onChange={(e) => setEditValue(e.target.value)}
          onBlur={commitEdit}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              commitEdit();
            } else if (e.key === "Escape") {
              setEditing(false);
            }
          }}
          style={{
            flex: 1,
            padding: "2px 6px",
            border: "1px solid var(--hairline)",
            borderRadius: 4,
            fontSize: 14,
            fontFamily: "inherit",
          }}
        />
      ) : (
        <span
          onClick={startEdit}
          style={{
            flex: 1,
            fontSize: 14,
            lineHeight: 1.4,
            cursor: "text",
            textDecoration: completing ? "line-through" : "none",
            opacity: completing ? 0.5 : 1,
            transition: "opacity 200ms ease",
          }}
        >
          {task.title}
        </span>
      )}

      {hovering && !editing && !completing && (
        <div style={{ display: "flex", gap: 4, opacity: 0.6, transition: "opacity 100ms ease" }}>
          <button
            onClick={startEdit}
            aria-label="edit"
            style={{
              width: 24,
              height: 24,
              padding: 0,
              background: "transparent",
              border: "none",
              cursor: "pointer",
              fontSize: 14,
            }}
          >
            ✎
          </button>
          <button
            onClick={armOrConfirmDelete}
            aria-label={deleteArmed ? "confirm delete" : "delete"}
            style={{
              width: 24,
              height: 24,
              padding: 0,
              background: "transparent",
              border: "none",
              cursor: "pointer",
              fontSize: 14,
              color: deleteArmed ? "var(--imessage-red)" : "inherit",
            }}
          >
            🗑
          </button>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm tsc`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/components/Today/TaskRow.tsx
git commit -m "feat(today): TaskRow with hover edit/delete + 4s undo on complete"
```

---

### Task 10: `TasksCard` component

**Files:**
- Create: `apps/desktop/src/components/Today/TasksCard.tsx`

- [ ] **Step 1: Write the component**

Create `apps/desktop/src/components/Today/TasksCard.tsx`:

```tsx
import { useState, useRef, useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";
import { addTask } from "../../lib/today/ipc";
import TaskRow from "./TaskRow";

const cardStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
};

const sectionHeader: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  margin: 0,
  marginBottom: 8,
  fontSize: 11,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)",
  fontWeight: 700,
};

const addLink: React.CSSProperties = {
  background: "transparent",
  border: "none",
  color: "var(--imessage-blue)",
  fontWeight: 700,
  fontSize: 12,
  cursor: "pointer",
  padding: 0,
  letterSpacing: 0,
  textTransform: "none",
};

export default function TasksCard() {
  const tasks = useTodayStore((s) => s.tasks);
  const upsertTask = useTodayStore((s) => s.upsertTask);

  const [adding, setAdding] = useState(false);
  const [addValue, setAddValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (adding) inputRef.current?.focus();
  }, [adding]);

  const commitAdd = async () => {
    const trimmed = addValue.trim();
    if (trimmed.length === 0) {
      setAdding(false);
      return;
    }
    const task = await addTask(trimmed);
    upsertTask(task);
    setAddValue("");
    // Stay in adding mode for follow-up adds
  };

  const headerCount = tasks.length;
  const headerLabel = headerCount > 0 ? `Tasks · ${headerCount} open` : `Tasks`;

  return (
    <div style={cardStyle}>
      <div style={sectionHeader}>
        <span>{headerLabel}</span>
        <button onClick={() => setAdding(true)} style={addLink}>
          + Add
        </button>
      </div>

      {tasks.length === 0 && !adding && (
        <p style={{ color: "rgba(0,0,0,0.5)", margin: 0, fontSize: 13 }}>
          Nothing on your plate. Type <code>/task</code> or click + Add to add one.
        </p>
      )}

      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        {tasks.map((t) => (
          <TaskRow key={t.id} task={t} />
        ))}
      </div>

      {adding && (
        <input
          ref={inputRef}
          type="text"
          value={addValue}
          placeholder="New task title"
          onChange={(e) => setAddValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void commitAdd();
            } else if (e.key === "Escape") {
              setAddValue("");
              setAdding(false);
            }
          }}
          onBlur={() => {
            if (addValue.trim().length === 0) {
              setAdding(false);
            } else {
              void commitAdd();
            }
          }}
          style={{
            marginTop: 8,
            width: "100%",
            padding: "6px 10px",
            border: "1px dashed rgba(0,0,0,0.2)",
            borderRadius: 6,
            fontSize: 14,
            fontFamily: "inherit",
            outline: "none",
            background: "rgba(0,0,0,0.02)",
          }}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm tsc`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/components/Today/TasksCard.tsx
git commit -m "feat(today): TasksCard with + Add inline reveal + empty state"
```

---

### Task 11: `ProposalBanner` component

**Files:**
- Create: `apps/desktop/src/components/Today/ProposalBanner.tsx`
- Modify: `apps/desktop/src/styles.css` (add `bannerIn` keyframe)

- [ ] **Step 1: Add the bannerIn keyframe to styles.css**

Append to `apps/desktop/src/styles.css`:

```css
@keyframes bannerIn {
  from {
    opacity: 0;
    transform: translateY(-8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
```

- [ ] **Step 2: Create the component**

Create `apps/desktop/src/components/Today/ProposalBanner.tsx`:

```tsx
import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";
import {
  approveProposal,
  rejectProposal,
  listProposals,
  type Proposal,
} from "../../lib/today/ipc";

interface DiffSummary {
  title: string;
  due_date?: string;
}

function summarise(proposal: Proposal): string {
  if (proposal.kind === "add_task") {
    try {
      const parsed = JSON.parse(proposal.diff) as DiffSummary;
      const dateSuffix = parsed.due_date ? ` (due ${parsed.due_date})` : "";
      return `Add task: ${parsed.title}${dateSuffix}`;
    } catch {
      return "Add task";
    }
  }
  return proposal.kind;
}

function rationaleLine(proposal: Proposal): string {
  const r = proposal.rationale.trim();
  if (r.length === 0) return "Manor proposed this from your message";
  const truncated = r.length > 120 ? `${r.slice(0, 117)}...` : r;
  return `Manor: "${truncated}"`;
}

export default function ProposalBanner() {
  const pending = useTodayStore((s) => s.pendingProposals);
  const setPendingProposals = useTodayStore((s) => s.setPendingProposals);
  const setTasks = useTodayStore((s) => s.setTasks);
  const removeProposal = useTodayStore((s) => s.removeProposal);

  // Hydrate pending proposals on first mount.
  useEffect(() => {
    void listProposals("pending").then(setPendingProposals);
  }, [setPendingProposals]);

  if (pending.length === 0) return null;

  const handleApprove = async (id: number) => {
    removeProposal(id); // optimistic
    try {
      const refreshedTasks = await approveProposal(id);
      setTasks(refreshedTasks);
    } catch (e) {
      // On failure, re-fetch pending so the banner reappears.
      void listProposals("pending").then(setPendingProposals);
    }
  };

  const handleReject = async (id: number) => {
    removeProposal(id); // optimistic
    try {
      await rejectProposal(id);
    } catch {
      void listProposals("pending").then(setPendingProposals);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      {pending.map((p) => (
        <div
          key={p.id}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            background: "linear-gradient(180deg, #FFF3D6, #FFE4A8)",
            border: "1px solid #FFC15C",
            borderRadius: "var(--radius-md)",
            padding: "10px 14px",
            animation: "bannerIn 200ms ease-out",
          }}
        >
          <span
            style={{
              background: "#FF8800",
              color: "white",
              fontSize: 10,
              fontWeight: 700,
              padding: "2px 8px",
              borderRadius: 999,
              letterSpacing: 0.6,
              flexShrink: 0,
            }}
          >
            PROPOSAL
          </span>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontWeight: 700, fontSize: 13, color: "#3a2818" }}>
              {summarise(p)}
            </div>
            <div
              style={{
                fontSize: 11,
                color: "rgba(58, 40, 24, 0.7)",
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
              }}
            >
              {rationaleLine(p)}
            </div>
          </div>
          <button
            onClick={() => void handleApprove(p.id)}
            aria-label="approve"
            style={{
              padding: "4px 10px",
              borderRadius: 999,
              fontSize: 12,
              fontWeight: 700,
              border: "none",
              background: "var(--imessage-green)",
              color: "white",
              cursor: "pointer",
            }}
          >
            ✓
          </button>
          <button
            onClick={() => void handleReject(p.id)}
            aria-label="reject"
            style={{
              padding: "4px 10px",
              borderRadius: 999,
              fontSize: 12,
              fontWeight: 700,
              border: "none",
              background: "rgba(255,255,255,0.7)",
              color: "rgba(0,0,0,0.6)",
              cursor: "pointer",
            }}
          >
            ✗
          </button>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Verify typecheck**

Run: `pnpm tsc`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Today/ProposalBanner.tsx apps/desktop/src/styles.css
git commit -m "feat(today): ProposalBanner — amber stacked banners with optimistic approve/reject"
```

---

### Task 12: `Today.tsx` composer + mount in `App.tsx`

**Files:**
- Create: `apps/desktop/src/components/Today/Today.tsx`
- Modify: `apps/desktop/src/App.tsx`

- [ ] **Step 1: Today composer**

Create `apps/desktop/src/components/Today/Today.tsx`:

```tsx
import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";
import { listTasks } from "../../lib/today/ipc";
import { AVATAR_FOOTPRINT_PX } from "../../lib/layout";
import HeaderCard from "./HeaderCard";
import EventsCard from "./EventsCard";
import TasksCard from "./TasksCard";
import ProposalBanner from "./ProposalBanner";

export default function Today() {
  const setTasks = useTodayStore((s) => s.setTasks);

  // Hydrate tasks on mount.
  useEffect(() => {
    void listTasks().then(setTasks);
  }, [setTasks]);

  return (
    <main
      style={{
        maxWidth: 760,
        margin: "0 auto",
        padding: `24px 24px ${AVATAR_FOOTPRINT_PX}px 24px`,
        display: "flex",
        flexDirection: "column",
        gap: 12,
      }}
    >
      <ProposalBanner />
      <HeaderCard />
      <EventsCard />
      <TasksCard />
    </main>
  );
}
```

- [ ] **Step 2: Modify App.tsx to mount Today**

Replace `apps/desktop/src/App.tsx` with:

```tsx
import Assistant from "./components/Assistant/Assistant";
import Today from "./components/Today/Today";

export default function App() {
  return (
    <>
      <Today />
      <Assistant />
    </>
  );
}
```

- [ ] **Step 3: Verify build + tests**

Run: `pnpm tsc`
Run: `pnpm --filter manor-desktop test`
Run: `cargo check -p manor-desktop`
All clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Today/Today.tsx apps/desktop/src/App.tsx
git commit -m "feat(today): Today composer + mounted in App"
```

---

### Task 13: Wire slash detection into `Assistant.tsx` + `ConversationDrawer` submit

**Files:**
- Modify: `apps/desktop/src/components/Assistant/Assistant.tsx`
- Modify: `apps/desktop/src/components/Assistant/ConversationDrawer.tsx`

- [ ] **Step 1: Read current Assistant.tsx handleSubmit**

Run: `grep -n "handleSubmit" apps/desktop/src/components/Assistant/Assistant.tsx`
Note the function's signature and the position of the `setAvatarState("listening")` call (the entry point).

- [ ] **Step 2: Add slash detection to Assistant.tsx**

Edit `apps/desktop/src/components/Assistant/Assistant.tsx`. At the top of the file, add the imports:

```ts
import { parseSlash } from "../../lib/today/slash";
import { addTask, listTasks, listProposals } from "../../lib/today/ipc";
import { useTodayStore } from "../../lib/today/state";
```

Inside the `Assistant` function, add a hook after the existing `useAssistantStore` selectors:

```ts
const setTasks = useTodayStore((s) => s.setTasks);
const setPendingProposals = useTodayStore((s) => s.setPendingProposals);
```

At the top of `handleSubmit` (before the existing `setAvatarState("listening")` call), insert:

```ts
const slash = parseSlash(content);
if (slash?.type === "task") {
  try {
    const task = await addTask(slash.title);
    useTodayStore.getState().upsertTask(task);
    // Lightweight toast — re-uses the bubble layer pattern but for a 2s system message.
    // Defer a real toast component to Task 14.
    return;
  } catch (e) {
    setAvatarState("confused");
    enqueueBubble({
      id: newBubbleId(),
      kind: "error",
      content: `Couldn't add task: ${String(e)}`,
      messageId: null,
      ttlMs: 7000,
    });
    return;
  }
}
// 'unknown' slashes fall through to send_message as a normal chat turn.
```

Inside the existing `onEvent` handler, find the `Done` branch and add a refresh after the unread count update:

```ts
} else if (chunk.type === "Proposal") {
  void listProposals("pending").then(setPendingProposals);
  // also refresh tasks in case the proposal was approved server-side somehow
  // (it won't be in 3a, but cheap to be defensive)
  void listTasks().then(setTasks);
}
```

The full updated `onEvent` switch should now handle `Started`, `Token`, `Proposal`, `Done`, and `Error`.

- [ ] **Step 3: Add slash detection to ConversationDrawer.tsx submit**

Edit `apps/desktop/src/components/Assistant/ConversationDrawer.tsx`. Add the import at the top:

```ts
import { parseSlash } from "../../lib/today/slash";
import { addTask } from "../../lib/today/ipc";
import { useTodayStore } from "../../lib/today/state";
```

In the existing `submit` function, before calling `onSubmit(t)`:

```ts
const slash = parseSlash(t);
if (slash?.type === "task") {
  void addTask(slash.title).then((task) => {
    useTodayStore.getState().upsertTask(task);
  });
  setValue("");
  return;
}
```

Then `onSubmit(t)` continues for non-task or unknown slashes.

- [ ] **Step 4: Verify**

Run: `pnpm tsc`
Expected: clean.

Run: `pnpm --filter manor-desktop test`
Expected: 20 tests still pass (no test changes here).

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/Assistant/Assistant.tsx apps/desktop/src/components/Assistant/ConversationDrawer.tsx
git commit -m "feat(assistant): slash detection in pill + drawer submit; Proposal chunk refresh"
```

---

### Task 14: Toast component + polish

**Files:**
- Create: `apps/desktop/src/components/Today/Toast.tsx`
- Modify: `apps/desktop/src/components/Today/Today.tsx` (mount the toast container)
- Modify: `apps/desktop/src/lib/today/state.ts` (add `toast` slice)
- Modify: `apps/desktop/src/components/Assistant/Assistant.tsx` (use toast for slash success)
- Modify: `apps/desktop/src/components/Assistant/ConversationDrawer.tsx` (same)

- [ ] **Step 1: Extend the today store with a toast slice**

Edit `apps/desktop/src/lib/today/state.ts`. Add to the `TodayStore` interface:

```ts
toast: { message: string; expiresAt: number } | null;
showToast: (message: string) => void;
clearToast: () => void;
```

And to the implementation block:

```ts
toast: null,
showToast: (message) =>
  set({ toast: { message, expiresAt: Date.now() + 2000 } }),
clearToast: () => set({ toast: null }),
```

- [ ] **Step 2: Toast component**

Create `apps/desktop/src/components/Today/Toast.tsx`:

```tsx
import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";

export default function Toast() {
  const toast = useTodayStore((s) => s.toast);
  const clearToast = useTodayStore((s) => s.clearToast);

  useEffect(() => {
    if (!toast) return;
    const ms = Math.max(0, toast.expiresAt - Date.now());
    const id = setTimeout(() => clearToast(), ms);
    return () => clearTimeout(id);
  }, [toast, clearToast]);

  if (!toast) return null;

  return (
    <div
      role="status"
      style={{
        position: "fixed",
        bottom: 24,
        left: "50%",
        transform: "translateX(-50%)",
        background: "rgba(20, 20, 30, 0.85)",
        color: "white",
        padding: "8px 16px",
        borderRadius: "var(--radius-pill)",
        fontSize: 13,
        fontWeight: 600,
        boxShadow: "var(--shadow-md)",
        zIndex: 1100,
        animation: "bannerIn 200ms ease-out",
      }}
    >
      {toast.message}
    </div>
  );
}
```

- [ ] **Step 3: Mount the Toast in Today**

Edit `apps/desktop/src/components/Today/Today.tsx` — add the import and the mount inside the JSX (alongside the other components). Update file to:

```tsx
import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";
import { listTasks } from "../../lib/today/ipc";
import { AVATAR_FOOTPRINT_PX } from "../../lib/layout";
import HeaderCard from "./HeaderCard";
import EventsCard from "./EventsCard";
import TasksCard from "./TasksCard";
import ProposalBanner from "./ProposalBanner";
import Toast from "./Toast";

export default function Today() {
  const setTasks = useTodayStore((s) => s.setTasks);

  useEffect(() => {
    void listTasks().then(setTasks);
  }, [setTasks]);

  return (
    <>
      <main
        style={{
          maxWidth: 760,
          margin: "0 auto",
          padding: `24px 24px ${AVATAR_FOOTPRINT_PX}px 24px`,
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}
      >
        <ProposalBanner />
        <HeaderCard />
        <EventsCard />
        <TasksCard />
      </main>
      <Toast />
    </>
  );
}
```

- [ ] **Step 4: Wire the toast call into both slash entry points**

Edit `apps/desktop/src/components/Assistant/Assistant.tsx`. In the slash-task success path, replace the `return;` with:

```ts
useTodayStore.getState().upsertTask(task);
useTodayStore.getState().showToast(`Added: ${slash.title}`);
return;
```

Edit `apps/desktop/src/components/Assistant/ConversationDrawer.tsx`. In the slash-task path, after the upsert:

```ts
useTodayStore.getState().showToast(`Added: ${slash.title}`);
```

- [ ] **Step 5: Verify + commit**

Run: `pnpm tsc`
Run: `pnpm --filter manor-desktop test`
Both clean / passing.

```bash
git add apps/desktop/src/components/Today/Toast.tsx apps/desktop/src/components/Today/Today.tsx apps/desktop/src/lib/today/state.ts apps/desktop/src/components/Assistant/Assistant.tsx apps/desktop/src/components/Assistant/ConversationDrawer.tsx
git commit -m "feat(today): Toast component + wire showToast into slash success paths"
```

---

### Task 15: Manual smoke + tag + PR

**Files:** none (verification + git operations)

- [ ] **Step 1: Full verification suite**

Run: `cargo fmt --all --check`
Expected: clean (or run `cargo fmt --all` to fix).

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo test --workspace --all-targets`
Run: `pnpm tsc`
Run: `pnpm --filter manor-desktop test`

All should pass: workspace tests count is now ~31 (Phase 2's 17 + Phase 3a's 14: 7 task + 7 proposal/wiremock); frontend vitest is 20 (Phase 2's 8 + Phase 3a's 12: 5 slash + 7 today-store).

- [ ] **Step 2: Manual end-to-end smoke**

Ensure Ollama is running with `qwen2.5:7b-instruct`:

```bash
pgrep -f "ollama serve" >/dev/null || ollama serve &
ollama list | grep qwen2.5:7b-instruct || ollama pull qwen2.5:7b-instruct
```

From the worktree root: `./scripts/dev.sh`

Walk the 12-step list from spec §10.4. Document any failures here as bullets and fix before tagging:

- [ ] Today view shows date + empty Events + empty Tasks
- [ ] "+ Add" reveals input → typing + Enter creates task → row appears
- [ ] `/task <title>` in pill creates task + shows toast
- [ ] Checkbox click → strikethrough + dim → row vanishes after 4s; click again within 4s undoes
- [ ] Hover on row → pencil + trash icons fade in
- [ ] Pencil → inline rename → Enter saves
- [ ] Trash → row red briefly → second click removes
- [ ] "Remind me to book a dentist" in pill → Manor streams a reply, then PROPOSAL banner appears
- [ ] Approve → banner clears, task appears in Tasks card
- [ ] Reject → banner clears, no task added
- [ ] Quit + relaunch → pending proposals reappear; tasks persist

- [ ] **Step 3: Push branch and open PR**

```bash
git push -u origin feature/phase-3a-today-tasks
gh pr create --base main --head feature/phase-3a-today-tasks --title "Phase 3a — Today view + manual tasks + first proposal flow" --body "$(cat <<'EOF'
## Summary

Manor's main window stops being a placeholder. The Today view becomes the first real surface — stacked cards (Header, Events, Tasks). Hana can add/edit/complete/delete tasks directly. Manor can propose task additions through Ollama tool calling; proposals appear as a calm amber banner for review.

- **Schema:** \`task\` table per the 3c spec contract (\`due_date\` as 'YYYY-MM-DD' TEXT, \`completed_at\` unix seconds NULL = open, \`proposal_id\` traceability)
- **Backend:** Ollama \`tools\` array in chat request, \`tool_calls\` parsing, new \`StreamChunk::Proposal\` variant; \`approve_add_task\` transactional apply; 9 new Tauri commands
- **Frontend:** new \`<Today>\` tree (HeaderCard / EventsCard / TasksCard / TaskRow / ProposalBanner / Toast), \`useTodayStore\` zustand slice, slash command \`/task <title>\`
- **First proposal flow E2E:** Hana asks Manor to remember something → Manor calls \`add_task\` tool → backend creates pending proposal → banner appears → approve creates task

## Test plan

- [x] \`cargo test --workspace --all-targets\` green (~31 tests)
- [x] \`cargo fmt --check\` + \`cargo clippy --all-targets -- -D warnings\` clean
- [x] \`pnpm tsc\` clean
- [x] \`pnpm --filter manor-desktop test\` green (~20 vitest tests)
- [x] Manual smoke (12-step list per spec §10.4)
- [ ] CI green on this PR

EOF
)"
```

- [ ] **Step 4: Wait for CI green, then merge**

```bash
gh pr checks <pr-number>  # repeat until both pass
gh pr merge <pr-number> --merge --delete-branch
```

- [ ] **Step 5: Sync local main, prune, tag**

```bash
git checkout main
git pull origin main
git fetch --prune origin
git push origin --delete feature/phase-3a-today-tasks 2>&1 || true  # may already be deleted by gh
git tag -a phase-3a-today-tasks-complete -m "Phase 3a Today view + manual tasks + first proposal flow"
git push origin phase-3a-today-tasks-complete
```

- [ ] **Step 6: Cleanup worktree**

From the parent repo (`/Users/hanamori/life-assistant`):

```bash
git worktree remove .worktrees/phase-3a-today-tasks
git worktree list
```

---

## Self-Review Checklist

**1. Spec coverage (3a spec § → task):**
- §3.1 Rust modules → Tasks 1, 2, 3, 4, 5, 6
- §3.2 Frontend file structure → Tasks 7, 8, 9, 10, 11, 12, 14
- §3.3 IPC contract → Task 6 (commands), Task 7 (typed wrappers)
- §3.4 Separate stores → Task 7 (`useTodayStore` lives apart from `useAssistantStore`)
- §4 Today layout → Task 8 (Header/Events) + Task 10 (Tasks) + Task 12 (composer)
- §5 Task interactions → Task 9 (TaskRow with all states)
- §6 Slash commands → Task 7 (parser + tests) + Task 13 (wiring) + Task 14 (toast)
- §7 Tool-use flow → Task 3 (schema) + Task 4 (Ollama plumbing) + Task 5 (prompt) + Task 6 (commands wiring)
- §8 Proposal lifecycle + banner → Task 2 (apply/reject) + Task 6 (IPC) + Task 11 (banner)
- §9 Data model → Task 1 (migration + Task type + CRUD)
- §10 Tests → Tasks 1, 2, 4, 7 each include their TDD tests
- §11 Completion criteria → Task 15

**2. Placeholder scan:** None. Every code block is complete runnable content.

**3. Type consistency:**
- `Task` struct (Rust) vs `Task` interface (TS) — fields match: `id`, `title`, `due_date`, `completed_at`, `created_at`, `proposal_id`. Numeric vs Optional handled.
- `Proposal` struct (Rust) vs `Proposal` interface (TS) — fields match.
- `StreamChunk` enum (Rust): `Started(i64)`, `Token(String)`, `Proposal(i64)`, `Done`, `Error(ErrorCode)`. TS union: `{ type: "Started"; value: number } | { type: "Token"; value: string } | { type: "Proposal"; value: number } | { type: "Done" } | { type: "Error"; value: ... }`. Match.
- `parseSlash` return type — same shape in Rust spec (not implemented because client-only) and TS.
- `AddTaskArgs` (Rust) — `{ title: String, due_date: Option<String> }`. Tool schema declares matching fields.
- `add_task` IPC: TS calls with `{ title, dueDate }` (camelCase via Tauri auto-convert) → Rust receives `due_date` (snake_case). Matches Phase 2's `markSeen` / `messageIds` precedent.

---

## Plan deviations (to be appended by implementers, as per Phase 1/2 convention)

*(None yet. Implementers should append each deviation with reasoning.)*
