# L4b Maintenance Schedules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Manor's second v0.5 Bones slice — per-asset maintenance schedules with a cross-asset "Due soon" overview banded by urgency, plus Today + Rhythm surfacing. Users answer "what's due?" at a glance.

**Architecture:** Two-crate split mirroring L4a. Core holds schema, DAL, and a pure `due.rs` date computer. App layer holds Tauri commands + the extension of L4a's `permanent_delete_asset` to cascade-soft-delete schedules. Frontend introduces Bones sub-nav (mirror of L3b's Hearth pattern), Due soon view, Maintenance section inside the asset detail, and a Today-band summary. Rhythm gets a virtual-render addition that merges overdue maintenance into its chore list.

**Tech Stack:** Rust (rusqlite, chrono with `Months`), React + TypeScript + Zustand, Lucide icons.

**Spec:** `docs/superpowers/specs/2026-04-19-l4b-maintenance-schedules-design.md`

---

## File structure

### New Rust files
- `crates/core/migrations/V19__maintenance_schedule.sql`
- `crates/core/src/maintenance/mod.rs` — types.
- `crates/core/src/maintenance/due.rs` — pure date computer.
- `crates/core/src/maintenance/dal.rs` — CRUD + list helpers + mark_done.
- `crates/app/src/maintenance/mod.rs` — module root.
- `crates/app/src/maintenance/commands.rs` — Tauri IPC.

### New frontend files
- `apps/desktop/src/lib/maintenance/ipc.ts`
- `apps/desktop/src/lib/maintenance/state.ts`
- `apps/desktop/src/lib/bones/view-state.ts`
- `apps/desktop/src/components/Bones/BonesSubNav.tsx`
- `apps/desktop/src/components/Bones/AssetsView.tsx`
- `apps/desktop/src/components/Bones/DueSoon/DueSoonView.tsx`
- `apps/desktop/src/components/Bones/DueSoon/ScheduleRow.tsx`
- `apps/desktop/src/components/Bones/DueSoon/ScheduleDrawer.tsx`
- `apps/desktop/src/components/Bones/MaintenanceSection.tsx`
- `apps/desktop/src/components/Today/MaintenanceOverdueBand.tsx`

### Modified files
- `crates/core/src/lib.rs` — `pub mod maintenance;`.
- `crates/core/src/trash.rs` — append `("maintenance_schedule", "task")` to REGISTRY.
- `crates/core/src/asset/dal.rs` — extend `permanent_delete_asset` with schedule cascade.
- `crates/app/src/lib.rs` — register new commands + `pub mod maintenance;`.
- `crates/app/src/safety/trash_commands.rs` — add `"maintenance_schedule"` arms.
- `apps/desktop/src/components/Bones/BonesTab.tsx` — sub-view router.
- `apps/desktop/src/components/Bones/AssetDetail.tsx` — mount `<MaintenanceSection />`.
- `apps/desktop/src/components/Today/Today.tsx` — mount `<MaintenanceOverdueBand />`.
- `apps/desktop/src/components/Chores/ChoresView.tsx` — merge maintenance items into the list.

---

## Task 1: Migration V19 + trash registry

**Files:**
- Create: `crates/core/migrations/V19__maintenance_schedule.sql`
- Modify: `crates/core/src/trash.rs`

- [ ] **Step 1: Write the migration SQL**

```sql
-- V19__maintenance_schedule.sql
-- L4b Maintenance Schedules: per-asset time-based maintenance.

CREATE TABLE maintenance_schedule (
    id              TEXT PRIMARY KEY,
    asset_id        TEXT NOT NULL REFERENCES asset(id),
    task            TEXT NOT NULL,
    interval_months INTEGER NOT NULL CHECK (interval_months >= 1),
    last_done_date  TEXT,
    next_due_date   TEXT NOT NULL,
    notes           TEXT NOT NULL DEFAULT '',
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER
);

CREATE INDEX idx_maint_asset    ON maintenance_schedule(asset_id);
CREATE INDEX idx_maint_deleted  ON maintenance_schedule(deleted_at);
CREATE INDEX idx_maint_next_due ON maintenance_schedule(next_due_date) WHERE deleted_at IS NULL;
```

- [ ] **Step 2: Add `("maintenance_schedule", "task")` to REGISTRY**

Open `crates/core/src/trash.rs` at the `const REGISTRY: &[(&str, &str)] = &[` list. Append the entry in alphabetical-ish position — match where `("recipe", "title")`, `("staple_item", "name")`, `("asset", "name")` entries sit.

- [ ] **Step 3: Run migrations + tests**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4b-maintenance-schedules
cargo test -p manor-core --lib -- migrations
cargo test --workspace --lib
```
Expected: green. Baseline before L4b is 333 total (245 core + 85 app + 3 integration).

- [ ] **Step 4: Commit**

```bash
git add crates/core/migrations/V19__maintenance_schedule.sql crates/core/src/trash.rs
git commit -m "feat(maintenance): migration V19 + trash registry entry"
```

---

## Task 2: Core types + pure `due.rs` computation

**Files:**
- Create: `crates/core/src/maintenance/mod.rs`
- Create: `crates/core/src/maintenance/due.rs`
- Create: stub `crates/core/src/maintenance/dal.rs` (filled in Task 3)
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Create `mod.rs` with types**

`crates/core/src/maintenance/mod.rs`:

```rust
//! Maintenance schedules — types + pure computation + DAL.

pub mod dal;
pub mod due;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceScheduleDraft {
    pub asset_id: String,
    pub task: String,
    pub interval_months: i32,
    pub last_done_date: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceSchedule {
    pub id: String,
    pub asset_id: String,
    pub task: String,
    pub interval_months: i32,
    pub last_done_date: Option<String>,
    pub next_due_date: String,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DueBand {
    Overdue,
    DueThisWeek,
    Upcoming,
    Far,
}
```

- [ ] **Step 2: Stub DAL**

`crates/core/src/maintenance/dal.rs`:

```rust
//! Maintenance schedule DAL — filled in Task 3.
```

- [ ] **Step 3: Add `pub mod maintenance;` to `crates/core/src/lib.rs`**

Insert alphabetically (between `ledger;` and `meal_plan;` likely — check the existing order and fit in).

- [ ] **Step 4: Write failing tests for `due.rs`**

Create `crates/core/src/maintenance/due.rs`:

```rust
//! Pure date-math for maintenance schedules — compute next_due_date and classify into bands.

use super::DueBand;
use anyhow::Result;
use chrono::{Months, NaiveDate};

/// Compute next_due_date given last_done (or None → use fallback_start).
/// fallback_start is the schedule creation date when last_done is absent.
pub fn compute_next_due(
    last_done_date: Option<&str>,
    interval_months: i32,
    fallback_start: &str,
) -> Result<String> {
    let anchor = last_done_date.unwrap_or(fallback_start);
    let parsed = NaiveDate::parse_from_str(anchor, "%Y-%m-%d")?;
    let next = parsed
        .checked_add_months(Months::new(interval_months as u32))
        .ok_or_else(|| anyhow::anyhow!("date overflow adding {} months", interval_months))?;
    Ok(next.format("%Y-%m-%d").to_string())
}

pub fn classify(next_due_date: &str, today: &str) -> Result<DueBand> {
    let due = NaiveDate::parse_from_str(next_due_date, "%Y-%m-%d")?;
    let today_date = NaiveDate::parse_from_str(today, "%Y-%m-%d")?;
    let days = (due - today_date).num_days();
    Ok(match days {
        n if n <= 0 => DueBand::Overdue,
        n if n <= 7 => DueBand::DueThisWeek,
        n if n <= 30 => DueBand::Upcoming,
        _ => DueBand::Far,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_uses_fallback_when_never_done() {
        let next = compute_next_due(None, 12, "2025-06-15").unwrap();
        assert_eq!(next, "2026-06-15");
    }

    #[test]
    fn compute_month_end_edge_case() {
        // Jan 31 + 1 month = Feb 28 (2025 is not a leap year).
        let next = compute_next_due(Some("2025-01-31"), 1, "2025-01-01").unwrap();
        assert_eq!(next, "2025-02-28");
    }

    #[test]
    fn compute_month_end_edge_leap_year() {
        // Jan 31 + 1 month = Feb 29 (2024 is a leap year).
        let next = compute_next_due(Some("2024-01-31"), 1, "2024-01-01").unwrap();
        assert_eq!(next, "2024-02-29");
    }

    #[test]
    fn compute_multi_month_basic() {
        let next = compute_next_due(Some("2025-03-15"), 3, "2025-01-01").unwrap();
        assert_eq!(next, "2025-06-15");
    }

    #[test]
    fn classify_overdue_past() {
        assert_eq!(classify("2025-06-14", "2025-06-15").unwrap(), DueBand::Overdue);
    }

    #[test]
    fn classify_due_today_is_overdue() {
        assert_eq!(classify("2025-06-15", "2025-06-15").unwrap(), DueBand::Overdue);
    }

    #[test]
    fn classify_due_this_week() {
        assert_eq!(classify("2025-06-18", "2025-06-15").unwrap(), DueBand::DueThisWeek);
    }

    #[test]
    fn classify_upcoming() {
        assert_eq!(classify("2025-06-30", "2025-06-15").unwrap(), DueBand::Upcoming);
    }

    #[test]
    fn classify_far() {
        assert_eq!(classify("2025-08-01", "2025-06-15").unwrap(), DueBand::Far);
    }
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p manor-core --lib maintenance::due
cargo test --workspace --lib
```
Expected: 9 new tests pass. Workspace total 333 + 9 = 342 lib (245 core + 9 = 254 core).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/maintenance/ crates/core/src/lib.rs
git commit -m "feat(maintenance): types + pure due-date computation (compute_next_due + classify)"
```

---

## Task 3: DAL (CRUD + mark_done + list helpers)

**Files:**
- Modify: `crates/core/src/maintenance/dal.rs` (replace stub)

- [ ] **Step 1: Write DAL + tests**

Overwrite `crates/core/src/maintenance/dal.rs`:

```rust
//! Maintenance schedule DAL: CRUD, mark_done, and band-query helpers.

use super::{due, MaintenanceSchedule, MaintenanceScheduleDraft};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

fn today_local() -> String {
    chrono::Local::now().date_naive().format("%Y-%m-%d").to_string()
}

pub fn insert_schedule(conn: &Connection, draft: &MaintenanceScheduleDraft) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    let today = today_local();
    let next_due = due::compute_next_due(draft.last_done_date.as_deref(), draft.interval_months, &today)?;
    conn.execute(
        "INSERT INTO maintenance_schedule
           (id, asset_id, task, interval_months, last_done_date, next_due_date, notes,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            id, draft.asset_id, draft.task, draft.interval_months,
            draft.last_done_date, next_due, draft.notes, now,
        ],
    )?;
    Ok(id)
}

pub fn get_schedule(conn: &Connection, id: &str) -> Result<Option<MaintenanceSchedule>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, task, interval_months, last_done_date, next_due_date, notes,
                created_at, updated_at, deleted_at
         FROM maintenance_schedule WHERE id = ?1 AND deleted_at IS NULL",
    )?;
    stmt.query_row(params![id], row_to_schedule).optional().map_err(Into::into)
}

pub fn list_for_asset(conn: &Connection, asset_id: &str) -> Result<Vec<MaintenanceSchedule>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, task, interval_months, last_done_date, next_due_date, notes,
                created_at, updated_at, deleted_at
         FROM maintenance_schedule
         WHERE asset_id = ?1 AND deleted_at IS NULL
         ORDER BY next_due_date ASC",
    )?;
    let rows = stmt.query_map(params![asset_id], row_to_schedule)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn list_due_before(conn: &Connection, cutoff_date: &str) -> Result<Vec<MaintenanceSchedule>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, task, interval_months, last_done_date, next_due_date, notes,
                created_at, updated_at, deleted_at
         FROM maintenance_schedule
         WHERE next_due_date <= ?1 AND deleted_at IS NULL
         ORDER BY next_due_date ASC",
    )?;
    let rows = stmt.query_map(params![cutoff_date], row_to_schedule)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn list_due_today_and_overdue(conn: &Connection, today: &str) -> Result<Vec<MaintenanceSchedule>> {
    // "Due today" and "overdue" both satisfy next_due_date <= today.
    list_due_before(conn, today)
}

pub fn overdue_count(conn: &Connection, today: &str) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM maintenance_schedule
         WHERE next_due_date <= ?1 AND deleted_at IS NULL",
        params![today],
        |r| r.get(0),
    )?;
    Ok(count)
}

pub fn update_schedule(conn: &Connection, id: &str, draft: &MaintenanceScheduleDraft) -> Result<()> {
    let now = now_secs();
    let today = today_local();
    let next_due = due::compute_next_due(draft.last_done_date.as_deref(), draft.interval_months, &today)?;
    conn.execute(
        "UPDATE maintenance_schedule
         SET asset_id = ?1, task = ?2, interval_months = ?3,
             last_done_date = ?4, next_due_date = ?5, notes = ?6, updated_at = ?7
         WHERE id = ?8",
        params![
            draft.asset_id, draft.task, draft.interval_months,
            draft.last_done_date, next_due, draft.notes, now, id,
        ],
    )?;
    Ok(())
}

pub fn mark_done(conn: &Connection, id: &str, today: &str) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT interval_months FROM maintenance_schedule WHERE id = ?1 AND deleted_at IS NULL",
    )?;
    let interval: i32 = stmt.query_row(params![id], |r| r.get(0))?;
    let next_due = due::compute_next_due(Some(today), interval, today)?;
    let now = now_secs();
    conn.execute(
        "UPDATE maintenance_schedule
         SET last_done_date = ?1, next_due_date = ?2, updated_at = ?3
         WHERE id = ?4",
        params![today, next_due, now, id],
    )?;
    Ok(())
}

pub fn soft_delete_schedule(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = ?1 WHERE id = ?2",
        params![now_secs(), id],
    )?;
    Ok(())
}

pub fn restore_schedule(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn permanent_delete_schedule(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM maintenance_schedule WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
}

fn row_to_schedule(row: &rusqlite::Row) -> rusqlite::Result<MaintenanceSchedule> {
    Ok(MaintenanceSchedule {
        id: row.get(0)?,
        asset_id: row.get(1)?,
        task: row.get(2)?,
        interval_months: row.get(3)?,
        last_done_date: row.get(4)?,
        next_due_date: row.get(5)?,
        notes: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        deleted_at: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh() -> (tempfile::TempDir, Connection, String) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let asset = AssetDraft {
            name: "Boiler".into(),
            category: AssetCategory::Appliance,
            make: None, model: None, serial_number: None, purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        };
        let asset_id = asset_dal::insert_asset(&conn, &asset).unwrap();
        (dir, conn, asset_id)
    }

    fn simple_draft(asset_id: &str) -> MaintenanceScheduleDraft {
        MaintenanceScheduleDraft {
            asset_id: asset_id.into(),
            task: "Annual service".into(),
            interval_months: 12,
            last_done_date: None,
            notes: String::new(),
        }
    }

    #[test]
    fn insert_and_get_populates_next_due_from_fallback() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        let s = get_schedule(&conn, &id).unwrap().unwrap();
        assert_eq!(s.task, "Annual service");
        assert!(s.last_done_date.is_none());
        // next_due = today + 12 months; don't hardcode — just verify non-empty + YYYY-MM-DD format.
        assert_eq!(s.next_due_date.len(), 10);
        assert!(s.next_due_date.chars().nth(4) == Some('-'));
    }

    #[test]
    fn insert_with_last_done_uses_that_as_anchor() {
        let (_d, conn, asset_id) = fresh();
        let mut draft = simple_draft(&asset_id);
        draft.last_done_date = Some("2024-08-15".into());
        let id = insert_schedule(&conn, &draft).unwrap();
        let s = get_schedule(&conn, &id).unwrap().unwrap();
        assert_eq!(s.next_due_date, "2025-08-15");
    }

    #[test]
    fn update_recomputes_next_due_when_interval_changes() {
        let (_d, conn, asset_id) = fresh();
        let mut draft = simple_draft(&asset_id);
        draft.last_done_date = Some("2024-08-15".into());
        let id = insert_schedule(&conn, &draft).unwrap();

        draft.interval_months = 24;
        update_schedule(&conn, &id, &draft).unwrap();
        let s = get_schedule(&conn, &id).unwrap().unwrap();
        assert_eq!(s.next_due_date, "2026-08-15");
    }

    #[test]
    fn mark_done_bumps_both_dates() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        mark_done(&conn, &id, "2025-06-15").unwrap();
        let s = get_schedule(&conn, &id).unwrap().unwrap();
        assert_eq!(s.last_done_date.as_deref(), Some("2025-06-15"));
        assert_eq!(s.next_due_date, "2026-06-15");
    }

    #[test]
    fn list_for_asset_excludes_trashed_and_orders_by_due() {
        let (_d, conn, asset_id) = fresh();
        let a = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Task A".into();
            d.last_done_date = Some("2024-06-15".into());  // next_due 2025-06-15
            d
        }).unwrap();
        let b = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Task B".into();
            d.last_done_date = Some("2024-01-15".into());  // next_due 2025-01-15
            d
        }).unwrap();
        let c = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Task C".into();
            d.last_done_date = Some("2024-03-15".into());  // next_due 2025-03-15
            d
        }).unwrap();
        soft_delete_schedule(&conn, &c).unwrap();

        let list = list_for_asset(&conn, &asset_id).unwrap();
        let tasks: Vec<_> = list.iter().map(|s| s.task.as_str()).collect();
        assert_eq!(tasks, vec!["Task B", "Task A"]);
        let _ = (a, b);
    }

    #[test]
    fn list_due_before_includes_overdue_and_cutoff_day() {
        let (_d, conn, asset_id) = fresh();
        insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Past".into();
            d.last_done_date = Some("2023-06-15".into());  // next_due 2024-06-15
            d
        }).unwrap();
        insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Future".into();
            d.last_done_date = Some("2026-01-15".into());  // next_due 2027-01-15
            d
        }).unwrap();

        let list = list_due_before(&conn, "2025-01-01").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].task, "Past");
    }

    #[test]
    fn overdue_count_respects_trashed() {
        let (_d, conn, asset_id) = fresh();
        insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.last_done_date = Some("2023-06-15".into());
            d
        }).unwrap();
        let id2 = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.last_done_date = Some("2023-07-15".into());
            d
        }).unwrap();
        assert_eq!(overdue_count(&conn, "2025-01-01").unwrap(), 2);
        soft_delete_schedule(&conn, &id2).unwrap();
        assert_eq!(overdue_count(&conn, "2025-01-01").unwrap(), 1);
    }

    #[test]
    fn restore_clears_deleted_at() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        soft_delete_schedule(&conn, &id).unwrap();
        assert!(get_schedule(&conn, &id).unwrap().is_none());
        restore_schedule(&conn, &id).unwrap();
        assert!(get_schedule(&conn, &id).unwrap().is_some());
    }

    #[test]
    fn permanent_delete_only_removes_trashed() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        permanent_delete_schedule(&conn, &id).unwrap();
        assert!(get_schedule(&conn, &id).unwrap().is_some(), "active row survives permanent_delete");
        soft_delete_schedule(&conn, &id).unwrap();
        permanent_delete_schedule(&conn, &id).unwrap();
        // Verify the row is gone (even including trashed).
        let row: Option<i64> = conn.query_row(
            "SELECT 1 FROM maintenance_schedule WHERE id = ?1", params![id], |r| r.get(0),
        ).optional().unwrap();
        assert!(row.is_none());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p manor-core --lib maintenance::dal
cargo test --workspace --lib
```
Expected: 9 new tests pass. Workspace up to 351.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/maintenance/dal.rs
git commit -m "feat(maintenance): DAL — CRUD + mark_done + band-query helpers + trash integration"
```

---

## Task 4: Extend `permanent_delete_asset` with schedule cascade

**Files:**
- Modify: `crates/core/src/asset/dal.rs`

- [ ] **Step 1: Write failing test**

Append to the existing `asset::dal::tests` module:

```rust
    #[test]
    fn permanent_delete_cascades_to_maintenance_schedules() {
        let (_d, conn) = fresh();
        let asset_id = insert_asset(&conn, &draft("Boiler", AssetCategory::Appliance)).unwrap();

        // Insert a schedule directly (skipping the mod.rs type to avoid circular test deps;
        // the test verifies the cascade SQL runs regardless of schedule validity).
        let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
        conn.execute(
            "INSERT INTO maintenance_schedule
               (id, asset_id, task, interval_months, next_due_date, created_at, updated_at)
             VALUES ('sched1', ?1, 'Service', 12, ?2, 0, 0)",
            rusqlite::params![asset_id, today],
        ).unwrap();

        // Trash + permanent-delete the asset.
        soft_delete_asset(&conn, &asset_id).unwrap();
        permanent_delete_asset(&conn, &asset_id).unwrap();

        // The schedule should be soft-deleted (deleted_at IS NOT NULL).
        let sched_trashed: Option<i64> = conn.query_row(
            "SELECT deleted_at FROM maintenance_schedule WHERE id = 'sched1'",
            [],
            |r| r.get(0),
        ).optional().unwrap().flatten();
        assert!(sched_trashed.is_some(), "schedule should be soft-deleted when asset is purged");
    }
```

- [ ] **Step 2: Verify test fails**

```bash
cargo test -p manor-core --lib asset::dal::tests::permanent_delete_cascades_to_maintenance_schedules
```
Expected: FAIL — current `permanent_delete_asset` doesn't touch schedules.

- [ ] **Step 3: Extend `permanent_delete_asset`**

Modify the function in `crates/core/src/asset/dal.rs`. Current signature and body are already present (shipped in L4a post-review fix). Add one more UPDATE for maintenance_schedule before the hard-delete:

```rust
pub fn permanent_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    // Soft-delete linked attachments (L4a).
    conn.execute(
        "UPDATE attachment SET deleted_at = ?1 WHERE entity_type = 'asset' AND entity_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    // Soft-delete linked maintenance schedules (L4b).
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    // Hard-delete the asset (only if trashed).
    conn.execute(
        "DELETE FROM asset WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
}
```

- [ ] **Step 4: Verify test passes + no regressions**

```bash
cargo test -p manor-core --lib asset
cargo test --workspace --lib
```

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/asset/dal.rs
git commit -m "feat(asset): permanent_delete cascades to maintenance_schedule (L4b)"
```

---

## Task 5: Tauri commands + trash-commands extension

**Files:**
- Create: `crates/app/src/maintenance/mod.rs`
- Create: `crates/app/src/maintenance/commands.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/app/src/safety/trash_commands.rs`

- [ ] **Step 1: Module root**

`crates/app/src/maintenance/mod.rs`:

```rust
//! Maintenance schedules — Tauri command layer.

pub mod commands;
```

- [ ] **Step 2: Commands**

Inspect `crates/app/src/asset/commands.rs` for the `Db` state pattern — same `use crate::assistant::commands::Db;` + `state.0.lock().map_err(|e| e.to_string())?`.

`crates/app/src/maintenance/commands.rs`:

```rust
use crate::assistant::commands::Db;
use manor_core::maintenance::{dal, MaintenanceSchedule, MaintenanceScheduleDraft};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct ScheduleWithAsset {
    pub schedule: MaintenanceSchedule,
    pub asset_name: String,
    pub asset_category: String,
}

fn today_local_string() -> String {
    chrono::Local::now().date_naive().format("%Y-%m-%d").to_string()
}

fn join_with_asset(
    conn: &rusqlite::Connection,
    schedules: Vec<MaintenanceSchedule>,
) -> Result<Vec<ScheduleWithAsset>, String> {
    let mut out = Vec::with_capacity(schedules.len());
    for s in schedules {
        let asset = manor_core::asset::dal::get_asset(conn, &s.asset_id)
            .map_err(|e| e.to_string())?;
        let (name, category) = asset
            .map(|a| (a.name, a.category.as_str().to_string()))
            .unwrap_or_else(|| ("(deleted asset)".to_string(), "other".to_string()));
        out.push(ScheduleWithAsset { schedule: s, asset_name: name, asset_category: category });
    }
    Ok(out)
}

#[tauri::command]
pub fn maintenance_schedule_list_for_asset(
    asset_id: String, state: State<'_, Db>,
) -> Result<Vec<MaintenanceSchedule>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::list_for_asset(&conn, &asset_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_schedule_get(
    id: String, state: State<'_, Db>,
) -> Result<Option<MaintenanceSchedule>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::get_schedule(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_schedule_create(
    draft: MaintenanceScheduleDraft, state: State<'_, Db>,
) -> Result<String, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::insert_schedule(&conn, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_schedule_update(
    id: String, draft: MaintenanceScheduleDraft, state: State<'_, Db>,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::update_schedule(&conn, &id, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_schedule_mark_done(
    id: String, state: State<'_, Db>,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::mark_done(&conn, &id, &today_local_string()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_schedule_delete(
    id: String, state: State<'_, Db>,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::soft_delete_schedule(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_schedule_restore(
    id: String, state: State<'_, Db>,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::restore_schedule(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maintenance_due_soon(state: State<'_, Db>) -> Result<Vec<ScheduleWithAsset>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let today = chrono::Local::now().date_naive();
    let cutoff = today + chrono::Duration::days(30);
    let cutoff_str = cutoff.format("%Y-%m-%d").to_string();
    let schedules = dal::list_due_before(&conn, &cutoff_str).map_err(|e| e.to_string())?;
    join_with_asset(&conn, schedules)
}

#[tauri::command]
pub fn maintenance_due_today_and_overdue(
    state: State<'_, Db>,
) -> Result<Vec<ScheduleWithAsset>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let today = today_local_string();
    let schedules = dal::list_due_today_and_overdue(&conn, &today)
        .map_err(|e| e.to_string())?;
    join_with_asset(&conn, schedules)
}

#[tauri::command]
pub fn maintenance_overdue_count(state: State<'_, Db>) -> Result<i64, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let today = today_local_string();
    dal::overdue_count(&conn, &today).map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register module + commands in `crates/app/src/lib.rs`**

Add `pub mod maintenance;` near other module declarations (alphabetical-ish).

Append to `invoke_handler!`:

```rust
            maintenance::commands::maintenance_schedule_list_for_asset,
            maintenance::commands::maintenance_schedule_get,
            maintenance::commands::maintenance_schedule_create,
            maintenance::commands::maintenance_schedule_update,
            maintenance::commands::maintenance_schedule_mark_done,
            maintenance::commands::maintenance_schedule_delete,
            maintenance::commands::maintenance_schedule_restore,
            maintenance::commands::maintenance_due_soon,
            maintenance::commands::maintenance_due_today_and_overdue,
            maintenance::commands::maintenance_overdue_count,
```

- [ ] **Step 4: Extend `trash_commands.rs` for maintenance_schedule**

Open `crates/app/src/safety/trash_commands.rs`. Find the `match entity_type.as_str()` blocks in `trash_restore` and `trash_permanent_delete`. Add arms:

For `trash_restore`:
```rust
"maintenance_schedule" => manor_core::maintenance::dal::restore_schedule(&conn, &entity_id)
    .map_err(|e| e.to_string()),
```

For `trash_permanent_delete`:
```rust
"maintenance_schedule" => manor_core::maintenance::dal::permanent_delete_schedule(&conn, &entity_id)
    .map_err(|e| e.to_string()),
```

Mirror the existing `"asset"` / `"recipe"` arms' exact shape.

- [ ] **Step 5: Build + clippy**

```bash
cargo build -p manor-app
cargo clippy --workspace -- -D warnings
cargo test --workspace --lib
```
Clean, no regressions.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/maintenance/ crates/app/src/lib.rs crates/app/src/safety/trash_commands.rs
git commit -m "feat(maintenance): Tauri commands + trash routes"
```

---

## Task 6: Frontend IPC + Zustand stores

**Files:**
- Create: `apps/desktop/src/lib/maintenance/ipc.ts`
- Create: `apps/desktop/src/lib/maintenance/state.ts`
- Create: `apps/desktop/src/lib/bones/view-state.ts`

- [ ] **Step 1: IPC**

Create `apps/desktop/src/lib/maintenance/ipc.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export interface MaintenanceSchedule {
  id: string;
  asset_id: string;
  task: string;
  interval_months: number;
  last_done_date: string | null;
  next_due_date: string;
  notes: string;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface MaintenanceScheduleDraft {
  asset_id: string;
  task: string;
  interval_months: number;
  last_done_date: string | null;
  notes: string;
}

export interface ScheduleWithAsset {
  schedule: MaintenanceSchedule;
  asset_name: string;
  asset_category: string;
}

export async function listForAsset(assetId: string): Promise<MaintenanceSchedule[]> {
  return await invoke<MaintenanceSchedule[]>("maintenance_schedule_list_for_asset", { assetId });
}

export async function get(id: string): Promise<MaintenanceSchedule | null> {
  return await invoke<MaintenanceSchedule | null>("maintenance_schedule_get", { id });
}

export async function create(draft: MaintenanceScheduleDraft): Promise<string> {
  return await invoke<string>("maintenance_schedule_create", { draft });
}

export async function update(id: string, draft: MaintenanceScheduleDraft): Promise<void> {
  await invoke("maintenance_schedule_update", { id, draft });
}

export async function markDone(id: string): Promise<void> {
  await invoke("maintenance_schedule_mark_done", { id });
}

export async function deleteSchedule(id: string): Promise<void> {
  await invoke("maintenance_schedule_delete", { id });
}

export async function dueSoon(): Promise<ScheduleWithAsset[]> {
  return await invoke<ScheduleWithAsset[]>("maintenance_due_soon");
}

export async function dueTodayAndOverdue(): Promise<ScheduleWithAsset[]> {
  return await invoke<ScheduleWithAsset[]>("maintenance_due_today_and_overdue");
}

export async function overdueCount(): Promise<number> {
  return await invoke<number>("maintenance_overdue_count");
}
```

- [ ] **Step 2: Zustand store**

Create `apps/desktop/src/lib/maintenance/state.ts`:

```ts
import { create } from "zustand";
import * as ipc from "./ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface MaintenanceStore {
  dueSoon: ipc.ScheduleWithAsset[];
  schedulesByAsset: Record<string, ipc.MaintenanceSchedule[]>;
  overdueCount: number;
  loadStatus: LoadStatus;

  loadDueSoon(): Promise<void>;
  loadForAsset(assetId: string): Promise<void>;
  loadOverdueCount(): Promise<void>;

  create(draft: ipc.MaintenanceScheduleDraft): Promise<string>;
  update(id: string, draft: ipc.MaintenanceScheduleDraft): Promise<void>;
  markDone(id: string): Promise<void>;
  deleteSchedule(id: string): Promise<void>;
}

export const useMaintenanceStore = create<MaintenanceStore>((set, get) => ({
  dueSoon: [],
  schedulesByAsset: {},
  overdueCount: 0,
  loadStatus: { kind: "idle" },

  async loadDueSoon() {
    set({ loadStatus: { kind: "loading" } });
    try {
      const dueSoon = await ipc.dueSoon();
      set({ dueSoon, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  async loadForAsset(assetId) {
    try {
      const rows = await ipc.listForAsset(assetId);
      set((s) => ({ schedulesByAsset: { ...s.schedulesByAsset, [assetId]: rows } }));
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  async loadOverdueCount() {
    try { set({ overdueCount: await ipc.overdueCount() }); } catch { /* swallow */ }
  },

  async create(draft) {
    const id = await ipc.create(draft);
    await get().loadDueSoon();
    await get().loadForAsset(draft.asset_id);
    await get().loadOverdueCount();
    return id;
  },

  async update(id, draft) {
    await ipc.update(id, draft);
    await get().loadDueSoon();
    await get().loadForAsset(draft.asset_id);
    await get().loadOverdueCount();
  },

  async markDone(id) {
    const sch = (await ipc.get(id));
    await ipc.markDone(id);
    await get().loadDueSoon();
    if (sch) await get().loadForAsset(sch.asset_id);
    await get().loadOverdueCount();
  },

  async deleteSchedule(id) {
    const sch = (await ipc.get(id));
    await ipc.deleteSchedule(id);
    await get().loadDueSoon();
    if (sch) await get().loadForAsset(sch.asset_id);
    await get().loadOverdueCount();
  },
}));
```

- [ ] **Step 3: Bones view state store**

Create `apps/desktop/src/lib/bones/view-state.ts`. Mirror `apps/desktop/src/lib/hearth/view-state.ts` — inspect that file first for the exact shape, then copy-adapt:

```ts
import { create } from "zustand";
import { settingGet, settingSet } from "../foundation/ipc";

export type BonesSubview = "assets" | "due_soon";

interface BonesViewStore {
  subview: BonesSubview;
  hydrated: boolean;
  pendingAssetDetailId: string | null;

  hydrate(): Promise<void>;
  setSubview(v: BonesSubview): void;
  openAssetDetail(id: string): void;
  clearPendingDetail(): void;
}

export const useBonesViewStore = create<BonesViewStore>((set) => ({
  subview: "assets",
  hydrated: false,
  pendingAssetDetailId: null,

  async hydrate() {
    try {
      const v = await settingGet("bones.last_subview");
      if (v === "assets" || v === "due_soon") {
        set({ subview: v, hydrated: true });
      } else {
        set({ hydrated: true });
      }
    } catch {
      set({ hydrated: true });
    }
  },
  setSubview(v) {
    set({ subview: v });
    void settingSet("bones.last_subview", v).catch(() => {});
  },
  openAssetDetail(id) {
    set({ subview: "assets", pendingAssetDetailId: id });
    void settingSet("bones.last_subview", "assets").catch(() => {});
  },
  clearPendingDetail() {
    set({ pendingAssetDetailId: null });
  },
}));
```

- [ ] **Step 4: Typecheck**

```bash
cd apps/desktop && pnpm tsc --noEmit
```

- [ ] **Step 5: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4b-maintenance-schedules
git add apps/desktop/src/lib/maintenance/ apps/desktop/src/lib/bones/
git commit -m "feat(maintenance): frontend IPC + Zustand stores (maintenance + bones view)"
```

---

## Task 7: Bones sub-nav + AssetsView extraction

**Files:**
- Create: `apps/desktop/src/components/Bones/BonesSubNav.tsx`
- Create: `apps/desktop/src/components/Bones/AssetsView.tsx`
- Overwrite: `apps/desktop/src/components/Bones/BonesTab.tsx` — router.
- Create stub: `apps/desktop/src/components/Bones/DueSoon/DueSoonView.tsx` (Task 8 replaces).

- [ ] **Step 1: BonesSubNav**

Mirror `apps/desktop/src/components/Hearth/HearthSubNav.tsx` exactly — inspect it, then:

```tsx
import { useBonesViewStore, type BonesSubview } from "../../lib/bones/view-state";

const TABS: { key: BonesSubview; label: string }[] = [
  { key: "assets", label: "Assets" },
  { key: "due_soon", label: "Due soon" },
];

export function BonesSubNav() {
  const { subview, setSubview } = useBonesViewStore();
  return (
    <div style={{
      display: "flex",
      gap: 24,
      borderBottom: "1px solid var(--hairline, #e5e5e5)",
      marginBottom: 24,
    }}>
      {TABS.map((t) => {
        const active = subview === t.key;
        return (
          <button
            key={t.key}
            type="button"
            onClick={() => setSubview(t.key)}
            style={{
              background: "transparent",
              border: "none",
              padding: "8px 0",
              fontSize: 14,
              fontWeight: active ? 600 : 500,
              color: active ? "var(--ink-strong, #111)" : "var(--ink-soft, #999)",
              borderBottom: active ? "2px solid var(--ink-strong, #111)" : "2px solid transparent",
              cursor: "pointer",
            }}
          >
            {t.label}
          </button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Extract existing BonesTab body into AssetsView**

1. Read the current `apps/desktop/src/components/Bones/BonesTab.tsx`.
2. Copy its entire body verbatim into a new `apps/desktop/src/components/Bones/AssetsView.tsx` exporting `export function AssetsView() { ... }`.
3. Preserve all local state/hooks/imports the body depends on.
4. **Important**: the current file also owns the `view: list | detail` state machine for click-through to `<AssetDetail />`. That state machine stays inside `AssetsView`.
5. Also wire `useBonesViewStore().pendingAssetDetailId` — when non-null on mount, jump to detail view and call `clearPendingDetail()`:

```tsx
const { pendingAssetDetailId, clearPendingDetail } = useBonesViewStore();
useEffect(() => {
  if (pendingAssetDetailId) {
    setView({ mode: "detail", id: pendingAssetDetailId });
    clearPendingDetail();
  }
}, [pendingAssetDetailId, clearPendingDetail]);
```

- [ ] **Step 3: Stub DueSoonView**

Create `apps/desktop/src/components/Bones/DueSoon/DueSoonView.tsx`:

```tsx
export function DueSoonView() {
  return <p style={{ color: "var(--ink-soft, #999)" }}>Due soon view — coming in Task 8.</p>;
}
```

- [ ] **Step 4: Overwrite BonesTab as router**

Overwrite `apps/desktop/src/components/Bones/BonesTab.tsx`:

```tsx
import { useEffect } from "react";
import { BonesSubNav } from "./BonesSubNav";
import { AssetsView } from "./AssetsView";
import { DueSoonView } from "./DueSoon/DueSoonView";
import { useBonesViewStore } from "../../lib/bones/view-state";

export function BonesTab() {
  const { subview, hydrate, hydrated } = useBonesViewStore();
  useEffect(() => { void hydrate(); }, [hydrate]);
  if (!hydrated) return null;
  return (
    <div style={{ padding: 32, maxWidth: 1200, margin: "0 auto" }}>
      <BonesSubNav />
      {subview === "assets"   && <AssetsView />}
      {subview === "due_soon" && <DueSoonView />}
    </div>
  );
}
```

Note: AssetsView brings its own `padding: 32, maxWidth: 1200` wrapper inside it currently. Remove the duplicate wrapper from AssetsView since BonesTab now owns it (same refactor Hearth went through in L3b — mirror it).

- [ ] **Step 5: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 6: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4b-maintenance-schedules
git add apps/desktop/src/components/Bones/
git commit -m "feat(bones): sub-nav (Assets · Due soon) + extract AssetsView"
```

---

## Task 8: DueSoonView + ScheduleRow

**Files:**
- Create: `apps/desktop/src/components/Bones/DueSoon/ScheduleRow.tsx`
- Overwrite: `apps/desktop/src/components/Bones/DueSoon/DueSoonView.tsx` — full banded list.

- [ ] **Step 1: ScheduleRow**

```tsx
import { Wrench, MoreHorizontal } from "lucide-react";
import type { MaintenanceSchedule } from "../../../lib/maintenance/ipc";

interface Props {
  schedule: MaintenanceSchedule;
  assetName?: string;
  onMarkDone: () => void;
  onEdit: () => void;
  onDelete?: () => void;
}

function todayIso(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function formatRelativeDue(nextDueDate: string): string {
  const today = new Date(todayIso() + "T00:00:00");
  const due = new Date(nextDueDate + "T00:00:00");
  const diffDays = Math.round((due.getTime() - today.getTime()) / (1000 * 60 * 60 * 24));
  if (diffDays < 0) {
    const n = -diffDays;
    return `${n} day${n === 1 ? "" : "s"} overdue`;
  }
  if (diffDays === 0) return "due today";
  if (diffDays === 1) return "due tomorrow";
  if (diffDays <= 30) return `due in ${diffDays} days`;
  const weeks = Math.round(diffDays / 7);
  return `due in ${weeks} weeks`;
}

export function ScheduleRow({ schedule, assetName, onMarkDone, onEdit, onDelete }: Props) {
  const isOverdue = formatRelativeDue(schedule.next_due_date).includes("overdue")
    || formatRelativeDue(schedule.next_due_date) === "due today";
  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 12,
      padding: "10px 12px",
      borderBottom: "1px solid var(--hairline, #e5e5e5)",
    }}>
      <Wrench size={16} strokeWidth={1.8} color="var(--ink-soft, #999)" />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 14, fontWeight: 600, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          {schedule.task}
        </div>
        <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
          {assetName ? `${assetName} · ` : ""}
          <span style={{ color: isOverdue ? "var(--ink-danger, #b00020)" : undefined }}>
            {formatRelativeDue(schedule.next_due_date)}
          </span>
        </div>
      </div>
      <button type="button" onClick={onMarkDone}>Mark done</button>
      <button type="button" onClick={onEdit} aria-label="Edit schedule">
        <MoreHorizontal size={14} strokeWidth={1.8} />
      </button>
      {onDelete && (
        <button type="button" onClick={onDelete} aria-label="Delete schedule"
          style={{ background: "transparent", border: "none", cursor: "pointer" }}>
          ✕
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 2: DueSoonView with 3 bands + drawer**

Overwrite `apps/desktop/src/components/Bones/DueSoon/DueSoonView.tsx`:

```tsx
import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { useMaintenanceStore } from "../../../lib/maintenance/state";
import type { ScheduleWithAsset, MaintenanceSchedule } from "../../../lib/maintenance/ipc";
import { ScheduleRow } from "./ScheduleRow";
import { ScheduleDrawer } from "./ScheduleDrawer";    // Task 9

function todayIso(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function classifyBand(nextDueDate: string): "overdue" | "this_week" | "upcoming" | "far" {
  const today = new Date(todayIso() + "T00:00:00");
  const due = new Date(nextDueDate + "T00:00:00");
  const days = Math.round((due.getTime() - today.getTime()) / (1000 * 60 * 60 * 24));
  if (days <= 0) return "overdue";
  if (days <= 7) return "this_week";
  if (days <= 30) return "upcoming";
  return "far";
}

export function DueSoonView() {
  const { dueSoon, loadStatus, loadDueSoon, markDone, deleteSchedule } = useMaintenanceStore();
  const [editing, setEditing] = useState<MaintenanceSchedule | null>(null);
  const [adding, setAdding] = useState(false);

  useEffect(() => { void loadDueSoon(); }, [loadDueSoon]);

  const overdue = dueSoon.filter((s) => classifyBand(s.schedule.next_due_date) === "overdue");
  const thisWeek = dueSoon.filter((s) => classifyBand(s.schedule.next_due_date) === "this_week");
  const upcoming = dueSoon.filter((s) => classifyBand(s.schedule.next_due_date) === "upcoming");

  const renderBand = (title: string, rows: ScheduleWithAsset[]) => {
    if (rows.length === 0) return null;
    return (
      <div style={{ marginBottom: 24 }}>
        <h2 style={{ fontSize: 14, fontWeight: 600, marginBottom: 8,
                     color: title === "Overdue" ? "var(--ink-danger, #b00020)" : undefined }}>
          {title}  <span style={{ color: "var(--ink-soft, #999)", fontWeight: 500 }}>({rows.length})</span>
        </h2>
        <div style={{ border: "1px solid var(--hairline, #e5e5e5)", borderRadius: 6, overflow: "hidden" }}>
          {rows.map((r) => (
            <ScheduleRow
              key={r.schedule.id}
              schedule={r.schedule}
              assetName={r.asset_name}
              onMarkDone={() => void markDone(r.schedule.id)}
              onEdit={() => setEditing(r.schedule)}
            />
          ))}
        </div>
      </div>
    );
  };

  const allEmpty = dueSoon.length === 0;

  return (
    <div>
      {loadStatus.kind === "loading" && <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>}
      {loadStatus.kind === "error" && (
        <p style={{ color: "var(--ink-danger, #b00020)" }}>
          {loadStatus.message} — <button onClick={() => void loadDueSoon()}>Retry</button>
        </p>
      )}

      {loadStatus.kind === "idle" && allEmpty && (
        <div style={{ padding: 48, textAlign: "center" }}>
          <p style={{ color: "var(--ink-soft, #999)", marginBottom: 16 }}>
            Nothing due in the next 30 days. Everything in order.
          </p>
          <button onClick={() => setAdding(true)}
            style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
            <Plus size={14} strokeWidth={1.8} /> New schedule
          </button>
        </div>
      )}

      {loadStatus.kind === "idle" && !allEmpty && (
        <>
          {renderBand("Overdue", overdue)}
          {renderBand("Due this week", thisWeek)}
          {renderBand("Upcoming (next 30 days)", upcoming)}
          <div style={{ marginTop: 24, textAlign: "right" }}>
            <button onClick={() => setAdding(true)}
              style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
              <Plus size={14} strokeWidth={1.8} /> New schedule
            </button>
          </div>
        </>
      )}

      {adding && (
        <ScheduleDrawer
          onClose={() => setAdding(false)}
          onSaved={() => { setAdding(false); void loadDueSoon(); }}
        />
      )}
      {editing && (
        <ScheduleDrawer
          schedule={editing}
          onClose={() => setEditing(null)}
          onSaved={() => { setEditing(null); void loadDueSoon(); }}
          onDeleted={() => { void deleteSchedule(editing.id); setEditing(null); }}
        />
      )}
    </div>
  );
}
```

(ScheduleDrawer stub for now — Task 9 replaces.)

Create a minimal stub `apps/desktop/src/components/Bones/DueSoon/ScheduleDrawer.tsx`:

```tsx
interface Props {
  schedule?: unknown;
  initialAssetId?: string;
  lockAsset?: boolean;
  onClose: () => void;
  onSaved: () => void;
  onDeleted?: () => void;
}
export function ScheduleDrawer({ onClose }: Props) {
  return <div onClick={onClose}>Stub — Task 9</div>;
}
```

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4b-maintenance-schedules
git add apps/desktop/src/components/Bones/DueSoon/
git commit -m "feat(maintenance): Due soon view with 3 bands + schedule row + stubs"
```

---

## Task 9: ScheduleDrawer (create/edit + delete)

**Files:**
- Overwrite: `apps/desktop/src/components/Bones/DueSoon/ScheduleDrawer.tsx` — full drawer.

- [ ] **Step 1: ScheduleDrawer**

Overwrite the stub:

```tsx
import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import { useMaintenanceStore } from "../../../lib/maintenance/state";
import { useAssetStore } from "../../../lib/asset/state";
import type {
  MaintenanceSchedule, MaintenanceScheduleDraft,
} from "../../../lib/maintenance/ipc";

interface Props {
  schedule?: MaintenanceSchedule;         // undefined = create mode
  initialAssetId?: string;
  lockAsset?: boolean;
  onClose: () => void;
  onSaved: () => void;
  onDeleted?: () => void;
}

const EMPTY_DRAFT: MaintenanceScheduleDraft = {
  asset_id: "",
  task: "",
  interval_months: 12,
  last_done_date: null,
  notes: "",
};

export function ScheduleDrawer({
  schedule, initialAssetId, lockAsset, onClose, onSaved, onDeleted,
}: Props) {
  const { create, update, deleteSchedule } = useMaintenanceStore();
  const { assets, load: loadAssets } = useAssetStore();

  const [draft, setDraft] = useState<MaintenanceScheduleDraft>(() => {
    if (schedule) {
      return {
        asset_id: schedule.asset_id,
        task: schedule.task,
        interval_months: schedule.interval_months,
        last_done_date: schedule.last_done_date,
        notes: schedule.notes,
      };
    }
    return { ...EMPTY_DRAFT, asset_id: initialAssetId ?? "" };
  });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => { void loadAssets(); }, [loadAssets]);

  const save = async () => {
    if (!draft.asset_id) { setError("Pick an asset"); return; }
    if (!draft.task.trim()) { setError("Task required"); return; }
    if (draft.interval_months < 1) { setError("Interval must be at least 1 month"); return; }
    setSaving(true); setError(null);
    try {
      if (schedule) await update(schedule.id, draft);
      else await create(draft);
      onSaved();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const del = async () => {
    if (!schedule) return;
    if (!window.confirm("Move this schedule to Trash?")) return;
    try {
      await deleteSchedule(schedule.id);
      onDeleted?.();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, width: 480,
      background: "var(--paper, #fff)", borderLeft: "1px solid var(--hairline, #e5e5e5)",
      padding: 24, overflow: "auto", zIndex: 50,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 20 }}>
          {schedule ? "Edit schedule" : "New schedule"}
        </h2>
        <button type="button" onClick={onClose} aria-label="Close">✕</button>
      </div>

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Asset</label>
      <select
        value={draft.asset_id}
        onChange={(e) => setDraft({ ...draft, asset_id: e.target.value })}
        disabled={lockAsset}
        style={{ width: "100%", marginBottom: 12, padding: 6 }}
      >
        <option value="">— Pick one —</option>
        {assets.map((a) => (
          <option key={a.id} value={a.id}>{a.name}</option>
        ))}
      </select>

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Task</label>
      <input
        value={draft.task}
        onChange={(e) => setDraft({ ...draft, task: e.target.value })}
        placeholder="e.g. Annual boiler service"
        style={{ width: "100%", marginBottom: 12, padding: 6 }}
      />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Interval (months)</label>
      <input
        type="number" min={1}
        value={draft.interval_months}
        onChange={(e) => setDraft({ ...draft, interval_months: parseInt(e.target.value) || 1 })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }}
      />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>
        Last done (optional)
      </label>
      <input
        type="date"
        value={draft.last_done_date ?? ""}
        onChange={(e) => setDraft({ ...draft, last_done_date: e.target.value || null })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }}
      />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Notes (markdown)</label>
      <textarea
        value={draft.notes}
        onChange={(e) => setDraft({ ...draft, notes: e.target.value })}
        rows={5} style={{ width: "100%", fontFamily: "inherit", padding: 6 }}
      />

      {error && <div style={{ color: "var(--ink-danger, #b00020)", marginTop: 8 }}>{error}</div>}

      <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
        <button type="button" onClick={onClose}>Cancel</button>
        <button type="button" onClick={save} disabled={saving}>
          {saving ? "Saving…" : "Save"}
        </button>
        {schedule && (
          <button type="button" onClick={del}
            style={{ marginLeft: "auto", display: "flex", alignItems: "center", gap: 4 }}>
            <Trash2 size={14} strokeWidth={1.8} /> Delete
          </button>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 3: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4b-maintenance-schedules
git add apps/desktop/src/components/Bones/DueSoon/ScheduleDrawer.tsx
git commit -m "feat(maintenance): ScheduleDrawer with asset picker + interval validation + delete"
```

---

## Task 10: MaintenanceSection on AssetDetail

**Files:**
- Create: `apps/desktop/src/components/Bones/MaintenanceSection.tsx`
- Modify: `apps/desktop/src/components/Bones/AssetDetail.tsx` — mount MaintenanceSection.

- [ ] **Step 1: MaintenanceSection**

```tsx
import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { useMaintenanceStore } from "../../lib/maintenance/state";
import type { MaintenanceSchedule } from "../../lib/maintenance/ipc";
import { ScheduleRow } from "./DueSoon/ScheduleRow";
import { ScheduleDrawer } from "./DueSoon/ScheduleDrawer";

interface Props { assetId: string }

export function MaintenanceSection({ assetId }: Props) {
  const { schedulesByAsset, loadForAsset, markDone, deleteSchedule } = useMaintenanceStore();
  const [editing, setEditing] = useState<MaintenanceSchedule | null>(null);
  const [adding, setAdding] = useState(false);

  useEffect(() => { void loadForAsset(assetId); }, [assetId, loadForAsset]);

  const schedules = schedulesByAsset[assetId] ?? [];

  return (
    <div>
      {schedules.length === 0 && (
        <p style={{ color: "var(--ink-soft, #999)", fontStyle: "italic" }}>
          No maintenance schedules yet.
        </p>
      )}
      {schedules.length > 0 && (
        <div style={{ border: "1px solid var(--hairline, #e5e5e5)", borderRadius: 6, overflow: "hidden" }}>
          {schedules.map((s) => (
            <ScheduleRow
              key={s.id}
              schedule={s}
              onMarkDone={() => void markDone(s.id)}
              onEdit={() => setEditing(s)}
              onDelete={() => void deleteSchedule(s.id)}
            />
          ))}
        </div>
      )}
      <button type="button" onClick={() => setAdding(true)}
        style={{ marginTop: 12, display: "flex", alignItems: "center", gap: 4 }}>
        <Plus size={14} strokeWidth={1.8} /> Add schedule
      </button>

      {adding && (
        <ScheduleDrawer
          initialAssetId={assetId}
          lockAsset
          onClose={() => setAdding(false)}
          onSaved={() => { setAdding(false); void loadForAsset(assetId); }}
        />
      )}
      {editing && (
        <ScheduleDrawer
          schedule={editing}
          initialAssetId={assetId}
          lockAsset
          onClose={() => setEditing(null)}
          onSaved={() => { setEditing(null); void loadForAsset(assetId); }}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Mount inside AssetDetail.tsx**

Modify `apps/desktop/src/components/Bones/AssetDetail.tsx`:

Import `MaintenanceSection`. Between the Notes section and the Documents section, insert:

```tsx
<h2 style={{ marginTop: 32, fontSize: 18 }}>Maintenance</h2>
<MaintenanceSection assetId={id} />
```

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4b-maintenance-schedules
git add apps/desktop/src/components/Bones/
git commit -m "feat(maintenance): Maintenance section on AssetDetail with inline add/edit/delete"
```

---

## Task 11: Today MaintenanceOverdueBand

**Files:**
- Create: `apps/desktop/src/components/Today/MaintenanceOverdueBand.tsx`
- Modify: `apps/desktop/src/components/Today/Today.tsx`

- [ ] **Step 1: MaintenanceOverdueBand**

```tsx
import { useEffect, useState } from "react";
import { Wrench } from "lucide-react";
import { useMaintenanceStore } from "../../lib/maintenance/state";
import { useBonesViewStore } from "../../lib/bones/view-state";
import { useNavStore } from "../../lib/nav";    // adapt to actual hook name
import { settingGet } from "../../lib/foundation/ipc";

export function MaintenanceOverdueBand() {
  const { overdueCount, loadOverdueCount } = useMaintenanceStore();
  const { setSubview } = useBonesViewStore();
  const { setView } = useNavStore();    // adapt to how the main nav View state is actually managed
  const [visible, setVisible] = useState<boolean>(true);

  useEffect(() => { void loadOverdueCount(); }, [loadOverdueCount]);
  useEffect(() => {
    void settingGet("bones.show_maintenance_band").then((v) => setVisible(v !== "false")).catch(() => {});
  }, []);

  if (!visible) return null;
  if (overdueCount === 0) return null;

  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 12,
      height: 56, padding: "0 16px",
      background: "var(--paper, #fff)",
      border: "1px solid var(--hairline, #e5e5e5)",
      borderRadius: 6,
    }}>
      <Wrench size={18} strokeWidth={1.6} color="var(--ink-soft, #999)" />
      <span style={{ flex: 1 }}>
        {overdueCount} maintenance item{overdueCount === 1 ? "" : "s"} overdue
      </span>
      <button type="button" onClick={() => {
        setSubview("due_soon");
        setView("bones");
      }}>View →</button>
    </div>
  );
}
```

**Implementer note:** The main nav "View" state is managed somewhere — inspect `apps/desktop/src/lib/nav.ts` and the component that reads it (likely `App.tsx` or a top-level nav store). Adapt the `useNavStore().setView("bones")` call to whatever pattern actually exists. If there's no Zustand store for it and the view is just local state in `App.tsx`, we'll need to hoist it to a store — check existing `Plan one →` / `Review →` buttons on `TonightBand.tsx` (L3b) for the canonical pattern; that component does the same cross-tab navigation and will show what's available.

- [ ] **Step 2: Mount in Today.tsx**

Modify `apps/desktop/src/components/Today/Today.tsx`:

Import `MaintenanceOverdueBand`. Find where `<TonightBand />` is rendered. Insert `<MaintenanceOverdueBand />` directly after it.

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4b-maintenance-schedules
git add apps/desktop/src/components/Today/
git commit -m "feat(maintenance): Today band — 'N overdue' summary with jump-to-Bones"
```

---

## Task 12: Rhythm (Chores) integration

**Files:**
- Modify: `apps/desktop/src/components/Chores/ChoresView.tsx` — merge maintenance items.

- [ ] **Step 0: Inspect ChoresView**

```bash
sed -n '1,40p' /Users/hanamori/life-assistant/.worktrees/l4b-maintenance-schedules/apps/desktop/src/components/Chores/ChoresView.tsx
```

Understand the existing shape: how it fetches chores via `listChoresDueToday`, how it renders rows. The merge pattern: fetch maintenance alongside chores, render with a `Wrench` icon prefix + "maintenance" chip, reuse the same click-to-complete flow (tick → `markDone`).

- [ ] **Step 1: Extend ChoresView**

Mirror the existing chore-fetch useEffect with a parallel maintenance fetch. Concrete template (adapt to the actual existing code patterns you find):

```tsx
// Additions near the top of ChoresView:
import { useMaintenanceStore } from "../../lib/maintenance/state";
import { Wrench } from "lucide-react";

// Inside the component body, alongside the existing chores-load effect:
const { dueSoon: allDueSoon, loadDueTodayAndOverdue, markDone: markMaintenanceDone } =
  useMaintenanceStore() as any;  // adapt: may not have a dueTodayAndOverdue state slice yet
```

**Simpler + cleaner approach:** since `useMaintenanceStore` already has `overdueCount` + `dueSoon` but not a specific "due today + overdue" slice, add one.

Modify `apps/desktop/src/lib/maintenance/state.ts`:

Add fields/methods:
```ts
  dueTodayAndOverdue: ipc.ScheduleWithAsset[];
  loadDueTodayAndOverdue(): Promise<void>;
```

Implementation:
```ts
  dueTodayAndOverdue: [],

  async loadDueTodayAndOverdue() {
    try {
      const rows = await ipc.dueTodayAndOverdue();
      set({ dueTodayAndOverdue: rows });
    } catch { /* swallow */ }
  },
```

Now extend `ChoresView.tsx` (pattern — adapt to actual file):

```tsx
const maintStore = useMaintenanceStore();
useEffect(() => { void maintStore.loadDueTodayAndOverdue(); }, []);

// In render, alongside chore rows:
{maintStore.dueTodayAndOverdue.map((r) => (
  <div key={`maint-${r.schedule.id}`}
       style={{ display: "flex", alignItems: "center", gap: 8, padding: 8,
                borderBottom: "1px solid var(--hairline, #e5e5e5)" }}>
    <Wrench size={16} strokeWidth={1.8} color="var(--ink-soft, #999)" />
    <div style={{ flex: 1 }}>
      <div style={{ fontSize: 14 }}>{r.schedule.task}</div>
      <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
        {r.asset_name} · maintenance
      </div>
    </div>
    <button type="button" onClick={async () => {
      await maintStore.markDone(r.schedule.id);
      await maintStore.loadDueTodayAndOverdue();
    }}>Mark done</button>
  </div>
))}
```

**Implementer note:** placement matters — the maintenance items should ideally appear alongside the existing chore list, not in a separate section. If ChoresView sorts or groups rows, insert the merge into that sort. If the rendering is a flat list, concatenate.

Visual cue: `Wrench` icon + "maintenance" label chip distinguishes maintenance from actual chores, so the user knows what kind of item they're checking off.

- [ ] **Step 2: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 3: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4b-maintenance-schedules
git add apps/desktop/src/components/Chores/ apps/desktop/src/lib/maintenance/
git commit -m "feat(maintenance): Rhythm (Chores) view renders overdue + due-today items inline"
```

---

## Task 13: Final QA

**Files:** verification only.

- [ ] **Step 1: Full test suite**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4b-maintenance-schedules
cargo test --workspace
```
Expected: baseline 333 + 9 due tests + 9 DAL tests + 1 asset-cascade test = 352 lib + 3 integration.

- [ ] **Step 2: Clippy + typecheck + build**

```bash
cargo clippy --workspace -- -D warnings
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 3: Dev-server golden path**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4b-maintenance-schedules && pnpm tauri dev
```

Walk:
- Bones tab has sub-nav `Assets · Due soon`. Default shows assets (existing L4a grid).
- Switch to Due soon — empty state renders with "+ New schedule" button.
- Click `+ New schedule`: drawer with asset-picker, task input, interval, last-done. Pick a boiler asset, "Annual boiler service" task, 12 months, last-done 2024-08-15.
- Save → row appears in Upcoming band (if today is before 2025-08-15).
- Create another: "Smoke alarm battery", 12 months, last-done 2023-04-01 → row appears in Overdue band (red).
- Sub-nav switch back to Assets → click the boiler → AssetDetail shows a Maintenance section with the schedule. Add another inline via "+ Add schedule".
- Mark done on the overdue item → row moves (out of Overdue, into Upcoming with a next_due_date of today+12 months).
- Today view: "N maintenance items overdue" band renders (plural correctness). View → jumps to Bones Due soon.
- Rhythm (Chores tab): overdue maintenance items appear inline with Wrench icon. Tick one → it disappears from Rhythm + Today overdue count decrements.
- Delete a schedule via the drawer's Delete button → confirms → goes to Trash.
- Trash UI: restore + permanent-delete both work on the maintenance_schedule entry.
- Delete the asset (from AssetDetail) → Trash shows it → permanent-delete the asset → verify the asset's schedules are all soft-deleted (they appear in Trash too, then sweep eventually purges them).

- [ ] **Step 4: If all green → invoke `superpowers:finishing-a-development-branch`.**

---

## Self-review

**Spec coverage:**
- §3 architecture → Tasks 2–6. ✓
- §4 migration V19 → Task 1. ✓
- §5 types → Task 2. ✓
- §6 due.rs → Task 2. ✓
- §7 DAL + asset cascade → Tasks 3–4. ✓
- §8 Tauri commands + trash-commands → Task 5. ✓
- §9 UI (sub-nav, DueSoonView, ScheduleRow, ScheduleDrawer, MaintenanceSection, Today band, Rhythm) → Tasks 7–12. ✓
- §10 Zustand stores → Task 6. ✓
- §11 error handling → inline across tasks. ✓
- §12 testing — core unit in Tasks 2–3; asset-cascade in Task 4; manual QA in Task 13. ✓

**Placeholder scan:** None. Several implementer notes direct inspection of existing files (HearthSubNav, TonightBand, ChoresView) to mirror rather than leaving TBDs.

**Type consistency:** `MaintenanceSchedule`, `MaintenanceScheduleDraft`, `DueBand`, `ScheduleWithAsset` consistent Rust↔TS. Store method names (`loadDueSoon`, `loadForAsset`, `loadOverdueCount`, `create`, `update`, `markDone`, `deleteSchedule`, `loadDueTodayAndOverdue`) match across ipc.ts/state.ts. `useBonesViewStore` mirrors `useHearthViewStore` as explicitly called out.

---

*End of plan. Next: `superpowers:subagent-driven-development`.*
