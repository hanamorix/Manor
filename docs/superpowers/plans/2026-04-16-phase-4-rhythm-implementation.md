# Phase 4 — v0.2 Rhythm Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add chores (with rotations), time blocks (with pattern detection), and the icon-sidebar navigation shell to Manor, implementing the v0.2 Rhythm design spec in a single feature branch.

**Architecture:** Two-layer Rust (DAL in `manor-core`, Tauri commands in `manor-app`), React + Zustand frontend. New `person`, `chore`, `chore_completion`, `rotation`, `time_block` tables via V4 migration. Navigation shell is a Zustand-backed view switcher; no client-side routing. AI nudges are pure SQL queries, no LLM.

**Tech Stack:** Rust + `rusqlite` + `rrule 0.13` for next-due computation; React 18 + TypeScript + Zustand for state; Tauri 2 commands as the IPC bridge.

**Branch:** `feature/phase-4-rhythm` (worktree at `/Users/hanamori/life-assistant/.worktrees/phase-4-rhythm`).

**Spec:** `docs/superpowers/specs/2026-04-16-phase-4-rhythm-design.md`

---

## File structure

**New files:**

| Path | Purpose |
|---|---|
| `crates/core/migrations/V4__rhythm.sql` | Five new tables (person, chore, chore_completion, rotation, time_block) |
| `crates/core/src/assistant/chore.rs` | Chore + ChoreCompletion + Rotation DAL |
| `crates/core/src/assistant/time_block.rs` | TimeBlock DAL + pattern detection + fairness queries |
| `crates/app/src/rhythm/mod.rs` | Module declaration |
| `crates/app/src/rhythm/commands.rs` | Tauri command wrappers for both features |
| `apps/desktop/src/lib/chores/ipc.ts` | Typed invoke wrappers + TS types for chores |
| `apps/desktop/src/lib/chores/state.ts` | Zustand store for chores |
| `apps/desktop/src/lib/chores/state.test.ts` | Vitest tests for the chores store |
| `apps/desktop/src/lib/timeblocks/ipc.ts` | Typed invoke wrappers + TS types for time blocks |
| `apps/desktop/src/lib/timeblocks/state.ts` | Zustand store for time blocks |
| `apps/desktop/src/lib/timeblocks/state.test.ts` | Vitest tests for the time blocks store |
| `apps/desktop/src/lib/nav.ts` | Zustand store for active view |
| `apps/desktop/src/components/Nav/Sidebar.tsx` | Icon sidebar component |
| `apps/desktop/src/components/Today/ChoresCard.tsx` | Today view — due-today chores |
| `apps/desktop/src/components/Today/TimeBlocksCard.tsx` | Today view — time blocks with pattern nudge |
| `apps/desktop/src/components/Chores/ChoresView.tsx` | Full chores management view |
| `apps/desktop/src/components/Chores/ChoreDrawer.tsx` | Create / edit drawer |
| `apps/desktop/src/components/TimeBlocks/TimeBlocksView.tsx` | Full time blocks management view |
| `apps/desktop/src/components/TimeBlocks/BlockDrawer.tsx` | Create / edit drawer |

**Modified files:**

| Path | Change |
|---|---|
| `crates/core/Cargo.toml` | Add `rrule.workspace = true` |
| `crates/core/src/assistant/mod.rs` | `pub mod chore; pub mod time_block;` |
| `crates/core/src/assistant/db.rs` | Update migration test to cover V4 tables |
| `crates/app/src/lib.rs` | `pub mod rhythm;`, register new commands |
| `apps/desktop/src/App.tsx` | Wrap with sidebar layout, switch active view |
| `apps/desktop/src/components/Today/Today.tsx` | Include ChoresCard + TimeBlocksCard, hydrate stores |

---

## Worktree setup

Before Task 1, create a worktree so this work is isolated from `main`.

```bash
cd /Users/hanamori/life-assistant
git worktree add .worktrees/phase-4-rhythm -b feature/phase-4-rhythm main
cd .worktrees/phase-4-rhythm
```

Run all subsequent commands from inside the worktree.

---

### Task 1: V4 migration

**Files:**
- Create: `crates/core/migrations/V4__rhythm.sql`
- Modify: `crates/core/src/assistant/db.rs` (migration test)

- [ ] **Step 1: Write the migration SQL**

Create `crates/core/migrations/V4__rhythm.sql`:

```sql
CREATE TABLE person (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    created_at  INTEGER NOT NULL
);

CREATE TABLE chore (
    id          INTEGER PRIMARY KEY,
    title       TEXT    NOT NULL,
    emoji       TEXT    NOT NULL DEFAULT '🧹',
    rrule       TEXT    NOT NULL,
    next_due    INTEGER NOT NULL,
    rotation    TEXT    NOT NULL DEFAULT 'none',
    active      INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL,
    deleted_at  INTEGER
);

CREATE INDEX idx_chore_next_due ON chore(next_due) WHERE deleted_at IS NULL AND active = 1;

CREATE TABLE chore_completion (
    id              INTEGER PRIMARY KEY,
    chore_id        INTEGER NOT NULL REFERENCES chore(id) ON DELETE CASCADE,
    completed_at    INTEGER NOT NULL,
    completed_by    INTEGER REFERENCES person(id),
    created_at      INTEGER NOT NULL
);

CREATE INDEX idx_chore_completion_chore ON chore_completion(chore_id);
CREATE INDEX idx_chore_completion_person ON chore_completion(completed_by) WHERE completed_by IS NOT NULL;

CREATE TABLE rotation (
    id          INTEGER PRIMARY KEY,
    chore_id    INTEGER NOT NULL REFERENCES chore(id) ON DELETE CASCADE,
    person_id   INTEGER NOT NULL REFERENCES person(id),
    position    INTEGER NOT NULL,
    current     INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);

CREATE INDEX idx_rotation_chore ON rotation(chore_id);

CREATE TABLE time_block (
    id                          INTEGER PRIMARY KEY,
    title                       TEXT    NOT NULL,
    kind                        TEXT    NOT NULL,
    date                        INTEGER NOT NULL,
    start_time                  TEXT    NOT NULL,
    end_time                    TEXT    NOT NULL,
    rrule                       TEXT,
    is_pattern                  INTEGER NOT NULL DEFAULT 0,
    pattern_nudge_dismissed_at  INTEGER,
    created_at                  INTEGER NOT NULL,
    deleted_at                  INTEGER
);

CREATE INDEX idx_time_block_date ON time_block(date) WHERE deleted_at IS NULL;
```

- [ ] **Step 2: Update the migration test to check V4 tables**

Open `crates/core/src/assistant/db.rs` and replace the `for table in [...]` loop in `init_creates_db_file_and_runs_migrations` with:

```rust
        // Migrations ran — all expected tables should exist.
        for table in [
            "conversation",
            "message",
            "proposal",
            "task",
            "calendar_account",
            "event",
            "person",
            "chore",
            "chore_completion",
            "rotation",
            "time_block",
        ] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "table {table} should exist after migrations");
        }
```

- [ ] **Step 3: Run the migration test**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
cargo test -p manor-core db:: 2>&1 | grep -E "^test result|FAILED"
```

Expected: `test result: ok. 2 passed; 0 failed`

- [ ] **Step 4: Commit**

```bash
git add crates/core/migrations/V4__rhythm.sql crates/core/src/assistant/db.rs
git commit -m "feat(core): V4 migration — rhythm tables (person, chore, rotation, time_block)"
```

---

### Task 2: Chore DAL

**Files:**
- Modify: `crates/core/Cargo.toml` (add rrule)
- Create: `crates/core/src/assistant/chore.rs`
- Modify: `crates/core/src/assistant/mod.rs` (add `pub mod chore;`)

- [ ] **Step 1: Add rrule to manor-core**

Edit `crates/core/Cargo.toml`. Under `[dependencies]`, add after `chrono-tz.workspace = true`:

```toml
rrule.workspace = true
```

- [ ] **Step 2: Write the failing tests**

Create `crates/core/src/assistant/chore.rs`:

```rust
//! Chores — recurring household tasks with rotation support.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rrule::{RRuleSet, Tz as RruleTz};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chore {
    pub id: i64,
    pub title: String,
    pub emoji: String,
    pub rrule: String,
    pub next_due: i64,
    pub rotation: String,
    pub active: bool,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChoreCompletion {
    pub id: i64,
    pub chore_id: i64,
    pub completed_at: i64,
    pub completed_by: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RotationMember {
    pub id: i64,
    pub chore_id: i64,
    pub person_id: i64,
    pub position: i32,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FairnessNudge {
    pub chore_id: i64,
    pub chore_title: String,
    pub person_id: i64,
    pub person_name: String,
    pub days_ago: u32,
}

impl Chore {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            emoji: row.get("emoji")?,
            rrule: row.get("rrule")?,
            next_due: row.get("next_due")?,
            rotation: row.get("rotation")?,
            active: row.get::<_, i64>("active")? != 0,
            created_at: row.get("created_at")?,
            deleted_at: row.get("deleted_at")?,
        })
    }
}

impl ChoreCompletion {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            chore_id: row.get("chore_id")?,
            completed_at: row.get("completed_at")?,
            completed_by: row.get("completed_by")?,
            created_at: row.get("created_at")?,
        })
    }
}

impl RotationMember {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            chore_id: row.get("chore_id")?,
            person_id: row.get("person_id")?,
            position: row.get("position")?,
            current: row.get::<_, i64>("current")? != 0,
        })
    }
}

/// Compute the next occurrence of an RRULE strictly after `after_ms`.
/// Used to advance `next_due` after completion or skip.
pub fn next_occurrence_after(rrule_str: &str, after_ms: i64) -> Result<i64> {
    let after_secs = after_ms / 1000;
    let sub_nanos = ((after_ms % 1000) * 1_000_000).max(0) as u32;
    let after_dt = DateTime::<Utc>::from_timestamp(after_secs, sub_nanos)
        .ok_or_else(|| anyhow::anyhow!("invalid after_ms"))?;
    let rule_block = format!(
        "DTSTART:{}\nRRULE:{}",
        after_dt.format("%Y%m%dT%H%M%SZ"),
        rrule_str
    );
    let rset = RRuleSet::from_str(&rule_block)?;
    let after_rrule = after_dt.with_timezone(&RruleTz::UTC);
    let result = rset.after(after_rrule).all(2);
    let occ = result
        .dates
        .into_iter()
        .find(|d| d.timestamp_millis() > after_ms)
        .ok_or_else(|| anyhow::anyhow!("no next occurrence"))?;
    Ok(occ.with_timezone(&Utc).timestamp_millis())
}

/// Insert a person (minimal household member row). Returns new row id.
pub fn insert_person(conn: &Connection, name: &str) -> Result<i64> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO person (name, created_at) VALUES (?1, ?2)",
        params![name, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Create a new chore. `rrule` is an RFC 5545 RRULE string (without the
/// `RRULE:` prefix), e.g. `FREQ=WEEKLY`. `first_due` is the initial next_due
/// timestamp in unix ms.
pub fn insert(
    conn: &Connection,
    title: &str,
    emoji: &str,
    rrule: &str,
    first_due: i64,
    rotation: &str,
) -> Result<i64> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO chore (title, emoji, rrule, next_due, rotation, active, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
        params![title, emoji, rrule, first_due, rotation, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Chores whose next_due is ≤ end_of_today_ms, active, not deleted.
pub fn list_due_today(conn: &Connection, end_of_today_ms: i64) -> Result<Vec<Chore>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, emoji, rrule, next_due, rotation, active, created_at, deleted_at
         FROM chore
         WHERE active = 1 AND deleted_at IS NULL AND next_due <= ?1
         ORDER BY next_due",
    )?;
    let rows = stmt
        .query_map([end_of_today_ms], Chore::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// All active, non-deleted chores, sorted by next_due.
pub fn list_all(conn: &Connection) -> Result<Vec<Chore>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, emoji, rrule, next_due, rotation, active, created_at, deleted_at
         FROM chore
         WHERE deleted_at IS NULL
         ORDER BY next_due",
    )?;
    let rows = stmt
        .query_map([], Chore::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Fetch a single chore by id (including soft-deleted).
pub fn get(conn: &Connection, id: i64) -> Result<Option<Chore>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, emoji, rrule, next_due, rotation, active, created_at, deleted_at
         FROM chore WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], Chore::from_row)?;
    Ok(rows.next().transpose()?)
}

pub fn update(
    conn: &Connection,
    id: i64,
    title: &str,
    emoji: &str,
    rrule: &str,
    rotation: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE chore SET title = ?1, emoji = ?2, rrule = ?3, rotation = ?4
         WHERE id = ?5",
        params![title, emoji, rrule, rotation, id],
    )?;
    Ok(())
}

/// Soft-delete a chore.
pub fn soft_delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE chore SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Complete a chore: record a chore_completion, advance next_due using the
/// original next_due as the base (predictable schedule), advance rotation.
pub fn complete(
    conn: &Connection,
    chore_id: i64,
    completed_by: Option<i64>,
) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    let chore = get(conn, chore_id)?
        .ok_or_else(|| anyhow::anyhow!("chore {chore_id} not found"))?;

    conn.execute(
        "INSERT INTO chore_completion (chore_id, completed_at, completed_by, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![chore_id, now, completed_by, now],
    )?;

    let next = next_occurrence_after(&chore.rrule, chore.next_due)?;
    conn.execute(
        "UPDATE chore SET next_due = ?1 WHERE id = ?2",
        params![next, chore_id],
    )?;

    if chore.rotation == "round_robin" {
        advance_rotation(conn, chore_id)?;
    }
    Ok(())
}

/// Skip a chore: advance next_due without recording a completion.
pub fn skip(conn: &Connection, chore_id: i64) -> Result<()> {
    let chore = get(conn, chore_id)?
        .ok_or_else(|| anyhow::anyhow!("chore {chore_id} not found"))?;
    let next = next_occurrence_after(&chore.rrule, chore.next_due)?;
    conn.execute(
        "UPDATE chore SET next_due = ?1 WHERE id = ?2",
        params![next, chore_id],
    )?;
    Ok(())
}

/// List the last N completions for a chore, newest first.
pub fn list_completions(
    conn: &Connection,
    chore_id: i64,
    limit: u32,
) -> Result<Vec<ChoreCompletion>> {
    let mut stmt = conn.prepare(
        "SELECT id, chore_id, completed_at, completed_by, created_at
         FROM chore_completion
         WHERE chore_id = ?1
         ORDER BY completed_at DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![chore_id, limit], ChoreCompletion::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Insert a rotation member for a chore at the given position. The first
/// member (position 0) is current by default.
pub fn insert_rotation_member(
    conn: &Connection,
    chore_id: i64,
    person_id: i64,
    position: i32,
) -> Result<i64> {
    let now = Utc::now().timestamp_millis();
    let current = if position == 0 { 1 } else { 0 };
    conn.execute(
        "INSERT INTO rotation (chore_id, person_id, position, current, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![chore_id, person_id, position, current, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Fetch rotation members for a chore, ordered by position.
pub fn list_rotation(conn: &Connection, chore_id: i64) -> Result<Vec<RotationMember>> {
    let mut stmt = conn.prepare(
        "SELECT id, chore_id, person_id, position, current
         FROM rotation WHERE chore_id = ?1 ORDER BY position",
    )?;
    let rows = stmt
        .query_map([chore_id], RotationMember::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Advance the current pointer in a round-robin rotation to the next position,
/// wrapping around.
pub fn advance_rotation(conn: &Connection, chore_id: i64) -> Result<()> {
    let members = list_rotation(conn, chore_id)?;
    if members.len() <= 1 {
        return Ok(());
    }
    let current_idx = members.iter().position(|m| m.current).unwrap_or(0);
    let next_idx = (current_idx + 1) % members.len();

    conn.execute(
        "UPDATE rotation SET current = 0 WHERE id = ?1",
        [members[current_idx].id],
    )?;
    conn.execute(
        "UPDATE rotation SET current = 1 WHERE id = ?1",
        [members[next_idx].id],
    )?;
    Ok(())
}

/// For each active chore with a rotation, flag the assignee whose days-since-
/// last-completion is > 2× the group median (with a 7-day floor to avoid
/// noise when everyone's recent). Returns at most one `FairnessNudge` per chore.
pub fn check_fairness(conn: &Connection, now_ms: i64) -> Result<Vec<FairnessNudge>> {
    let mut chore_stmt = conn.prepare(
        "SELECT c.id, c.title FROM chore c
         WHERE c.active = 1 AND c.deleted_at IS NULL AND c.rotation != 'none'",
    )?;
    let chores: Vec<(i64, String)> = chore_stmt
        .query_map([], |r| Ok((r.get("id")?, r.get("title")?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut nudges = Vec::new();
    for (chore_id, chore_title) in chores {
        let mut member_stmt = conn.prepare(
            "SELECT r.person_id AS pid, p.name AS name,
                    (SELECT MAX(cc.completed_at) FROM chore_completion cc
                     WHERE cc.chore_id = r.chore_id AND cc.completed_by = r.person_id) AS last_ms
             FROM rotation r
             JOIN person p ON p.id = r.person_id
             WHERE r.chore_id = ?1",
        )?;
        let members: Vec<(i64, String, Option<i64>)> = member_stmt
            .query_map([chore_id], |r| {
                Ok((r.get("pid")?, r.get("name")?, r.get("last_ms")?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        if members.len() < 2 {
            continue;
        }

        let days: Vec<(i64, String, u32)> = members
            .into_iter()
            .map(|(pid, name, last)| {
                let d = match last {
                    Some(ms) => ((now_ms - ms) / 86_400_000).max(0) as u32,
                    None => 9999,
                };
                (pid, name, d)
            })
            .collect();

        let mut days_sorted: Vec<u32> = days.iter().map(|(_, _, d)| *d).collect();
        days_sorted.sort_unstable();
        let median = days_sorted[days_sorted.len() / 2];
        let threshold = (median.saturating_mul(2)).max(7);

        if let Some((pid, name, d)) = days
            .into_iter()
            .filter(|(_, _, d)| *d > threshold)
            .max_by_key(|(_, _, d)| *d)
        {
            nudges.push(FairnessNudge {
                chore_id,
                chore_title: chore_title.clone(),
                person_id: pid,
                person_name: name,
                days_ago: d.min(9998),
            });
        }
    }
    Ok(nudges)
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
        let due = Utc::now().timestamp_millis();
        let id = insert(&conn, "Bins", "🗑️", "FREQ=WEEKLY", due, "none").unwrap();
        assert!(id > 0);
        let got = get(&conn, id).unwrap().unwrap();
        assert_eq!(got.title, "Bins");
        assert_eq!(got.rotation, "none");
        assert!(got.active);
    }

    #[test]
    fn list_due_today_includes_past_and_today_excludes_future() {
        let (_d, conn) = fresh_conn();
        let now = Utc::now().timestamp_millis();
        let end_of_today = now + 3_600_000; // ~1 hr in future
        // Overdue
        insert(&conn, "overdue", "🧹", "FREQ=WEEKLY", now - 86_400_000, "none").unwrap();
        // Due today
        insert(&conn, "today", "🧹", "FREQ=WEEKLY", now, "none").unwrap();
        // Future
        insert(&conn, "later", "🧹", "FREQ=WEEKLY", now + 7 * 86_400_000, "none").unwrap();

        let due = list_due_today(&conn, end_of_today).unwrap();
        let titles: Vec<&str> = due.iter().map(|c| c.title.as_str()).collect();
        assert!(titles.contains(&"overdue"));
        assert!(titles.contains(&"today"));
        assert!(!titles.contains(&"later"));
    }

    #[test]
    fn soft_delete_excludes_from_list() {
        let (_d, conn) = fresh_conn();
        let now = Utc::now().timestamp_millis();
        let id = insert(&conn, "Gone", "🧹", "FREQ=WEEKLY", now, "none").unwrap();
        soft_delete(&conn, id).unwrap();
        assert_eq!(list_all(&conn).unwrap().len(), 0);
        // But it still exists
        assert!(get(&conn, id).unwrap().unwrap().deleted_at.is_some());
    }

    #[test]
    fn next_occurrence_after_weekly_advances_seven_days() {
        let base = 1_776_259_200_000i64; // 2026-04-15T00:00:00Z in ms
        let next = next_occurrence_after("FREQ=WEEKLY", base).unwrap();
        // 7 days later in ms
        assert_eq!(next, base + 7 * 86_400_000);
    }

    #[test]
    fn complete_inserts_completion_and_advances_next_due() {
        let (_d, conn) = fresh_conn();
        let start = 1_776_259_200_000i64;
        let id = insert(&conn, "Bins", "🗑️", "FREQ=WEEKLY", start, "none").unwrap();
        complete(&conn, id, None).unwrap();

        let updated = get(&conn, id).unwrap().unwrap();
        assert_eq!(updated.next_due, start + 7 * 86_400_000);

        let comps = list_completions(&conn, id, 10).unwrap();
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].chore_id, id);
        assert!(comps[0].completed_by.is_none());
    }

    #[test]
    fn complete_with_person_records_completed_by() {
        let (_d, conn) = fresh_conn();
        let pid = insert_person(&conn, "Rosa").unwrap();
        let id = insert(&conn, "Bins", "🗑️", "FREQ=WEEKLY", 1_776_259_200_000, "none").unwrap();
        complete(&conn, id, Some(pid)).unwrap();

        let comps = list_completions(&conn, id, 10).unwrap();
        assert_eq!(comps[0].completed_by, Some(pid));
    }

    #[test]
    fn skip_advances_next_due_without_completion() {
        let (_d, conn) = fresh_conn();
        let start = 1_776_259_200_000i64;
        let id = insert(&conn, "Bins", "🗑️", "FREQ=WEEKLY", start, "none").unwrap();
        skip(&conn, id).unwrap();

        let updated = get(&conn, id).unwrap().unwrap();
        assert_eq!(updated.next_due, start + 7 * 86_400_000);
        assert_eq!(list_completions(&conn, id, 10).unwrap().len(), 0);
    }

    #[test]
    fn round_robin_rotation_advances_on_complete() {
        let (_d, conn) = fresh_conn();
        let a = insert_person(&conn, "A").unwrap();
        let b = insert_person(&conn, "B").unwrap();
        let c = insert_person(&conn, "C").unwrap();
        let chore_id = insert(
            &conn,
            "Bins",
            "🗑️",
            "FREQ=WEEKLY",
            1_776_259_200_000,
            "round_robin",
        )
        .unwrap();
        insert_rotation_member(&conn, chore_id, a, 0).unwrap();
        insert_rotation_member(&conn, chore_id, b, 1).unwrap();
        insert_rotation_member(&conn, chore_id, c, 2).unwrap();

        let members = list_rotation(&conn, chore_id).unwrap();
        assert_eq!(members[0].current, true);

        complete(&conn, chore_id, Some(a)).unwrap();
        let after_one = list_rotation(&conn, chore_id).unwrap();
        assert_eq!(after_one[0].current, false);
        assert_eq!(after_one[1].current, true);

        complete(&conn, chore_id, Some(b)).unwrap();
        complete(&conn, chore_id, Some(c)).unwrap();
        let after_three = list_rotation(&conn, chore_id).unwrap();
        // Should have wrapped back to A
        assert_eq!(after_three[0].current, true);
    }

    #[test]
    fn update_changes_fields() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "Old", "🧹", "FREQ=WEEKLY", 1_776_259_200_000, "none").unwrap();
        update(&conn, id, "New", "🧽", "FREQ=DAILY", "round_robin").unwrap();
        let got = get(&conn, id).unwrap().unwrap();
        assert_eq!(got.title, "New");
        assert_eq!(got.emoji, "🧽");
        assert_eq!(got.rrule, "FREQ=DAILY");
        assert_eq!(got.rotation, "round_robin");
    }

    #[test]
    fn check_fairness_flags_single_outlier() {
        let (_d, conn) = fresh_conn();
        let a = insert_person(&conn, "A").unwrap();
        let b = insert_person(&conn, "B").unwrap();

        let start = 1_776_259_200_000i64;
        let chore_id = insert(&conn, "Bins", "🗑️", "FREQ=WEEKLY", start, "round_robin").unwrap();
        insert_rotation_member(&conn, chore_id, a, 0).unwrap();
        insert_rotation_member(&conn, chore_id, b, 1).unwrap();

        // A completed 1 day ago, B completed 30 days ago.
        let now = start + 30 * 86_400_000;
        conn.execute(
            "INSERT INTO chore_completion (chore_id, completed_at, completed_by, created_at)
             VALUES (?1, ?2, ?3, ?2)",
            params![chore_id, now - 86_400_000, a],
        ).unwrap();
        conn.execute(
            "INSERT INTO chore_completion (chore_id, completed_at, completed_by, created_at)
             VALUES (?1, ?2, ?3, ?2)",
            params![chore_id, now - 30 * 86_400_000, b],
        ).unwrap();

        let nudges = check_fairness(&conn, now).unwrap();
        assert_eq!(nudges.len(), 1);
        assert_eq!(nudges[0].person_name, "B");
    }

    #[test]
    fn check_fairness_empty_when_distribution_even() {
        let (_d, conn) = fresh_conn();
        let a = insert_person(&conn, "A").unwrap();
        let b = insert_person(&conn, "B").unwrap();
        let start = 1_776_259_200_000i64;
        let chore_id = insert(&conn, "Bins", "🗑️", "FREQ=WEEKLY", start, "round_robin").unwrap();
        insert_rotation_member(&conn, chore_id, a, 0).unwrap();
        insert_rotation_member(&conn, chore_id, b, 1).unwrap();

        let now = start;
        conn.execute(
            "INSERT INTO chore_completion (chore_id, completed_at, completed_by, created_at)
             VALUES (?1, ?2, ?3, ?2)",
            params![chore_id, now - 2 * 86_400_000, a],
        ).unwrap();
        conn.execute(
            "INSERT INTO chore_completion (chore_id, completed_at, completed_by, created_at)
             VALUES (?1, ?2, ?3, ?2)",
            params![chore_id, now - 3 * 86_400_000, b],
        ).unwrap();

        let nudges = check_fairness(&conn, now).unwrap();
        assert!(nudges.is_empty());
    }
}
```

- [ ] **Step 3: Register the module**

Edit `crates/core/src/assistant/mod.rs` and add the line:

```rust
pub mod chore;
```

Place it alphabetically with the existing `pub mod calendar_account; pub mod conversation; ...` declarations.

- [ ] **Step 4: Run the tests**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
cargo test -p manor-core chore:: 2>&1 | grep -E "^test result|FAILED"
```

Expected: `test result: ok. 12 passed; 0 failed`

- [ ] **Step 5: Commit**

```bash
git add crates/core/Cargo.toml crates/core/src/assistant/chore.rs crates/core/src/assistant/mod.rs
git commit -m "feat(core): chore DAL — insert/list/complete/skip + rotation + fairness (TDD)"
```

---

### Task 3: Time block DAL

**Files:**
- Create: `crates/core/src/assistant/time_block.rs`
- Modify: `crates/core/src/assistant/mod.rs` (add `pub mod time_block;`)

- [ ] **Step 1: Write the failing tests + implementation**

Create `crates/core/src/assistant/time_block.rs`:

```rust
//! Time blocks — focus/errands/admin/dnd blocks on the calendar.
//! Pattern detection promotes repeated manual entries to recurring blocks.

use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc, Weekday};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeBlock {
    pub id: i64,
    pub title: String,
    pub kind: String,
    pub date: i64,
    pub start_time: String,
    pub end_time: String,
    pub rrule: Option<String>,
    pub is_pattern: bool,
    pub pattern_nudge_dismissed_at: Option<i64>,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatternSuggestion {
    pub trigger_id: i64,
    pub kind: String,
    pub start_time: String,
    pub end_time: String,
    pub weekday: String,
    pub count: u32,
}

impl TimeBlock {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            kind: row.get("kind")?,
            date: row.get("date")?,
            start_time: row.get("start_time")?,
            end_time: row.get("end_time")?,
            rrule: row.get("rrule")?,
            is_pattern: row.get::<_, i64>("is_pattern")? != 0,
            pattern_nudge_dismissed_at: row.get("pattern_nudge_dismissed_at")?,
            created_at: row.get("created_at")?,
            deleted_at: row.get("deleted_at")?,
        })
    }
}

const NUDGE_SUPPRESS_WINDOW_MS: i64 = 14 * 86_400_000;
const PATTERN_LOOKBACK_MS: i64 = 42 * 86_400_000; // 6 weeks
const PATTERN_MIN_MATCHES: u32 = 3;

pub fn insert(
    conn: &Connection,
    title: &str,
    kind: &str,
    date_ms: i64,
    start_time: &str,
    end_time: &str,
) -> Result<i64> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO time_block (title, kind, date, start_time, end_time, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![title, kind, date_ms, start_time, end_time, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<TimeBlock>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, kind, date, start_time, end_time, rrule, is_pattern,
                pattern_nudge_dismissed_at, created_at, deleted_at
         FROM time_block WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], TimeBlock::from_row)?;
    Ok(rows.next().transpose()?)
}

pub fn list_for_date(conn: &Connection, date_ms: i64) -> Result<Vec<TimeBlock>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, kind, date, start_time, end_time, rrule, is_pattern,
                pattern_nudge_dismissed_at, created_at, deleted_at
         FROM time_block
         WHERE deleted_at IS NULL AND date = ?1
         ORDER BY start_time",
    )?;
    let rows = stmt
        .query_map([date_ms], TimeBlock::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_for_week(conn: &Connection, week_start_ms: i64) -> Result<Vec<TimeBlock>> {
    let week_end = week_start_ms + 7 * 86_400_000;
    let mut stmt = conn.prepare(
        "SELECT id, title, kind, date, start_time, end_time, rrule, is_pattern,
                pattern_nudge_dismissed_at, created_at, deleted_at
         FROM time_block
         WHERE deleted_at IS NULL AND date >= ?1 AND date < ?2
         ORDER BY date, start_time",
    )?;
    let rows = stmt
        .query_map(params![week_start_ms, week_end], TimeBlock::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_recurring(conn: &Connection) -> Result<Vec<TimeBlock>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, kind, date, start_time, end_time, rrule, is_pattern,
                pattern_nudge_dismissed_at, created_at, deleted_at
         FROM time_block
         WHERE deleted_at IS NULL AND is_pattern = 1
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map([], TimeBlock::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn update(
    conn: &Connection,
    id: i64,
    title: &str,
    kind: &str,
    date_ms: i64,
    start_time: &str,
    end_time: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE time_block SET title = ?1, kind = ?2, date = ?3, start_time = ?4, end_time = ?5
         WHERE id = ?6",
        params![title, kind, date_ms, start_time, end_time, id],
    )?;
    Ok(())
}

pub fn soft_delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE time_block SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Promote a one-off block to recurring. Sets is_pattern=1 and rrule.
pub fn promote_to_pattern(conn: &Connection, id: i64, rrule: &str) -> Result<()> {
    conn.execute(
        "UPDATE time_block SET is_pattern = 1, rrule = ?1 WHERE id = ?2",
        params![rrule, id],
    )?;
    Ok(())
}

/// Mark that the user dismissed the pattern nudge for a specific block.
pub fn dismiss_pattern_nudge(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE time_block SET pattern_nudge_dismissed_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

fn weekday_name(wd: Weekday) -> &'static str {
    match wd {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

/// Check whether a newly-inserted block completes a pattern: same `kind`,
/// same `start_time`, same `end_time`, same weekday, at least 3 occurrences
/// (including the trigger) in the last 6 weeks. Suppressed for 14 days after
/// dismissal.
///
/// Returns `Some(PatternSuggestion)` if the nudge should be shown, else `None`.
pub fn check_pattern(
    conn: &Connection,
    trigger_id: i64,
    now_ms: i64,
) -> Result<Option<PatternSuggestion>> {
    let trigger = match get(conn, trigger_id)? {
        Some(t) => t,
        None => return Ok(None),
    };
    if trigger.is_pattern {
        return Ok(None);
    }
    if let Some(dismissed_at) = trigger.pattern_nudge_dismissed_at {
        if now_ms - dismissed_at < NUDGE_SUPPRESS_WINDOW_MS {
            return Ok(None);
        }
    }

    let weekday = Utc
        .timestamp_millis_opt(trigger.date)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid trigger.date"))?
        .weekday();
    let lookback_start = now_ms - PATTERN_LOOKBACK_MS;

    // Find blocks in the lookback window with same kind + times on the same weekday.
    let mut stmt = conn.prepare(
        "SELECT id, date FROM time_block
         WHERE deleted_at IS NULL
           AND is_pattern = 0
           AND kind = ?1 AND start_time = ?2 AND end_time = ?3
           AND date >= ?4",
    )?;
    let rows = stmt.query_map(
        params![
            trigger.kind,
            trigger.start_time,
            trigger.end_time,
            lookback_start,
        ],
        |r| {
            let id: i64 = r.get("id")?;
            let date: i64 = r.get("date")?;
            Ok((id, date))
        },
    )?;

    let mut count: u32 = 0;
    for row in rows {
        let (_id, date) = row?;
        let wd = Utc
            .timestamp_millis_opt(date)
            .single()
            .ok_or_else(|| anyhow::anyhow!("invalid row date"))?
            .weekday();
        if wd == weekday {
            count += 1;
        }
    }
    if count >= PATTERN_MIN_MATCHES {
        Ok(Some(PatternSuggestion {
            trigger_id,
            kind: trigger.kind,
            start_time: trigger.start_time,
            end_time: trigger.end_time,
            weekday: weekday_name(weekday).to_string(),
            count,
        }))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use chrono::TimeZone;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn day_ms(y: i32, m: u32, d: u32) -> i64 {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap().timestamp_millis()
    }

    #[test]
    fn insert_and_get_round_trip() {
        let (_d, conn) = fresh_conn();
        let date = day_ms(2026, 4, 15);
        let id = insert(&conn, "Deep work", "focus", date, "09:00", "11:00").unwrap();
        let got = get(&conn, id).unwrap().unwrap();
        assert_eq!(got.title, "Deep work");
        assert_eq!(got.kind, "focus");
        assert!(!got.is_pattern);
    }

    #[test]
    fn list_for_date_filters_correctly() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "A", "focus", day_ms(2026, 4, 15), "09:00", "11:00").unwrap();
        insert(&conn, "B", "admin", day_ms(2026, 4, 15), "14:00", "15:00").unwrap();
        insert(&conn, "C", "focus", day_ms(2026, 4, 16), "09:00", "11:00").unwrap();

        let today = list_for_date(&conn, day_ms(2026, 4, 15)).unwrap();
        assert_eq!(today.len(), 2);
    }

    #[test]
    fn soft_delete_hides_from_lists() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "A", "focus", day_ms(2026, 4, 15), "09:00", "11:00").unwrap();
        soft_delete(&conn, id).unwrap();
        assert_eq!(list_for_date(&conn, day_ms(2026, 4, 15)).unwrap().len(), 0);
    }

    #[test]
    fn promote_to_pattern_sets_flags() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "A", "focus", day_ms(2026, 4, 15), "09:00", "11:00").unwrap();
        promote_to_pattern(&conn, id, "FREQ=WEEKLY;BYDAY=TU").unwrap();
        let got = get(&conn, id).unwrap().unwrap();
        assert!(got.is_pattern);
        assert_eq!(got.rrule.as_deref(), Some("FREQ=WEEKLY;BYDAY=TU"));
    }

    #[test]
    fn check_pattern_fires_when_three_same_weekday_matches() {
        let (_d, conn) = fresh_conn();
        // Three consecutive Tuesdays, same focus block 09-11.
        insert(&conn, "Focus", "focus", day_ms(2026, 3, 31), "09:00", "11:00").unwrap();
        insert(&conn, "Focus", "focus", day_ms(2026, 4, 7), "09:00", "11:00").unwrap();
        let trigger = insert(&conn, "Focus", "focus", day_ms(2026, 4, 14), "09:00", "11:00").unwrap();

        let now = day_ms(2026, 4, 14) + 3_600_000;
        let sugg = check_pattern(&conn, trigger, now).unwrap();
        let s = sugg.expect("pattern suggestion expected");
        assert_eq!(s.weekday, "Tuesday");
        assert!(s.count >= 3);
    }

    #[test]
    fn check_pattern_suppresses_when_dismissed_recently() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "Focus", "focus", day_ms(2026, 3, 31), "09:00", "11:00").unwrap();
        insert(&conn, "Focus", "focus", day_ms(2026, 4, 7), "09:00", "11:00").unwrap();
        let trigger = insert(&conn, "Focus", "focus", day_ms(2026, 4, 14), "09:00", "11:00").unwrap();
        dismiss_pattern_nudge(&conn, trigger).unwrap();

        let now = day_ms(2026, 4, 14) + 3_600_000;
        assert!(check_pattern(&conn, trigger, now).unwrap().is_none());
    }

    #[test]
    fn check_pattern_no_match_when_weekdays_differ() {
        let (_d, conn) = fresh_conn();
        // Different weekdays.
        insert(&conn, "Focus", "focus", day_ms(2026, 3, 31), "09:00", "11:00").unwrap(); // Tue
        insert(&conn, "Focus", "focus", day_ms(2026, 4, 1), "09:00", "11:00").unwrap(); // Wed
        let trigger = insert(&conn, "Focus", "focus", day_ms(2026, 4, 14), "09:00", "11:00").unwrap();

        let now = day_ms(2026, 4, 14) + 3_600_000;
        // Only trigger (Tue) + 3/31 (Tue) = 2, < 3.
        assert!(check_pattern(&conn, trigger, now).unwrap().is_none());
    }

    #[test]
    fn list_recurring_only_patterns() {
        let (_d, conn) = fresh_conn();
        let a = insert(&conn, "One-off", "focus", day_ms(2026, 4, 15), "09:00", "11:00").unwrap();
        let b = insert(&conn, "Weekly", "focus", day_ms(2026, 4, 15), "14:00", "15:00").unwrap();
        promote_to_pattern(&conn, b, "FREQ=WEEKLY;BYDAY=TU").unwrap();

        let patterns = list_recurring(&conn).unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].id, b);
        let _ = a;
    }
}
```

- [ ] **Step 2: Register the module**

Edit `crates/core/src/assistant/mod.rs` and add `pub mod time_block;` (keeping alphabetical order).

- [ ] **Step 3: Run the tests**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
cargo test -p manor-core time_block:: 2>&1 | grep -E "^test result|FAILED"
```

Expected: `test result: ok. 8 passed; 0 failed`

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/assistant/time_block.rs crates/core/src/assistant/mod.rs
git commit -m "feat(core): time_block DAL + pattern detection (TDD)"
```

---

### Task 4: Rhythm Tauri commands

**Files:**
- Create: `crates/app/src/rhythm/mod.rs`
- Create: `crates/app/src/rhythm/commands.rs`
- Modify: `crates/app/src/lib.rs`

- [ ] **Step 1: Create the rhythm module**

Create `crates/app/src/rhythm/mod.rs`:

```rust
//! Rhythm feature — Tauri commands for chores and time blocks.

pub mod commands;
```

- [ ] **Step 2: Write the commands**

Create `crates/app/src/rhythm/commands.rs`:

```rust
//! Tauri commands for the Rhythm feature (chores + time blocks).

use crate::assistant::commands::Db;
use chrono::{Local, Utc};
use manor_core::assistant::{
    chore::{self, Chore, ChoreCompletion, FairnessNudge, RotationMember},
    time_block::{self, PatternSuggestion, TimeBlock},
};
use tauri::State;

fn end_of_today_ms() -> i64 {
    let now = Local::now();
    let end = now
        .date_naive()
        .and_hms_opt(23, 59, 59)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap();
    end.with_timezone(&Utc).timestamp_millis()
}

fn today_midnight_utc_ms() -> i64 {
    let now = Local::now();
    now.date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}

// ---------- Chore commands ----------

#[tauri::command]
pub fn list_chores_due_today(state: State<'_, Db>) -> Result<Vec<Chore>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::list_due_today(&conn, end_of_today_ms()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_all_chores(state: State<'_, Db>) -> Result<Vec<Chore>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::list_all(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct CreateChoreArgs {
    pub title: String,
    pub emoji: String,
    pub rrule: String,
    #[serde(rename = "firstDue")]
    pub first_due: i64,
    pub rotation: String,
}

#[tauri::command]
pub fn create_chore(state: State<'_, Db>, args: CreateChoreArgs) -> Result<Chore, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let id = chore::insert(
        &conn,
        &args.title,
        &args.emoji,
        &args.rrule,
        args.first_due,
        &args.rotation,
    )
    .map_err(|e| e.to_string())?;
    chore::get(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "failed to fetch new chore".to_string())
}

#[derive(serde::Deserialize)]
pub struct UpdateChoreArgs {
    pub id: i64,
    pub title: String,
    pub emoji: String,
    pub rrule: String,
    pub rotation: String,
}

#[tauri::command]
pub fn update_chore(state: State<'_, Db>, args: UpdateChoreArgs) -> Result<Chore, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::update(
        &conn,
        args.id,
        &args.title,
        &args.emoji,
        &args.rrule,
        &args.rotation,
    )
    .map_err(|e| e.to_string())?;
    chore::get(&conn, args.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "chore not found".to_string())
}

#[tauri::command]
pub fn delete_chore(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::soft_delete(&conn, id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct CompleteChoreArgs {
    pub id: i64,
    #[serde(rename = "completedBy")]
    pub completed_by: Option<i64>,
}

#[tauri::command]
pub fn complete_chore(state: State<'_, Db>, args: CompleteChoreArgs) -> Result<Chore, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::complete(&conn, args.id, args.completed_by).map_err(|e| e.to_string())?;
    chore::get(&conn, args.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "chore not found".to_string())
}

#[tauri::command]
pub fn skip_chore(state: State<'_, Db>, id: i64) -> Result<Chore, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::skip(&conn, id).map_err(|e| e.to_string())?;
    chore::get(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "chore not found".to_string())
}

#[tauri::command]
pub fn list_chore_completions(
    state: State<'_, Db>,
    #[allow(non_snake_case)] choreId: i64,
    limit: u32,
) -> Result<Vec<ChoreCompletion>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::list_completions(&conn, choreId, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_chore_rotation(
    state: State<'_, Db>,
    #[allow(non_snake_case)] choreId: i64,
) -> Result<Vec<RotationMember>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::list_rotation(&conn, choreId).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_chore_fairness(state: State<'_, Db>) -> Result<Vec<FairnessNudge>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().timestamp_millis();
    chore::check_fairness(&conn, now).map_err(|e| e.to_string())
}

// ---------- Time block commands ----------

#[tauri::command]
pub fn list_blocks_today(state: State<'_, Db>) -> Result<Vec<TimeBlock>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::list_for_date(&conn, today_midnight_utc_ms()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_blocks_for_week(
    state: State<'_, Db>,
    #[allow(non_snake_case)] weekStartMs: i64,
) -> Result<Vec<TimeBlock>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::list_for_week(&conn, weekStartMs).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_recurring_blocks(state: State<'_, Db>) -> Result<Vec<TimeBlock>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::list_recurring(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct CreateBlockArgs {
    pub title: String,
    pub kind: String,
    #[serde(rename = "dateMs")]
    pub date_ms: i64,
    #[serde(rename = "startTime")]
    pub start_time: String,
    #[serde(rename = "endTime")]
    pub end_time: String,
}

#[derive(serde::Serialize)]
pub struct CreateBlockResult {
    pub block: TimeBlock,
    pub suggestion: Option<PatternSuggestion>,
}

#[tauri::command]
pub fn create_time_block(
    state: State<'_, Db>,
    args: CreateBlockArgs,
) -> Result<CreateBlockResult, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let id = time_block::insert(
        &conn,
        &args.title,
        &args.kind,
        args.date_ms,
        &args.start_time,
        &args.end_time,
    )
    .map_err(|e| e.to_string())?;
    let block = time_block::get(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "failed to fetch new block".to_string())?;
    let now = Utc::now().timestamp_millis();
    let suggestion = time_block::check_pattern(&conn, id, now).map_err(|e| e.to_string())?;
    Ok(CreateBlockResult { block, suggestion })
}

#[derive(serde::Deserialize)]
pub struct UpdateBlockArgs {
    pub id: i64,
    pub title: String,
    pub kind: String,
    #[serde(rename = "dateMs")]
    pub date_ms: i64,
    #[serde(rename = "startTime")]
    pub start_time: String,
    #[serde(rename = "endTime")]
    pub end_time: String,
}

#[tauri::command]
pub fn update_time_block(
    state: State<'_, Db>,
    args: UpdateBlockArgs,
) -> Result<TimeBlock, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::update(
        &conn,
        args.id,
        &args.title,
        &args.kind,
        args.date_ms,
        &args.start_time,
        &args.end_time,
    )
    .map_err(|e| e.to_string())?;
    time_block::get(&conn, args.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "block not found".to_string())
}

#[tauri::command]
pub fn delete_time_block(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::soft_delete(&conn, id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct PromoteArgs {
    pub id: i64,
    pub rrule: String,
}

#[tauri::command]
pub fn promote_to_pattern(state: State<'_, Db>, args: PromoteArgs) -> Result<TimeBlock, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::promote_to_pattern(&conn, args.id, &args.rrule).map_err(|e| e.to_string())?;
    time_block::get(&conn, args.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "block not found".to_string())
}

#[tauri::command]
pub fn dismiss_pattern_nudge(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    time_block::dismiss_pattern_nudge(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_time_block_pattern(
    state: State<'_, Db>,
    id: i64,
) -> Result<Option<PatternSuggestion>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().timestamp_millis();
    time_block::check_pattern(&conn, id, now).map_err(|e| e.to_string())
}

// ---------- Person commands ----------

#[tauri::command]
pub fn add_person(state: State<'_, Db>, name: String) -> Result<i64, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    chore::insert_person(&conn, &name).map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register the module + commands in lib.rs**

Edit `crates/app/src/lib.rs`.

At the top, next to `pub mod sync;`, add:

```rust
pub mod rhythm;
```

Inside the `invoke_handler(tauri::generate_handler![...])` block, after `assistant::commands::list_events_today,`, append:

```rust
            rhythm::commands::list_chores_due_today,
            rhythm::commands::list_all_chores,
            rhythm::commands::create_chore,
            rhythm::commands::update_chore,
            rhythm::commands::delete_chore,
            rhythm::commands::complete_chore,
            rhythm::commands::skip_chore,
            rhythm::commands::list_chore_completions,
            rhythm::commands::list_chore_rotation,
            rhythm::commands::check_chore_fairness,
            rhythm::commands::list_blocks_today,
            rhythm::commands::list_blocks_for_week,
            rhythm::commands::list_recurring_blocks,
            rhythm::commands::create_time_block,
            rhythm::commands::update_time_block,
            rhythm::commands::delete_time_block,
            rhythm::commands::promote_to_pattern,
            rhythm::commands::dismiss_pattern_nudge,
            rhythm::commands::check_time_block_pattern,
            rhythm::commands::add_person,
```

- [ ] **Step 4: Build the workspace**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
cargo build --workspace 2>&1 | grep -E "^error|warning\[" | head -20
```

Expected: no `error` lines. Warnings are acceptable.

- [ ] **Step 5: Run full test suite**

```bash
cargo test --workspace 2>&1 | grep -E "^test result|FAILED"
```

Expected: all `test result: ok.` lines. Baseline 66 Rust tests + 20 new = 86+ total.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/rhythm crates/app/src/lib.rs
git commit -m "feat(app): rhythm Tauri commands — chores, time blocks, nudges"
```

---

### Task 5: Frontend chores lib (ipc + state + tests)

**Files:**
- Create: `apps/desktop/src/lib/chores/ipc.ts`
- Create: `apps/desktop/src/lib/chores/state.ts`
- Create: `apps/desktop/src/lib/chores/state.test.ts`

- [ ] **Step 1: Write the IPC wrapper**

Create `apps/desktop/src/lib/chores/ipc.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";

export type RotationKind = "round_robin" | "least_completed" | "fixed" | "none";

export interface Chore {
  id: number;
  title: string;
  emoji: string;
  rrule: string;
  next_due: number;
  rotation: RotationKind;
  active: boolean;
  created_at: number;
  deleted_at: number | null;
}

export interface ChoreCompletion {
  id: number;
  chore_id: number;
  completed_at: number;
  completed_by: number | null;
  created_at: number;
}

export interface RotationMember {
  id: number;
  chore_id: number;
  person_id: number;
  position: number;
  current: boolean;
}

export interface FairnessNudge {
  chore_id: number;
  chore_title: string;
  person_id: number;
  person_name: string;
  days_ago: number;
}

export async function listChoresDueToday(): Promise<Chore[]> {
  return invoke<Chore[]>("list_chores_due_today");
}

export async function listAllChores(): Promise<Chore[]> {
  return invoke<Chore[]>("list_all_chores");
}

export async function createChore(args: {
  title: string;
  emoji: string;
  rrule: string;
  firstDue: number;
  rotation: RotationKind;
}): Promise<Chore> {
  return invoke<Chore>("create_chore", { args });
}

export async function updateChore(args: {
  id: number;
  title: string;
  emoji: string;
  rrule: string;
  rotation: RotationKind;
}): Promise<Chore> {
  return invoke<Chore>("update_chore", { args });
}

export async function deleteChore(id: number): Promise<void> {
  return invoke<void>("delete_chore", { id });
}

export async function completeChore(id: number, completedBy: number | null = null): Promise<Chore> {
  return invoke<Chore>("complete_chore", { args: { id, completedBy } });
}

export async function skipChore(id: number): Promise<Chore> {
  return invoke<Chore>("skip_chore", { id });
}

export async function listChoreCompletions(choreId: number, limit: number = 20): Promise<ChoreCompletion[]> {
  return invoke<ChoreCompletion[]>("list_chore_completions", { choreId, limit });
}

export async function listChoreRotation(choreId: number): Promise<RotationMember[]> {
  return invoke<RotationMember[]>("list_chore_rotation", { choreId });
}

export async function checkChoreFairness(): Promise<FairnessNudge[]> {
  return invoke<FairnessNudge[]>("check_chore_fairness");
}

export async function addPerson(name: string): Promise<number> {
  return invoke<number>("add_person", { name });
}
```

- [ ] **Step 2: Write the Zustand store**

Create `apps/desktop/src/lib/chores/state.ts`:

```typescript
import { create } from "zustand";
import type { Chore, FairnessNudge } from "./ipc";

interface ChoresStore {
  choresDueToday: Chore[];
  allChores: Chore[];
  fairnessNudges: FairnessNudge[];

  setChoresDueToday: (c: Chore[]) => void;
  setAllChores: (c: Chore[]) => void;
  setFairnessNudges: (n: FairnessNudge[]) => void;
  upsertChore: (c: Chore) => void;
  removeChore: (id: number) => void;
  removeFromDueToday: (id: number) => void;
  dismissFairnessNudge: (choreId: number) => void;
}

export const useChoresStore = create<ChoresStore>((set) => ({
  choresDueToday: [],
  allChores: [],
  fairnessNudges: [],

  setChoresDueToday: (c) => set({ choresDueToday: c }),
  setAllChores: (c) => set({ allChores: c }),
  setFairnessNudges: (n) => set({ fairnessNudges: n }),

  upsertChore: (c) =>
    set((st) => {
      const updateList = (list: Chore[]) => {
        const idx = list.findIndex((x) => x.id === c.id);
        if (idx === -1) return [...list, c];
        const next = list.slice();
        next[idx] = c;
        return next;
      };
      return { allChores: updateList(st.allChores) };
    }),

  removeChore: (id) =>
    set((st) => ({
      allChores: st.allChores.filter((x) => x.id !== id),
      choresDueToday: st.choresDueToday.filter((x) => x.id !== id),
    })),

  removeFromDueToday: (id) =>
    set((st) => ({ choresDueToday: st.choresDueToday.filter((x) => x.id !== id) })),

  dismissFairnessNudge: (choreId) =>
    set((st) => ({ fairnessNudges: st.fairnessNudges.filter((n) => n.chore_id !== choreId) })),
}));
```

- [ ] **Step 3: Write the tests**

Create `apps/desktop/src/lib/chores/state.test.ts`:

```typescript
import { describe, it, expect, beforeEach } from "vitest";
import { useChoresStore } from "./state";
import type { Chore, FairnessNudge } from "./ipc";

const sampleChore = (overrides: Partial<Chore> = {}): Chore => ({
  id: 1,
  title: "Bins",
  emoji: "🗑️",
  rrule: "FREQ=WEEKLY",
  next_due: Date.now(),
  rotation: "none",
  active: true,
  created_at: Date.now(),
  deleted_at: null,
  ...overrides,
});

const sampleNudge = (overrides: Partial<FairnessNudge> = {}): FairnessNudge => ({
  chore_id: 1,
  chore_title: "Bins",
  person_id: 1,
  person_name: "Rosa",
  days_ago: 21,
  ...overrides,
});

describe("useChoresStore", () => {
  beforeEach(() => {
    useChoresStore.setState(useChoresStore.getInitialState(), true);
  });

  it("starts empty", () => {
    const s = useChoresStore.getState();
    expect(s.choresDueToday).toEqual([]);
    expect(s.allChores).toEqual([]);
    expect(s.fairnessNudges).toEqual([]);
  });

  it("setChoresDueToday replaces the list", () => {
    const a = sampleChore({ id: 1, title: "A" });
    const b = sampleChore({ id: 2, title: "B" });
    useChoresStore.getState().setChoresDueToday([a, b]);
    expect(useChoresStore.getState().choresDueToday).toEqual([a, b]);
  });

  it("upsertChore appends a new id to allChores", () => {
    const a = sampleChore({ id: 1 });
    const b = sampleChore({ id: 2 });
    useChoresStore.getState().setAllChores([a]);
    useChoresStore.getState().upsertChore(b);
    expect(useChoresStore.getState().allChores).toEqual([a, b]);
  });

  it("upsertChore updates an existing row", () => {
    const a = sampleChore({ id: 1, title: "Old" });
    useChoresStore.getState().setAllChores([a]);
    useChoresStore.getState().upsertChore({ ...a, title: "New" });
    expect(useChoresStore.getState().allChores[0].title).toBe("New");
  });

  it("removeChore strips it from both lists", () => {
    const a = sampleChore({ id: 1 });
    useChoresStore.getState().setAllChores([a]);
    useChoresStore.getState().setChoresDueToday([a]);
    useChoresStore.getState().removeChore(1);
    expect(useChoresStore.getState().allChores).toEqual([]);
    expect(useChoresStore.getState().choresDueToday).toEqual([]);
  });

  it("removeFromDueToday only touches the today list", () => {
    const a = sampleChore({ id: 1 });
    useChoresStore.getState().setAllChores([a]);
    useChoresStore.getState().setChoresDueToday([a]);
    useChoresStore.getState().removeFromDueToday(1);
    expect(useChoresStore.getState().allChores).toEqual([a]);
    expect(useChoresStore.getState().choresDueToday).toEqual([]);
  });

  it("dismissFairnessNudge drops the entry for that chore_id", () => {
    const n1 = sampleNudge({ chore_id: 1 });
    const n2 = sampleNudge({ chore_id: 2 });
    useChoresStore.getState().setFairnessNudges([n1, n2]);
    useChoresStore.getState().dismissFairnessNudge(1);
    expect(useChoresStore.getState().fairnessNudges).toEqual([n2]);
  });
});
```

- [ ] **Step 4: Run the tests**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm/apps/desktop
npx vitest run src/lib/chores 2>&1 | grep -E "Tests |FAIL" | tail -5
```

Expected: `Tests  7 passed`

- [ ] **Step 5: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
git add apps/desktop/src/lib/chores/
git commit -m "feat(chores): IPC wrappers + Zustand store + tests"
```

---

### Task 6: Frontend time blocks lib (ipc + state + tests)

**Files:**
- Create: `apps/desktop/src/lib/timeblocks/ipc.ts`
- Create: `apps/desktop/src/lib/timeblocks/state.ts`
- Create: `apps/desktop/src/lib/timeblocks/state.test.ts`

- [ ] **Step 1: Write the IPC wrapper**

Create `apps/desktop/src/lib/timeblocks/ipc.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";

export type BlockKind = "focus" | "errands" | "admin" | "dnd";

export interface TimeBlock {
  id: number;
  title: string;
  kind: BlockKind;
  date: number;
  start_time: string;
  end_time: string;
  rrule: string | null;
  is_pattern: boolean;
  pattern_nudge_dismissed_at: number | null;
  created_at: number;
  deleted_at: number | null;
}

export interface PatternSuggestion {
  trigger_id: number;
  kind: string;
  start_time: string;
  end_time: string;
  weekday: string;
  count: number;
}

export interface CreateBlockResult {
  block: TimeBlock;
  suggestion: PatternSuggestion | null;
}

export async function listBlocksToday(): Promise<TimeBlock[]> {
  return invoke<TimeBlock[]>("list_blocks_today");
}

export async function listBlocksForWeek(weekStartMs: number): Promise<TimeBlock[]> {
  return invoke<TimeBlock[]>("list_blocks_for_week", { weekStartMs });
}

export async function listRecurringBlocks(): Promise<TimeBlock[]> {
  return invoke<TimeBlock[]>("list_recurring_blocks");
}

export async function createTimeBlock(args: {
  title: string;
  kind: BlockKind;
  dateMs: number;
  startTime: string;
  endTime: string;
}): Promise<CreateBlockResult> {
  return invoke<CreateBlockResult>("create_time_block", { args });
}

export async function updateTimeBlock(args: {
  id: number;
  title: string;
  kind: BlockKind;
  dateMs: number;
  startTime: string;
  endTime: string;
}): Promise<TimeBlock> {
  return invoke<TimeBlock>("update_time_block", { args });
}

export async function deleteTimeBlock(id: number): Promise<void> {
  return invoke<void>("delete_time_block", { id });
}

export async function promoteToPattern(id: number, rrule: string): Promise<TimeBlock> {
  return invoke<TimeBlock>("promote_to_pattern", { args: { id, rrule } });
}

export async function dismissPatternNudge(id: number): Promise<void> {
  return invoke<void>("dismiss_pattern_nudge", { id });
}

export async function checkTimeBlockPattern(id: number): Promise<PatternSuggestion | null> {
  return invoke<PatternSuggestion | null>("check_time_block_pattern", { id });
}
```

- [ ] **Step 2: Write the Zustand store**

Create `apps/desktop/src/lib/timeblocks/state.ts`:

```typescript
import { create } from "zustand";
import type { TimeBlock, PatternSuggestion } from "./ipc";

interface TimeBlocksStore {
  todayBlocks: TimeBlock[];
  weekBlocks: TimeBlock[];
  recurringBlocks: TimeBlock[];
  patternSuggestion: PatternSuggestion | null;

  setTodayBlocks: (b: TimeBlock[]) => void;
  setWeekBlocks: (b: TimeBlock[]) => void;
  setRecurringBlocks: (b: TimeBlock[]) => void;
  setPatternSuggestion: (s: PatternSuggestion | null) => void;
  upsertBlock: (b: TimeBlock) => void;
  removeBlock: (id: number) => void;
}

export const useTimeBlocksStore = create<TimeBlocksStore>((set) => ({
  todayBlocks: [],
  weekBlocks: [],
  recurringBlocks: [],
  patternSuggestion: null,

  setTodayBlocks: (b) => set({ todayBlocks: b }),
  setWeekBlocks: (b) => set({ weekBlocks: b }),
  setRecurringBlocks: (b) => set({ recurringBlocks: b }),
  setPatternSuggestion: (s) => set({ patternSuggestion: s }),

  upsertBlock: (b) =>
    set((st) => {
      const updateList = (list: TimeBlock[]) => {
        const idx = list.findIndex((x) => x.id === b.id);
        if (idx === -1) return [...list, b];
        const next = list.slice();
        next[idx] = b;
        return next;
      };
      return {
        todayBlocks: updateList(st.todayBlocks),
        weekBlocks: updateList(st.weekBlocks),
      };
    }),

  removeBlock: (id) =>
    set((st) => ({
      todayBlocks: st.todayBlocks.filter((x) => x.id !== id),
      weekBlocks: st.weekBlocks.filter((x) => x.id !== id),
      recurringBlocks: st.recurringBlocks.filter((x) => x.id !== id),
    })),
}));
```

- [ ] **Step 3: Write the tests**

Create `apps/desktop/src/lib/timeblocks/state.test.ts`:

```typescript
import { describe, it, expect, beforeEach } from "vitest";
import { useTimeBlocksStore } from "./state";
import type { TimeBlock, PatternSuggestion } from "./ipc";

const sampleBlock = (overrides: Partial<TimeBlock> = {}): TimeBlock => ({
  id: 1,
  title: "Focus",
  kind: "focus",
  date: 1_776_259_200_000,
  start_time: "09:00",
  end_time: "11:00",
  rrule: null,
  is_pattern: false,
  pattern_nudge_dismissed_at: null,
  created_at: Date.now(),
  deleted_at: null,
  ...overrides,
});

const sampleSuggestion = (overrides: Partial<PatternSuggestion> = {}): PatternSuggestion => ({
  trigger_id: 1,
  kind: "focus",
  start_time: "09:00",
  end_time: "11:00",
  weekday: "Tuesday",
  count: 3,
  ...overrides,
});

describe("useTimeBlocksStore", () => {
  beforeEach(() => {
    useTimeBlocksStore.setState(useTimeBlocksStore.getInitialState(), true);
  });

  it("starts empty", () => {
    const s = useTimeBlocksStore.getState();
    expect(s.todayBlocks).toEqual([]);
    expect(s.weekBlocks).toEqual([]);
    expect(s.recurringBlocks).toEqual([]);
    expect(s.patternSuggestion).toBeNull();
  });

  it("setTodayBlocks replaces the list", () => {
    const a = sampleBlock({ id: 1 });
    const b = sampleBlock({ id: 2 });
    useTimeBlocksStore.getState().setTodayBlocks([a, b]);
    expect(useTimeBlocksStore.getState().todayBlocks).toEqual([a, b]);
  });

  it("upsertBlock appends a new id", () => {
    const a = sampleBlock({ id: 1 });
    const b = sampleBlock({ id: 2 });
    useTimeBlocksStore.getState().setTodayBlocks([a]);
    useTimeBlocksStore.getState().upsertBlock(b);
    expect(useTimeBlocksStore.getState().todayBlocks).toEqual([a, b]);
  });

  it("upsertBlock updates both today and week lists", () => {
    const a = sampleBlock({ id: 1, title: "Old" });
    useTimeBlocksStore.getState().setTodayBlocks([a]);
    useTimeBlocksStore.getState().setWeekBlocks([a]);
    useTimeBlocksStore.getState().upsertBlock({ ...a, title: "New" });
    expect(useTimeBlocksStore.getState().todayBlocks[0].title).toBe("New");
    expect(useTimeBlocksStore.getState().weekBlocks[0].title).toBe("New");
  });

  it("removeBlock strips it from all three lists", () => {
    const a = sampleBlock({ id: 1 });
    useTimeBlocksStore.getState().setTodayBlocks([a]);
    useTimeBlocksStore.getState().setWeekBlocks([a]);
    useTimeBlocksStore.getState().setRecurringBlocks([a]);
    useTimeBlocksStore.getState().removeBlock(1);
    expect(useTimeBlocksStore.getState().todayBlocks).toEqual([]);
    expect(useTimeBlocksStore.getState().weekBlocks).toEqual([]);
    expect(useTimeBlocksStore.getState().recurringBlocks).toEqual([]);
  });

  it("setPatternSuggestion stores and clears", () => {
    const s = sampleSuggestion();
    useTimeBlocksStore.getState().setPatternSuggestion(s);
    expect(useTimeBlocksStore.getState().patternSuggestion).toEqual(s);
    useTimeBlocksStore.getState().setPatternSuggestion(null);
    expect(useTimeBlocksStore.getState().patternSuggestion).toBeNull();
  });
});
```

- [ ] **Step 4: Run the tests**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm/apps/desktop
npx vitest run src/lib/timeblocks 2>&1 | grep -E "Tests |FAIL" | tail -5
```

Expected: `Tests  6 passed`

- [ ] **Step 5: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
git add apps/desktop/src/lib/timeblocks/
git commit -m "feat(timeblocks): IPC wrappers + Zustand store + tests"
```

---

### Task 7: Nav shell — sidebar + view switcher

**Files:**
- Create: `apps/desktop/src/lib/nav.ts`
- Create: `apps/desktop/src/components/Nav/Sidebar.tsx`
- Modify: `apps/desktop/src/App.tsx`

- [ ] **Step 1: Write the nav store**

Create `apps/desktop/src/lib/nav.ts`:

```typescript
import { create } from "zustand";

export type View = "today" | "chores" | "timeblocks";

interface NavStore {
  view: View;
  setView: (v: View) => void;
}

export const useNavStore = create<NavStore>((set) => ({
  view: "today",
  setView: (v) => set({ view: v }),
}));
```

- [ ] **Step 2: Write the Sidebar component**

Create `apps/desktop/src/components/Nav/Sidebar.tsx`:

```tsx
import { useNavStore, type View } from "../../lib/nav";

const SIDEBAR_WIDTH = 58;

const railStyle: React.CSSProperties = {
  width: SIDEBAR_WIDTH,
  background: "var(--paper-muted)",
  borderRight: "1px solid var(--hairline)",
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  padding: "14px 0 12px",
  gap: 6,
  flexShrink: 0,
  height: "100vh",
};

const avatarStyle: React.CSSProperties = {
  width: 32,
  height: 32,
  borderRadius: "50%",
  background: "linear-gradient(135deg, #FFC15C 0%, #FF8800 100%)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  fontSize: 16,
  boxShadow: "0 2px 6px rgba(255,136,0,0.3)",
  marginBottom: 10,
};

const iconStyle = (active: boolean): React.CSSProperties => ({
  width: 38,
  height: 38,
  borderRadius: 10,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  fontSize: 17,
  cursor: "pointer",
  background: active ? "var(--paper)" : "transparent",
  boxShadow: active ? "0 1px 4px rgba(20,20,30,0.1)" : "none",
  color: active ? "var(--imessage-blue)" : "rgba(20,20,30,0.35)",
  transition: "background 0.15s, color 0.15s",
});

interface NavIconProps {
  view: View;
  icon: string;
  title: string;
}

function NavIcon({ view, icon, title }: NavIconProps) {
  const current = useNavStore((s) => s.view);
  const setView = useNavStore((s) => s.setView);
  const active = current === view;
  return (
    <div
      role="button"
      aria-label={title}
      aria-current={active ? "page" : undefined}
      title={title}
      onClick={() => setView(view)}
      style={iconStyle(active)}
    >
      {icon}
    </div>
  );
}

export default function Sidebar() {
  return (
    <nav style={railStyle} aria-label="Primary navigation">
      <div style={avatarStyle} aria-hidden="true">🌸</div>
      <NavIcon view="today" icon="🏠" title="Today" />
      <NavIcon view="chores" icon="🧹" title="Chores" />
      <NavIcon view="timeblocks" icon="⏱" title="Time Blocks" />
      <div style={{ flex: 1 }} />
    </nav>
  );
}
```

- [ ] **Step 3: Rewire App.tsx with sidebar layout**

Replace the contents of `apps/desktop/src/App.tsx` with:

```tsx
import { useEffect } from "react";
import Assistant from "./components/Assistant/Assistant";
import Today from "./components/Today/Today";
import SettingsModal from "./components/Settings/SettingsModal";
import Sidebar from "./components/Nav/Sidebar";
import ChoresView from "./components/Chores/ChoresView";
import TimeBlocksView from "./components/TimeBlocks/TimeBlocksView";
import { useSettingsStore } from "./lib/settings/state";
import { useNavStore } from "./lib/nav";

const shellStyle: React.CSSProperties = {
  display: "flex",
  height: "100vh",
  width: "100vw",
  overflow: "hidden",
};

const mainStyle: React.CSSProperties = {
  flex: 1,
  overflow: "auto",
  position: "relative",
};

export default function App() {
  const setModalOpen = useSettingsStore((s) => s.setModalOpen);
  const modalOpen = useSettingsStore((s) => s.modalOpen);
  const view = useNavStore((s) => s.view);

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        setModalOpen(!modalOpen);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [modalOpen, setModalOpen]);

  return (
    <>
      <div style={shellStyle}>
        <Sidebar />
        <div style={mainStyle}>
          {view === "today" && <Today />}
          {view === "chores" && <ChoresView />}
          {view === "timeblocks" && <TimeBlocksView />}
        </div>
      </div>
      <Assistant />
      <SettingsModal />
    </>
  );
}
```

Note: `ChoresView` and `TimeBlocksView` are referenced but don't exist yet — that's fine, they're created in Tasks 9 and 10. Build will fail until those tasks complete.

- [ ] **Step 4: Create placeholder view components**

Create placeholder `apps/desktop/src/components/Chores/ChoresView.tsx`:

```tsx
export default function ChoresView() {
  return (
    <div style={{ padding: 32, color: "rgba(20,20,30,0.5)" }}>
      <h2>Chores</h2>
      <p>Management view coming in Task 9.</p>
    </div>
  );
}
```

Create placeholder `apps/desktop/src/components/TimeBlocks/TimeBlocksView.tsx`:

```tsx
export default function TimeBlocksView() {
  return (
    <div style={{ padding: 32, color: "rgba(20,20,30,0.5)" }}>
      <h2>Time Blocks</h2>
      <p>Management view coming in Task 10.</p>
    </div>
  );
}
```

- [ ] **Step 5: Verify TypeScript compiles**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm/apps/desktop
npx tsc --noEmit 2>&1 | head -10
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
git add apps/desktop/src/lib/nav.ts apps/desktop/src/components/Nav apps/desktop/src/App.tsx apps/desktop/src/components/Chores apps/desktop/src/components/TimeBlocks
git commit -m "feat(nav): icon sidebar shell + view switcher + placeholder views"
```

---

### Task 8: Today view cards — ChoresCard + TimeBlocksCard

**Files:**
- Create: `apps/desktop/src/components/Today/ChoresCard.tsx`
- Create: `apps/desktop/src/components/Today/TimeBlocksCard.tsx`
- Modify: `apps/desktop/src/components/Today/Today.tsx`

- [ ] **Step 1: Create ChoresCard**

Create `apps/desktop/src/components/Today/ChoresCard.tsx`:

```tsx
import { useChoresStore } from "../../lib/chores/state";
import { useNavStore } from "../../lib/nav";
import { completeChore, skipChore } from "../../lib/chores/ipc";

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

const manageLink: React.CSSProperties = {
  background: "transparent",
  border: "none",
  color: "var(--imessage-blue)",
  fontWeight: 600,
  fontSize: 12,
  cursor: "pointer",
  padding: 0,
  letterSpacing: 0,
  textTransform: "none",
};

const rowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "10px 4px",
  cursor: "pointer",
  borderRadius: 8,
  transition: "background 0.15s",
};

const emptyStyle: React.CSSProperties = {
  padding: "10px 4px",
  fontSize: 13,
  color: "rgba(20,20,30,0.5)",
};

export default function ChoresCard() {
  const chores = useChoresStore((s) => s.choresDueToday);
  const removeFromDueToday = useChoresStore((s) => s.removeFromDueToday);
  const upsertChore = useChoresStore((s) => s.upsertChore);
  const setView = useNavStore((s) => s.setView);

  async function onComplete(id: number) {
    const updated = await completeChore(id, null);
    removeFromDueToday(id);
    upsertChore(updated);
  }

  async function onSkip(e: React.MouseEvent, id: number) {
    e.preventDefault();
    const updated = await skipChore(id);
    removeFromDueToday(id);
    upsertChore(updated);
  }

  return (
    <section style={cardStyle} aria-label="Chores">
      <header style={sectionHeader}>
        <span>Chores</span>
        <button style={manageLink} onClick={() => setView("chores")}>
          Manage →
        </button>
      </header>
      {chores.length === 0 ? (
        <div style={emptyStyle}>All clear today 🧹</div>
      ) : (
        <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
          {chores.map((c) => (
            <li
              key={c.id}
              style={rowStyle}
              role="button"
              tabIndex={0}
              onClick={() => onComplete(c.id)}
              onContextMenu={(e) => onSkip(e, c.id)}
              onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onComplete(c.id); }}
              onMouseEnter={(e) => { e.currentTarget.style.background = "rgba(20,20,30,0.04)"; }}
              onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}
              title="Click to complete · Right-click to skip"
            >
              <span style={{ fontSize: 18 }}>{c.emoji}</span>
              <span style={{ flex: 1, fontSize: 14, color: "var(--ink)" }}>{c.title}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
```

- [ ] **Step 2: Create TimeBlocksCard**

Create `apps/desktop/src/components/Today/TimeBlocksCard.tsx`:

```tsx
import { useState } from "react";
import { useTimeBlocksStore } from "../../lib/timeblocks/state";
import { createTimeBlock, dismissPatternNudge, promoteToPattern, type BlockKind } from "../../lib/timeblocks/ipc";

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

const addBtn: React.CSSProperties = {
  background: "transparent",
  border: "none",
  color: "var(--imessage-blue)",
  fontWeight: 700,
  fontSize: 12,
  cursor: "pointer",
  padding: 0,
};

const KIND_COLOR: Record<BlockKind, string> = {
  focus: "#007aff",
  errands: "#FFC15C",
  admin: "#9b59b6",
  dnd: "#ff3b30",
};

const KIND_LABEL: Record<BlockKind, string> = {
  focus: "Focus",
  errands: "Errands",
  admin: "Admin",
  dnd: "DND",
};

const pillStyle = (kind: BlockKind): React.CSSProperties => ({
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "8px 12px",
  borderRadius: 8,
  borderLeft: `3px solid ${KIND_COLOR[kind]}`,
  background: "rgba(20,20,30,0.03)",
  fontSize: 13,
  color: "var(--ink)",
  marginBottom: 6,
});

const emptyStyle: React.CSSProperties = {
  padding: "10px 4px",
  fontSize: 13,
  color: "rgba(20,20,30,0.5)",
};

const nudgeStyle: React.CSSProperties = {
  marginTop: 10,
  padding: "10px 12px",
  background: "rgba(0,122,255,0.06)",
  borderRadius: 8,
  fontSize: 12,
  color: "rgba(20,20,30,0.7)",
  display: "flex",
  alignItems: "center",
  gap: 10,
};

const nudgeBtn: React.CSSProperties = {
  background: "var(--imessage-blue)",
  color: "white",
  border: "none",
  borderRadius: 999,
  padding: "4px 10px",
  fontSize: 11,
  fontWeight: 600,
  cursor: "pointer",
};

const nudgeBtnGhost: React.CSSProperties = {
  background: "transparent",
  color: "rgba(20,20,30,0.55)",
  border: "none",
  padding: "4px 8px",
  fontSize: 11,
  cursor: "pointer",
};

function suggestionToRrule(weekday: string): string {
  const map: Record<string, string> = {
    Monday: "MO", Tuesday: "TU", Wednesday: "WE", Thursday: "TH",
    Friday: "FR", Saturday: "SA", Sunday: "SU",
  };
  return `FREQ=WEEKLY;BYDAY=${map[weekday] || "MO"}`;
}

export default function TimeBlocksCard() {
  const blocks = useTimeBlocksStore((s) => s.todayBlocks);
  const suggestion = useTimeBlocksStore((s) => s.patternSuggestion);
  const upsertBlock = useTimeBlocksStore((s) => s.upsertBlock);
  const setPatternSuggestion = useTimeBlocksStore((s) => s.setPatternSuggestion);

  const [adding, setAdding] = useState(false);
  const [form, setForm] = useState({ title: "", kind: "focus" as BlockKind, startTime: "09:00", endTime: "10:00" });

  async function onAdd() {
    if (!form.title.trim()) {
      setAdding(false);
      return;
    }
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const { block, suggestion: sugg } = await createTimeBlock({
      title: form.title.trim(),
      kind: form.kind,
      dateMs: today.getTime(),
      startTime: form.startTime,
      endTime: form.endTime,
    });
    upsertBlock(block);
    if (sugg) setPatternSuggestion(sugg);
    setAdding(false);
    setForm({ title: "", kind: "focus", startTime: "09:00", endTime: "10:00" });
  }

  async function onPromote() {
    if (!suggestion) return;
    const rrule = suggestionToRrule(suggestion.weekday);
    const updated = await promoteToPattern(suggestion.trigger_id, rrule);
    upsertBlock(updated);
    setPatternSuggestion(null);
  }

  async function onDismiss() {
    if (!suggestion) return;
    await dismissPatternNudge(suggestion.trigger_id);
    setPatternSuggestion(null);
  }

  return (
    <section style={cardStyle} aria-label="Time Blocks">
      <header style={sectionHeader}>
        <span>Time Blocks</span>
        <button style={addBtn} onClick={() => setAdding(true)}>+ Add</button>
      </header>

      {blocks.length === 0 && !adding ? (
        <div style={emptyStyle}>No blocks today — time is yours.</div>
      ) : (
        <div>
          {blocks.map((b) => (
            <div key={b.id} style={pillStyle(b.kind)}>
              <strong style={{ color: KIND_COLOR[b.kind], fontWeight: 700, fontSize: 11, textTransform: "uppercase", letterSpacing: 0.5 }}>
                {KIND_LABEL[b.kind]}
              </strong>
              <span style={{ flex: 1 }}>{b.title}</span>
              <span style={{ color: "rgba(20,20,30,0.5)", fontSize: 12 }}>
                {b.start_time}–{b.end_time}
              </span>
            </div>
          ))}
        </div>
      )}

      {adding && (
        <div style={{ marginTop: 8, padding: 10, background: "rgba(20,20,30,0.03)", borderRadius: 8 }}>
          <div style={{ display: "flex", gap: 6, marginBottom: 6 }}>
            <input
              autoFocus
              value={form.title}
              onChange={(e) => setForm({ ...form, title: e.target.value })}
              placeholder="Block title…"
              style={{ flex: 1, padding: "6px 10px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: 13, fontFamily: "inherit" }}
              onKeyDown={(e) => { if (e.key === "Enter") onAdd(); }}
            />
            <select
              value={form.kind}
              onChange={(e) => setForm({ ...form, kind: e.target.value as BlockKind })}
              style={{ padding: "6px 8px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: 13, fontFamily: "inherit" }}
            >
              <option value="focus">Focus</option>
              <option value="errands">Errands</option>
              <option value="admin">Admin</option>
              <option value="dnd">DND</option>
            </select>
          </div>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <input
              type="time"
              value={form.startTime}
              onChange={(e) => setForm({ ...form, startTime: e.target.value })}
              style={{ padding: "6px 8px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: 13, fontFamily: "inherit" }}
            />
            <span style={{ color: "rgba(20,20,30,0.5)" }}>→</span>
            <input
              type="time"
              value={form.endTime}
              onChange={(e) => setForm({ ...form, endTime: e.target.value })}
              style={{ padding: "6px 8px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: 13, fontFamily: "inherit" }}
            />
            <button onClick={onAdd} style={{ marginLeft: "auto", ...nudgeBtn }}>Save</button>
            <button onClick={() => setAdding(false)} style={nudgeBtnGhost}>Cancel</button>
          </div>
        </div>
      )}

      {suggestion && (
        <div style={nudgeStyle}>
          <span style={{ flex: 1 }}>
            Looks like <b>{suggestion.weekday}s</b> {suggestion.start_time}–{suggestion.end_time} are your {suggestion.kind} time — make it recurring?
          </span>
          <button onClick={onPromote} style={nudgeBtn}>Yes</button>
          <button onClick={onDismiss} style={nudgeBtnGhost}>Not now</button>
        </div>
      )}
    </section>
  );
}
```

- [ ] **Step 3: Update Today.tsx to include new cards and hydrate stores**

Replace the contents of `apps/desktop/src/components/Today/Today.tsx` with:

```tsx
import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";
import { listTasks, listEventsToday } from "../../lib/today/ipc";
import { useChoresStore } from "../../lib/chores/state";
import { listChoresDueToday } from "../../lib/chores/ipc";
import { useTimeBlocksStore } from "../../lib/timeblocks/state";
import { listBlocksToday } from "../../lib/timeblocks/ipc";
import { AVATAR_FOOTPRINT_PX } from "../../lib/layout";
import HeaderCard from "./HeaderCard";
import EventsCard from "./EventsCard";
import TimeBlocksCard from "./TimeBlocksCard";
import ChoresCard from "./ChoresCard";
import TasksCard from "./TasksCard";
import ProposalBanner from "./ProposalBanner";
import Toast from "./Toast";

export default function Today() {
  const setTasks = useTodayStore((s) => s.setTasks);
  const setEvents = useTodayStore((s) => s.setEvents);
  const setChoresDueToday = useChoresStore((s) => s.setChoresDueToday);
  const setTodayBlocks = useTimeBlocksStore((s) => s.setTodayBlocks);

  useEffect(() => {
    void listTasks().then(setTasks);
    void listEventsToday().then(setEvents);
    void listChoresDueToday().then(setChoresDueToday);
    void listBlocksToday().then(setTodayBlocks);
  }, [setTasks, setEvents, setChoresDueToday, setTodayBlocks]);

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
        <TimeBlocksCard />
        <ChoresCard />
        <TasksCard />
      </main>
      <Toast />
    </>
  );
}
```

- [ ] **Step 4: Verify TypeScript + build**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm/apps/desktop
npx tsc --noEmit 2>&1 | head -10
```

Expected: no errors.

- [ ] **Step 5: Run all frontend tests**

```bash
npx vitest run 2>&1 | grep -E "Tests |FAIL" | tail -5
```

Expected: all tests pass (existing 27 + chores 7 + timeblocks 6 = 40 total).

- [ ] **Step 6: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
git add apps/desktop/src/components/Today/
git commit -m "feat(today): ChoresCard + TimeBlocksCard with quick-add and pattern nudge"
```

---

### Task 9: Chores management view + drawer

**Files:**
- Replace placeholder: `apps/desktop/src/components/Chores/ChoresView.tsx`
- Create: `apps/desktop/src/components/Chores/ChoreDrawer.tsx`

- [ ] **Step 1: Create ChoreDrawer**

Create `apps/desktop/src/components/Chores/ChoreDrawer.tsx`:

```tsx
import { useState, useEffect } from "react";
import type { Chore, RotationKind } from "../../lib/chores/ipc";
import {
  createChore,
  updateChore,
  deleteChore,
  listChoreCompletions,
  type ChoreCompletion,
} from "../../lib/chores/ipc";
import { useChoresStore } from "../../lib/chores/state";

const overlayStyle: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "rgba(20,20,30,0.2)",
  zIndex: 100,
  display: "flex",
  justifyContent: "flex-end",
};

const drawerStyle: React.CSSProperties = {
  width: 420,
  background: "var(--paper)",
  borderLeft: "1px solid var(--hairline)",
  boxShadow: "var(--shadow-lg)",
  padding: 24,
  overflowY: "auto",
  animation: "drawerIn 0.2s ease-out",
};

const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: 11,
  fontWeight: 700,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)",
  marginBottom: 6,
  marginTop: 14,
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "8px 12px",
  borderRadius: 8,
  border: "1px solid var(--hairline)",
  fontSize: 14,
  fontFamily: "inherit",
};

const tabBar: React.CSSProperties = {
  display: "flex",
  gap: 4,
  marginBottom: 16,
  borderBottom: "1px solid var(--hairline)",
};

const tabStyle = (active: boolean): React.CSSProperties => ({
  padding: "8px 14px",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
  borderBottom: active ? "2px solid var(--imessage-blue)" : "2px solid transparent",
  color: active ? "var(--imessage-blue)" : "rgba(20,20,30,0.55)",
  background: "transparent",
  border: "none",
  borderBottomStyle: "solid",
  borderBottomWidth: 2,
});

const btnPrimary: React.CSSProperties = {
  background: "var(--imessage-blue)",
  color: "white",
  border: "none",
  borderRadius: 999,
  padding: "8px 18px",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
};

const btnGhost: React.CSSProperties = {
  background: "transparent",
  color: "rgba(20,20,30,0.55)",
  border: "1px solid var(--hairline)",
  borderRadius: 999,
  padding: "8px 18px",
  fontSize: 13,
  cursor: "pointer",
};

const btnDanger: React.CSSProperties = {
  background: "transparent",
  color: "var(--imessage-red)",
  border: "none",
  fontSize: 12,
  cursor: "pointer",
  padding: "6px 0",
  marginTop: 12,
};

const RRULE_PRESETS: { label: string; rrule: string }[] = [
  { label: "Daily", rrule: "FREQ=DAILY" },
  { label: "Every week", rrule: "FREQ=WEEKLY" },
  { label: "Every 2 weeks", rrule: "FREQ=WEEKLY;INTERVAL=2" },
  { label: "Monthly", rrule: "FREQ=MONTHLY" },
];

interface Props {
  chore: Chore | null;
  onClose: () => void;
}

export default function ChoreDrawer({ chore, onClose }: Props) {
  const upsertChore = useChoresStore((s) => s.upsertChore);
  const removeChore = useChoresStore((s) => s.removeChore);

  const [tab, setTab] = useState<"details" | "history">("details");
  const [title, setTitle] = useState(chore?.title ?? "");
  const [emoji, setEmoji] = useState(chore?.emoji ?? "🧹");
  const [rrule, setRrule] = useState(chore?.rrule ?? "FREQ=WEEKLY");
  const [rotation, setRotation] = useState<RotationKind>(chore?.rotation ?? "none");
  const [history, setHistory] = useState<ChoreCompletion[]>([]);

  useEffect(() => {
    if (tab === "history" && chore) {
      void listChoreCompletions(chore.id, 20).then(setHistory);
    }
  }, [tab, chore]);

  async function onSave() {
    const trimmed = title.trim();
    if (!trimmed) return;
    if (chore) {
      const updated = await updateChore({
        id: chore.id, title: trimmed, emoji, rrule, rotation,
      });
      upsertChore(updated);
    } else {
      const created = await createChore({
        title: trimmed,
        emoji,
        rrule,
        firstDue: Date.now(),
        rotation,
      });
      upsertChore(created);
    }
    onClose();
  }

  async function onDelete() {
    if (!chore) return;
    await deleteChore(chore.id);
    removeChore(chore.id);
    onClose();
  }

  return (
    <div style={overlayStyle} onClick={onClose}>
      <aside style={drawerStyle} onClick={(e) => e.stopPropagation()}>
        <h2 style={{ margin: "0 0 16px", fontSize: 20, fontWeight: 700 }}>
          {chore ? "Edit chore" : "New chore"}
        </h2>

        {chore && (
          <div style={tabBar}>
            <button style={tabStyle(tab === "details")} onClick={() => setTab("details")}>Details</button>
            <button style={tabStyle(tab === "history")} onClick={() => setTab("history")}>History</button>
          </div>
        )}

        {tab === "details" && (
          <>
            <label style={labelStyle}>Emoji</label>
            <input
              style={{ ...inputStyle, width: 80, fontSize: 22, textAlign: "center" }}
              value={emoji}
              onChange={(e) => setEmoji(e.target.value.slice(0, 4))}
              aria-label="Emoji"
            />

            <label style={labelStyle}>Title</label>
            <input
              style={inputStyle}
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="e.g. Take the bins out"
              aria-label="Title"
            />

            <label style={labelStyle}>Recurrence</label>
            <select
              style={inputStyle}
              value={rrule}
              onChange={(e) => setRrule(e.target.value)}
              aria-label="Recurrence"
            >
              {RRULE_PRESETS.map((p) => (
                <option key={p.rrule} value={p.rrule}>{p.label}</option>
              ))}
            </select>

            <label style={labelStyle}>Rotation</label>
            <select
              style={inputStyle}
              value={rotation}
              onChange={(e) => setRotation(e.target.value as RotationKind)}
              aria-label="Rotation"
            >
              <option value="none">No rotation (single person)</option>
              <option value="round_robin">Round-robin</option>
              <option value="least_completed">Least recently completed</option>
              <option value="fixed">Fixed assignee</option>
            </select>

            <div style={{ display: "flex", gap: 8, marginTop: 24 }}>
              <button style={btnPrimary} onClick={onSave}>{chore ? "Save" : "Create"}</button>
              <button style={btnGhost} onClick={onClose}>Cancel</button>
            </div>

            {chore && (
              <button style={btnDanger} onClick={onDelete}>Delete chore</button>
            )}
          </>
        )}

        {tab === "history" && chore && (
          <div>
            {history.length === 0 ? (
              <p style={{ color: "rgba(20,20,30,0.5)", fontSize: 13 }}>No completions yet.</p>
            ) : (
              <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
                {history.map((h) => (
                  <li key={h.id} style={{ padding: "10px 0", borderBottom: "1px solid var(--hairline)", fontSize: 13 }}>
                    <div style={{ color: "var(--ink)" }}>
                      {new Date(h.completed_at).toLocaleString()}
                    </div>
                    <div style={{ color: "rgba(20,20,30,0.5)", fontSize: 12 }}>
                      {h.completed_by ? `by person #${h.completed_by}` : "completed"}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </aside>
    </div>
  );
}
```

- [ ] **Step 2: Replace ChoresView placeholder with real view**

Overwrite `apps/desktop/src/components/Chores/ChoresView.tsx`:

```tsx
import { useEffect, useState } from "react";
import { useChoresStore } from "../../lib/chores/state";
import { listAllChores, checkChoreFairness, type Chore } from "../../lib/chores/ipc";
import ChoreDrawer from "./ChoreDrawer";

const pageStyle: React.CSSProperties = {
  maxWidth: 760,
  margin: "0 auto",
  padding: "24px 24px 120px",
};

const sectionStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
  marginBottom: 12,
};

const headerStyle: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 700,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)",
  marginBottom: 10,
};

const rowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "10px 4px",
  borderBottom: "1px solid var(--hairline)",
  cursor: "pointer",
};

const dueBadge = (daysAway: number): React.CSSProperties => ({
  fontSize: 11,
  padding: "2px 8px",
  borderRadius: 999,
  background: daysAway <= 0 ? "rgba(255,59,48,0.1)" : "rgba(20,20,30,0.05)",
  color: daysAway <= 0 ? "var(--imessage-red)" : "rgba(20,20,30,0.55)",
  fontWeight: 600,
});

const addBtn: React.CSSProperties = {
  background: "var(--imessage-blue)",
  color: "white",
  border: "none",
  borderRadius: 999,
  padding: "10px 20px",
  fontSize: 14,
  fontWeight: 600,
  cursor: "pointer",
  marginTop: 12,
};

const fairnessBanner: React.CSSProperties = {
  background: "rgba(255,193,92,0.12)",
  borderRadius: "var(--radius-md)",
  padding: "10px 14px",
  marginBottom: 12,
  fontSize: 13,
  color: "rgba(20,20,30,0.7)",
};

function daysUntil(ms: number): number {
  return Math.round((ms - Date.now()) / 86_400_000);
}

function formatDueBadge(days: number): string {
  if (days < 0) return `${-days}d overdue`;
  if (days === 0) return "Due today";
  if (days === 1) return "Tomorrow";
  if (days < 7) return `In ${days}d`;
  return new Date(Date.now() + days * 86_400_000).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export default function ChoresView() {
  const allChores = useChoresStore((s) => s.allChores);
  const setAllChores = useChoresStore((s) => s.setAllChores);
  const fairnessNudges = useChoresStore((s) => s.fairnessNudges);
  const setFairnessNudges = useChoresStore((s) => s.setFairnessNudges);
  const dismissFairnessNudge = useChoresStore((s) => s.dismissFairnessNudge);

  const [editing, setEditing] = useState<Chore | null>(null);
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    void listAllChores().then(setAllChores);
    void checkChoreFairness().then(setFairnessNudges);
  }, [setAllChores, setFairnessNudges]);

  const dueSoon = allChores
    .filter((c) => daysUntil(c.next_due) <= 7)
    .sort((a, b) => a.next_due - b.next_due);

  return (
    <div style={pageStyle}>
      <h1 style={{ fontSize: 24, fontWeight: 700, margin: "0 0 16px" }}>Chores</h1>

      {fairnessNudges.map((n) => (
        <div key={n.chore_id} style={fairnessBanner}>
          <span>
            <b>{n.person_name}</b> hasn't done <b>{n.chore_title}</b> in {n.days_ago} days — might be worth a nudge.
          </span>
          <button
            onClick={() => dismissFairnessNudge(n.chore_id)}
            style={{ float: "right", background: "transparent", border: "none", color: "rgba(20,20,30,0.5)", cursor: "pointer", fontSize: 12 }}
          >
            Dismiss
          </button>
        </div>
      ))}

      <section style={sectionStyle}>
        <h2 style={headerStyle}>Due soon</h2>
        {dueSoon.length === 0 ? (
          <p style={{ color: "rgba(20,20,30,0.5)", fontSize: 13, margin: 0 }}>Nothing in the next 7 days.</p>
        ) : (
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {dueSoon.map((c) => {
              const days = daysUntil(c.next_due);
              return (
                <li key={c.id} style={rowStyle} onClick={() => setEditing(c)}>
                  <span style={{ fontSize: 18 }}>{c.emoji}</span>
                  <span style={{ flex: 1, fontSize: 14 }}>{c.title}</span>
                  <span style={dueBadge(days)}>{formatDueBadge(days)}</span>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <section style={sectionStyle}>
        <h2 style={headerStyle}>All chores</h2>
        {allChores.length === 0 ? (
          <p style={{ color: "rgba(20,20,30,0.5)", fontSize: 13, margin: 0 }}>No chores yet — add your first one.</p>
        ) : (
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {[...allChores].sort((a, b) => a.title.localeCompare(b.title)).map((c) => (
              <li key={c.id} style={rowStyle} onClick={() => setEditing(c)}>
                <span style={{ fontSize: 18 }}>{c.emoji}</span>
                <span style={{ flex: 1, fontSize: 14 }}>{c.title}</span>
                <span style={{ fontSize: 11, color: "rgba(20,20,30,0.45)" }}>{c.rotation === "none" ? "" : c.rotation}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <button style={addBtn} onClick={() => setCreating(true)}>+ Add chore</button>

      {creating && <ChoreDrawer chore={null} onClose={() => setCreating(false)} />}
      {editing && <ChoreDrawer chore={editing} onClose={() => setEditing(null)} />}
    </div>
  );
}
```

- [ ] **Step 3: Verify TypeScript + run tests**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm/apps/desktop
npx tsc --noEmit 2>&1 | head -10
npx vitest run 2>&1 | grep -E "Tests |FAIL" | tail -3
```

Expected: no TS errors, all tests pass.

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
git add apps/desktop/src/components/Chores/
git commit -m "feat(chores): management view with drawer (create/edit/history) + fairness banner"
```

---

### Task 10: Time Blocks management view + drawer

**Files:**
- Replace placeholder: `apps/desktop/src/components/TimeBlocks/TimeBlocksView.tsx`
- Create: `apps/desktop/src/components/TimeBlocks/BlockDrawer.tsx`

- [ ] **Step 1: Create BlockDrawer**

Create `apps/desktop/src/components/TimeBlocks/BlockDrawer.tsx`:

```tsx
import { useState } from "react";
import type { TimeBlock, BlockKind } from "../../lib/timeblocks/ipc";
import { createTimeBlock, updateTimeBlock, deleteTimeBlock } from "../../lib/timeblocks/ipc";
import { useTimeBlocksStore } from "../../lib/timeblocks/state";

const overlayStyle: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "rgba(20,20,30,0.2)",
  zIndex: 100,
  display: "flex",
  justifyContent: "flex-end",
};

const drawerStyle: React.CSSProperties = {
  width: 420,
  background: "var(--paper)",
  borderLeft: "1px solid var(--hairline)",
  boxShadow: "var(--shadow-lg)",
  padding: 24,
  overflowY: "auto",
  animation: "drawerIn 0.2s ease-out",
};

const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: 11,
  fontWeight: 700,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)",
  marginBottom: 6,
  marginTop: 14,
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "8px 12px",
  borderRadius: 8,
  border: "1px solid var(--hairline)",
  fontSize: 14,
  fontFamily: "inherit",
};

const btnPrimary: React.CSSProperties = {
  background: "var(--imessage-blue)",
  color: "white",
  border: "none",
  borderRadius: 999,
  padding: "8px 18px",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
};

const btnGhost: React.CSSProperties = {
  background: "transparent",
  color: "rgba(20,20,30,0.55)",
  border: "1px solid var(--hairline)",
  borderRadius: 999,
  padding: "8px 18px",
  fontSize: 13,
  cursor: "pointer",
};

const btnDanger: React.CSSProperties = {
  background: "transparent",
  color: "var(--imessage-red)",
  border: "none",
  fontSize: 12,
  cursor: "pointer",
  padding: "6px 0",
  marginTop: 12,
};

function toISODate(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10);
}

function fromISODate(iso: string): number {
  const d = new Date(iso + "T00:00:00Z");
  return d.getTime();
}

interface Props {
  block: TimeBlock | null;
  onClose: () => void;
}

export default function BlockDrawer({ block, onClose }: Props) {
  const upsertBlock = useTimeBlocksStore((s) => s.upsertBlock);
  const removeBlock = useTimeBlocksStore((s) => s.removeBlock);
  const setPatternSuggestion = useTimeBlocksStore((s) => s.setPatternSuggestion);

  const [title, setTitle] = useState(block?.title ?? "");
  const [kind, setKind] = useState<BlockKind>(block?.kind ?? "focus");
  const [dateStr, setDateStr] = useState(toISODate(block?.date ?? Date.now()));
  const [startTime, setStartTime] = useState(block?.start_time ?? "09:00");
  const [endTime, setEndTime] = useState(block?.end_time ?? "10:00");

  async function onSave() {
    if (!title.trim()) return;
    if (block) {
      const updated = await updateTimeBlock({
        id: block.id,
        title: title.trim(),
        kind,
        dateMs: fromISODate(dateStr),
        startTime,
        endTime,
      });
      upsertBlock(updated);
    } else {
      const result = await createTimeBlock({
        title: title.trim(),
        kind,
        dateMs: fromISODate(dateStr),
        startTime,
        endTime,
      });
      upsertBlock(result.block);
      if (result.suggestion) setPatternSuggestion(result.suggestion);
    }
    onClose();
  }

  async function onDelete() {
    if (!block) return;
    await deleteTimeBlock(block.id);
    removeBlock(block.id);
    onClose();
  }

  return (
    <div style={overlayStyle} onClick={onClose}>
      <aside style={drawerStyle} onClick={(e) => e.stopPropagation()}>
        <h2 style={{ margin: "0 0 16px", fontSize: 20, fontWeight: 700 }}>
          {block ? "Edit block" : "New block"}
        </h2>

        <label style={labelStyle}>Title</label>
        <input style={inputStyle} value={title} onChange={(e) => setTitle(e.target.value)} placeholder="e.g. Deep work" aria-label="Title" />

        <label style={labelStyle}>Kind</label>
        <select style={inputStyle} value={kind} onChange={(e) => setKind(e.target.value as BlockKind)} aria-label="Kind">
          <option value="focus">Focus</option>
          <option value="errands">Errands</option>
          <option value="admin">Admin</option>
          <option value="dnd">Do Not Disturb</option>
        </select>

        <label style={labelStyle}>Date</label>
        <input style={inputStyle} type="date" value={dateStr} onChange={(e) => setDateStr(e.target.value)} aria-label="Date" />

        <div style={{ display: "flex", gap: 10 }}>
          <div style={{ flex: 1 }}>
            <label style={labelStyle}>Start</label>
            <input style={inputStyle} type="time" value={startTime} onChange={(e) => setStartTime(e.target.value)} aria-label="Start time" />
          </div>
          <div style={{ flex: 1 }}>
            <label style={labelStyle}>End</label>
            <input style={inputStyle} type="time" value={endTime} onChange={(e) => setEndTime(e.target.value)} aria-label="End time" />
          </div>
        </div>

        <div style={{ display: "flex", gap: 8, marginTop: 24 }}>
          <button style={btnPrimary} onClick={onSave}>{block ? "Save" : "Create"}</button>
          <button style={btnGhost} onClick={onClose}>Cancel</button>
        </div>

        {block && <button style={btnDanger} onClick={onDelete}>Delete block</button>}
      </aside>
    </div>
  );
}
```

- [ ] **Step 2: Replace TimeBlocksView placeholder with real view**

Overwrite `apps/desktop/src/components/TimeBlocks/TimeBlocksView.tsx`:

```tsx
import { useEffect, useState } from "react";
import { useTimeBlocksStore } from "../../lib/timeblocks/state";
import { listBlocksForWeek, listRecurringBlocks, type TimeBlock, type BlockKind } from "../../lib/timeblocks/ipc";
import BlockDrawer from "./BlockDrawer";

const pageStyle: React.CSSProperties = {
  maxWidth: 760,
  margin: "0 auto",
  padding: "24px 24px 120px",
};

const sectionStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
  marginBottom: 12,
};

const headerStyle: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 700,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)",
  marginBottom: 10,
};

const dayHeading: React.CSSProperties = {
  fontSize: 12,
  fontWeight: 700,
  color: "rgba(20,20,30,0.6)",
  marginTop: 12,
  marginBottom: 4,
};

const KIND_COLOR: Record<BlockKind, string> = {
  focus: "#007aff",
  errands: "#FFC15C",
  admin: "#9b59b6",
  dnd: "#ff3b30",
};

const KIND_LABEL: Record<BlockKind, string> = {
  focus: "Focus",
  errands: "Errands",
  admin: "Admin",
  dnd: "DND",
};

const rowStyle = (kind: BlockKind): React.CSSProperties => ({
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "8px 10px",
  borderLeft: `3px solid ${KIND_COLOR[kind]}`,
  background: "rgba(20,20,30,0.02)",
  borderRadius: 6,
  marginBottom: 4,
  cursor: "pointer",
  fontSize: 13,
});

const addBtn: React.CSSProperties = {
  background: "var(--imessage-blue)",
  color: "white",
  border: "none",
  borderRadius: 999,
  padding: "10px 20px",
  fontSize: 14,
  fontWeight: 600,
  cursor: "pointer",
  marginTop: 12,
};

const DAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

function weekStartMs(): number {
  const now = new Date();
  const day = (now.getDay() + 6) % 7; // 0 = Monday
  const monday = new Date(now);
  monday.setDate(now.getDate() - day);
  monday.setHours(0, 0, 0, 0);
  return monday.getTime();
}

function rruleToEnglish(rrule: string): string {
  if (rrule.includes("FREQ=WEEKLY") && rrule.includes("BYDAY=")) {
    const day = rrule.match(/BYDAY=([A-Z]{2})/)?.[1];
    const map: Record<string, string> = {
      MO: "Mondays", TU: "Tuesdays", WE: "Wednesdays", TH: "Thursdays",
      FR: "Fridays", SA: "Saturdays", SU: "Sundays",
    };
    return `Every ${map[day ?? ""] ?? "week"}`;
  }
  if (rrule.includes("FREQ=WEEKDAY") || rrule === "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR") {
    return "Every weekday";
  }
  if (rrule === "FREQ=DAILY") return "Every day";
  if (rrule === "FREQ=MONTHLY") return "Every month";
  return rrule;
}

export default function TimeBlocksView() {
  const weekBlocks = useTimeBlocksStore((s) => s.weekBlocks);
  const setWeekBlocks = useTimeBlocksStore((s) => s.setWeekBlocks);
  const recurring = useTimeBlocksStore((s) => s.recurringBlocks);
  const setRecurring = useTimeBlocksStore((s) => s.setRecurringBlocks);

  const [editing, setEditing] = useState<TimeBlock | null>(null);
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    void listBlocksForWeek(weekStartMs()).then(setWeekBlocks);
    void listRecurringBlocks().then(setRecurring);
  }, [setWeekBlocks, setRecurring]);

  // Group week blocks by weekday
  const byDay: Record<string, TimeBlock[]> = Object.fromEntries(DAYS.map((d) => [d, []]));
  for (const b of weekBlocks) {
    const wd = new Date(b.date).getUTCDay();
    const name = DAYS[(wd + 6) % 7];
    byDay[name].push(b);
  }

  return (
    <div style={pageStyle}>
      <h1 style={{ fontSize: 24, fontWeight: 700, margin: "0 0 16px" }}>Time Blocks</h1>

      <section style={sectionStyle}>
        <h2 style={headerStyle}>This week</h2>
        {weekBlocks.length === 0 ? (
          <p style={{ color: "rgba(20,20,30,0.5)", fontSize: 13, margin: 0 }}>No blocks this week yet.</p>
        ) : (
          DAYS.map((day) => {
            const bs = byDay[day];
            if (bs.length === 0) return null;
            return (
              <div key={day}>
                <div style={dayHeading}>{day}</div>
                {bs.sort((a, b) => a.start_time.localeCompare(b.start_time)).map((b) => (
                  <div key={b.id} style={rowStyle(b.kind)} onClick={() => setEditing(b)}>
                    <strong style={{ color: KIND_COLOR[b.kind], fontWeight: 700, fontSize: 10, textTransform: "uppercase", letterSpacing: 0.5, minWidth: 50 }}>
                      {KIND_LABEL[b.kind]}
                    </strong>
                    <span style={{ flex: 1 }}>{b.title}</span>
                    <span style={{ color: "rgba(20,20,30,0.5)", fontSize: 12 }}>
                      {b.start_time}–{b.end_time}
                    </span>
                  </div>
                ))}
              </div>
            );
          })
        )}
      </section>

      <section style={sectionStyle}>
        <h2 style={headerStyle}>Recurring patterns</h2>
        {recurring.length === 0 ? (
          <p style={{ color: "rgba(20,20,30,0.5)", fontSize: 13, margin: 0 }}>No patterns yet. Nell will suggest one when she notices a repetition.</p>
        ) : (
          recurring.map((b) => (
            <div key={b.id} style={rowStyle(b.kind)} onClick={() => setEditing(b)}>
              <strong style={{ color: KIND_COLOR[b.kind], fontWeight: 700, fontSize: 10, textTransform: "uppercase", letterSpacing: 0.5, minWidth: 50 }}>
                {KIND_LABEL[b.kind]}
              </strong>
              <span style={{ flex: 1 }}>{b.title}</span>
              <span style={{ color: "rgba(20,20,30,0.5)", fontSize: 12 }}>
                {b.rrule ? rruleToEnglish(b.rrule) : ""} · {b.start_time}–{b.end_time}
              </span>
            </div>
          ))
        )}
      </section>

      <button style={addBtn} onClick={() => setCreating(true)}>+ Add block</button>

      {creating && <BlockDrawer block={null} onClose={() => setCreating(false)} />}
      {editing && <BlockDrawer block={editing} onClose={() => setEditing(null)} />}
    </div>
  );
}
```

- [ ] **Step 3: Verify TypeScript + run tests**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm/apps/desktop
npx tsc --noEmit 2>&1 | head -10
npx vitest run 2>&1 | grep -E "Tests |FAIL" | tail -3
```

Expected: no TS errors, all tests pass.

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
git add apps/desktop/src/components/TimeBlocks/
git commit -m "feat(timeblocks): management view with drawer + recurring patterns section"
```

---

### Task 11: Full-build sanity + smoke test + finish branch

**Files:**
- No file changes (verification-only)

- [ ] **Step 1: Full workspace build (debug)**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
cargo build --workspace 2>&1 | tail -20
```

Expected: `Finished` with no errors.

- [ ] **Step 2: Full Rust test suite**

```bash
cargo test --workspace 2>&1 | grep -E "^test result|FAILED"
```

Expected: all `test result: ok.` — baseline 66 + 20 new (~86 total).

- [ ] **Step 3: Frontend test suite**

```bash
cd apps/desktop
npx vitest run 2>&1 | grep -E "Tests |FAIL" | tail -3
```

Expected: `Tests  40 passed` (or higher).

- [ ] **Step 4: Lint**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
cargo clippy --workspace -- -D warnings 2>&1 | grep -E "^error|^warning" | head -10
```

Expected: no errors.

```bash
cargo fmt --all --check
```

Expected: no output (code is formatted).

- [ ] **Step 5: Manual smoke test**

Start the dev server:

```bash
cd apps/desktop
pnpm tauri dev
```

Walk through:

1. **Sidebar appears** on launch. Active icon is Today (house).
2. **Today view loads** — existing cards render, plus empty ChoresCard ("All clear today 🧹") and empty TimeBlocksCard ("No blocks today — time is yours.").
3. **Add a time block** from the TimeBlocksCard "+ Add" — fill Focus / 09:00–11:00, save. Pill appears.
4. **Click Chores in sidebar** → Chores view loads (empty states visible).
5. **+ Add chore** → drawer opens, fill "Bins" / 🗑️ / "Every week" / "No rotation" / Create. Row appears in "All chores".
6. **Back to Today** → ChoresCard now shows "Bins" (if next_due ≤ today).
7. **Tap the Bins row** → it completes and fades out.
8. **Navigate to Time Blocks view** → "This week" section shows the focus block under Tuesday (or whatever day).
9. **Cmd+,** opens Settings modal (existing behaviour, verify not broken).
10. **Pattern nudge flow (optional, slow)**: insert three focus blocks on the same weekday over the last 6 weeks manually via SQL or create 3 blocks → on the 3rd create call, the TimeBlocksCard shows a pattern nudge banner.

- [ ] **Step 6: Push branch**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-4-rhythm
git push -u origin feature/phase-4-rhythm
```

- [ ] **Step 7: Offer PR via finishing-a-development-branch**

Once all above is green, invoke the `superpowers:finishing-a-development-branch` skill to offer merge/PR/keep/discard options.

---

## Deliverable summary

| Area | Change |
|---|---|
| Migrations | V4 adds 5 tables (person, chore, chore_completion, rotation, time_block) |
| Rust DAL | `chore.rs` + `time_block.rs` in manor-core |
| Rust cmds | New `rhythm` module in manor-app with 20 commands |
| Frontend libs | `lib/chores/`, `lib/timeblocks/`, `lib/nav.ts` |
| Frontend UI | Sidebar + ChoresCard + TimeBlocksCard + ChoresView + TimeBlocksView + 2 drawers |
| Tests | 20 new Rust tests (12 chore + 8 time_block) + 13 new Vitest tests (7 chores + 6 timeblocks) |
| Dependencies | Add `rrule` to `manor-core` Cargo.toml (already in workspace) |

Baseline → After:
- Rust tests: 66 → ~86
- Frontend tests: 27 → ~40
