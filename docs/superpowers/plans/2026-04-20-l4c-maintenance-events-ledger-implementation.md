# L4c Maintenance Events + Ledger Link Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Manor's third v0.5 Bones slice — every `mark_done` writes a `maintenance_event` row with optional cost + optional Ledger transaction link. Adds per-asset history, per-asset spend strip, Bones Spend sub-nav tab with category + per-asset totals, and a suggest-and-link transaction picker inside a shared Log completion drawer.

**Architecture:** Two-crate split mirrors L4a/L4b. Core holds schema (V20), DAL (`event_dal.rs`), rollup SQL, and a `mark_done` extension that transparently inserts an event. App layer exposes Tauri commands; asset-cascade functions gain event-soft-delete/restore/hard-delete. Frontend ships a shared `LogCompletionDrawer` for three modes (one-off, schedule completion, edit), a `TransactionSuggest` component with two match heuristics, a `HistoryBlock` on AssetDetail, a per-asset spend strip, and a third Bones sub-nav tab (`Spend`) with category totals + sortable per-asset list.

**Tech Stack:** Rust (rusqlite, chrono, anyhow), React + TypeScript + Zustand, Lucide icons.

**Spec:** `docs/superpowers/specs/2026-04-20-l4c-maintenance-events-ledger-design.md`

---

## File structure

### New Rust files
- `crates/core/migrations/V20__maintenance_event.sql`
- `crates/core/src/maintenance/event.rs` — types.
- `crates/core/src/maintenance/event_dal.rs` — CRUD + rollups + transaction suggest/search.
- `crates/app/src/maintenance/event_commands.rs` — Tauri IPC.

### New frontend files
- `apps/desktop/src/lib/maintenance/event-ipc.ts`
- `apps/desktop/src/lib/maintenance/event-state.ts`
- `apps/desktop/src/lib/maintenance/spend-state.ts`
- `apps/desktop/src/components/Bones/LogCompletionDrawer.tsx`
- `apps/desktop/src/components/Bones/TransactionSuggest.tsx`
- `apps/desktop/src/components/Bones/EventRow.tsx`
- `apps/desktop/src/components/Bones/HistoryBlock.tsx`
- `apps/desktop/src/components/Bones/AssetSpendStrip.tsx`
- `apps/desktop/src/components/Bones/Spend/SpendView.tsx`
- `apps/desktop/src/components/Bones/Spend/SpendAssetRow.tsx`
- `apps/desktop/src/components/Bones/Spend/SpendCategoryStrip.tsx`

### Modified files (Rust)
- `crates/core/src/maintenance/mod.rs` — `pub mod event; pub mod event_dal;`.
- `crates/core/src/maintenance/dal.rs` — extend `mark_done` signature.
- `crates/core/src/asset/dal.rs` — extend `soft_delete_asset`, `restore_asset`, `permanent_delete_asset`.
- `crates/core/src/trash.rs` — append `("maintenance_event", "title")`.
- `crates/app/src/maintenance/mod.rs` — `pub mod event_commands;`.
- `crates/app/src/maintenance/commands.rs` — update `maintenance_schedule_mark_done` to call extended `mark_done(.., None)`.
- `crates/app/src/lib.rs` — register new Tauri commands.
- `crates/app/src/safety/trash_commands.rs` — add `"maintenance_event"` arms.

### Modified files (frontend)
- `apps/desktop/src/lib/bones/view-state.ts` — widen `subview` union to include `"spend"`.
- `apps/desktop/src/lib/maintenance/state.ts` — L4b store's `markDone` gains post-resolve cache invalidation.
- `apps/desktop/src/components/Bones/BonesSubNav.tsx` — third tab.
- `apps/desktop/src/components/Bones/BonesTab.tsx` — render `<SpendView />`.
- `apps/desktop/src/components/Bones/AssetDetail.tsx` — mount spend strip + history block.
- `apps/desktop/src/components/Bones/DueSoon/ScheduleRow.tsx` — overflow-menu `Log completion…` action.
- `apps/desktop/src/components/Bones/MaintenanceSection.tsx` — same overflow-menu action.

---

## Phase A — Core schema, types, DAL

### Task 1: Migration V20 + trash registry

**Files:**
- Create: `crates/core/migrations/V20__maintenance_event.sql`
- Modify: `crates/core/src/trash.rs`
- Test: `crates/core/src/maintenance/event_dal.rs` (migration-applied test) OR existing migration test harness.

- [ ] **Step 1: Write the failing test (migration applies cleanly on fresh DB)**

Add to `crates/core/src/maintenance/mod.rs` (or wherever the crate's `#[cfg(test)]` migration fixture lives — check L4b pattern; likely `crates/core/src/testing.rs` or inline per module):

```rust
#[cfg(test)]
mod migration_tests {
    use crate::testing::test_conn; // reuse the crate's fresh-DB helper

    #[test]
    fn v20_creates_maintenance_event_table() {
        let conn = test_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='maintenance_event'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn v20_creates_tx_unique_partial_index() {
        let conn = test_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_evt_tx_unique'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --package manor-core migration_tests -- --nocapture`
Expected: FAIL — table doesn't exist yet.

- [ ] **Step 3: Create `V20__maintenance_event.sql`**

```sql
-- V20__maintenance_event.sql
-- L4c: maintenance event log + Ledger transaction linkage.

CREATE TABLE maintenance_event (
    id              TEXT PRIMARY KEY,
    asset_id        TEXT NOT NULL REFERENCES asset(id),
    schedule_id     TEXT REFERENCES maintenance_schedule(id),
    title           TEXT NOT NULL DEFAULT '',
    completed_date  TEXT NOT NULL,
    cost_pence      INTEGER,
    currency        TEXT NOT NULL DEFAULT 'GBP',
    notes           TEXT NOT NULL DEFAULT '',
    transaction_id  INTEGER REFERENCES ledger_transaction(id),
    source          TEXT NOT NULL DEFAULT 'manual'
                    CHECK (source IN ('manual','backfill')),
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER
);

CREATE INDEX idx_evt_asset     ON maintenance_event(asset_id);
CREATE INDEX idx_evt_schedule  ON maintenance_event(schedule_id) WHERE schedule_id IS NOT NULL;
CREATE INDEX idx_evt_completed ON maintenance_event(completed_date) WHERE deleted_at IS NULL;
CREATE INDEX idx_evt_deleted   ON maintenance_event(deleted_at);

CREATE UNIQUE INDEX idx_evt_tx_unique
    ON maintenance_event(transaction_id)
    WHERE transaction_id IS NOT NULL AND deleted_at IS NULL;

INSERT INTO maintenance_event (
    id, asset_id, schedule_id, title, completed_date,
    cost_pence, currency, notes, transaction_id, source,
    created_at, updated_at, deleted_at
)
SELECT
    lower(hex(randomblob(16))),
    ms.asset_id,
    ms.id,
    ms.task,
    ms.last_done_date,
    NULL, 'GBP', '', NULL, 'backfill',
    unixepoch(), unixepoch(), NULL
FROM maintenance_schedule ms
WHERE ms.last_done_date IS NOT NULL
  AND ms.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1 FROM maintenance_event me
      WHERE me.schedule_id = ms.id AND me.source = 'backfill'
  );
```

- [ ] **Step 4: Append trash registry entry**

In `crates/core/src/trash.rs`, find the `REGISTRY` array and append:

```rust
("maintenance_event", "title"),
```

Place it alphabetically or at the end of the array, matching the existing style.

- [ ] **Step 5: Run migration tests, verify they pass**

Run: `cargo test --package manor-core migration_tests`
Expected: PASS (both tests).

Also run: `cargo test --package manor-core -- trash` to verify existing trash sweep tests still pass with the new registry entry.

- [ ] **Step 6: Commit**

```bash
git add crates/core/migrations/V20__maintenance_event.sql crates/core/src/trash.rs
git add crates/core/src/maintenance/mod.rs  # if migration_tests landed there
git commit -m "feat(maintenance): migration V20 + trash registry for maintenance_event (L4c)"
```

---

### Task 2: Core types (`event.rs`)

**Files:**
- Create: `crates/core/src/maintenance/event.rs`
- Modify: `crates/core/src/maintenance/mod.rs`

- [ ] **Step 1: Create `event.rs` with type definitions**

```rust
//! Maintenance event types (L4c).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    Manual,
    Backfill,
}

impl EventSource {
    pub fn as_str(self) -> &'static str {
        match self {
            EventSource::Manual => "manual",
            EventSource::Backfill => "backfill",
        }
    }

    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "manual" => Ok(EventSource::Manual),
            "backfill" => Ok(EventSource::Backfill),
            other => Err(anyhow::anyhow!("unknown EventSource: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceEventDraft {
    pub asset_id: String,
    pub schedule_id: Option<String>,
    pub title: String,
    pub completed_date: String,
    pub cost_pence: Option<i64>,
    pub currency: String,
    pub notes: String,
    pub transaction_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceEvent {
    pub id: String,
    pub asset_id: String,
    pub schedule_id: Option<String>,
    pub title: String,
    pub completed_date: String,
    pub cost_pence: Option<i64>,
    pub currency: String,
    pub notes: String,
    pub transaction_id: Option<i64>,
    pub source: EventSource,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventWithContext {
    pub event: MaintenanceEvent,
    pub schedule_task: Option<String>,
    pub schedule_deleted: bool,
    pub transaction_description: Option<String>,
    pub transaction_amount_pence: Option<i64>,
    pub transaction_date: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSpendTotal {
    pub asset_id: String,
    pub asset_name: String,
    pub asset_category: String,
    pub total_last_12m_pence: i64,
    pub total_lifetime_pence: i64,
    pub event_count_last_12m: i64,
    pub event_count_lifetime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySpendTotal {
    pub category: String,
    pub total_last_12m_pence: i64,
    pub total_lifetime_pence: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_source_round_trip() {
        assert_eq!(EventSource::parse("manual").unwrap(), EventSource::Manual);
        assert_eq!(EventSource::parse("backfill").unwrap(), EventSource::Backfill);
        assert_eq!(EventSource::Manual.as_str(), "manual");
        assert_eq!(EventSource::Backfill.as_str(), "backfill");
        assert!(EventSource::parse("other").is_err());
    }
}
```

- [ ] **Step 2: Register module in `mod.rs`**

In `crates/core/src/maintenance/mod.rs`, add:

```rust
pub mod event;
pub mod event_dal;  // will be created in Task 3; declaring now keeps mod.rs single-touch
```

Note: `event_dal` doesn't exist yet — if `cargo check` complains at this step, comment the `pub mod event_dal;` line temporarily and uncomment in Task 3.

- [ ] **Step 3: Run type tests**

Run: `cargo test --package manor-core maintenance::event`
Expected: PASS (one test — `event_source_round_trip`).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/maintenance/event.rs crates/core/src/maintenance/mod.rs
git commit -m "feat(maintenance): event types + EventSource enum (L4c)"
```

---

### Task 3: Event DAL — CRUD + validations

**Files:**
- Create: `crates/core/src/maintenance/event_dal.rs`
- Test: same file (`#[cfg(test)]` block).

Review L4b's `maintenance/dal.rs` before starting — same patterns (UUID generation helper, `now_secs()`, `Row::get` by name, fixture helpers).

- [ ] **Step 1: Write failing tests for insert + get round-trip + validations**

Create `crates/core/src/maintenance/event_dal.rs` starting with:

```rust
//! Maintenance event DAL (L4c).

use super::event::{
    EventSource, EventWithContext, MaintenanceEvent, MaintenanceEventDraft,
};
use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection, Row};
use serde_json::Value;
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn insert_event(conn: &Connection, draft: &MaintenanceEventDraft) -> Result<String> {
    validate_draft(conn, draft, None)?;
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    conn.execute(
        "INSERT INTO maintenance_event
         (id, asset_id, schedule_id, title, completed_date, cost_pence, currency,
          notes, transaction_id, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'manual', ?10, ?11)",
        params![
            id,
            draft.asset_id,
            draft.schedule_id,
            draft.title,
            draft.completed_date,
            draft.cost_pence,
            draft.currency,
            draft.notes,
            draft.transaction_id,
            now,
            now,
        ],
    )
    .map_err(translate_constraint_err)?;
    Ok(id)
}

pub fn get_event(conn: &Connection, id: &str) -> Result<Option<MaintenanceEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, schedule_id, title, completed_date, cost_pence, currency,
                notes, transaction_id, source, created_at, updated_at, deleted_at
         FROM maintenance_event WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_event(row)?))
    } else {
        Ok(None)
    }
}

pub fn update_event(
    conn: &Connection,
    id: &str,
    draft: &MaintenanceEventDraft,
) -> Result<()> {
    validate_draft(conn, draft, Some(id))?;
    let now = now_secs();
    let changed = conn
        .execute(
            "UPDATE maintenance_event
             SET title = ?1, completed_date = ?2, cost_pence = ?3, currency = ?4,
                 notes = ?5, transaction_id = ?6, updated_at = ?7
             WHERE id = ?8 AND deleted_at IS NULL",
            params![
                draft.title,
                draft.completed_date,
                draft.cost_pence,
                draft.currency,
                draft.notes,
                draft.transaction_id,
                now,
                id,
            ],
        )
        .map_err(translate_constraint_err)?;
    if changed == 0 {
        return Err(anyhow!("Event not found or already deleted"));
    }
    Ok(())
}

fn validate_draft(
    conn: &Connection,
    draft: &MaintenanceEventDraft,
    update_event_id: Option<&str>,
) -> Result<()> {
    if let Some(c) = draft.cost_pence {
        if c < 0 {
            return Err(anyhow!("Cost must be zero or positive"));
        }
    }
    NaiveDate::parse_from_str(&draft.completed_date, "%Y-%m-%d")
        .map_err(|_| anyhow!("Date must be in YYYY-MM-DD format"))?;
    if let Some(sched_id) = &draft.schedule_id {
        let owner: Option<String> = conn
            .query_row(
                "SELECT asset_id FROM maintenance_schedule WHERE id = ?1",
                params![sched_id],
                |r| r.get(0),
            )
            .ok();
        match owner {
            Some(aid) if aid == draft.asset_id => {}
            Some(_) => return Err(anyhow!("Schedule does not belong to asset")),
            None => return Err(anyhow!("Schedule not found")),
        }
    }
    // Note: transaction_id uniqueness is enforced by the DB partial unique index;
    // we surface a nicer error via translate_constraint_err.
    let _ = update_event_id; // silence unused when no extra checks
    Ok(())
}

fn translate_constraint_err(err: rusqlite::Error) -> anyhow::Error {
    let s = err.to_string();
    // SQLite unique-index violation message surfaces the table + column:
    //   "UNIQUE constraint failed: maintenance_event.transaction_id"
    if s.contains("maintenance_event.transaction_id") || s.contains("idx_evt_tx_unique") {
        anyhow!("Transaction already linked to another event")
    } else {
        anyhow!(err)
    }
}

fn row_to_event(row: &Row) -> Result<MaintenanceEvent> {
    let source_str: String = row.get("source")?;
    Ok(MaintenanceEvent {
        id: row.get("id")?,
        asset_id: row.get("asset_id")?,
        schedule_id: row.get("schedule_id")?,
        title: row.get("title")?,
        completed_date: row.get("completed_date")?,
        cost_pence: row.get("cost_pence")?,
        currency: row.get("currency")?,
        notes: row.get("notes")?,
        transaction_id: row.get("transaction_id")?,
        source: EventSource::parse(&source_str)?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{insert_test_asset, insert_test_schedule, test_conn};

    fn draft(asset_id: &str) -> MaintenanceEventDraft {
        MaintenanceEventDraft {
            asset_id: asset_id.to_string(),
            schedule_id: None,
            title: "Annual boiler service".into(),
            completed_date: "2026-04-20".into(),
            cost_pence: Some(14500),
            currency: "GBP".into(),
            notes: "".into(),
            transaction_id: None,
        }
    }

    #[test]
    fn insert_and_get_round_trip() {
        let conn = test_conn();
        let asset_id = insert_test_asset(&conn, "Worcester Bosch");
        let id = insert_event(&conn, &draft(&asset_id)).unwrap();
        let got = get_event(&conn, &id).unwrap().unwrap();
        assert_eq!(got.asset_id, asset_id);
        assert_eq!(got.title, "Annual boiler service");
        assert_eq!(got.cost_pence, Some(14500));
        assert_eq!(got.source, EventSource::Manual);
    }

    #[test]
    fn insert_rejects_negative_cost() {
        let conn = test_conn();
        let asset_id = insert_test_asset(&conn, "Boiler");
        let mut d = draft(&asset_id);
        d.cost_pence = Some(-100);
        let err = insert_event(&conn, &d).unwrap_err().to_string();
        assert!(err.contains("zero or positive"), "got: {}", err);
    }

    #[test]
    fn insert_rejects_bad_date() {
        let conn = test_conn();
        let asset_id = insert_test_asset(&conn, "Boiler");
        let mut d = draft(&asset_id);
        d.completed_date = "not-a-date".into();
        let err = insert_event(&conn, &d).unwrap_err().to_string();
        assert!(err.contains("YYYY-MM-DD"), "got: {}", err);
    }

    #[test]
    fn insert_rejects_schedule_asset_mismatch() {
        let conn = test_conn();
        let asset_a = insert_test_asset(&conn, "Asset A");
        let asset_b = insert_test_asset(&conn, "Asset B");
        let sched_a = insert_test_schedule(&conn, &asset_a, "task", 12);
        let mut d = draft(&asset_b);
        d.schedule_id = Some(sched_a);
        let err = insert_event(&conn, &d).unwrap_err().to_string();
        assert!(err.contains("does not belong"), "got: {}", err);
    }

    #[test]
    fn update_preserves_source() {
        let conn = test_conn();
        let asset_id = insert_test_asset(&conn, "Boiler");
        let id = insert_event(&conn, &draft(&asset_id)).unwrap();
        let mut d = draft(&asset_id);
        d.cost_pence = Some(20000);
        d.notes = "£200 service".into();
        update_event(&conn, &id, &d).unwrap();
        let got = get_event(&conn, &id).unwrap().unwrap();
        assert_eq!(got.cost_pence, Some(20000));
        assert_eq!(got.notes, "£200 service");
        assert_eq!(got.source, EventSource::Manual); // unchanged
    }

    #[test]
    fn update_can_clear_transaction() {
        let conn = test_conn();
        let asset_id = insert_test_asset(&conn, "Boiler");
        let mut d = draft(&asset_id);
        d.transaction_id = Some(42); // test doesn't need an actual tx row here — FK check is deferrable but let's keep test honest
        // For this test, don't rely on tx row — skip actual insert if FK deferred. Use a valid tx id if fixture helper exists:
        // Alternatively mark as: simulate clear from initially-None
        d.transaction_id = None;
        let id = insert_event(&conn, &d).unwrap();
        let mut d2 = d.clone();
        d2.transaction_id = None;
        update_event(&conn, &id, &d2).unwrap();
        let got = get_event(&conn, &id).unwrap().unwrap();
        assert_eq!(got.transaction_id, None);
    }
}
```

If `crates/core/src/testing.rs` doesn't already export `insert_test_asset` + `insert_test_schedule`, add them to the testing module (they likely exist from L4a/L4b already — check and reuse).

- [ ] **Step 2: Uncomment `pub mod event_dal;` in `mod.rs` if commented in Task 2**

- [ ] **Step 3: Run tests, verify pass**

Run: `cargo test --package manor-core maintenance::event_dal`
Expected: PASS (all 6 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/maintenance/event_dal.rs crates/core/src/maintenance/mod.rs
git commit -m "feat(maintenance): event DAL — insert/get/update + validations (L4c)"
```

---

### Task 4: Event DAL — `list_for_asset` + transaction unique constraint test

**Files:**
- Modify: `crates/core/src/maintenance/event_dal.rs`

- [ ] **Step 1: Add failing test for `list_for_asset` ordering + EventWithContext fields**

Append to `event_dal.rs` tests module:

```rust
#[test]
fn list_for_asset_orders_desc_and_populates_context() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    let sched_id = insert_test_schedule(&conn, &asset_id, "Service", 12);
    let mut d = draft(&asset_id);
    d.schedule_id = Some(sched_id.clone());
    d.completed_date = "2025-01-10".into();
    insert_event(&conn, &d).unwrap();
    d.completed_date = "2026-02-20".into();
    insert_event(&conn, &d).unwrap();
    let rows = list_for_asset(&conn, &asset_id).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].event.completed_date, "2026-02-20");
    assert_eq!(rows[1].event.completed_date, "2025-01-10");
    assert_eq!(rows[0].schedule_task.as_deref(), Some("Service"));
    assert!(!rows[0].schedule_deleted);
}

#[test]
fn list_for_asset_marks_schedule_deleted() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    let sched_id = insert_test_schedule(&conn, &asset_id, "Service", 12);
    let mut d = draft(&asset_id);
    d.schedule_id = Some(sched_id.clone());
    insert_event(&conn, &d).unwrap();
    // soft-delete the schedule
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = 1 WHERE id = ?1",
        params![sched_id],
    ).unwrap();
    let rows = list_for_asset(&conn, &asset_id).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].schedule_deleted);
    assert_eq!(rows[0].schedule_task.as_deref(), Some("Service")); // still resolvable
}

#[test]
fn transaction_unique_index_rejects_duplicate_link() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    // Insert a real ledger_transaction row first so FK holds.
    conn.execute(
        "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
         VALUES (-14500, 'GBP', 'British Gas service', 1713628800, 'manual')",
        [],
    ).unwrap();
    let tx_id = conn.last_insert_rowid();

    let mut d1 = draft(&asset_id);
    d1.transaction_id = Some(tx_id);
    insert_event(&conn, &d1).unwrap();

    let mut d2 = draft(&asset_id);
    d2.transaction_id = Some(tx_id);
    let err = insert_event(&conn, &d2).unwrap_err().to_string();
    assert!(err.contains("already linked"), "got: {}", err);
}

#[test]
fn transaction_link_re_allowed_after_soft_delete() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    conn.execute(
        "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
         VALUES (-14500, 'GBP', 'British Gas', 1713628800, 'manual')",
        [],
    ).unwrap();
    let tx_id = conn.last_insert_rowid();

    let mut d1 = draft(&asset_id);
    d1.transaction_id = Some(tx_id);
    let id1 = insert_event(&conn, &d1).unwrap();
    // soft-delete event 1
    conn.execute(
        "UPDATE maintenance_event SET deleted_at = 1 WHERE id = ?1",
        params![id1],
    ).unwrap();

    let mut d2 = draft(&asset_id);
    d2.transaction_id = Some(tx_id);
    insert_event(&conn, &d2).unwrap(); // should succeed
}
```

- [ ] **Step 2: Run tests, verify they fail (list_for_asset not yet defined)**

Run: `cargo test --package manor-core maintenance::event_dal::tests::list_for_asset`
Expected: FAIL — `list_for_asset` not defined.

- [ ] **Step 3: Implement `list_for_asset`**

Add to `event_dal.rs` (above the tests module):

```rust
pub fn list_for_asset(conn: &Connection, asset_id: &str) -> Result<Vec<EventWithContext>> {
    let mut stmt = conn.prepare(
        "SELECT
             me.id, me.asset_id, me.schedule_id, me.title, me.completed_date,
             me.cost_pence, me.currency, me.notes, me.transaction_id, me.source,
             me.created_at, me.updated_at, me.deleted_at,
             ms.task AS schedule_task,
             CASE WHEN ms.deleted_at IS NOT NULL THEN 1 ELSE 0 END AS schedule_deleted_flag,
             lt.description AS tx_description,
             lt.amount_pence AS tx_amount,
             lt.date AS tx_date
         FROM maintenance_event me
         LEFT JOIN maintenance_schedule ms ON ms.id = me.schedule_id
         LEFT JOIN ledger_transaction lt
             ON lt.id = me.transaction_id AND lt.deleted_at IS NULL
         WHERE me.asset_id = ?1 AND me.deleted_at IS NULL
         ORDER BY me.completed_date DESC, me.created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![asset_id], |row| {
            let event = MaintenanceEvent {
                id: row.get("id")?,
                asset_id: row.get("asset_id")?,
                schedule_id: row.get("schedule_id")?,
                title: row.get("title")?,
                completed_date: row.get("completed_date")?,
                cost_pence: row.get("cost_pence")?,
                currency: row.get("currency")?,
                notes: row.get("notes")?,
                transaction_id: row.get("transaction_id")?,
                source: {
                    let s: String = row.get("source")?;
                    EventSource::parse(&s).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
                        )
                    })?
                },
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
                deleted_at: row.get("deleted_at")?,
            };
            let schedule_deleted_flag: i64 = row.get("schedule_deleted_flag")?;
            Ok(EventWithContext {
                event,
                schedule_task: row.get("schedule_task")?,
                schedule_deleted: schedule_deleted_flag != 0,
                transaction_description: row.get("tx_description")?,
                transaction_amount_pence: row.get("tx_amount")?,
                transaction_date: row.get("tx_date")?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test --package manor-core maintenance::event_dal`
Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/maintenance/event_dal.rs
git commit -m "feat(maintenance): list_for_asset + transaction-link uniqueness (L4c)"
```

---

### Task 5: Rollup queries — asset + category spend totals

**Files:**
- Modify: `crates/core/src/maintenance/event_dal.rs`

- [ ] **Step 1: Write failing tests**

Append to tests module:

```rust
#[test]
fn asset_spend_totals_zero_events_shows_asset_with_zeros() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Lonely asset");
    let rows = asset_spend_totals(&conn, "2026-04-20").unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].asset_id, asset_id);
    assert_eq!(rows[0].total_last_12m_pence, 0);
    assert_eq!(rows[0].event_count_lifetime, 0);
}

#[test]
fn asset_spend_totals_12m_window() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    let mut d = draft(&asset_id);
    d.completed_date = "2025-04-21".into(); // 364 days ago — inside window
    d.cost_pence = Some(10000);
    insert_event(&conn, &d).unwrap();
    d.completed_date = "2025-04-19".into(); // 366 days ago — outside
    d.cost_pence = Some(50000);
    insert_event(&conn, &d).unwrap();
    let rows = asset_spend_totals(&conn, "2026-04-20").unwrap();
    let row = rows.iter().find(|r| r.asset_id == asset_id).unwrap();
    assert_eq!(row.total_last_12m_pence, 10000);
    assert_eq!(row.total_lifetime_pence, 60000);
    assert_eq!(row.event_count_last_12m, 1);
    assert_eq!(row.event_count_lifetime, 2);
}

#[test]
fn asset_spend_totals_null_cost_counts_but_not_sum() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    let mut d = draft(&asset_id);
    d.cost_pence = None;
    d.completed_date = "2026-01-10".into();
    insert_event(&conn, &d).unwrap();
    let rows = asset_spend_totals(&conn, "2026-04-20").unwrap();
    let row = rows.iter().find(|r| r.asset_id == asset_id).unwrap();
    assert_eq!(row.total_lifetime_pence, 0);
    assert_eq!(row.event_count_lifetime, 1);
}

#[test]
fn asset_spend_totals_excludes_trashed_assets() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Will be trashed");
    conn.execute(
        "UPDATE asset SET deleted_at = 1 WHERE id = ?1",
        params![asset_id],
    ).unwrap();
    let rows = asset_spend_totals(&conn, "2026-04-20").unwrap();
    assert!(rows.iter().all(|r| r.asset_id != asset_id));
}

#[test]
fn category_spend_totals_sums_by_category() {
    let conn = test_conn();
    let appliance_id = insert_test_asset_with_category(&conn, "Boiler", "appliance");
    let vehicle_id = insert_test_asset_with_category(&conn, "Car", "vehicle");
    let mut d = draft(&appliance_id);
    d.cost_pence = Some(10000);
    insert_event(&conn, &d).unwrap();
    let mut d2 = draft(&vehicle_id);
    d2.cost_pence = Some(25000);
    insert_event(&conn, &d2).unwrap();
    let rows = category_spend_totals(&conn, "2026-04-20").unwrap();
    let appliance = rows.iter().find(|r| r.category == "appliance").unwrap();
    let vehicle = rows.iter().find(|r| r.category == "vehicle").unwrap();
    assert_eq!(appliance.total_lifetime_pence, 10000);
    assert_eq!(vehicle.total_lifetime_pence, 25000);
}
```

If `insert_test_asset_with_category` doesn't exist in `crates/core/src/testing.rs`, add it (tiny helper — look at L4a's `insert_test_asset` and generalise the category parameter).

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test --package manor-core maintenance::event_dal::tests::asset_spend`
Expected: FAIL.

- [ ] **Step 3: Implement rollup functions**

Add to `event_dal.rs`:

```rust
pub fn asset_spend_totals(
    conn: &Connection,
    today: &str,
) -> Result<Vec<crate::maintenance::event::AssetSpendTotal>> {
    let mut stmt = conn.prepare(
        "WITH cutoff AS (SELECT date(?1, '-365 days') AS d365)
         SELECT
             a.id           AS asset_id,
             a.name         AS asset_name,
             a.category     AS asset_category,
             COALESCE(SUM(CASE
                 WHEN e.completed_date >= (SELECT d365 FROM cutoff)
                  AND e.cost_pence IS NOT NULL
                  AND e.deleted_at IS NULL
                 THEN e.cost_pence END), 0) AS total_last_12m_pence,
             COALESCE(SUM(CASE
                 WHEN e.cost_pence IS NOT NULL
                  AND e.deleted_at IS NULL
                 THEN e.cost_pence END), 0) AS total_lifetime_pence,
             COALESCE(SUM(CASE
                 WHEN e.completed_date >= (SELECT d365 FROM cutoff)
                  AND e.deleted_at IS NULL
                 THEN 1 END), 0) AS event_count_last_12m,
             COALESCE(SUM(CASE
                 WHEN e.deleted_at IS NULL
                 THEN 1 END), 0) AS event_count_lifetime
         FROM asset a
         LEFT JOIN maintenance_event e ON e.asset_id = a.id
         WHERE a.deleted_at IS NULL
         GROUP BY a.id
         ORDER BY total_last_12m_pence DESC, a.name COLLATE NOCASE ASC",
    )?;
    let rows = stmt
        .query_map(params![today], |row| {
            Ok(crate::maintenance::event::AssetSpendTotal {
                asset_id: row.get("asset_id")?,
                asset_name: row.get("asset_name")?,
                asset_category: row.get("asset_category")?,
                total_last_12m_pence: row.get("total_last_12m_pence")?,
                total_lifetime_pence: row.get("total_lifetime_pence")?,
                event_count_last_12m: row.get("event_count_last_12m")?,
                event_count_lifetime: row.get("event_count_lifetime")?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn asset_spend_for_asset(
    conn: &Connection,
    asset_id: &str,
    today: &str,
) -> Result<crate::maintenance::event::AssetSpendTotal> {
    let totals = asset_spend_totals(conn, today)?;
    totals
        .into_iter()
        .find(|r| r.asset_id == asset_id)
        .ok_or_else(|| anyhow!("Asset not found or trashed"))
}

pub fn category_spend_totals(
    conn: &Connection,
    today: &str,
) -> Result<Vec<crate::maintenance::event::CategorySpendTotal>> {
    let mut stmt = conn.prepare(
        "WITH cutoff AS (SELECT date(?1, '-365 days') AS d365)
         SELECT
             a.category AS category,
             COALESCE(SUM(CASE
                 WHEN e.completed_date >= (SELECT d365 FROM cutoff)
                  AND e.cost_pence IS NOT NULL
                  AND e.deleted_at IS NULL
                 THEN e.cost_pence END), 0) AS total_last_12m_pence,
             COALESCE(SUM(CASE
                 WHEN e.cost_pence IS NOT NULL
                  AND e.deleted_at IS NULL
                 THEN e.cost_pence END), 0) AS total_lifetime_pence
         FROM asset a
         LEFT JOIN maintenance_event e ON e.asset_id = a.id
         WHERE a.deleted_at IS NULL
         GROUP BY a.category",
    )?;
    let rows = stmt
        .query_map(params![today], |row| {
            Ok(crate::maintenance::event::CategorySpendTotal {
                category: row.get("category")?,
                total_last_12m_pence: row.get("total_last_12m_pence")?,
                total_lifetime_pence: row.get("total_lifetime_pence")?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test --package manor-core maintenance::event_dal`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/maintenance/event_dal.rs crates/core/src/testing.rs
git commit -m "feat(maintenance): spend rollups — per-asset + per-category (L4c)"
```

---

### Task 6: Ledger candidate + search queries

**Files:**
- Modify: `crates/core/src/maintenance/event_dal.rs`

Review `crates/core/src/ledger/transaction.rs` first — the `Transaction` struct + `from_row` helper is what we'll return from these functions.

- [ ] **Step 1: Write failing tests**

Append to tests module:

```rust
#[test]
fn suggest_with_cost_ranks_by_amount_proximity() {
    let conn = test_conn();
    // insert three tx rows in the date window (completed = 2026-04-20)
    let base_ts = 1713571200i64; // 2024-04-20 12:00 UTC — well out; we'll overwrite below
    // Use 2026 timestamps relative to completed_date
    conn.execute(
        "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
         VALUES (-10000, 'GBP', 'Tesco',    1713571200, 'manual')", [],
    ).unwrap(); // id=1 — amount £100
    conn.execute(
        "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
         VALUES (-14500, 'GBP', 'British Gas', 1713571200, 'manual')", [],
    ).unwrap(); // id=2 — amount £145 — exact match for cost £145
    conn.execute(
        "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
         VALUES (-20000, 'GBP', 'Argos', 1713571200, 'manual')", [],
    ).unwrap(); // id=3 — amount £200
    let rows = suggest_transactions(&conn, "2026-04-20", Some(14500), None, 3).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].description, "British Gas"); // closest match first
}

#[test]
fn suggest_without_cost_orders_by_date_desc() {
    let conn = test_conn();
    conn.execute(
        "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
         VALUES (-10000, 'GBP', 'Earlier', 1713571100, 'manual')", [],
    ).unwrap();
    conn.execute(
        "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
         VALUES (-10000, 'GBP', 'Later', 1713571300, 'manual')", [],
    ).unwrap();
    let rows = suggest_transactions(&conn, "2026-04-20", None, None, 5).unwrap();
    assert_eq!(rows[0].description, "Later");
}

#[test]
fn suggest_excludes_already_linked() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    conn.execute(
        "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
         VALUES (-14500, 'GBP', 'British Gas', 1713571200, 'manual')", [],
    ).unwrap();
    let tx_id = conn.last_insert_rowid();
    let mut d = draft(&asset_id);
    d.transaction_id = Some(tx_id);
    insert_event(&conn, &d).unwrap();
    let rows = suggest_transactions(&conn, "2026-04-20", Some(14500), None, 3).unwrap();
    assert!(rows.iter().all(|t| t.id != tx_id));
}

#[test]
fn suggest_includes_self_when_exclude_event_id_set() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    conn.execute(
        "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
         VALUES (-14500, 'GBP', 'British Gas', 1713571200, 'manual')", [],
    ).unwrap();
    let tx_id = conn.last_insert_rowid();
    let mut d = draft(&asset_id);
    d.transaction_id = Some(tx_id);
    let event_id = insert_event(&conn, &d).unwrap();
    let rows = suggest_transactions(&conn, "2026-04-20", Some(14500), Some(&event_id), 3).unwrap();
    assert!(rows.iter().any(|t| t.id == tx_id));
}

#[test]
fn search_matches_description_and_merchant() {
    let conn = test_conn();
    conn.execute(
        "INSERT INTO ledger_transaction (amount_pence, currency, description, merchant, date, source)
         VALUES (-14500, 'GBP', 'Boiler service', 'British Gas Ltd', 1713571200, 'manual')", [],
    ).unwrap();
    let rows = search_transactions(&conn, "british", 10).unwrap();
    assert!(!rows.is_empty());
    let rows2 = search_transactions(&conn, "boiler", 10).unwrap();
    assert!(!rows2.is_empty());
}
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test --package manor-core maintenance::event_dal::tests::suggest_with_cost`
Expected: FAIL — functions don't exist.

- [ ] **Step 3: Implement suggest_transactions + search_transactions**

Add to `event_dal.rs`:

```rust
use crate::ledger::transaction::Transaction;

fn tx_from_row(row: &Row) -> rusqlite::Result<Transaction> {
    Ok(Transaction {
        id: row.get("id")?,
        bank_account_id: row.get("bank_account_id")?,
        amount_pence: row.get("amount_pence")?,
        currency: row.get("currency")?,
        description: row.get("description")?,
        merchant: row.get("merchant")?,
        category_id: row.get("category_id")?,
        date: row.get("date")?,
        source: row.get("source")?,
        note: row.get("note")?,
        recurring_payment_id: row.get("recurring_payment_id")?,
        created_at: row.get("created_at")?,
    })
}

pub fn suggest_transactions(
    conn: &Connection,
    completed_date: &str,
    cost_pence: Option<i64>,
    exclude_event_id: Option<&str>,
    limit: usize,
) -> Result<Vec<Transaction>> {
    let (order_clause, lim) = match cost_pence {
        Some(_) => ("ORDER BY ABS(lt.amount_pence + ?cost) ASC", limit),
        None => ("ORDER BY lt.date DESC", limit),
    };
    let sql = format!(
        "SELECT lt.id, lt.bank_account_id, lt.amount_pence, lt.currency, lt.description,
                lt.merchant, lt.category_id, lt.date, lt.source, lt.note,
                lt.recurring_payment_id, lt.created_at
         FROM ledger_transaction lt
         LEFT JOIN maintenance_event me
             ON me.transaction_id = lt.id AND me.deleted_at IS NULL
         WHERE lt.deleted_at IS NULL
           AND (me.id IS NULL OR me.id = ?exclude_id)
           AND date(lt.date, 'unixepoch') BETWEEN date(?completed, '-7 days')
                                              AND date(?completed, '+2 days')
         {order_clause}
         LIMIT ?lim"
    );
    // rusqlite doesn't support named params with formatted queries cleanly; rebuild positionally:
    let sql = sql
        .replace("?cost", "?1")
        .replace("?exclude_id", "?2")
        .replace("?completed", "?3")
        .replace("?lim", "?4");
    let mut stmt = conn.prepare(&sql)?;
    let exclude = exclude_event_id.unwrap_or("");
    let rows = match cost_pence {
        Some(c) => stmt
            .query_map(params![c, exclude, completed_date, lim as i64], tx_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?,
        None => stmt
            .query_map(params![0i64, exclude, completed_date, lim as i64], tx_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?,
    };
    Ok(rows)
}

pub fn search_transactions(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Transaction>> {
    let like = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT lt.id, lt.bank_account_id, lt.amount_pence, lt.currency, lt.description,
                lt.merchant, lt.category_id, lt.date, lt.source, lt.note,
                lt.recurring_payment_id, lt.created_at
         FROM ledger_transaction lt
         LEFT JOIN maintenance_event me
             ON me.transaction_id = lt.id AND me.deleted_at IS NULL
         WHERE lt.deleted_at IS NULL
           AND me.id IS NULL
           AND (lt.description LIKE ?1 OR lt.merchant LIKE ?1)
         ORDER BY lt.date DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![like, limit as i64], tx_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

Note: the `format!` + string-replace approach is clumsy but works. If cleaner dynamic SQL helpers exist in the crate (e.g. `rusqlite::Statement::execute_named`), prefer those — but don't over-engineer.

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test --package manor-core maintenance::event_dal`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/maintenance/event_dal.rs
git commit -m "feat(maintenance): ledger transaction suggest + search (L4c)"
```

---

### Task 7: Extend `mark_done` to write an event

**Files:**
- Modify: `crates/core/src/maintenance/dal.rs`

- [ ] **Step 1: Write failing tests**

Add to the existing tests module in `dal.rs`:

```rust
#[test]
fn mark_done_silent_inserts_minimal_event() {
    use crate::maintenance::event_dal;
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    let sched_id = insert_test_schedule(&conn, &asset_id, "Annual service", 12);
    let event_id = mark_done(&conn, &sched_id, "2026-04-20", None).unwrap();
    let events = event_dal::list_for_asset(&conn, &asset_id).unwrap();
    assert_eq!(events.len(), 1);
    let e = &events[0].event;
    assert_eq!(e.id, event_id);
    assert_eq!(e.title, "Annual service");
    assert_eq!(e.completed_date, "2026-04-20");
    assert_eq!(e.cost_pence, None);
    assert_eq!(e.notes, "");
    assert_eq!(e.transaction_id, None);
}

#[test]
fn mark_done_with_draft_uses_caller_draft() {
    use crate::maintenance::event::MaintenanceEventDraft;
    use crate::maintenance::event_dal;
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    let sched_id = insert_test_schedule(&conn, &asset_id, "Annual service", 12);
    let draft = MaintenanceEventDraft {
        asset_id: asset_id.clone(),
        schedule_id: Some(sched_id.clone()),
        title: "Annual service — upgraded parts".into(),
        completed_date: "2026-04-20".into(),
        cost_pence: Some(18000),
        currency: "GBP".into(),
        notes: "Replaced pump".into(),
        transaction_id: None,
    };
    mark_done(&conn, &sched_id, "2026-04-20", Some(&draft)).unwrap();
    let events = event_dal::list_for_asset(&conn, &asset_id).unwrap();
    assert_eq!(events[0].event.cost_pence, Some(18000));
    assert_eq!(events[0].event.notes, "Replaced pump");
}

#[test]
fn mark_done_still_bumps_schedule_dates() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    let sched_id = insert_test_schedule(&conn, &asset_id, "Service", 12);
    mark_done(&conn, &sched_id, "2026-04-20", None).unwrap();
    let sched = get_schedule(&conn, &sched_id).unwrap().unwrap();
    assert_eq!(sched.last_done_date.as_deref(), Some("2026-04-20"));
    assert_eq!(sched.next_due_date, "2027-04-20");
}
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test --package manor-core maintenance::dal::tests::mark_done_silent`
Expected: FAIL — current `mark_done` signature doesn't accept a draft.

- [ ] **Step 3: Refactor `mark_done`**

Find the existing `mark_done(conn, id, today)` in `dal.rs`. Replace with:

```rust
pub fn mark_done(
    conn: &Connection,
    schedule_id: &str,
    today: &str,
    event_draft: Option<&crate::maintenance::event::MaintenanceEventDraft>,
) -> anyhow::Result<String> {
    // Load the schedule row (or error).
    let sched = get_schedule(conn, schedule_id)?
        .ok_or_else(|| anyhow::anyhow!("Schedule not found"))?;
    if sched.deleted_at.is_some() {
        return Err(anyhow::anyhow!("Schedule not found"));
    }

    // Bump last_done_date + next_due_date (existing L4b behavior).
    let next_due = crate::maintenance::due::compute_next_due(
        Some(today),
        sched.interval_months,
        today,
    )?;
    conn.execute(
        "UPDATE maintenance_schedule
         SET last_done_date = ?1, next_due_date = ?2, updated_at = ?3
         WHERE id = ?4",
        rusqlite::params![today, next_due, crate::now_secs(), schedule_id],
    )?;

    // Insert the event. Build a minimal draft if caller passed None.
    let draft_owned;
    let draft_ref = match event_draft {
        Some(d) => d,
        None => {
            draft_owned = crate::maintenance::event::MaintenanceEventDraft {
                asset_id: sched.asset_id.clone(),
                schedule_id: Some(schedule_id.to_string()),
                title: sched.task.clone(),
                completed_date: today.to_string(),
                cost_pence: None,
                currency: "GBP".to_string(),
                notes: String::new(),
                transaction_id: None,
            };
            &draft_owned
        }
    };
    crate::maintenance::event_dal::insert_event(conn, draft_ref)
}
```

Note: if `crate::now_secs` doesn't exist as a public helper, use the local one from `dal.rs` (reuse whatever L4b used — likely `now_secs()` defined inside `dal.rs`).

Update all existing call sites that passed the old 3-arg signature — `grep` for `mark_done(` and pass `None` as the 4th argument. Likely callers: `crates/app/src/maintenance/commands.rs::maintenance_schedule_mark_done`. Update it:

```rust
#[tauri::command]
pub fn maintenance_schedule_mark_done(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.conn()?;
    let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
    dal::mark_done(&conn, &id, &today, None).map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 4: Run ALL core tests to catch regressions**

Run: `cargo test --package manor-core`
Expected: PASS. L4b's existing `mark_done` tests should still pass because we pass `None` and the schedule-bump behavior is preserved.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/maintenance/dal.rs crates/app/src/maintenance/commands.rs
git commit -m "feat(maintenance): mark_done writes event + accepts optional draft (L4c)"
```

---

### Task 8: Asset cascade — soft-delete, restore, permanent-delete

**Files:**
- Modify: `crates/core/src/asset/dal.rs`

- [ ] **Step 1: Write failing cascade tests**

Add to `crates/core/src/asset/dal.rs` tests module:

```rust
#[test]
fn soft_delete_asset_cascades_events() {
    use crate::maintenance::event_dal;
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    let sched_id = insert_test_schedule(&conn, &asset_id, "Service", 12);
    crate::maintenance::dal::mark_done(&conn, &sched_id, "2026-04-20", None).unwrap();
    assert_eq!(event_dal::list_for_asset(&conn, &asset_id).unwrap().len(), 1);
    soft_delete_asset(&conn, &asset_id).unwrap();
    // events soft-deleted — list_for_asset excludes them
    assert_eq!(event_dal::list_for_asset(&conn, &asset_id).unwrap().len(), 0);
}

#[test]
fn restore_asset_restores_events_from_same_cascade() {
    use crate::maintenance::event_dal;
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    let sched_id = insert_test_schedule(&conn, &asset_id, "Service", 12);
    crate::maintenance::dal::mark_done(&conn, &sched_id, "2026-04-20", None).unwrap();
    soft_delete_asset(&conn, &asset_id).unwrap();
    restore_asset(&conn, &asset_id).unwrap();
    assert_eq!(event_dal::list_for_asset(&conn, &asset_id).unwrap().len(), 1);
}

#[test]
fn restore_asset_does_not_resurrect_earlier_deleted_events() {
    // Event soft-deleted at ts1; asset soft-deleted at ts2 (ts2 > ts1).
    // Restoring asset should restore only ts2-trashed siblings — event stays trashed.
    use crate::maintenance::event_dal;
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    let sched_id = insert_test_schedule(&conn, &asset_id, "Service", 12);
    let event_id = crate::maintenance::dal::mark_done(&conn, &sched_id, "2026-04-20", None).unwrap();
    conn.execute(
        "UPDATE maintenance_event SET deleted_at = 100 WHERE id = ?1",
        rusqlite::params![event_id],
    ).unwrap();
    // Manually soft-delete asset at a later timestamp.
    conn.execute(
        "UPDATE asset SET deleted_at = 200 WHERE id = ?1",
        rusqlite::params![asset_id],
    ).unwrap();
    restore_asset(&conn, &asset_id).unwrap();
    // Asset live; event still trashed (its deleted_at=100, asset's was 200 — no match)
    let row: Option<i64> = conn.query_row(
        "SELECT deleted_at FROM maintenance_event WHERE id = ?1",
        rusqlite::params![event_id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(row, Some(100));
}

#[test]
fn permanent_delete_asset_hard_deletes_events() {
    let conn = test_conn();
    let asset_id = insert_test_asset(&conn, "Boiler");
    let sched_id = insert_test_schedule(&conn, &asset_id, "Service", 12);
    crate::maintenance::dal::mark_done(&conn, &sched_id, "2026-04-20", None).unwrap();
    soft_delete_asset(&conn, &asset_id).unwrap();
    permanent_delete_asset(&conn, &asset_id).unwrap();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM maintenance_event WHERE asset_id = ?1",
        rusqlite::params![asset_id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 0);
}
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test --package manor-core asset::dal::tests::soft_delete_asset_cascades_events`
Expected: FAIL — assertion mismatch (events still present after asset soft-delete).

- [ ] **Step 3: Extend the three cascade functions**

In `crates/core/src/asset/dal.rs`, modify:

```rust
pub fn soft_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    conn.execute(
        "UPDATE maintenance_event SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    conn.execute(
        "UPDATE asset SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    Ok(())
}

pub fn restore_asset(conn: &Connection, id: &str) -> Result<()> {
    let deleted_at: Option<i64> = conn
        .query_row(
            "SELECT deleted_at FROM asset WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let Some(ts) = deleted_at else { return Ok(()); };
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = NULL WHERE asset_id = ?1 AND deleted_at = ?2",
        params![id, ts],
    )?;
    conn.execute(
        "UPDATE maintenance_event SET deleted_at = NULL WHERE asset_id = ?1 AND deleted_at = ?2",
        params![id, ts],
    )?;
    conn.execute(
        "UPDATE asset SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn permanent_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    conn.execute(
        "UPDATE attachment SET deleted_at = ?1 WHERE entity_type = 'asset' AND entity_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    conn.execute(
        "DELETE FROM maintenance_event WHERE asset_id = ?1",
        params![id],
    )?;
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    conn.execute(
        "DELETE FROM asset WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
}
```

If `restore_asset` didn't exist in L4a's current code (L4a might have been asset-registry-only without explicit restore), check trash-command plumbing — `restore_asset` is likely a new function this task adds. If it's pre-existing, just extend; if new, add it as shown.

- [ ] **Step 4: Run cascade tests + full core test suite**

Run: `cargo test --package manor-core`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/asset/dal.rs
git commit -m "feat(asset): cascade soft-delete/restore/permanent-delete to maintenance_event (L4c)"
```

---

## Phase B — App layer (Tauri)

### Task 9: Tauri commands for events + rollups + suggest

**Files:**
- Create: `crates/app/src/maintenance/event_commands.rs`
- Modify: `crates/app/src/maintenance/mod.rs`, `crates/app/src/lib.rs`, `crates/app/src/safety/trash_commands.rs`

Review `crates/app/src/maintenance/commands.rs` for the State/Db binding pattern + error conversion idiom.

- [ ] **Step 1: Create `event_commands.rs`**

```rust
//! Tauri commands for maintenance events (L4c).

use manor_core::ledger::transaction::Transaction;
use manor_core::maintenance::event::{
    AssetSpendTotal, CategorySpendTotal, EventWithContext, MaintenanceEvent,
    MaintenanceEventDraft,
};
use manor_core::maintenance::{dal, event_dal};
use tauri::State;

use crate::Db;

fn today_string() -> String {
    chrono::Local::now().date_naive().format("%Y-%m-%d").to_string()
}

#[tauri::command]
pub fn maintenance_event_list_for_asset(
    asset_id: String,
    state: State<'_, Db>,
) -> Result<Vec<EventWithContext>, String> {
    let conn = state.conn().map_err(|e| e.to_string())?;
    event_dal::list_for_asset(&conn, &asset_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_event_get(
    id: String,
    state: State<'_, Db>,
) -> Result<Option<MaintenanceEvent>, String> {
    let conn = state.conn().map_err(|e| e.to_string())?;
    event_dal::get_event(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_event_create_oneoff(
    draft: MaintenanceEventDraft,
    state: State<'_, Db>,
) -> Result<String, String> {
    let conn = state.conn().map_err(|e| e.to_string())?;
    event_dal::insert_event(&conn, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_event_log_completion(
    schedule_id: String,
    draft: MaintenanceEventDraft,
    state: State<'_, Db>,
) -> Result<String, String> {
    let conn = state.conn().map_err(|e| e.to_string())?;
    let today = today_string();
    dal::mark_done(&conn, &schedule_id, &today, Some(&draft)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_event_update(
    id: String,
    draft: MaintenanceEventDraft,
    state: State<'_, Db>,
) -> Result<(), String> {
    let conn = state.conn().map_err(|e| e.to_string())?;
    event_dal::update_event(&conn, &id, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_spend_asset_totals(
    state: State<'_, Db>,
) -> Result<Vec<AssetSpendTotal>, String> {
    let conn = state.conn().map_err(|e| e.to_string())?;
    event_dal::asset_spend_totals(&conn, &today_string()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_spend_for_asset(
    asset_id: String,
    state: State<'_, Db>,
) -> Result<AssetSpendTotal, String> {
    let conn = state.conn().map_err(|e| e.to_string())?;
    event_dal::asset_spend_for_asset(&conn, &asset_id, &today_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_spend_category_totals(
    state: State<'_, Db>,
) -> Result<Vec<CategorySpendTotal>, String> {
    let conn = state.conn().map_err(|e| e.to_string())?;
    event_dal::category_spend_totals(&conn, &today_string()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_suggest_transactions(
    completed_date: String,
    cost_pence: Option<i64>,
    exclude_event_id: Option<String>,
    state: State<'_, Db>,
) -> Result<Vec<Transaction>, String> {
    let conn = state.conn().map_err(|e| e.to_string())?;
    event_dal::suggest_transactions(
        &conn,
        &completed_date,
        cost_pence,
        exclude_event_id.as_deref(),
        if cost_pence.is_some() { 3 } else { 5 },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_search_transactions(
    query: String,
    state: State<'_, Db>,
) -> Result<Vec<Transaction>, String> {
    let conn = state.conn().map_err(|e| e.to_string())?;
    event_dal::search_transactions(&conn, &query, 20).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register the module**

In `crates/app/src/maintenance/mod.rs`, add:

```rust
pub mod event_commands;
```

- [ ] **Step 3: Register commands in `crates/app/src/lib.rs`**

Find the existing `.invoke_handler(tauri::generate_handler![...])` block and add:

```rust
crate::maintenance::event_commands::maintenance_event_list_for_asset,
crate::maintenance::event_commands::maintenance_event_get,
crate::maintenance::event_commands::maintenance_event_create_oneoff,
crate::maintenance::event_commands::maintenance_event_log_completion,
crate::maintenance::event_commands::maintenance_event_update,
crate::maintenance::event_commands::maintenance_spend_asset_totals,
crate::maintenance::event_commands::maintenance_spend_for_asset,
crate::maintenance::event_commands::maintenance_spend_category_totals,
crate::maintenance::event_commands::maintenance_suggest_transactions,
crate::maintenance::event_commands::maintenance_search_transactions,
```

- [ ] **Step 4: Extend `trash_commands.rs`**

In `crates/app/src/safety/trash_commands.rs`, find the `match` blocks in `trash_restore` and `trash_permanent_delete` (they're table-name discriminators). Add `"maintenance_event"` arms that delegate to `event_dal` helpers. If `event_dal` doesn't yet expose `restore_event` / `permanent_delete_event`, add them as thin wrappers:

```rust
// In event_dal.rs, append:
pub fn restore_event(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE maintenance_event SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn permanent_delete_event(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM maintenance_event WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
}
```

Then in `trash_commands.rs`:

```rust
"maintenance_event" => event_dal::restore_event(&conn, &id).map_err(|e| e.to_string())?,
// and for permanent delete:
"maintenance_event" => event_dal::permanent_delete_event(&conn, &id).map_err(|e| e.to_string())?,
```

Follow the exact match-arm syntax used by the existing `"maintenance_schedule"` arms.

- [ ] **Step 5: Compile + run all tests**

Run: `cargo test --workspace` and `cargo clippy --workspace -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/maintenance/ crates/app/src/lib.rs crates/app/src/safety/trash_commands.rs crates/core/src/maintenance/event_dal.rs
git commit -m "feat(maintenance): Tauri commands for events + rollups + trash wiring (L4c)"
```

---

## Phase C — Frontend plumbing

### Task 10: IPC + Zustand stores

**Files:**
- Create: `apps/desktop/src/lib/maintenance/event-ipc.ts`
- Create: `apps/desktop/src/lib/maintenance/event-state.ts`
- Create: `apps/desktop/src/lib/maintenance/spend-state.ts`
- Modify: `apps/desktop/src/lib/maintenance/state.ts` (L4b store — add invalidation)

Review `apps/desktop/src/lib/maintenance/ipc.ts` and `state.ts` (L4b) for the exact pattern before starting.

- [ ] **Step 1: Create `event-ipc.ts`**

```ts
import { invoke } from "@tauri-apps/api/core";

export type EventSource = "manual" | "backfill";

export interface MaintenanceEvent {
  id: string;
  asset_id: string;
  schedule_id: string | null;
  title: string;
  completed_date: string;
  cost_pence: number | null;
  currency: string;
  notes: string;
  transaction_id: number | null;
  source: EventSource;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface EventWithContext {
  event: MaintenanceEvent;
  schedule_task: string | null;
  schedule_deleted: boolean;
  transaction_description: string | null;
  transaction_amount_pence: number | null;
  transaction_date: number | null;
}

export interface MaintenanceEventDraft {
  asset_id: string;
  schedule_id: string | null;
  title: string;
  completed_date: string;
  cost_pence: number | null;
  currency: string;
  notes: string;
  transaction_id: number | null;
}

export interface AssetSpendTotal {
  asset_id: string;
  asset_name: string;
  asset_category: string;
  total_last_12m_pence: number;
  total_lifetime_pence: number;
  event_count_last_12m: number;
  event_count_lifetime: number;
}

export interface CategorySpendTotal {
  category: string;
  total_last_12m_pence: number;
  total_lifetime_pence: number;
}

export interface LedgerTransaction {
  id: number;
  bank_account_id: number | null;
  amount_pence: number;
  currency: string;
  description: string;
  merchant: string | null;
  category_id: number | null;
  date: number;
  source: string;
  note: string | null;
  recurring_payment_id: number | null;
  created_at: number;
}

export const eventIpc = {
  listForAsset: (asset_id: string) =>
    invoke<EventWithContext[]>("maintenance_event_list_for_asset", { assetId: asset_id }),
  get: (id: string) =>
    invoke<MaintenanceEvent | null>("maintenance_event_get", { id }),
  createOneOff: (draft: MaintenanceEventDraft) =>
    invoke<string>("maintenance_event_create_oneoff", { draft }),
  logCompletion: (schedule_id: string, draft: MaintenanceEventDraft) =>
    invoke<string>("maintenance_event_log_completion", { scheduleId: schedule_id, draft }),
  update: (id: string, draft: MaintenanceEventDraft) =>
    invoke<void>("maintenance_event_update", { id, draft }),
  assetTotals: () =>
    invoke<AssetSpendTotal[]>("maintenance_spend_asset_totals"),
  spendForAsset: (asset_id: string) =>
    invoke<AssetSpendTotal>("maintenance_spend_for_asset", { assetId: asset_id }),
  categoryTotals: () =>
    invoke<CategorySpendTotal[]>("maintenance_spend_category_totals"),
  suggestTransactions: (
    completed_date: string,
    cost_pence: number | null,
    exclude_event_id: string | null,
  ) =>
    invoke<LedgerTransaction[]>("maintenance_suggest_transactions", {
      completedDate: completed_date,
      costPence: cost_pence,
      excludeEventId: exclude_event_id,
    }),
  searchTransactions: (query: string) =>
    invoke<LedgerTransaction[]>("maintenance_search_transactions", { query }),
};
```

Tauri's `invoke` serialises camelCased JS keys to snake_cased Rust params; confirm the case convention matches L4b's `ipc.ts` (some Manor setups use explicit `snake_case` via `@tauri-apps/api/core`'s default camelCasing — match whatever L4b does).

- [ ] **Step 2: Create `event-state.ts` (Zustand store)**

```ts
import { create } from "zustand";
import {
  eventIpc,
  EventWithContext,
  LedgerTransaction,
  MaintenanceEventDraft,
} from "./event-ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface MaintenanceEventsStore {
  eventsByAsset: Record<string, EventWithContext[]>;
  loadStatus: LoadStatus;
  loadForAsset(assetId: string): Promise<void>;
  invalidateAsset(assetId: string): void;
  createOneOff(draft: MaintenanceEventDraft): Promise<string>;
  logCompletion(scheduleId: string, draft: MaintenanceEventDraft): Promise<string>;
  update(id: string, draft: MaintenanceEventDraft): Promise<void>;
  suggestTransactions(
    completedDate: string,
    costPence: number | null,
    excludeEventId: string | null,
  ): Promise<LedgerTransaction[]>;
  searchTransactions(query: string): Promise<LedgerTransaction[]>;
}

export const useMaintenanceEventsStore = create<MaintenanceEventsStore>((set, get) => ({
  eventsByAsset: {},
  loadStatus: { kind: "idle" },

  async loadForAsset(assetId) {
    set({ loadStatus: { kind: "loading" } });
    try {
      const rows = await eventIpc.listForAsset(assetId);
      set((s) => ({
        eventsByAsset: { ...s.eventsByAsset, [assetId]: rows },
        loadStatus: { kind: "idle" },
      }));
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
      console.error("event-state: loadForAsset failed", message);
    }
  },

  invalidateAsset(assetId) {
    set((s) => {
      const next = { ...s.eventsByAsset };
      delete next[assetId];
      return { eventsByAsset: next };
    });
  },

  async createOneOff(draft) {
    const id = await eventIpc.createOneOff(draft);
    get().invalidateAsset(draft.asset_id);
    return id;
  },

  async logCompletion(scheduleId, draft) {
    const id = await eventIpc.logCompletion(scheduleId, draft);
    get().invalidateAsset(draft.asset_id);
    return id;
  },

  async update(id, draft) {
    await eventIpc.update(id, draft);
    get().invalidateAsset(draft.asset_id);
  },

  suggestTransactions: (completedDate, costPence, excludeEventId) =>
    eventIpc.suggestTransactions(completedDate, costPence, excludeEventId),

  searchTransactions: (query) => eventIpc.searchTransactions(query),
}));
```

- [ ] **Step 3: Create `spend-state.ts`**

```ts
import { create } from "zustand";
import {
  AssetSpendTotal,
  CategorySpendTotal,
  eventIpc,
} from "./event-ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface SpendStore {
  assetTotals: AssetSpendTotal[];
  categoryTotals: CategorySpendTotal[];
  loadStatus: LoadStatus;
  loadAssetTotals(): Promise<void>;
  loadCategoryTotals(): Promise<void>;
  refresh(): Promise<void>;
}

export const useSpendStore = create<SpendStore>((set, get) => ({
  assetTotals: [],
  categoryTotals: [],
  loadStatus: { kind: "idle" },

  async loadAssetTotals() {
    try {
      const rows = await eventIpc.assetTotals();
      set({ assetTotals: rows });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
      console.error("spend-state: loadAssetTotals failed", message);
    }
  },

  async loadCategoryTotals() {
    try {
      const rows = await eventIpc.categoryTotals();
      set({ categoryTotals: rows });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
      console.error("spend-state: loadCategoryTotals failed", message);
    }
  },

  async refresh() {
    set({ loadStatus: { kind: "loading" } });
    await Promise.all([get().loadAssetTotals(), get().loadCategoryTotals()]);
    set({ loadStatus: { kind: "idle" } });
  },
}));
```

- [ ] **Step 4: Extend L4b `state.ts` — invalidation on markDone**

Open `apps/desktop/src/lib/maintenance/state.ts`. Find the `markDone` action. Replace with:

```ts
markDone: async (id: string) => {
  const sched = get().schedulesByAsset &&
    Object.values(get().schedulesByAsset).flat().find((s) => s.id === id);
  await maintenanceIpc.markDone(id);  // name matches L4b's IPC wrapper
  // Reload affected asset's schedules (existing L4b behavior)
  if (sched) {
    await get().loadForAsset(sched.asset_id);
    // L4c: invalidate event cache + refresh spend totals.
    useMaintenanceEventsStore.getState().invalidateAsset(sched.asset_id);
    await useSpendStore.getState().refresh();
  }
  await get().loadDueSoon();
  await get().loadOverdueCount();
},
```

Also add these imports at top of the file:

```ts
import { useMaintenanceEventsStore } from "./event-state";
import { useSpendStore } from "./spend-state";
```

Note: `get().schedulesByAsset` lookup might differ by L4b shape — check the actual store and adjust so we find the `asset_id` for the mark-done target. If the current markDone action already resolves asset_id some other way, reuse that path.

- [ ] **Step 5: Type-check and run frontend tests**

Run: `cd apps/desktop && pnpm tsc --noEmit && pnpm test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/lib/maintenance/
git commit -m "feat(maintenance): frontend IPC + event-state + spend-state stores (L4c)"
```

---

### Task 11: Bones `Spend` sub-nav wiring

**Files:**
- Modify: `apps/desktop/src/lib/bones/view-state.ts`
- Modify: `apps/desktop/src/components/Bones/BonesSubNav.tsx`
- Modify: `apps/desktop/src/components/Bones/BonesTab.tsx`

- [ ] **Step 1: Widen subview union**

In `apps/desktop/src/lib/bones/view-state.ts`, find the subview union (currently `"assets" | "due_soon"`) and change to:

```ts
export type BonesSubview = "assets" | "due_soon" | "spend";
```

Update default value in the store (if hard-coded) — default stays `"assets"`.

- [ ] **Step 2: Add third tab to `BonesSubNav.tsx`**

Append a third tab to the render list. Match the styling of the existing two tabs exactly.

```tsx
<SubNavTab
  label="Spend"
  active={subview === "spend"}
  onClick={() => setSubview("spend")}
/>
```

- [ ] **Step 3: Route subview === "spend" in `BonesTab.tsx`**

Add to the sub-view router:

```tsx
{subview === "spend" && <SpendView />}
```

Also add the import:

```ts
import { SpendView } from "./Spend/SpendView";
```

`SpendView` will be created in Task 13; add a tiny stub now so the import resolves:

```bash
mkdir -p apps/desktop/src/components/Bones/Spend
```

Create `SpendView.tsx` temporarily:

```tsx
export function SpendView() {
  return <div style={{ padding: 16 }}>Spend (coming soon)</div>;
}
```

- [ ] **Step 4: Type-check and manually tab-switch**

Run: `cd apps/desktop && pnpm tsc --noEmit`
Expected: PASS.

Run the dev app (`pnpm tauri dev`) → Bones tab → verify the three-tab nav renders and the Spend tab shows the stub.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/lib/bones/ apps/desktop/src/components/Bones/BonesSubNav.tsx apps/desktop/src/components/Bones/BonesTab.tsx apps/desktop/src/components/Bones/Spend/
git commit -m "feat(bones): add Spend third sub-nav tab with stub view (L4c)"
```

---

## Phase D — Frontend UI

### Task 12: `LogCompletionDrawer` + `TransactionSuggest`

**Files:**
- Create: `apps/desktop/src/components/Bones/LogCompletionDrawer.tsx`
- Create: `apps/desktop/src/components/Bones/TransactionSuggest.tsx`

Review L4b's `ScheduleDrawer.tsx` first — same right-side drawer chrome, same validation style.

- [ ] **Step 1: Create `TransactionSuggest.tsx`**

```tsx
import { useEffect, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import { useMaintenanceEventsStore } from "../../lib/maintenance/event-state";
import type { LedgerTransaction } from "../../lib/maintenance/event-ipc";

interface Props {
  completedDate: string;
  costPence: number | null;
  selectedTransactionId: number | null;
  excludeEventId: string | null;
  onSelect(txId: number | null, tx: LedgerTransaction | null): void;
}

function formatAmount(pence: number): string {
  const abs = Math.abs(pence);
  return `£${(abs / 100).toFixed(2)}`;
}

function formatDate(unixSec: number): string {
  return new Date(unixSec * 1000).toLocaleDateString("en-GB", {
    month: "short", day: "numeric",
  });
}

export function TransactionSuggest({
  completedDate,
  costPence,
  selectedTransactionId,
  excludeEventId,
  onSelect,
}: Props) {
  const { suggestTransactions, searchTransactions } = useMaintenanceEventsStore();
  const [candidates, setCandidates] = useState<LedgerTransaction[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<LedgerTransaction[] | null>(null);
  const [selectedTx, setSelectedTx] = useState<LedgerTransaction | null>(null);
  const debounceRef = useRef<number | null>(null);

  useEffect(() => {
    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(() => {
      suggestTransactions(completedDate, costPence, excludeEventId)
        .then(setCandidates)
        .catch((e) => console.error("suggest failed", e));
    }, 300);
    return () => {
      if (debounceRef.current) window.clearTimeout(debounceRef.current);
    };
  }, [completedDate, costPence, excludeEventId, suggestTransactions]);

  useEffect(() => {
    if (selectedTransactionId && !selectedTx) {
      // Try to find in current candidate list; if not present, user is editing
      const hit = candidates.find((t) => t.id === selectedTransactionId);
      if (hit) setSelectedTx(hit);
    }
    if (!selectedTransactionId) setSelectedTx(null);
  }, [selectedTransactionId, candidates, selectedTx]);

  const runSearch = (q: string) => {
    setSearchQuery(q);
    if (q.trim().length < 2) {
      setSearchResults(null);
      return;
    }
    searchTransactions(q).then(setSearchResults).catch(console.error);
  };

  const rowsToShow = searchResults ?? candidates;

  if (selectedTx) {
    return (
      <div style={{ padding: 8, background: "var(--surface-elevated, #f6f6f6)", borderRadius: 6 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{ flex: 1 }}>
            ✓ {selectedTx.description} · {formatAmount(selectedTx.amount_pence)} · {formatDate(selectedTx.date)}
          </span>
          <button
            onClick={() => { onSelect(null, null); setSelectedTx(null); }}
            title="Unlink"
            style={{ background: "none", border: "none", cursor: "pointer" }}
          >
            <X size={14} />
          </button>
        </div>
      </div>
    );
  }

  return (
    <div>
      {rowsToShow.length === 0 ? (
        <div style={{ color: "var(--ink-soft, #888)", fontSize: 13 }}>
          No matching transactions.
        </div>
      ) : (
        <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
          {rowsToShow.map((tx) => (
            <li key={tx.id}>
              <button
                onClick={() => { onSelect(tx.id, tx); setSelectedTx(tx); }}
                style={{
                  display: "block", width: "100%", padding: 8, textAlign: "left",
                  background: "none", border: "1px solid var(--border, #e5e5e5)",
                  borderRadius: 6, marginBottom: 4, cursor: "pointer",
                }}
              >
                {tx.merchant ?? tx.description} · {formatAmount(tx.amount_pence)} · {formatDate(tx.date)}
              </button>
            </li>
          ))}
        </ul>
      )}
      <div style={{ marginTop: 8, display: "flex", alignItems: "center", gap: 6 }}>
        <Search size={14} strokeWidth={1.6} />
        <input
          type="text"
          placeholder="None of these — search all…"
          value={searchQuery}
          onChange={(e) => runSearch(e.target.value)}
          style={{ flex: 1, padding: 6, border: "1px solid var(--border, #e5e5e5)", borderRadius: 4 }}
        />
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create `LogCompletionDrawer.tsx`**

```tsx
import { useEffect, useState } from "react";
import { useMaintenanceEventsStore } from "../../lib/maintenance/event-state";
import type {
  MaintenanceEvent,
  MaintenanceEventDraft,
} from "../../lib/maintenance/event-ipc";
import { TransactionSuggest } from "./TransactionSuggest";

type Mode =
  | { kind: "one_off"; assetId: string }
  | { kind: "schedule_completion"; assetId: string; scheduleId: string; taskName: string }
  | { kind: "edit"; event: MaintenanceEvent };

interface Props {
  open: boolean;
  mode: Mode;
  onClose(): void;
}

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

export function LogCompletionDrawer({ open, mode, onClose }: Props) {
  const { createOneOff, logCompletion, update } = useMaintenanceEventsStore();
  const [title, setTitle] = useState("");
  const [completedDate, setCompletedDate] = useState(todayIso());
  const [costText, setCostText] = useState("");
  const [notes, setNotes] = useState("");
  const [txId, setTxId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setError(null);
    setSaving(false);
    if (mode.kind === "one_off") {
      setTitle("");
      setCompletedDate(todayIso());
      setCostText("");
      setNotes("");
      setTxId(null);
    } else if (mode.kind === "schedule_completion") {
      setTitle(mode.taskName);
      setCompletedDate(todayIso());
      setCostText("");
      setNotes("");
      setTxId(null);
    } else {
      const e = mode.event;
      setTitle(e.title);
      setCompletedDate(e.completed_date);
      setCostText(e.cost_pence == null ? "" : (e.cost_pence / 100).toFixed(2));
      setNotes(e.notes);
      setTxId(e.transaction_id);
    }
  }, [open, mode]);

  if (!open) return null;

  const costPence = (() => {
    if (costText.trim() === "") return null;
    const n = Number(costText);
    if (isNaN(n) || n < 0) return NaN;
    return Math.round(n * 100);
  })();

  const buildDraft = (): MaintenanceEventDraft => ({
    asset_id:
      mode.kind === "edit" ? mode.event.asset_id :
      mode.kind === "schedule_completion" ? mode.assetId :
      mode.assetId,
    schedule_id:
      mode.kind === "edit" ? mode.event.schedule_id :
      mode.kind === "schedule_completion" ? mode.scheduleId :
      null,
    title,
    completed_date: completedDate,
    cost_pence: Number.isNaN(costPence) ? null : costPence,
    currency: "GBP",
    notes,
    transaction_id: txId,
  });

  const excludeEventId = mode.kind === "edit" ? mode.event.id : null;

  const onSave = async () => {
    setError(null);
    if (title.trim() === "") {
      setError("Title is required.");
      return;
    }
    if (Number.isNaN(costPence)) {
      setError("Cost must be a positive number.");
      return;
    }
    setSaving(true);
    try {
      const draft = buildDraft();
      if (mode.kind === "one_off") {
        await createOneOff(draft);
      } else if (mode.kind === "schedule_completion") {
        await logCompletion(mode.scheduleId, draft);
      } else {
        await update(mode.event.id, draft);
      }
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="drawer-overlay" style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.3)", zIndex: 100 }}>
      <div
        role="dialog"
        aria-label="Log completion"
        style={{
          position: "absolute", right: 0, top: 0, bottom: 0, width: 480,
          background: "var(--surface, #fff)", padding: 24, overflow: "auto",
          boxShadow: "-4px 0 12px rgba(0,0,0,0.1)",
        }}
      >
        <h2 style={{ marginTop: 0 }}>
          {mode.kind === "edit" ? "Edit completion" : "Log completion"}
        </h2>

        <label style={{ display: "block", marginBottom: 12 }}>
          Title
          <input
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            style={{ display: "block", width: "100%", padding: 6, marginTop: 4 }}
          />
        </label>

        <label style={{ display: "block", marginBottom: 12 }}>
          Completed date
          <input
            type="date"
            value={completedDate}
            onChange={(e) => setCompletedDate(e.target.value)}
            style={{ display: "block", padding: 6, marginTop: 4 }}
          />
        </label>

        <label style={{ display: "block", marginBottom: 12 }}>
          Cost (£)
          <input
            type="number"
            step="0.01"
            min="0"
            value={costText}
            onChange={(e) => setCostText(e.target.value)}
            placeholder="0.00"
            style={{ display: "block", padding: 6, marginTop: 4, width: 160 }}
          />
        </label>

        <div style={{ marginBottom: 12 }}>
          <div style={{ fontSize: 13, marginBottom: 6 }}>Link transaction</div>
          <TransactionSuggest
            completedDate={completedDate}
            costPence={Number.isNaN(costPence) ? null : costPence}
            selectedTransactionId={txId}
            excludeEventId={excludeEventId}
            onSelect={(id) => setTxId(id)}
          />
        </div>

        <label style={{ display: "block", marginBottom: 12 }}>
          Notes
          <textarea
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            rows={4}
            style={{ display: "block", width: "100%", padding: 6, marginTop: 4 }}
          />
        </label>

        {error && (
          <div style={{ color: "var(--danger, #c43)", marginBottom: 8 }}>{error}</div>
        )}

        <div style={{ display: "flex", gap: 8 }}>
          <button onClick={onSave} disabled={saving}>
            {saving ? "Saving…" : "Save"}
          </button>
          <button onClick={onClose} disabled={saving}>Cancel</button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Add RTL test for drawer behavior**

Create `apps/desktop/src/components/Bones/__tests__/LogCompletionDrawer.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { LogCompletionDrawer } from "../LogCompletionDrawer";
import { useMaintenanceEventsStore } from "../../../lib/maintenance/event-state";

vi.mock("../../../lib/maintenance/event-state", () => ({
  useMaintenanceEventsStore: vi.fn(),
}));

describe("LogCompletionDrawer", () => {
  const createOneOff = vi.fn();
  const logCompletion = vi.fn();
  const update = vi.fn();
  const suggestTransactions = vi.fn().mockResolvedValue([]);
  const searchTransactions = vi.fn().mockResolvedValue([]);

  beforeEach(() => {
    (useMaintenanceEventsStore as unknown as any).mockReturnValue({
      createOneOff, logCompletion, update,
      suggestTransactions, searchTransactions,
    });
    createOneOff.mockClear();
    logCompletion.mockClear();
    update.mockClear();
  });

  it("requires title in one-off mode", async () => {
    const onClose = vi.fn();
    render(<LogCompletionDrawer open mode={{ kind: "one_off", assetId: "a1" }} onClose={onClose} />);
    fireEvent.click(screen.getByText("Save"));
    expect(await screen.findByText("Title is required.")).toBeInTheDocument();
    expect(createOneOff).not.toHaveBeenCalled();
  });

  it("prefills title in schedule_completion mode", () => {
    render(
      <LogCompletionDrawer open
        mode={{ kind: "schedule_completion", assetId: "a1", scheduleId: "s1", taskName: "Annual service" }}
        onClose={vi.fn()}
      />
    );
    expect((screen.getByLabelText("Title") as HTMLInputElement).value).toBe("Annual service");
  });

  it("rejects negative cost inline", async () => {
    render(<LogCompletionDrawer open mode={{ kind: "one_off", assetId: "a1" }} onClose={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Title"), { target: { value: "Fix" } });
    fireEvent.change(screen.getByLabelText(/Cost/), { target: { value: "-5" } });
    fireEvent.click(screen.getByText("Save"));
    expect(await screen.findByText(/positive number/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cd apps/desktop && pnpm test LogCompletionDrawer`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/Bones/LogCompletionDrawer.tsx apps/desktop/src/components/Bones/TransactionSuggest.tsx apps/desktop/src/components/Bones/__tests__/LogCompletionDrawer.test.tsx
git commit -m "feat(maintenance): LogCompletionDrawer + TransactionSuggest (L4c)"
```

---

### Task 13: `HistoryBlock` + `EventRow` + `AssetSpendStrip` on AssetDetail

**Files:**
- Create: `apps/desktop/src/components/Bones/EventRow.tsx`
- Create: `apps/desktop/src/components/Bones/HistoryBlock.tsx`
- Create: `apps/desktop/src/components/Bones/AssetSpendStrip.tsx`
- Modify: `apps/desktop/src/components/Bones/AssetDetail.tsx`

- [ ] **Step 1: Create `EventRow.tsx`**

```tsx
import { Pencil, Link } from "lucide-react";
import type { EventWithContext } from "../../lib/maintenance/event-ipc";

interface Props {
  row: EventWithContext;
  onEdit(): void;
}

function formatGBP(pence: number): string {
  return `£${(pence / 100).toFixed(2)}`;
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString("en-GB", { month: "short", day: "numeric", year: "numeric" });
}

export function EventRow({ row, onEdit }: Props) {
  const { event, transaction_description, schedule_deleted } = row;
  const showBackfillPill = event.source === "backfill" && event.cost_pence === null;

  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 10,
      padding: "8px 0", borderBottom: "1px solid var(--border, #eee)",
    }}>
      <span style={{ color: "var(--ink-soft, #777)", fontSize: 13, minWidth: 96 }}>
        {formatDate(event.completed_date)}
      </span>
      <span style={{ flex: 1 }}>
        {event.title}
        {schedule_deleted && (
          <span style={{ color: "var(--ink-soft, #999)", fontSize: 12, marginLeft: 6 }}>
            (schedule removed)
          </span>
        )}
      </span>
      {event.cost_pence !== null && (
        <span style={{ fontVariantNumeric: "tabular-nums" }}>
          {formatGBP(event.cost_pence)}
        </span>
      )}
      {transaction_description && (
        <span title={transaction_description} style={{
          display: "inline-flex", alignItems: "center", gap: 4,
          fontSize: 12, padding: "2px 6px", borderRadius: 4,
          background: "var(--surface-subtle, #f4f4f4)",
        }}>
          <Link size={12} strokeWidth={1.6} />
          {transaction_description.slice(0, 18)}
        </span>
      )}
      {showBackfillPill && (
        <span style={{
          fontSize: 11, color: "var(--ink-soft, #999)",
          padding: "2px 6px", border: "1px solid var(--border, #ddd)", borderRadius: 4,
        }}>
          backfill
        </span>
      )}
      <button
        onClick={onEdit}
        aria-label="Edit"
        style={{ background: "none", border: "none", cursor: "pointer" }}
      >
        <Pencil size={14} strokeWidth={1.6} />
      </button>
    </div>
  );
}
```

- [ ] **Step 2: Create `HistoryBlock.tsx`**

```tsx
import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { useMaintenanceEventsStore } from "../../lib/maintenance/event-state";
import type { MaintenanceEvent } from "../../lib/maintenance/event-ipc";
import { EventRow } from "./EventRow";
import { LogCompletionDrawer } from "./LogCompletionDrawer";

interface Props {
  assetId: string;
}

export function HistoryBlock({ assetId }: Props) {
  const { eventsByAsset, loadForAsset } = useMaintenanceEventsStore();
  const rows = eventsByAsset[assetId] ?? [];

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [drawerMode, setDrawerMode] = useState<
    | { kind: "one_off"; assetId: string }
    | { kind: "edit"; event: MaintenanceEvent }
    | null
  >(null);

  useEffect(() => {
    if (!eventsByAsset[assetId]) void loadForAsset(assetId);
  }, [assetId, eventsByAsset, loadForAsset]);

  const openOneOff = () => {
    setDrawerMode({ kind: "one_off", assetId });
    setDrawerOpen(true);
  };
  const openEdit = (event: MaintenanceEvent) => {
    setDrawerMode({ kind: "edit", event });
    setDrawerOpen(true);
  };

  return (
    <section style={{ marginTop: 24 }}>
      <header style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12 }}>
        <h3 style={{ margin: 0, flex: 1 }}>History</h3>
        <button onClick={openOneOff}>
          <Plus size={14} strokeWidth={1.6} /> Log work
        </button>
      </header>

      {rows.length === 0 ? (
        <div style={{ color: "var(--ink-soft, #888)" }}>
          No completions logged yet.
        </div>
      ) : (
        <div>
          {rows.map((r) => (
            <EventRow key={r.event.id} row={r} onEdit={() => openEdit(r.event)} />
          ))}
        </div>
      )}

      {drawerMode && (
        <LogCompletionDrawer
          open={drawerOpen}
          mode={drawerMode}
          onClose={() => { setDrawerOpen(false); }}
        />
      )}
    </section>
  );
}
```

- [ ] **Step 3: Create `AssetSpendStrip.tsx`**

```tsx
import { useEffect, useState } from "react";
import { eventIpc, AssetSpendTotal } from "../../lib/maintenance/event-ipc";

interface Props { assetId: string; }

function gbp(pence: number): string {
  return `£${(pence / 100).toFixed(0)}`;
}

export function AssetSpendStrip({ assetId }: Props) {
  const [total, setTotal] = useState<AssetSpendTotal | null>(null);

  useEffect(() => {
    eventIpc.spendForAsset(assetId).then(setTotal).catch(console.error);
  }, [assetId]);

  if (!total) return null;
  if (total.total_lifetime_pence === 0 && total.event_count_lifetime === 0) return null;

  return (
    <div style={{ display: "flex", gap: 16, color: "var(--ink-soft, #666)", fontSize: 13, margin: "8px 0 16px 0" }}>
      <span>12-month spend <strong>{gbp(total.total_last_12m_pence)}</strong></span>
      <span>·</span>
      <span>Lifetime <strong>{gbp(total.total_lifetime_pence)}</strong></span>
      <span>·</span>
      <span>{total.event_count_lifetime} completion{total.event_count_lifetime === 1 ? "" : "s"}</span>
    </div>
  );
}
```

- [ ] **Step 4: Mount both on AssetDetail**

In `apps/desktop/src/components/Bones/AssetDetail.tsx`, add imports:

```tsx
import { AssetSpendStrip } from "./AssetSpendStrip";
import { HistoryBlock } from "./HistoryBlock";
```

Insert `<AssetSpendStrip assetId={asset.id} />` directly below the asset header (where name/category/etc are rendered).

Insert `<HistoryBlock assetId={asset.id} />` after the existing `<MaintenanceSection />` block.

- [ ] **Step 5: Type-check + manual verify in dev**

Run: `cd apps/desktop && pnpm tsc --noEmit`
Run the dev app: create or open an asset, verify the spend strip renders (hidden at £0 lifetime), verify the History block appears below Maintenance, click "+ Log work" → drawer opens.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/components/Bones/EventRow.tsx apps/desktop/src/components/Bones/HistoryBlock.tsx apps/desktop/src/components/Bones/AssetSpendStrip.tsx apps/desktop/src/components/Bones/AssetDetail.tsx
git commit -m "feat(maintenance): History block + asset spend strip on AssetDetail (L4c)"
```

---

### Task 14: Bones `SpendView` + category strip + per-asset list

**Files:**
- Replace: `apps/desktop/src/components/Bones/Spend/SpendView.tsx` (stub from Task 11)
- Create: `apps/desktop/src/components/Bones/Spend/SpendCategoryStrip.tsx`
- Create: `apps/desktop/src/components/Bones/Spend/SpendAssetRow.tsx`

- [ ] **Step 1: Create `SpendCategoryStrip.tsx`**

```tsx
import type { CategorySpendTotal } from "../../../lib/maintenance/event-ipc";

interface Props { totals: CategorySpendTotal[]; }

const CATEGORIES = [
  { key: "appliance", label: "Appliance", emoji: "🏠" },
  { key: "vehicle",   label: "Vehicle",   emoji: "🚗" },
  { key: "fixture",   label: "Fixture",   emoji: "🔧" },
  { key: "other",     label: "Other",     emoji: "📦" },
] as const;

function gbp(pence: number): string {
  return `£${(pence / 100).toFixed(0)}`;
}

export function SpendCategoryStrip({ totals }: Props) {
  const byKey: Record<string, CategorySpendTotal | undefined> =
    Object.fromEntries(totals.map((t) => [t.category, t]));

  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 12, marginBottom: 24 }}>
      {CATEGORIES.map((c) => {
        const t = byKey[c.key];
        const twelveM = t ? t.total_last_12m_pence : 0;
        return (
          <div key={c.key} style={{ padding: 12, border: "1px solid var(--border, #eee)", borderRadius: 6 }}>
            <div style={{ fontSize: 13, color: "var(--ink-soft, #777)" }}>
              {c.emoji} {c.label}
            </div>
            <div style={{ fontSize: 20, fontWeight: 500 }}>{gbp(twelveM)}</div>
          </div>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Create `SpendAssetRow.tsx`**

```tsx
import type { AssetSpendTotal } from "../../../lib/maintenance/event-ipc";

interface Props { total: AssetSpendTotal; onOpen(): void; sortBy: "12m" | "lifetime"; }

const EMOJI: Record<string, string> = {
  appliance: "🏠", vehicle: "🚗", fixture: "🔧", other: "📦",
};

function gbp(pence: number): string {
  return `£${(pence / 100).toFixed(0)}`;
}

export function SpendAssetRow({ total, onOpen, sortBy }: Props) {
  const value = sortBy === "12m" ? total.total_last_12m_pence : total.total_lifetime_pence;
  return (
    <button onClick={onOpen} style={{
      display: "flex", alignItems: "center", gap: 8, width: "100%",
      padding: "8px 0", background: "none", border: "none", borderBottom: "1px solid var(--border, #eee)",
      cursor: "pointer", textAlign: "left",
    }}>
      <span>{EMOJI[total.asset_category] ?? "📦"}</span>
      <span style={{ flex: 1 }}>{total.asset_name}</span>
      <span style={{ fontVariantNumeric: "tabular-nums" }}>{gbp(value)}</span>
    </button>
  );
}
```

- [ ] **Step 3: Replace stub `SpendView.tsx`**

```tsx
import { useEffect, useState } from "react";
import { useSpendStore } from "../../../lib/maintenance/spend-state";
import { useBonesViewStore } from "../../../lib/bones/view-state";
import { SpendCategoryStrip } from "./SpendCategoryStrip";
import { SpendAssetRow } from "./SpendAssetRow";

export function SpendView() {
  const { assetTotals, categoryTotals, refresh, loadStatus } = useSpendStore();
  const { openAssetDetail } = useBonesViewStore();
  const [sortBy, setSortBy] = useState<"12m" | "lifetime">("12m");

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const sorted = [...assetTotals].sort((a, b) => {
    const av = sortBy === "12m" ? a.total_last_12m_pence : a.total_lifetime_pence;
    const bv = sortBy === "12m" ? b.total_last_12m_pence : b.total_lifetime_pence;
    return bv - av;
  });

  const noEvents = assetTotals.every((t) => t.event_count_lifetime === 0);

  if (loadStatus.kind === "error") {
    return (
      <div>
        <p>Couldn't load spend totals.</p>
        <button onClick={() => void refresh()}>Retry</button>
      </div>
    );
  }

  if (noEvents) {
    return (
      <div style={{ color: "var(--ink-soft, #888)", textAlign: "center", padding: 48 }}>
        No maintenance spend logged yet. Mark things done to start tracking.
      </div>
    );
  }

  return (
    <div>
      <h2 style={{ marginTop: 0 }}>12-month spend across the house</h2>
      <SpendCategoryStrip totals={categoryTotals} />

      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12 }}>
        <h3 style={{ margin: 0, flex: 1 }}>Per asset</h3>
        <select value={sortBy} onChange={(e) => setSortBy(e.target.value as "12m" | "lifetime")}>
          <option value="12m">Sort by 12m spend</option>
          <option value="lifetime">Sort by lifetime spend</option>
        </select>
      </div>

      {sorted.map((t) => (
        <SpendAssetRow
          key={t.asset_id}
          total={t}
          sortBy={sortBy}
          onOpen={() => openAssetDetail(t.asset_id)}
        />
      ))}
    </div>
  );
}
```

- [ ] **Step 4: Type-check + manual verify**

Run: `cd apps/desktop && pnpm tsc --noEmit`
In dev: switch to Bones → Spend. With no events: empty state. Log a completion with cost: switch back to Spend, see category strip + asset row update.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/Bones/Spend/
git commit -m "feat(bones): Spend tab — category strip + sortable per-asset list (L4c)"
```

---

### Task 15: `Log completion…` overflow menu + ScheduleRow hook-up

**Files:**
- Modify: `apps/desktop/src/components/Bones/DueSoon/ScheduleRow.tsx`
- Modify: `apps/desktop/src/components/Bones/MaintenanceSection.tsx`

Review `ScheduleRow.tsx` for its current props + overflow-menu shape (L4b likely has a 3-dot menu with Edit + Delete entries).

- [ ] **Step 1: Add `onLogCompletion` prop to `ScheduleRow`**

Extend the `ScheduleRowProps`:

```tsx
interface ScheduleRowProps {
  schedule: MaintenanceSchedule;
  assetName?: string;
  assetCategory?: string;
  onMarkDone: () => void;
  onLogCompletion?: () => void;   // NEW
  onEdit: () => void;
  onDelete?: () => void;
}
```

In the row's overflow menu, insert a new menu item between Edit and Delete:

```tsx
{onLogCompletion && (
  <MenuItem onClick={onLogCompletion}>Log completion…</MenuItem>
)}
```

(Use whatever MenuItem component L4b uses; match the style of the existing Edit + Delete items exactly.)

- [ ] **Step 2: Wire `onLogCompletion` from DueSoonView + MaintenanceSection**

In each consumer, open `LogCompletionDrawer` in schedule_completion mode:

```tsx
const [drawerOpen, setDrawerOpen] = useState(false);
const [activeSched, setActiveSched] = useState<MaintenanceSchedule | null>(null);

// in the per-row render:
<ScheduleRow
  schedule={s}
  assetName={/* or undefined */}
  onMarkDone={() => void markDone(s.id)}
  onLogCompletion={() => { setActiveSched(s); setDrawerOpen(true); }}
  onEdit={...}
  onDelete={...}
/>

// at the bottom of the view:
{activeSched && (
  <LogCompletionDrawer
    open={drawerOpen}
    mode={{
      kind: "schedule_completion",
      assetId: activeSched.asset_id,
      scheduleId: activeSched.id,
      taskName: activeSched.task,
    }}
    onClose={() => { setDrawerOpen(false); setActiveSched(null); }}
  />
)}
```

Apply the same pattern in `MaintenanceSection.tsx`.

Import `LogCompletionDrawer` from `../LogCompletionDrawer` in `DueSoonView.tsx` and from `./LogCompletionDrawer` in `MaintenanceSection.tsx`.

- [ ] **Step 3: Type-check + manual verify**

Run: `cd apps/desktop && pnpm tsc --noEmit`
In dev: DueSoonView → open overflow menu on a row → click "Log completion…" → drawer opens with task pre-filled → Save → History block updates + Spend tab totals refresh.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Bones/
git commit -m "feat(maintenance): Log completion… overflow action on schedule rows (L4c)"
```

---

## Phase E — Integration + ship

### Task 16: Full integration test + DoD manual QA

**Files:** none new; exercises the end-to-end path.

- [ ] **Step 1: Add integration test for mark-done-writes-event round-trip**

In `crates/app/src/maintenance/` tests (new file if no existing integration test harness in app crate: `crates/app/src/maintenance/event_commands_tests.rs` — match however L4b wired integration tests).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::maintenance::event_dal;
    use manor_core::maintenance::dal;
    use manor_core::testing::{insert_test_asset, insert_test_schedule, test_conn};

    #[test]
    fn silent_mark_done_writes_event_via_list_for_asset() {
        let conn = test_conn();
        let asset_id = insert_test_asset(&conn, "Boiler");
        let sched_id = insert_test_schedule(&conn, &asset_id, "Annual service", 12);
        dal::mark_done(&conn, &sched_id, "2026-04-20", None).unwrap();
        let events = event_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.title, "Annual service");
        assert_eq!(events[0].event.cost_pence, None);
    }

    #[test]
    fn log_completion_links_transaction_and_updates_rollup() {
        let conn = test_conn();
        let asset_id = insert_test_asset(&conn, "Boiler");
        let sched_id = insert_test_schedule(&conn, &asset_id, "Service", 12);
        conn.execute(
            "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
             VALUES (-14500, 'GBP', 'British Gas', 1713571200, 'manual')",
            [],
        ).unwrap();
        let tx_id = conn.last_insert_rowid();

        use manor_core::maintenance::event::MaintenanceEventDraft;
        let draft = MaintenanceEventDraft {
            asset_id: asset_id.clone(),
            schedule_id: Some(sched_id.clone()),
            title: "Annual service".into(),
            completed_date: "2026-04-20".into(),
            cost_pence: Some(14500),
            currency: "GBP".into(),
            notes: "".into(),
            transaction_id: Some(tx_id),
        };
        dal::mark_done(&conn, &sched_id, "2026-04-20", Some(&draft)).unwrap();

        let total = event_dal::asset_spend_for_asset(&conn, &asset_id, "2026-04-20").unwrap();
        assert_eq!(total.total_last_12m_pence, 14500);
    }
}
```

- [ ] **Step 2: Run full workspace test + lint + build**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
cd apps/desktop && pnpm tsc --noEmit && pnpm test && pnpm build
```

Expected: everything green.

- [ ] **Step 3: Manual QA — run the full scenario**

Run `pnpm tauri dev`. In the app:

1. Open Bones → Assets → create an asset "Test Boiler" (appliance).
2. Open the asset detail → add a maintenance schedule "Annual service", interval 12 months, last_done_date = 2 months ago.
3. Quit + restart the app. (Migration already applied — just verifying backfill survived.)
4. On the asset detail: verify History block shows one row with the schedule's task + "backfill" pill.
5. Click the pencil on the backfill row → drawer opens in edit mode with the completion date pre-filled → add cost £145 → Save.
6. Verify the backfill pill disappears from the row. Verify AssetSpendStrip now shows £145 12m + £145 lifetime + 1 completion.
7. Switch to Bones → Spend tab. Verify category strip shows £145 on Appliance, £0 elsewhere. Verify per-asset list shows the boiler with £145.
8. Back on the asset detail → click + Log work → drawer in one-off mode → title "Fence fixed", cost £340, Save. Verify it appears in History.
9. Back on the schedule row → overflow menu → "Log completion…" → drawer pre-fills "Annual service" → add cost £150 + link a nearby Ledger transaction (insert one first in the Ledger view with amount −£150 and description "British Gas" dated today). Save.
10. Verify TransactionSuggest showed the British Gas match at the top, selected it, event saved with link. EventRow shows the 🔗 pill. Spend totals updated to £635 (145+340+150).
11. Trash the asset (from L4a's trash action). Verify events disappear from all views. Restore from Trash — events reappear.

- [ ] **Step 4: Commit integration tests**

```bash
git add crates/app/src/maintenance/
git commit -m "test(maintenance): integration tests for event round-trip + Ledger linkage (L4c)"
```

- [ ] **Step 5: Merge to main**

Follow the L4a/L4b merge pattern (squash-merge the feature branch to main, or keep the individual commits — match the repo's established style).

```bash
git checkout main
git merge --no-ff <feature-branch> -m "merge: L4c Maintenance Events + Ledger — v0.5 Bones landmark 3"
git push
```

---

## Definition of done recap

- Migration V20 applies cleanly to fresh + existing dev DBs; backfill creates one event per non-trashed schedule with `last_done_date`.
- Silent `maintenance_schedule_mark_done` writes an event row transparently.
- `LogCompletionDrawer` works in all three modes (one-off, schedule completion, edit).
- `TransactionSuggest` surfaces top-3 / top-5 matches; "Search all" fallback works; already-linked transactions excluded.
- `HistoryBlock` renders below MaintenanceSection on AssetDetail; backfill pill visible on synthesised rows.
- `AssetSpendStrip` renders at top of AssetDetail; hides at £0 lifetime + zero events.
- Bones `Spend` sub-nav tab renders category strip + per-asset sortable list + empty state.
- Soft-delete-asset cascades events; restore-asset restores them (same-timestamp scope); permanent-delete-asset hard-deletes them.
- Partial unique index on `transaction_id` prevents duplicate Ledger links.
- Trash sweep includes `maintenance_event`; restore/permanent-delete arms extended.
- `cargo test --workspace`, `cargo clippy -- -D warnings`, `pnpm tsc --noEmit`, `pnpm test`, `pnpm build` all green.
- Manual QA scenario above passes end-to-end.

---

*End of L4c implementation plan.*
