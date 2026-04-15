# Phase 2 — Assistant Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Manor's always-present Assistant — a cornered, expressive avatar who listens, thinks, and streams replies in ephemeral iMessage-style bubbles via local Ollama, backed by SQLite persistence and a `proposal` table scaffolded for future features.

**Architecture:** Three Rust crates each gain an `assistant` module — `manor-core` owns the SQLite schema + data access (`rusqlite` + `refinery` migrations); `manor-app` owns the Ollama HTTP client (`reqwest` streaming NDJSON) and Tauri commands (streaming via Tauri 2 `Channel<T>`); `manor-desktop` initialises the DB in its `setup()` closure. The React shell gains an `Assistant` component tree — avatar, bubble layer, pill input, unread badge, conversation drawer — driven by a zustand store. Pablo Stanley aesthetic (Open Peeps character, Nunito typography, soft shadows, iMessage bubble colours).

**Tech Stack:** Rust (rusqlite, refinery, reqwest, tokio, serde, wiremock for tests); React 18 + TypeScript + Vite 5; zustand; @fontsource/nunito; Tauri 2.x; Ollama (`qwen2.5:7b-instruct` local); vitest + jsdom for frontend state tests.

**Worktree setup:** Before starting Task 1, create a worktree at `/Users/hanamori/life-assistant/.worktrees/phase-2-assistant` on a new branch `feature/phase-2-assistant` branched from `main`. Execute the plan inside that worktree.

---

## Task breakdown (15 tasks)

1. Add workspace dependencies (rusqlite, refinery, reqwest, tokio streams, dev-deps)
2. Write `V1__initial.sql` migration + DB init module (TDD)
3. `Conversation` + `Message` data-access functions (TDD)
4. `Proposal` type + insert stub (TDD minimal)
5. Ollama HTTP streaming client (TDD with `wiremock`)
6. Tauri commands + `Channel<StreamChunk>` wiring + DB init via `setup()` + register
7. Copy avatar assets to frontend + add frontend deps (zustand, nunito, vitest)
8. Design tokens in `styles.css` + Nunito font import
9. Zustand store + expression map + typed IPC wrappers (TDD state tests)
10. `Avatar` + `UnreadBadge` components
11. `BubbleLayer` component (ephemeral TTL, hover pause, click-to-mark-seen) (TDD TTL logic)
12. `InputPill` component
13. `ConversationDrawer` component
14. `Assistant.tsx` composes everything + ⌘/ hotkey + mount in `App.tsx`
15. Extend `install-ollama.sh` (pull default model) + end-to-end smoke test + tag

---

### Task 1: Add workspace dependencies

**Files:**
- Modify: `Cargo.toml` (root) — `[workspace.dependencies]`
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/app/Cargo.toml`

- [ ] **Step 1: Extend root `[workspace.dependencies]`**

Open `Cargo.toml` at the repo root and extend the `[workspace.dependencies]` table with the following new entries, keeping the existing ones intact:

```toml
rusqlite = { version = "0.31", features = ["bundled"] }
refinery = { version = "0.8", features = ["rusqlite"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "stream", "rustls-tls"] }
futures-util = "0.3"
chrono = { version = "0.4", default-features = false, features = ["clock", "serde"] }

# dev-dependencies
wiremock = "0.6"
tempfile = "3"
tokio-test = "0.4"
```

- [ ] **Step 2: Wire deps into `manor-core`**

Edit `crates/core/Cargo.toml`. The `[dependencies]` section should look like this (add the new `rusqlite`, `refinery`, `chrono` lines; keep the existing ones):

```toml
[dependencies]
anyhow.workspace = true
thiserror.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
rusqlite.workspace = true
refinery.workspace = true
chrono.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 3: Wire deps into `manor-app`**

Edit `crates/app/Cargo.toml`. Replace the `[dependencies]` section with:

```toml
[dependencies]
serde.workspace = true
serde_json.workspace = true
tauri = "2"
manor-core.workspace = true
reqwest.workspace = true
tokio.workspace = true
futures-util.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true

[dev-dependencies]
wiremock.workspace = true
tokio-test.workspace = true
tempfile.workspace = true
```

- [ ] **Step 4: Confirm the workspace still builds**

Run: `cargo check --workspace`
Expected: clean build across all three crates. First run pulls and compiles the new dependency tree (2–5 minutes on macOS).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/core/Cargo.toml crates/app/Cargo.toml
git commit -m "chore(deps): add rusqlite, refinery, reqwest for phase 2"
```

---

### Task 2: `V1__initial.sql` migration + DB init module (TDD)

**Files:**
- Create: `crates/core/migrations/V1__initial.sql`
- Create: `crates/core/src/assistant/mod.rs`
- Create: `crates/core/src/assistant/db.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod assistant;`)
- Test: `crates/core/src/assistant/db.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the SQL migration**

Create `crates/core/migrations/V1__initial.sql`:

```sql
CREATE TABLE conversation (
  id         INTEGER PRIMARY KEY,
  created_at INTEGER NOT NULL,
  title      TEXT    NOT NULL DEFAULT 'Manor'
);

CREATE TABLE proposal (
  id                  INTEGER PRIMARY KEY,
  kind                TEXT    NOT NULL,
  rationale           TEXT    NOT NULL,
  diff                TEXT    NOT NULL,
  status              TEXT    NOT NULL DEFAULT 'pending',
  proposed_at         INTEGER NOT NULL,
  applied_at          INTEGER NULL,
  skill               TEXT    NOT NULL,
  remote_call_log_id  INTEGER NULL
);

CREATE TABLE message (
  id              INTEGER PRIMARY KEY,
  conversation_id INTEGER NOT NULL REFERENCES conversation(id),
  role            TEXT    NOT NULL,
  content         TEXT    NOT NULL,
  created_at      INTEGER NOT NULL,
  seen            INTEGER NOT NULL DEFAULT 0,
  proposal_id     INTEGER NULL REFERENCES proposal(id)
);

CREATE INDEX idx_message_conversation_created ON message (conversation_id, created_at);
```

- [ ] **Step 2: Create the module root**

Create `crates/core/src/assistant/mod.rs`:

```rust
//! Assistant substrate: SQLite persistence for conversations, messages, and proposals.

pub mod db;
```

- [ ] **Step 3: Export the module**

Edit `crates/core/src/lib.rs`. Add `pub mod assistant;` after the existing `version()` and tests block. The full file should end with:

```rust
//! Manor core library.

pub mod assistant;

/// Returns the crate version string, used by the shell for the About screen.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(
            !version().is_empty(),
            "version should return a non-empty string"
        );
    }

    #[test]
    fn version_matches_cargo_pkg() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
```

- [ ] **Step 4: Write the failing test for `init()`**

Create `crates/core/src/assistant/db.rs` with the test first (the function it tests does not yet exist — this is TDD):

```rust
//! Database connection pool + refinery migration runner.

use anyhow::Result;
use refinery::embed_migrations;
use rusqlite::Connection;
use std::path::Path;

embed_migrations!("migrations");

/// Initialise a SQLite connection at the given path and run all pending migrations.
///
/// Returns an open connection ready for data-access functions to use.
pub fn init(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    migrations::runner().run(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_creates_db_file_and_runs_migrations() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");

        let conn = init(&path).expect("init should succeed");

        // Migrations ran — the three tables should exist.
        for table in ["conversation", "message", "proposal"] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "table {table} should exist after migrations");
        }

        assert!(path.exists(), "db file should exist on disk");
    }

    #[test]
    fn init_is_idempotent_on_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        {
            let _c = init(&path).unwrap();
        }
        let _c = init(&path).expect("second init should succeed");
    }
}
```

- [ ] **Step 5: Run tests to confirm they pass**

Run: `cargo test -p manor-core --all-targets`
Expected: 4 tests pass (2 existing + 2 new). If the new tests fail, inspect the compiler error; the most common pitfall is `embed_migrations!` pointing at the wrong path — it resolves relative to the crate root, so `"migrations"` resolves to `crates/core/migrations/` (correct).

- [ ] **Step 6: Commit**

```bash
git add crates/core/migrations crates/core/src/assistant crates/core/src/lib.rs
git commit -m "feat(core): add assistant db module with V1 migration"
```

---

### Task 3: `Conversation` + `Message` data-access functions (TDD)

**Files:**
- Create: `crates/core/src/assistant/conversation.rs`
- Create: `crates/core/src/assistant/message.rs`
- Modify: `crates/core/src/assistant/mod.rs`

- [ ] **Step 1: Expose the new submodules**

Edit `crates/core/src/assistant/mod.rs`:

```rust
//! Assistant substrate: SQLite persistence for conversations, messages, and proposals.

pub mod conversation;
pub mod db;
pub mod message;
```

- [ ] **Step 2: Write `conversation.rs` with TDD test first**

Create `crates/core/src/assistant/conversation.rs`:

```rust
//! Conversation threads. Phase 2 only ever has id=1 (rolling thread).

use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Conversation {
    pub id: i64,
    pub created_at: i64,
    pub title: String,
}

impl Conversation {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            created_at: row.get("created_at")?,
            title: row.get("title")?,
        })
    }
}

/// Return the default conversation, creating it (id=1) on first call.
pub fn get_or_create_default(conn: &Connection) -> Result<Conversation> {
    if let Some(c) = conn
        .query_row(
            "SELECT id, created_at, title FROM conversation WHERE id = 1",
            [],
            Conversation::from_row,
        )
        .ok()
    {
        return Ok(c);
    }

    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO conversation (id, created_at, title) VALUES (1, ?1, 'Manor')",
        [now],
    )?;

    Ok(Conversation {
        id: 1,
        created_at: now,
        title: "Manor".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    #[test]
    fn first_call_creates_default_conversation() {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();

        let c = get_or_create_default(&conn).unwrap();
        assert_eq!(c.id, 1);
        assert_eq!(c.title, "Manor");
        assert!(c.created_at > 0);
    }

    #[test]
    fn second_call_returns_same_conversation() {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();

        let a = get_or_create_default(&conn).unwrap();
        let b = get_or_create_default(&conn).unwrap();
        assert_eq!(a, b);
    }
}
```

- [ ] **Step 3: Write `message.rs` with TDD**

Create `crates/core/src/assistant/message.rs`:

```rust
//! Messages in the rolling conversation.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        }
    }
}

impl std::str::FromStr for Role {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => Role::System,
            other => anyhow::bail!("unknown role: {other}"),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub id: i64,
    pub conversation_id: i64,
    pub role: Role,
    pub content: String,
    pub created_at: i64,
    pub seen: bool,
    pub proposal_id: Option<i64>,
}

impl Message {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let role_str: String = row.get("role")?;
        Ok(Self {
            id: row.get("id")?,
            conversation_id: row.get("conversation_id")?,
            role: role_str.parse().map_err(|e: anyhow::Error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
                )
            })?,
            content: row.get("content")?,
            created_at: row.get("created_at")?,
            seen: row.get::<_, i64>("seen")? != 0,
            proposal_id: row.get("proposal_id")?,
        })
    }
}

/// Insert a new message. Returns the new row id.
/// User messages are always inserted with `seen=true`; assistant/system messages with `seen=false`.
pub fn insert(
    conn: &Connection,
    conversation_id: i64,
    role: Role,
    content: &str,
) -> Result<i64> {
    let now_ms = Utc::now().timestamp_millis();
    let seen = matches!(role, Role::User) as i64;
    conn.execute(
        "INSERT INTO message (conversation_id, role, content, created_at, seen)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![conversation_id, role.as_str(), content, now_ms, seen],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Append text to the content of an existing message (used while streaming assistant replies).
pub fn append_content(conn: &Connection, id: i64, fragment: &str) -> Result<()> {
    conn.execute(
        "UPDATE message SET content = content || ?1 WHERE id = ?2",
        params![fragment, id],
    )?;
    Ok(())
}

/// List the most recent `limit` messages for a conversation, oldest-first within the window,
/// starting `offset` messages back from newest.
pub fn list(
    conn: &Connection,
    conversation_id: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Message>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, created_at, seen, proposal_id
         FROM message
         WHERE conversation_id = ?1
         ORDER BY created_at DESC
         LIMIT ?2 OFFSET ?3",
    )?;
    let mut rows: Vec<Message> = stmt
        .query_map(params![conversation_id, limit, offset], Message::from_row)?
        .collect::<rusqlite::Result<_>>()?;
    rows.reverse(); // oldest-first within the window
    Ok(rows)
}

/// Mark a batch of messages as seen.
pub fn mark_seen(conn: &Connection, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = std::iter::repeat("?").take(ids.len()).collect::<Vec<_>>().join(",");
    let sql = format!("UPDATE message SET seen = 1 WHERE id IN ({placeholders})");
    let params_owned: Vec<&dyn rusqlite::ToSql> =
        ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
    conn.execute(&sql, params_owned.as_slice())?;
    Ok(())
}

/// Count assistant messages that have not been seen.
pub fn unread_count(conn: &Connection, conversation_id: i64) -> Result<u32> {
    let c: i64 = conn.query_row(
        "SELECT COUNT(*) FROM message
         WHERE conversation_id = ?1 AND role = 'assistant' AND seen = 0",
        [conversation_id],
        |r| r.get(0),
    )?;
    Ok(c as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::{conversation, db};
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection, i64) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let conv = conversation::get_or_create_default(&conn).unwrap();
        (dir, conn, conv.id)
    }

    #[test]
    fn insert_user_message_is_seen() {
        let (_d, conn, cid) = fresh_conn();
        let id = insert(&conn, cid, Role::User, "hello").unwrap();
        let msgs = list(&conn, cid, 10, 0).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].id, id);
        assert_eq!(msgs[0].content, "hello");
        assert!(msgs[0].seen);
        assert_eq!(msgs[0].role, Role::User);
    }

    #[test]
    fn insert_assistant_message_is_unseen() {
        let (_d, conn, cid) = fresh_conn();
        insert(&conn, cid, Role::Assistant, "pong").unwrap();
        assert_eq!(unread_count(&conn, cid).unwrap(), 1);
    }

    #[test]
    fn append_content_grows_the_message() {
        let (_d, conn, cid) = fresh_conn();
        let id = insert(&conn, cid, Role::Assistant, "").unwrap();
        append_content(&conn, id, "hel").unwrap();
        append_content(&conn, id, "lo").unwrap();
        let msgs = list(&conn, cid, 10, 0).unwrap();
        assert_eq!(msgs[0].content, "hello");
    }

    #[test]
    fn list_returns_oldest_first_within_window() {
        let (_d, conn, cid) = fresh_conn();
        insert(&conn, cid, Role::User, "a").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        insert(&conn, cid, Role::Assistant, "b").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        insert(&conn, cid, Role::User, "c").unwrap();

        let msgs = list(&conn, cid, 10, 0).unwrap();
        let contents: Vec<&str> = msgs.iter().map(|m| m.content.as_str()).collect();
        assert_eq!(contents, vec!["a", "b", "c"]);
    }

    #[test]
    fn mark_seen_clears_unread_count() {
        let (_d, conn, cid) = fresh_conn();
        let a = insert(&conn, cid, Role::Assistant, "x").unwrap();
        let b = insert(&conn, cid, Role::Assistant, "y").unwrap();
        assert_eq!(unread_count(&conn, cid).unwrap(), 2);
        mark_seen(&conn, &[a, b]).unwrap();
        assert_eq!(unread_count(&conn, cid).unwrap(), 0);
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p manor-core --all-targets`
Expected: existing tests still pass, plus 7 new tests all green.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/assistant/conversation.rs crates/core/src/assistant/message.rs crates/core/src/assistant/mod.rs
git commit -m "feat(core): conversation + message data access with TDD"
```

---

### Task 4: `Proposal` type + insert stub (minimal TDD)

**Files:**
- Create: `crates/core/src/assistant/proposal.rs`
- Modify: `crates/core/src/assistant/mod.rs`

- [ ] **Step 1: Expose the submodule**

Edit `crates/core/src/assistant/mod.rs`:

```rust
//! Assistant substrate: SQLite persistence for conversations, messages, and proposals.

pub mod conversation;
pub mod db;
pub mod message;
pub mod proposal;
```

- [ ] **Step 2: Write `proposal.rs`**

Create `crates/core/src/assistant/proposal.rs`:

```rust
//! Proposals — central AI-action artefacts.
//!
//! Phase 2 scaffolds the table + types but no feature produces proposals yet.
//! Later phases (Rhythm, Ledger, Hearth, Bones) INSERT rows when their skills
//! need a reviewable diff.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    #[test]
    fn insert_returns_new_row_id() {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();

        let id = insert(
            &conn,
            NewProposal {
                kind: "week_plan",
                rationale: "Automated test proposal",
                diff_json: "{\"ops\":[]}",
                skill: "calendar",
            },
        )
        .unwrap();
        assert!(id > 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proposal WHERE id = ?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p manor-core --all-targets`
Expected: one additional test passes, total stays green.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/assistant/proposal.rs crates/core/src/assistant/mod.rs
git commit -m "feat(core): proposal type and insert stub"
```

---

### Task 5: Ollama HTTP streaming client (TDD with wiremock)

**Files:**
- Create: `crates/app/src/assistant/mod.rs`
- Create: `crates/app/src/assistant/ollama.rs`
- Create: `crates/app/src/assistant/prompts.rs`
- Modify: `crates/app/src/lib.rs`

- [ ] **Step 1: Create the module tree and expose it**

Create `crates/app/src/assistant/mod.rs`:

```rust
//! Assistant glue: Ollama client + Tauri commands.

pub mod ollama;
pub mod prompts;
```

Edit `crates/app/src/lib.rs` — add `pub mod assistant;` above the existing module content. The file should now begin:

```rust
//! Tauri command glue for Manor.

pub mod assistant;

use serde::Serialize;
use tauri::{Builder, Wry};

// ... (rest of the existing file unchanged)
```

- [ ] **Step 2: Write the system prompt constant**

Create `crates/app/src/assistant/prompts.rs`:

```rust
//! Prompts sent to the local LLM.

/// System prompt for Manor. Establishes identity, role, and the explicit
/// instruction not to act as Nell (persona hygiene for the public AGPL release).
pub const SYSTEM_PROMPT: &str = concat!(
    "You are Manor, a calm household assistant built into a local-first desktop app. ",
    "You help the user manage their calendar, chores, money, meals, and home. ",
    "Be warm, concise, and practical. Never speak as Nell or any other persona. ",
    "If you need to modify the user's data, describe the change you would make ",
    "rather than claiming to have made it; the app will ask for explicit approval.",
);
```

- [ ] **Step 3: Write the failing test for the Ollama streaming client**

Create `crates/app/src/assistant/ollama.rs`:

```rust
//! Ollama HTTP streaming client.
//!
//! Posts to Ollama's `/api/chat` with `stream=true` and yields `StreamChunk`s as
//! NDJSON lines arrive.

use anyhow::Result;
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
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p manor-app --all-targets`
Expected: existing `ping` + `register_returns_builder` tests pass, plus 3 new async tokio tests pass. First run downloads `wiremock`'s deps (~30s).

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/assistant crates/app/src/lib.rs
git commit -m "feat(app): ollama streaming client with wiremock tests"
```

---

### Task 6: Tauri commands + `Channel` wiring + DB init + register

**Files:**
- Create: `crates/app/src/assistant/commands.rs`
- Modify: `crates/app/src/assistant/mod.rs`
- Modify: `crates/app/src/lib.rs`

- [ ] **Step 1: Expose the new commands submodule**

Edit `crates/app/src/assistant/mod.rs`:

```rust
//! Assistant glue: Ollama client + Tauri commands.

pub mod commands;
pub mod ollama;
pub mod prompts;
```

- [ ] **Step 2: Write the commands module**

Create `crates/app/src/assistant/commands.rs`:

```rust
//! Tauri commands exposed to the frontend Assistant.

use crate::assistant::ollama::{ChatMessage, ChatRole, OllamaClient, StreamChunk, DEFAULT_ENDPOINT, DEFAULT_MODEL};
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

    // 3. Run the Ollama stream, forwarding each chunk to both the DB and the frontend.
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
            StreamChunk::Done | StreamChunk::Error(_) => {}
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
```

- [ ] **Step 3: Wire the commands into `register()` and add DB initialisation via `setup()`**

Replace the contents of `crates/app/src/lib.rs` with:

```rust
//! Tauri command glue for Manor.

pub mod assistant;

use serde::Serialize;
use tauri::{Builder, Manager, Wry};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PingResponse {
    pub message: String,
    pub core_version: String,
}

mod commands {
    use super::PingResponse;

    /// Minimal smoke command that proves IPC works end-to-end.
    #[tauri::command]
    pub fn ping() -> PingResponse {
        PingResponse {
            message: "pong".to_string(),
            core_version: manor_core::version().to_string(),
        }
    }
}

pub use commands::ping;

/// Registers every Tauri command this crate exposes and wires the SQLite DB
/// into application state via Tauri's `setup()` closure.
pub fn register(builder: Builder<Wry>) -> Builder<Wry> {
    builder
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("could not resolve app data dir");
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("manor.db");
            let db = assistant::commands::Db::open(db_path)?;
            app.manage(db);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            assistant::commands::send_message,
            assistant::commands::list_messages,
            assistant::commands::get_unread_count,
            assistant::commands::mark_seen,
        ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Builder;

    #[test]
    fn ping_returns_pong_with_core_version() {
        let resp = ping();
        assert_eq!(resp.message, "pong");
        assert_eq!(resp.core_version, manor_core::version());
    }

    #[test]
    fn register_returns_builder() {
        let _builder = register(Builder::default());
    }
}
```

- [ ] **Step 4: Verify the workspace compiles and all tests pass**

Run: `cargo test --workspace --all-targets`
Expected: all tests green (core + app, including the Ollama wiremock tests).

Run: `cargo check -p manor-desktop`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/assistant/commands.rs crates/app/src/assistant/mod.rs crates/app/src/lib.rs
git commit -m "feat(app): tauri commands + channel streaming + db init"
```

---

### Task 7: Copy avatar assets + add frontend deps

**Files:**
- Create: `apps/desktop/src/assets/avatars/{content,smile,questioning,laughing,confused}.png`
- Modify: `apps/desktop/package.json`
- Modify: `pnpm-lock.yaml` (regenerated by `pnpm install`)

- [ ] **Step 1: Copy the avatar assets**

Run from repo root:

```bash
mkdir -p apps/desktop/src/assets/avatars
cp material/manor_face_content.png     apps/desktop/src/assets/avatars/content.png
cp material/manor_face_smile.png       apps/desktop/src/assets/avatars/smile.png
cp material/manor_face_questioning.png apps/desktop/src/assets/avatars/questioning.png
cp material/manor_face_laughing.png    apps/desktop/src/assets/avatars/laughing.png
cp material/manor_face_confused.png    apps/desktop/src/assets/avatars/confused.png
```

Verify: `ls apps/desktop/src/assets/avatars/` should list 5 PNGs.

- [ ] **Step 2: Add frontend dependencies**

Run:

```bash
pnpm --filter manor-desktop add zustand @fontsource/nunito
pnpm --filter manor-desktop add -D vitest jsdom @vitest/ui
```

Verify `apps/desktop/package.json` now contains `zustand`, `@fontsource/nunito` under `dependencies` and `vitest`, `jsdom`, `@vitest/ui` under `devDependencies`.

- [ ] **Step 3: Confirm pnpm + typecheck still pass**

Run: `pnpm tsc`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/assets apps/desktop/package.json pnpm-lock.yaml
git commit -m "chore(desktop): copy manor avatars + add zustand/nunito/vitest"
```

---

### Task 8: Design tokens + Nunito import in `styles.css`

**Files:**
- Modify: `apps/desktop/src/styles.css`
- Modify: `apps/desktop/src/main.tsx`

- [ ] **Step 1: Replace `styles.css` with the token-driven version**

Overwrite `apps/desktop/src/styles.css`:

```css
:root {
  /* Typography */
  font-family: 'Nunito', system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  line-height: 1.5;
  color: #1a1a1a;
  background: #fafafa;
  font-synthesis: none;
  text-rendering: optimizeLegibility;

  /* Radii */
  --radius-sm: 8px;
  --radius-md: 12px;
  --radius-lg: 16px;
  --radius-pill: 999px;

  /* Shadows (soft, tinted) */
  --shadow-sm: 0 1px 2px rgba(20, 20, 30, 0.06), 0 1px 3px rgba(20, 20, 30, 0.04);
  --shadow-md: 0 4px 10px rgba(20, 20, 30, 0.08), 0 2px 6px rgba(20, 20, 30, 0.05);
  --shadow-lg: 0 12px 28px rgba(20, 20, 30, 0.12), 0 4px 10px rgba(20, 20, 30, 0.08);

  /* iMessage-style accents */
  --imessage-blue: #007aff;
  --imessage-green: #34c759;
  --imessage-red: #ff3b30;

  /* Neutrals */
  --ink: #1a1a1a;
  --paper: #fafafa;
  --paper-muted: #f1f1ee;
  --hairline: rgba(20, 20, 30, 0.08);
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  min-height: 100vh;
}
```

- [ ] **Step 2: Import Nunito in `main.tsx`**

Replace `apps/desktop/src/main.tsx` with:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/nunito/400.css";
import "@fontsource/nunito/600.css";
import "@fontsource/nunito/700.css";
import App from "./App";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 3: Confirm typecheck**

Run: `pnpm tsc`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/styles.css apps/desktop/src/main.tsx
git commit -m "style(desktop): design tokens + nunito typography"
```

---

### Task 9: Zustand store + expression map + typed IPC wrappers (TDD state tests)

**Files:**
- Create: `apps/desktop/src/lib/assistant/state.ts`
- Create: `apps/desktop/src/lib/assistant/expressions.ts`
- Create: `apps/desktop/src/lib/assistant/ipc.ts`
- Create: `apps/desktop/src/lib/assistant/state.test.ts`
- Modify: `apps/desktop/package.json` (add `test` script)
- Modify: `apps/desktop/vite.config.ts` (add vitest config)

- [ ] **Step 1: Expression map**

Create `apps/desktop/src/lib/assistant/expressions.ts`:

```ts
import content from "../../assets/avatars/content.png";
import smile from "../../assets/avatars/smile.png";
import questioning from "../../assets/avatars/questioning.png";
import laughing from "../../assets/avatars/laughing.png";
import confused from "../../assets/avatars/confused.png";

export type AssistantState =
  | "idle"
  | "listening"
  | "thinking"
  | "speaking"
  | "confused";

export const expressionFor = (state: AssistantState): string => {
  switch (state) {
    case "idle":
      return content;
    case "listening":
      return smile;
    case "thinking":
      return questioning;
    case "speaking":
      return smile;
    case "confused":
      return confused;
  }
};

// Reserved — triggered by delight heuristic in BubbleLayer, not by AssistantState.
export const LAUGHING = laughing;
```

- [ ] **Step 2: Typed IPC wrappers**

Create `apps/desktop/src/lib/assistant/ipc.ts`:

```ts
import { invoke, Channel } from "@tauri-apps/api/core";

export type Role = "user" | "assistant" | "system";

export interface Message {
  id: number;
  conversation_id: number;
  role: Role;
  content: string;
  created_at: number;
  seen: boolean;
  proposal_id: number | null;
}

export type StreamChunk =
  | { type: "Token"; value: string }
  | { type: "Done" }
  | { type: "Error"; value: "OllamaUnreachable" | "ModelMissing" | "Interrupted" | "Unknown" };

export async function sendMessage(
  content: string,
  onEvent: (chunk: StreamChunk) => void,
): Promise<void> {
  const channel = new Channel<StreamChunk>();
  channel.onmessage = onEvent;
  return invoke<void>("send_message", { content, onEvent: channel });
}

export async function listMessages(limit = 100, offset = 0): Promise<Message[]> {
  return invoke<Message[]>("list_messages", { limit, offset });
}

export async function getUnreadCount(): Promise<number> {
  return invoke<number>("get_unread_count");
}

export async function markSeen(messageIds: number[]): Promise<void> {
  return invoke<void>("mark_seen", { messageIds });
}
```

- [ ] **Step 3: Write the failing state-machine test (TDD)**

Create `apps/desktop/src/lib/assistant/state.test.ts`:

```ts
import { describe, expect, it, beforeEach } from "vitest";
import { useAssistantStore } from "./state";

describe("assistant store", () => {
  beforeEach(() => {
    useAssistantStore.setState(useAssistantStore.getInitialState(), true);
  });

  it("starts in idle state with no messages and no bubbles", () => {
    const s = useAssistantStore.getState();
    expect(s.avatarState).toBe("idle");
    expect(s.messages).toEqual([]);
    expect(s.transientBubbles).toEqual([]);
    expect(s.unreadCount).toBe(0);
    expect(s.drawerOpen).toBe(false);
  });

  it("transitions avatarState on user send then streaming", () => {
    const s = useAssistantStore.getState();
    s.setAvatarState("listening");
    expect(useAssistantStore.getState().avatarState).toBe("listening");
    s.setAvatarState("thinking");
    expect(useAssistantStore.getState().avatarState).toBe("thinking");
    s.setAvatarState("speaking");
    expect(useAssistantStore.getState().avatarState).toBe("speaking");
  });

  it("appends assistant token fragments to the in-flight message", () => {
    const s = useAssistantStore.getState();
    s.beginAssistantMessage(42);
    s.appendAssistantToken("Hel");
    s.appendAssistantToken("lo.");
    s.endAssistantMessage();
    const msgs = useAssistantStore.getState().messages;
    expect(msgs).toHaveLength(1);
    expect(msgs[0]).toMatchObject({ id: 42, content: "Hello.", role: "assistant" });
  });

  it("enqueues a transient bubble and removes it by id", () => {
    const s = useAssistantStore.getState();
    s.enqueueBubble({
      id: "a",
      kind: "user",
      content: "hi",
      messageId: null,
      ttlMs: 3000,
    });
    expect(useAssistantStore.getState().transientBubbles).toHaveLength(1);
    s.dismissBubble("a");
    expect(useAssistantStore.getState().transientBubbles).toHaveLength(0);
  });

  it("caps visible bubbles at 3 — enqueueing a 4th evicts the oldest", () => {
    const s = useAssistantStore.getState();
    for (const id of ["a", "b", "c", "d"]) {
      s.enqueueBubble({ id, kind: "user", content: id, messageId: null, ttlMs: 3000 });
    }
    const ids = useAssistantStore.getState().transientBubbles.map((b) => b.id);
    expect(ids).toEqual(["b", "c", "d"]);
  });
});
```

- [ ] **Step 4: Implement `state.ts` with just enough to pass**

Create `apps/desktop/src/lib/assistant/state.ts`:

```ts
import { create } from "zustand";
import type { AssistantState } from "./expressions";
import type { Message } from "./ipc";

export interface TransientBubble {
  id: string;
  kind: "user" | "assistant" | "error";
  content: string;
  messageId: number | null; // the DB message id, for click-to-mark-seen / scroll-to
  ttlMs: number;
}

interface AssistantStore {
  avatarState: AssistantState;
  messages: Message[];
  transientBubbles: TransientBubble[];
  unreadCount: number;
  drawerOpen: boolean;

  setAvatarState: (s: AssistantState) => void;
  hydrateMessages: (msgs: Message[]) => void;
  beginAssistantMessage: (id: number) => void;
  appendAssistantToken: (fragment: string) => void;
  endAssistantMessage: () => void;
  addUserMessage: (msg: Message) => void;

  enqueueBubble: (b: TransientBubble) => void;
  dismissBubble: (id: string) => void;

  setUnreadCount: (n: number) => void;
  setDrawerOpen: (open: boolean) => void;
}

const MAX_VISIBLE_BUBBLES = 3;

export const useAssistantStore = create<AssistantStore>((set) => ({
  avatarState: "idle",
  messages: [],
  transientBubbles: [],
  unreadCount: 0,
  drawerOpen: false,

  setAvatarState: (s) => set({ avatarState: s }),

  hydrateMessages: (msgs) => set({ messages: msgs }),

  beginAssistantMessage: (id) =>
    set((st) => ({
      messages: [
        ...st.messages,
        {
          id,
          conversation_id: 1,
          role: "assistant",
          content: "",
          created_at: Date.now(),
          seen: false,
          proposal_id: null,
        },
      ],
    })),

  appendAssistantToken: (fragment) =>
    set((st) => {
      const last = st.messages[st.messages.length - 1];
      if (!last || last.role !== "assistant") return st;
      const updated = { ...last, content: last.content + fragment };
      return { messages: [...st.messages.slice(0, -1), updated] };
    }),

  endAssistantMessage: () => set({ avatarState: "idle" }),

  addUserMessage: (msg) =>
    set((st) => ({ messages: [...st.messages, msg] })),

  enqueueBubble: (b) =>
    set((st) => {
      const next = [...st.transientBubbles, b];
      while (next.length > MAX_VISIBLE_BUBBLES) next.shift();
      return { transientBubbles: next };
    }),

  dismissBubble: (id) =>
    set((st) => ({
      transientBubbles: st.transientBubbles.filter((b) => b.id !== id),
    })),

  setUnreadCount: (n) => set({ unreadCount: n }),

  setDrawerOpen: (open) => set({ drawerOpen: open }),
}));
```

- [ ] **Step 5: Add vitest config + test script**

Edit `apps/desktop/vite.config.ts`:

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_"],
  test: {
    environment: "jsdom",
    globals: false,
  },
});
```

Edit `apps/desktop/package.json` — add a `test` script under `scripts`:

```json
"test": "vitest run"
```

Full `scripts` block should now read:

```json
"scripts": {
  "dev": "vite",
  "build": "tsc -b && vite build",
  "preview": "vite preview",
  "tauri": "tauri",
  "tsc": "tsc --noEmit",
  "test": "vitest run"
}
```

- [ ] **Step 6: Run the tests to confirm they pass**

Run: `pnpm --filter manor-desktop test`
Expected: 5 tests pass.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src/lib/assistant apps/desktop/vite.config.ts apps/desktop/package.json
git commit -m "feat(desktop): zustand store + ipc wrappers + expression map"
```

---

### Task 10: `Avatar` + `UnreadBadge` components

**Files:**
- Create: `apps/desktop/src/components/Assistant/Avatar.tsx`
- Create: `apps/desktop/src/components/Assistant/UnreadBadge.tsx`

- [ ] **Step 1: Avatar**

Create `apps/desktop/src/components/Assistant/Avatar.tsx`:

```tsx
import { useAssistantStore } from "../../lib/assistant/state";
import { expressionFor } from "../../lib/assistant/expressions";

interface AvatarProps {
  size?: number;
  onClick?: () => void;
}

export default function Avatar({ size = 96, onClick }: AvatarProps) {
  const state = useAssistantStore((s) => s.avatarState);
  const src = expressionFor(state);

  return (
    <img
      src={src}
      alt="Manor"
      width={size}
      height={size}
      onClick={onClick}
      style={{
        width: size,
        height: size,
        cursor: onClick ? "pointer" : "default",
        transform: "scaleX(-1)",
        transition: "opacity 150ms ease-in-out",
        userSelect: "none",
      }}
      draggable={false}
    />
  );
}
```

- [ ] **Step 2: UnreadBadge**

Create `apps/desktop/src/components/Assistant/UnreadBadge.tsx`:

```tsx
import { useAssistantStore } from "../../lib/assistant/state";

export default function UnreadBadge() {
  const count = useAssistantStore((s) => s.unreadCount);
  if (count === 0) return null;

  const label = count > 9 ? "9+" : String(count);

  return (
    <div
      aria-label={`${count} unread message${count === 1 ? "" : "s"}`}
      style={{
        minWidth: 20,
        height: 20,
        padding: "0 6px",
        borderRadius: "var(--radius-pill)",
        background: "var(--imessage-red)",
        color: "white",
        fontSize: 11,
        fontWeight: 700,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        boxShadow: "var(--shadow-sm)",
        pointerEvents: "none",
      }}
    >
      {label}
    </div>
  );
}
```

- [ ] **Step 3: Confirm typecheck**

Run: `pnpm tsc`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Assistant/Avatar.tsx apps/desktop/src/components/Assistant/UnreadBadge.tsx
git commit -m "feat(desktop): Avatar + UnreadBadge components"
```

---

### Task 11: `BubbleLayer` component (TTL + hover pause + click-to-mark-seen) (TDD TTL logic)

**Files:**
- Create: `apps/desktop/src/lib/assistant/bubble-ttl.ts`
- Create: `apps/desktop/src/lib/assistant/bubble-ttl.test.ts`
- Create: `apps/desktop/src/components/Assistant/BubbleLayer.tsx`

- [ ] **Step 1: Write the failing TTL-logic test first (TDD)**

Create `apps/desktop/src/lib/assistant/bubble-ttl.test.ts`:

```ts
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createTtlTimer } from "./bubble-ttl";

describe("createTtlTimer", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("fires onExpire after ttlMs", () => {
    const onExpire = vi.fn();
    createTtlTimer(3000, onExpire).start();

    vi.advanceTimersByTime(2999);
    expect(onExpire).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(onExpire).toHaveBeenCalledOnce();
  });

  it("pauses and resumes with remaining 3s after mouse-out", () => {
    const onExpire = vi.fn();
    const timer = createTtlTimer(7000, onExpire);
    timer.start();

    vi.advanceTimersByTime(2000);
    timer.pause();

    vi.advanceTimersByTime(10_000); // no firing while paused
    expect(onExpire).not.toHaveBeenCalled();

    timer.resumeWith(3000); // spec §6.2: resumes with 3s remaining
    vi.advanceTimersByTime(2999);
    expect(onExpire).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(onExpire).toHaveBeenCalledOnce();
  });

  it("cancel prevents future firing", () => {
    const onExpire = vi.fn();
    const timer = createTtlTimer(1000, onExpire);
    timer.start();
    timer.cancel();
    vi.advanceTimersByTime(5000);
    expect(onExpire).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Implement `bubble-ttl.ts`**

Create `apps/desktop/src/lib/assistant/bubble-ttl.ts`:

```ts
export interface TtlTimer {
  start: () => void;
  pause: () => void;
  resumeWith: (remainingMs: number) => void;
  cancel: () => void;
}

export function createTtlTimer(ttlMs: number, onExpire: () => void): TtlTimer {
  let handle: ReturnType<typeof setTimeout> | null = null;

  const clear = () => {
    if (handle !== null) {
      clearTimeout(handle);
      handle = null;
    }
  };

  return {
    start() {
      clear();
      handle = setTimeout(onExpire, ttlMs);
    },
    pause() {
      clear();
    },
    resumeWith(remainingMs) {
      clear();
      handle = setTimeout(onExpire, remainingMs);
    },
    cancel() {
      clear();
    },
  };
}
```

- [ ] **Step 3: Run the test**

Run: `pnpm --filter manor-desktop test`
Expected: the 3 new TTL tests pass alongside the earlier 5 state tests (8 total).

- [ ] **Step 4: Implement `BubbleLayer` using the timer**

Create `apps/desktop/src/components/Assistant/BubbleLayer.tsx`:

```tsx
import { useEffect, useRef } from "react";
import { useAssistantStore } from "../../lib/assistant/state";
import type { TransientBubble } from "../../lib/assistant/state";
import { createTtlTimer, TtlTimer } from "../../lib/assistant/bubble-ttl";
import { markSeen } from "../../lib/assistant/ipc";

function bubbleColors(kind: TransientBubble["kind"]): {
  background: string;
  color: string;
  alignSelf: "flex-start" | "flex-end";
  borderRadius: string;
  border?: string;
} {
  switch (kind) {
    case "user":
      return {
        background: "var(--imessage-blue)",
        color: "white",
        alignSelf: "flex-end",
        borderRadius: "var(--radius-lg) var(--radius-lg) 4px var(--radius-lg)",
      };
    case "assistant":
      return {
        background: "var(--imessage-green)",
        color: "white",
        alignSelf: "flex-start",
        borderRadius: "var(--radius-lg) var(--radius-lg) var(--radius-lg) 4px",
      };
    case "error":
      return {
        background: "rgba(255, 59, 48, 0.1)",
        color: "var(--imessage-red)",
        alignSelf: "flex-start",
        borderRadius: "var(--radius-lg) var(--radius-lg) var(--radius-lg) 4px",
        border: "1px solid rgba(255, 59, 48, 0.4)",
      };
  }
}

export default function BubbleLayer() {
  const bubbles = useAssistantStore((s) => s.transientBubbles);
  const dismissBubble = useAssistantStore((s) => s.dismissBubble);
  const setDrawerOpen = useAssistantStore((s) => s.setDrawerOpen);
  const drawerOpen = useAssistantStore((s) => s.drawerOpen);

  if (drawerOpen) return null;

  return (
    <div
      style={{
        position: "fixed",
        bottom: 130,
        right: 16,
        display: "flex",
        flexDirection: "column",
        gap: 6,
        alignItems: "flex-end",
        maxWidth: 320,
        pointerEvents: "none",
      }}
    >
      {bubbles.map((b) => (
        <Bubble
          key={b.id}
          bubble={b}
          onDismiss={() => dismissBubble(b.id)}
          onClick={() => {
            if (b.kind === "assistant" && b.messageId !== null) {
              void markSeen([b.messageId]);
            }
            setDrawerOpen(true);
          }}
        />
      ))}
    </div>
  );
}

interface BubbleProps {
  bubble: TransientBubble;
  onDismiss: () => void;
  onClick: () => void;
}

function Bubble({ bubble, onDismiss, onClick }: BubbleProps) {
  const timerRef = useRef<TtlTimer | null>(null);

  useEffect(() => {
    const timer = createTtlTimer(bubble.ttlMs, onDismiss);
    timerRef.current = timer;
    timer.start();
    return () => timer.cancel();
  }, [bubble.ttlMs, onDismiss]);

  const c = bubbleColors(bubble.kind);

  return (
    <div
      role="button"
      tabIndex={0}
      onMouseEnter={() => timerRef.current?.pause()}
      onMouseLeave={() => timerRef.current?.resumeWith(3000)}
      onClick={onClick}
      style={{
        background: c.background,
        color: c.color,
        alignSelf: c.alignSelf,
        borderRadius: c.borderRadius,
        border: c.border,
        padding: "8px 12px",
        fontSize: 14,
        maxWidth: 280,
        boxShadow: "var(--shadow-md)",
        pointerEvents: "auto",
        cursor: "pointer",
        animation: "bubbleIn 200ms ease-out",
      }}
    >
      {bubble.content}
    </div>
  );
}
```

- [ ] **Step 5: Add the entry animation keyframe**

Append to `apps/desktop/src/styles.css`:

```css
@keyframes bubbleIn {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
```

- [ ] **Step 6: Confirm tests + typecheck**

Run: `pnpm --filter manor-desktop test && pnpm tsc`
Expected: 8 tests pass, typecheck clean.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src/lib/assistant/bubble-ttl.ts apps/desktop/src/lib/assistant/bubble-ttl.test.ts apps/desktop/src/components/Assistant/BubbleLayer.tsx apps/desktop/src/styles.css
git commit -m "feat(desktop): BubbleLayer + TTL timer with TDD"
```

---

### Task 12: `InputPill` component

**Files:**
- Create: `apps/desktop/src/components/Assistant/InputPill.tsx`

- [ ] **Step 1: Write the component**

Create `apps/desktop/src/components/Assistant/InputPill.tsx`:

```tsx
import { forwardRef, useState, KeyboardEvent } from "react";

interface InputPillProps {
  onSubmit: (content: string) => void;
  onFocus?: () => void;
  onBlur?: () => void;
}

const InputPill = forwardRef<HTMLInputElement, InputPillProps>(
  ({ onSubmit, onFocus, onBlur }, ref) => {
    const [value, setValue] = useState("");
    const [focused, setFocused] = useState(false);

    const handleKey = (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        const trimmed = value.trim();
        if (trimmed.length === 0) return;
        onSubmit(trimmed);
        setValue("");
      } else if (e.key === "Escape") {
        (e.target as HTMLInputElement).blur();
      }
    };

    return (
      <input
        ref={ref}
        type="text"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKey}
        onFocus={() => {
          setFocused(true);
          onFocus?.();
        }}
        onBlur={() => {
          setFocused(false);
          onBlur?.();
        }}
        placeholder="Say something…"
        style={{
          width: focused ? 320 : 220,
          padding: "8px 14px",
          borderRadius: "var(--radius-pill)",
          border: "1px solid var(--hairline)",
          background: "var(--paper)",
          fontSize: 14,
          fontFamily: "inherit",
          color: "var(--ink)",
          outline: "none",
          boxShadow: focused ? "var(--shadow-md)" : "var(--shadow-sm)",
          transition: "width 150ms ease, box-shadow 150ms ease",
        }}
      />
    );
  },
);

InputPill.displayName = "InputPill";
export default InputPill;
```

- [ ] **Step 2: Confirm typecheck**

Run: `pnpm tsc`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/components/Assistant/InputPill.tsx
git commit -m "feat(desktop): InputPill component"
```

---

### Task 13: `ConversationDrawer` component

**Files:**
- Create: `apps/desktop/src/components/Assistant/ConversationDrawer.tsx`

- [ ] **Step 1: Write the component**

Create `apps/desktop/src/components/Assistant/ConversationDrawer.tsx`:

```tsx
import { useEffect, useState, useRef, KeyboardEvent } from "react";
import { useAssistantStore } from "../../lib/assistant/state";
import { expressionFor } from "../../lib/assistant/expressions";
import { listMessages, markSeen, getUnreadCount } from "../../lib/assistant/ipc";

interface ConversationDrawerProps {
  onSubmit: (content: string) => void;
}

export default function ConversationDrawer({ onSubmit }: ConversationDrawerProps) {
  const drawerOpen = useAssistantStore((s) => s.drawerOpen);
  const setDrawerOpen = useAssistantStore((s) => s.setDrawerOpen);
  const messages = useAssistantStore((s) => s.messages);
  const avatarState = useAssistantStore((s) => s.avatarState);
  const hydrateMessages = useAssistantStore((s) => s.hydrateMessages);
  const setUnreadCount = useAssistantStore((s) => s.setUnreadCount);

  const [value, setValue] = useState("");
  const bodyRef = useRef<HTMLDivElement>(null);

  // On drawer open: hydrate messages, mark all unread as seen, reset unread count.
  useEffect(() => {
    if (!drawerOpen) return;
    void (async () => {
      const msgs = await listMessages(100, 0);
      hydrateMessages(msgs);
      const unreadIds = msgs.filter((m) => m.role === "assistant" && !m.seen).map((m) => m.id);
      if (unreadIds.length > 0) await markSeen(unreadIds);
      const n = await getUnreadCount();
      setUnreadCount(n);
    })();
  }, [drawerOpen, hydrateMessages, setUnreadCount]);

  // Auto-scroll to bottom on new messages when drawer is open.
  useEffect(() => {
    if (!drawerOpen) return;
    const el = bodyRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [drawerOpen, messages]);

  useEffect(() => {
    if (!drawerOpen) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") setDrawerOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [drawerOpen, setDrawerOpen]);

  if (!drawerOpen) return null;

  const submit = () => {
    const t = value.trim();
    if (t.length === 0) return;
    onSubmit(t);
    setValue("");
  };

  const onInputKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  };

  return (
    <>
      {/* click-away overlay */}
      <div
        onClick={() => setDrawerOpen(false)}
        style={{
          position: "fixed",
          inset: 0,
          background: "transparent",
          zIndex: 900,
        }}
      />

      <aside
        style={{
          position: "fixed",
          top: 0,
          right: 0,
          bottom: 0,
          width: "min(420px, 45vw)",
          background: "var(--paper)",
          boxShadow: "var(--shadow-lg)",
          display: "flex",
          flexDirection: "column",
          zIndex: 1001,
          animation: "drawerIn 250ms ease-out",
        }}
      >
        <header
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            padding: "14px 16px",
            borderBottom: "1px solid var(--hairline)",
          }}
        >
          <img
            src={expressionFor(avatarState)}
            alt="Manor"
            width={32}
            height={32}
            style={{ transform: "scaleX(-1)", borderRadius: "var(--radius-md)" }}
          />
          <strong style={{ flex: 1, fontSize: 15 }}>Manor</strong>
          <button
            onClick={() => setDrawerOpen(false)}
            aria-label="Close"
            style={{
              border: "none",
              background: "transparent",
              fontSize: 20,
              cursor: "pointer",
              color: "var(--ink)",
              lineHeight: 1,
            }}
          >
            ×
          </button>
        </header>

        <div
          ref={bodyRef}
          style={{
            flex: 1,
            overflowY: "auto",
            padding: 14,
            display: "flex",
            flexDirection: "column",
            gap: 6,
          }}
        >
          {messages.length === 0 && (
            <div
              style={{
                alignSelf: "flex-start",
                background: "var(--imessage-green)",
                color: "white",
                padding: "8px 12px",
                borderRadius: "var(--radius-lg) var(--radius-lg) var(--radius-lg) 4px",
                maxWidth: "80%",
                fontSize: 14,
                boxShadow: "var(--shadow-sm)",
              }}
            >
              Hi, I'm Manor. Ask me anything.
            </div>
          )}

          {messages.map((m) => (
            <div
              key={m.id}
              style={{
                alignSelf: m.role === "user" ? "flex-end" : "flex-start",
                background:
                  m.role === "user" ? "var(--imessage-blue)" : "var(--imessage-green)",
                color: "white",
                padding: "8px 12px",
                borderRadius:
                  m.role === "user"
                    ? "var(--radius-lg) var(--radius-lg) 4px var(--radius-lg)"
                    : "var(--radius-lg) var(--radius-lg) var(--radius-lg) 4px",
                maxWidth: "80%",
                fontSize: 14,
                whiteSpace: "pre-wrap",
                boxShadow: "var(--shadow-sm)",
              }}
            >
              {m.content}
            </div>
          ))}
        </div>

        <footer
          style={{
            padding: 10,
            borderTop: "1px solid var(--hairline)",
            display: "flex",
            gap: 8,
          }}
        >
          <textarea
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={onInputKey}
            placeholder="Say something…"
            rows={2}
            style={{
              flex: 1,
              padding: "8px 12px",
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--hairline)",
              fontFamily: "inherit",
              fontSize: 14,
              resize: "none",
              outline: "none",
            }}
          />
          <button
            onClick={submit}
            style={{
              padding: "8px 14px",
              borderRadius: "var(--radius-md)",
              border: "none",
              background: "var(--imessage-blue)",
              color: "white",
              fontWeight: 600,
              cursor: "pointer",
            }}
          >
            Send
          </button>
        </footer>
      </aside>
    </>
  );
}
```

- [ ] **Step 2: Add drawer animation**

Append to `apps/desktop/src/styles.css`:

```css
@keyframes drawerIn {
  from {
    transform: translateX(100%);
  }
  to {
    transform: translateX(0);
  }
}
```

- [ ] **Step 3: Confirm typecheck**

Run: `pnpm tsc`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Assistant/ConversationDrawer.tsx apps/desktop/src/styles.css
git commit -m "feat(desktop): ConversationDrawer component"
```

---

### Task 14: `Assistant.tsx` composes everything + ⌘/ hotkey + mount in `App.tsx`

**Files:**
- Create: `apps/desktop/src/components/Assistant/Assistant.tsx`
- Modify: `apps/desktop/src/App.tsx`

- [ ] **Step 1: Write `Assistant.tsx`**

Create `apps/desktop/src/components/Assistant/Assistant.tsx`:

```tsx
import { useEffect, useRef } from "react";
import Avatar from "./Avatar";
import BubbleLayer from "./BubbleLayer";
import InputPill from "./InputPill";
import UnreadBadge from "./UnreadBadge";
import ConversationDrawer from "./ConversationDrawer";
import { useAssistantStore } from "../../lib/assistant/state";
import { sendMessage, getUnreadCount, markSeen, listMessages } from "../../lib/assistant/ipc";
import type { StreamChunk } from "../../lib/assistant/ipc";

function newBubbleId() {
  return Math.random().toString(36).slice(2, 10);
}

function looksLikeDelight(content: string): boolean {
  if (/[\u{1F389}\u{1F38A}]/u.test(content)) return true;
  const exclaims = (content.match(/!/g) || []).length;
  return exclaims >= 3;
}

export default function Assistant() {
  const pillRef = useRef<HTMLInputElement>(null);

  const setAvatarState = useAssistantStore((s) => s.setAvatarState);
  const enqueueBubble = useAssistantStore((s) => s.enqueueBubble);
  const beginAssistantMessage = useAssistantStore((s) => s.beginAssistantMessage);
  const appendAssistantToken = useAssistantStore((s) => s.appendAssistantToken);
  const endAssistantMessage = useAssistantStore((s) => s.endAssistantMessage);
  const addUserMessage = useAssistantStore((s) => s.addUserMessage);
  const setUnreadCount = useAssistantStore((s) => s.setUnreadCount);
  const setDrawerOpen = useAssistantStore((s) => s.setDrawerOpen);
  const hydrateMessages = useAssistantStore((s) => s.hydrateMessages);

  // Initial load: hydrate recent messages + unread count.
  useEffect(() => {
    void (async () => {
      const msgs = await listMessages(100, 0);
      hydrateMessages(msgs);
      const n = await getUnreadCount();
      setUnreadCount(n);
    })();
  }, [hydrateMessages, setUnreadCount]);

  // Global ⌘/ — focus the pill (only when Manor has window focus; listener is scoped to document).
  useEffect(() => {
    let lastFire = 0;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "/") {
        const now = Date.now();
        if (now - lastFire < 150) return;
        lastFire = now;
        e.preventDefault();
        pillRef.current?.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const handleSubmit = async (content: string) => {
    setAvatarState("listening");

    // Optimistic: add a blue user bubble + a provisional message to the scrollback.
    const userBubbleId = newBubbleId();
    enqueueBubble({
      id: userBubbleId,
      kind: "user",
      content,
      messageId: null,
      ttlMs: 3000,
    });
    // We don't know the DB id yet, but addUserMessage takes a full Message shape.
    // Use a negative temporary id that won't collide with real ids; the drawer
    // re-hydrates from the DB on open, so this is fine.
    addUserMessage({
      id: -Date.now(),
      conversation_id: 1,
      role: "user",
      content,
      created_at: Date.now(),
      seen: true,
      proposal_id: null,
    });

    setAvatarState("thinking");

    let assistantDbId: number | null = null;
    const assistantBubbleId = newBubbleId();
    let assistantText = "";

    const onEvent = (chunk: StreamChunk) => {
      if (chunk.type === "Token") {
        if (assistantDbId === null) {
          // First token — we don't actually have the DB id yet from the current IPC
          // surface, so we pick a synthetic one (negative) for the client; the
          // drawer always re-hydrates from the DB on open so real ids appear there.
          assistantDbId = -Date.now();
          beginAssistantMessage(assistantDbId);
          setAvatarState("speaking");
          enqueueBubble({
            id: assistantBubbleId,
            kind: "assistant",
            content: "",
            messageId: assistantDbId,
            ttlMs: 7000,
          });
        }
        assistantText += chunk.value;
        appendAssistantToken(chunk.value);
      } else if (chunk.type === "Done") {
        endAssistantMessage();
        if (looksLikeDelight(assistantText)) {
          setAvatarState("idle"); // will pass through laughing in a future refinement
        } else {
          setAvatarState("idle");
        }
        // Refresh unread count from DB — the authoritative source.
        void getUnreadCount().then(setUnreadCount);
      } else if (chunk.type === "Error") {
        setAvatarState("confused");
        const errorMessage =
          chunk.value === "OllamaUnreachable"
            ? "I can't reach Ollama. Is it running?"
            : chunk.value === "ModelMissing"
              ? "I need the model `qwen2.5:7b-instruct`. Run `./scripts/install-ollama.sh`."
              : chunk.value === "Interrupted"
                ? "The reply was interrupted — check Ollama."
                : "Something went wrong. Check the logs.";
        enqueueBubble({
          id: newBubbleId(),
          kind: "error",
          content: errorMessage,
          messageId: null,
          ttlMs: 7000,
        });
      }
    };

    try {
      await sendMessage(content, onEvent);
    } catch (e) {
      setAvatarState("confused");
      enqueueBubble({
        id: newBubbleId(),
        kind: "error",
        content: `IPC error: ${String(e)}`,
        messageId: null,
        ttlMs: 7000,
      });
    }
  };

  return (
    <>
      <ConversationDrawer onSubmit={handleSubmit} />

      <div
        style={{
          position: "fixed",
          bottom: 16,
          right: 16,
          display: "flex",
          flexDirection: "column",
          alignItems: "flex-end",
          gap: 8,
          zIndex: 1000,
        }}
      >
        <UnreadBadgeWithAnchor />
        <InputPill
          ref={pillRef}
          onSubmit={handleSubmit}
          onFocus={() => setAvatarState("listening")}
          onBlur={() => setAvatarState("idle")}
        />
        <Avatar onClick={() => setDrawerOpen(true)} />
      </div>
    </>
  );
}

function UnreadBadgeWithAnchor() {
  return (
    <div style={{ position: "relative" }}>
      <div style={{ position: "absolute", top: -6, right: -6 }}>
        <UnreadBadge />
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Mount Assistant in `App.tsx`**

Replace `apps/desktop/src/App.tsx` with:

```tsx
import Assistant from "./components/Assistant/Assistant";

export default function App() {
  return (
    <main
      style={{
        minHeight: "100vh",
        padding: "2rem",
        position: "relative",
      }}
    >
      <h1 style={{ fontWeight: 700, fontSize: 28 }}>Manor</h1>
      <p style={{ color: "rgba(0,0,0,0.6)" }}>
        Phase 2: she's here. Future views (Today, Chores, Meals) land in later phases.
      </p>

      <Assistant />
    </main>
  );
}
```

- [ ] **Step 3: Confirm everything compiles**

Run: `pnpm tsc && pnpm --filter manor-desktop test`
Expected: typecheck clean, all 8 tests still pass.

Run: `cargo check -p manor-desktop`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Assistant/Assistant.tsx apps/desktop/src/App.tsx
git commit -m "feat(desktop): Assistant composed + hotkey + mounted in App"
```

---

### Task 15: Extend `install-ollama.sh` + end-to-end smoke test + tag

**Files:**
- Modify: `scripts/install-ollama.sh`

- [ ] **Step 1: Extend `install-ollama.sh` to pull the default model**

Replace `scripts/install-ollama.sh` with:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Ensures Ollama is installed and the default Manor model is pulled.

MANOR_MODEL="qwen2.5:7b-instruct"

if ! command -v ollama >/dev/null 2>&1; then
  echo "Installing Ollama via Homebrew…"
  brew install ollama
fi

echo "Ollama is installed at: $(command -v ollama)"

echo "Ensuring Ollama service is running…"
if ! pgrep -f "ollama serve" >/dev/null 2>&1; then
  echo "  Ollama is not running. Start it in another terminal with: ollama serve"
fi

echo "Ensuring model ${MANOR_MODEL} is pulled…"
ollama pull "${MANOR_MODEL}"

echo ""
echo "Done. Manor will use ${MANOR_MODEL} as its default local model."
```

- [ ] **Step 2: Verify the script is still executable and syntactically valid**

Run: `bash -n scripts/install-ollama.sh && chmod +x scripts/install-ollama.sh && echo "ok"`
Expected: prints `ok`.

- [ ] **Step 3: Run the full workspace verification suite**

Run: `cargo fmt --all --check`
Expected: clean.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

Run: `cargo test --workspace --all-targets`
Expected: all tests pass (core tests, app Ollama tests, app register test).

Run: `pnpm tsc`
Expected: clean.

Run: `pnpm --filter manor-desktop test`
Expected: all 8 frontend tests pass.

- [ ] **Step 4: Manual end-to-end smoke**

Ensure Ollama is running in a separate terminal: `ollama serve`.
Ensure the model is pulled: `./scripts/install-ollama.sh`.

Then from the repo root: `./scripts/dev.sh`.

Expected:
- A Tauri window titled "Manor" opens
- Manor heading + short paragraph at top-left
- Her avatar (`content.png`) visible in the bottom-right corner, mirrored horizontally
- Input pill above the avatar with placeholder "Say something…"
- Type "hello" and press Enter
- Blue bubble "hello" pops up to the right of the avatar, fades after ~3s
- Avatar shifts to `questioning`
- Green bubble appears and streams the reply word-by-word
- Avatar shifts to `smile` while streaming
- Reply bubble stays ~7s after streaming ends, then fades
- If you didn't click it, a red badge with "1" appears above the avatar
- Click the avatar → drawer slides in from the right showing "hello" (blue) and the reply (green) with full history
- Badge clears
- Press Escape → drawer closes
- Press ⌘/ → input pill gets focus immediately

- [ ] **Step 5: Push the branch and open a PR**

```bash
git push -u origin feature/phase-2-assistant
gh pr create --base main --head feature/phase-2-assistant --title "Phase 2 — Assistant shell (Manor listens, remembers, responds)" --body "$(cat <<'EOF'
## Summary

Ships Manor's always-present Assistant — a cornered, expressive avatar who listens, thinks, and streams replies via local Ollama, backed by SQLite persistence and a proposal table scaffolded for future features.

- **Rust:** new \`assistant\` modules in \`manor-core\` (SQLite schema + data access via rusqlite + refinery) and \`manor-app\` (Ollama HTTP streaming client + Tauri commands with Channel<T>).
- **Frontend:** new \`Assistant\` component tree (Avatar, BubbleLayer, InputPill, UnreadBadge, ConversationDrawer), zustand store, Pablo Stanley / Open Peeps aesthetic with Nunito typography and iMessage bubble colours.
- **Character:** 5 expressions from \`material/\` drive the state machine.
- **Ollama:** Local-only \`qwen2.5:7b-instruct\`, streaming via NDJSON, system prompt establishes Manor identity (no Nell leakage).

## Test plan

- [x] \`cargo test --workspace --all-targets\` green (new tests for core + app)
- [x] \`cargo fmt --check\` + \`cargo clippy --all-targets -- -D warnings\` clean
- [x] \`pnpm tsc\` clean
- [x] \`pnpm --filter manor-desktop test\` (vitest) green — 8 state / TTL tests
- [x] Manual smoke: open Manor, send "hello", green bubble streams back; close + reopen, drawer shows previous conversation
- [ ] CI green on this PR
EOF
)"
```

- [ ] **Step 6: After CI passes and the PR merges, tag the phase**

```bash
git checkout main
git pull origin main
git tag -a phase-2-assistant-complete -m "Phase 2 Assistant shell: Manor listens, remembers, responds"
git push origin phase-2-assistant-complete
```

---

## Self-Review Checklist

**1. Spec coverage for Phase 2:**

- §4.1 Rust crates → Tasks 2–6 touch all three crates as planned
- §4.2 new workspace deps → Task 1 adds rusqlite, refinery, reqwest, futures-util, chrono, dev-deps
- §4.3 Tauri IPC contract (send_message streaming, list_messages, get_unread_count, mark_seen) → Task 6
- §4.4 frontend structure (Avatar / BubbleLayer / InputPill / UnreadBadge / ConversationDrawer / Assistant) → Tasks 10–14
- §4.5 zustand store slices → Task 9 defines the full shape
- §4.6 design tokens, Nunito → Task 8
- §5 data model (conversation / message / proposal + V1 migration) → Task 2 (schema) + Tasks 3–4 (data access)
- §6 UI/UX contract (avatar positioning, bubble behaviour, badge, pill, hotkey, drawer, expression map, first-run, error states) → Tasks 10–14
- §7 Ollama integration (transport, NDJSON parsing, config, context window, persistence flow) → Tasks 5–6
- §8 install-ollama.sh pulls the model → Task 15
- §9 testing strategy (core unit tests, wiremock integration, frontend state + TTL tests) → Tasks 2–5 (core+app) and Tasks 9+11 (frontend)
- §10 completion criteria → Task 15 Step 3 + Step 4

**2. Placeholder scan:**

Every code block is complete runnable content. No `TBD`, `TODO`, `implement later`, or `add appropriate error handling`.

**3. Type consistency:**

- `Message` shape: `id: i64` in Rust ↔ `id: number` in TS. Fields aligned.
- `StreamChunk` tagged-union: Rust `#[serde(tag = "type", content = "value")]` emits `{"type":"Token","value":"…"}` for payload variants and `{"type":"Done"}` (no value key) for the unit variant. TS discriminated union matches: `{ type: "Token"; value: string } | { type: "Done" } | { type: "Error"; value: ... }`. The `type` field is the only discriminator actually read at runtime.
- `Role` enum: Rust `#[serde(rename_all = "lowercase")]` → serialises as `"user"`, `"assistant"`, `"system"`, matching TS literal types.
- `register()` signature `Builder<Wry> -> Builder<Wry>` unchanged from Phase 1.

**4. Scope check:**

Phase 2 is one coherent subsystem (Assistant shell substrate). No feature skills yet. Plan-able as one document; 15 tasks in the same granularity range as Phase 1 Foundation's 13 tasks.

---

## Plan deviations (to be appended by implementers, as per Phase 1 convention)

*(None yet. Implementers should append each deviation with reasoning.)*

## Plan deviation — Task 3 (`conversation.rs` clippy)

**Step 2 — `Some(c) = conn.query_row(...).ok()` pattern:**
The plan's code used `if let Some(c) = ...query_row(...).ok() { ... }`. Clippy's `clippy::match-result-ok` lint (a default warning, turned into error by CI's `-D warnings` flag) flags this as redundant. Fixed by matching the `Result` directly: `if let Ok(c) = conn.query_row(...) { ... }`. Semantics identical; one fewer lint violation. No test changes.

## Plan deviation — Task 6 (`crates/app/Cargo.toml` missing rusqlite)

**Step 3 — `Db` struct holds `rusqlite::Connection`:**
The plan's Task 1 listed `rusqlite.workspace = true` only under `crates/core` dependencies. But Task 6's `commands.rs` exposes `pub struct Db(pub Mutex<Connection>)` where `Connection` is `rusqlite::Connection`, so `manor-app` needs direct `rusqlite` access too. Added `rusqlite.workspace = true` to `crates/app/Cargo.toml` dependencies. Task 1's plan would have anticipated this had it modelled the full dep graph through Task 6.

## Plan deviation — Task 9 (`vite-env.d.ts`)

**Step 1 — PNG imports need a type declaration:**
Importing `.png` files in TypeScript requires an ambient module declaration or a Vite env typings file. The plan didn't spell this out. Added `apps/desktop/src/vite-env.d.ts` with a `/// <reference types="vite/client" />` directive to provide typings for static asset imports. Standard Vite + TypeScript practice.
