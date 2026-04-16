# v0.3 Ledger Core — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Ledger view to Manor with manual transaction entry, editable categories, per-category monthly budgets, and a dark-gradient summary card with budget health badges.

**Architecture:** New `crates/core/src/ledger/` module (category, transaction, budget DALs) exposed from `manor-core`. Thin Tauri command glue in `crates/app/src/ledger/commands.rs`. React frontend follows the same stacked-cards pattern as Today/Chores/TimeBlocks. Bank sync is Phase 5b — this plan has zero external API dependencies.

**Tech Stack:** Rust + rusqlite (DAL), `chrono` for month boundaries, Tauri 2 commands, React 18 + TypeScript + Zustand, `@tauri-apps/api/core` invoke.

---

## File Map

**New files:**
- `crates/core/migrations/V5__ledger.sql` — schema + seed data
- `crates/core/src/ledger/mod.rs` — re-exports category, transaction, budget
- `crates/core/src/ledger/category.rs` — category CRUD
- `crates/core/src/ledger/transaction.rs` — transaction insert/update/delete/list
- `crates/core/src/ledger/budget.rs` — budget upsert/delete/monthly_summary
- `crates/app/src/ledger/mod.rs` — re-exports commands
- `crates/app/src/ledger/commands.rs` — all 13 Tauri commands
- `apps/desktop/src/lib/ledger/ipc.ts` — typed invoke wrappers
- `apps/desktop/src/lib/ledger/state.ts` — Zustand store
- `apps/desktop/src/components/Ledger/LedgerView.tsx` — top-level view
- `apps/desktop/src/components/Ledger/SummaryCard.tsx` — dark gradient header card
- `apps/desktop/src/components/Ledger/TransactionFeed.tsx` — day-grouped list
- `apps/desktop/src/components/Ledger/TransactionRow.tsx` — single row
- `apps/desktop/src/components/Ledger/AddTransactionForm.tsx` — manual entry drawer
- `apps/desktop/src/components/Ledger/BudgetSheet.tsx` — budget management drawer

**Modified files:**
- `crates/core/src/lib.rs` — add `pub mod ledger;`
- `crates/app/src/lib.rs` — add `pub mod ledger;` + register 13 commands
- `apps/desktop/src/lib/nav.ts` — add `"ledger"` to `View` type
- `apps/desktop/src/components/Nav/Sidebar.tsx` — add 💰 nav icon
- `apps/desktop/src/App.tsx` — add `{view === "ledger" && <LedgerView />}`
- `apps/desktop/src/lib/today/slash.ts` — add `/spent` command

---

### Task 1: DB schema

**Files:**
- Create: `crates/core/migrations/V5__ledger.sql`

- [ ] **Step 1: Write the migration file**

```sql
-- crates/core/migrations/V5__ledger.sql

-- Category — fixed defaults + user-editable
CREATE TABLE category (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    emoji       TEXT    NOT NULL DEFAULT '💳',
    is_income   INTEGER NOT NULL DEFAULT 0,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    is_default  INTEGER NOT NULL DEFAULT 0,
    deleted_at  INTEGER
);

INSERT INTO category (id, name, emoji, is_income, sort_order, is_default) VALUES
    (1,  'Groceries',     '🛒', 0,  1, 1),
    (2,  'Eating Out',    '🍕', 0,  2, 1),
    (3,  'Transport',     '🚇', 0,  3, 1),
    (4,  'Utilities',     '⚡', 0,  4, 1),
    (5,  'Subscriptions', '📱', 0,  5, 1),
    (6,  'Health',        '💊', 0,  6, 1),
    (7,  'Shopping',      '🛍', 0,  7, 1),
    (8,  'Entertainment', '🎬', 0,  8, 1),
    (9,  'Other',         '💳', 0,  9, 1),
    (10, 'Income',        '💼', 1, 10, 1);

-- Bank account stub (Phase 5b will populate this)
CREATE TABLE bank_account (
    id               INTEGER PRIMARY KEY,
    provider         TEXT    NOT NULL,
    institution_name TEXT    NOT NULL,
    account_name     TEXT    NOT NULL,
    account_type     TEXT    NOT NULL DEFAULT 'current',
    currency         TEXT    NOT NULL DEFAULT 'GBP',
    external_id      TEXT    NOT NULL,
    requisition_id   TEXT,
    token_expires_at INTEGER,
    last_synced_at   INTEGER,
    last_nudge_at    INTEGER,
    created_at       INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at       INTEGER
);

-- Transaction — manual or bank-synced
CREATE TABLE ledger_transaction (
    id              INTEGER PRIMARY KEY,
    bank_account_id INTEGER REFERENCES bank_account(id),
    external_id     TEXT,
    amount_pence    INTEGER NOT NULL,
    currency        TEXT    NOT NULL DEFAULT 'GBP',
    description     TEXT    NOT NULL,
    merchant        TEXT,
    category_id     INTEGER REFERENCES category(id),
    date            INTEGER NOT NULL,
    source          TEXT    NOT NULL DEFAULT 'manual',
    note            TEXT,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at      INTEGER,
    UNIQUE(bank_account_id, external_id)
);

CREATE INDEX idx_ledger_transaction_date     ON ledger_transaction(date);
CREATE INDEX idx_ledger_transaction_category ON ledger_transaction(category_id);

-- Monthly budget per category
CREATE TABLE budget (
    id           INTEGER PRIMARY KEY,
    category_id  INTEGER NOT NULL REFERENCES category(id),
    amount_pence INTEGER NOT NULL,
    created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at   INTEGER,
    UNIQUE(category_id)
);
```

- [ ] **Step 2: Verify the migration runs**

```bash
cd /Users/hanamori/life-assistant
cargo test -p manor-core 2>&1 | tail -5
```

Expected: all existing tests still pass. The `db::init` function in `crates/core/src/assistant/db.rs` runs Refinery migrations automatically — V5 runs on every fresh test DB.

- [ ] **Step 3: Commit**

```bash
git add crates/core/migrations/V5__ledger.sql
git commit -m "feat(ledger): add V5 ledger schema — category, transaction, budget tables"
```

---

### Task 2: Category DAL

**Files:**
- Create: `crates/core/src/ledger/mod.rs`
- Create: `crates/core/src/ledger/category.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Create the ledger mod file**

Create `crates/core/src/ledger/mod.rs`:

```rust
//! Ledger subsystem: categories, transactions, budgets.

pub mod budget;
pub mod category;
pub mod transaction;
```

- [ ] **Step 2: Add `pub mod ledger;` to core lib.rs**

Current `crates/core/src/lib.rs`:
```rust
pub mod assistant;
```

Add after `pub mod assistant;`:
```rust
pub mod ledger;
```

- [ ] **Step 3: Write the failing test**

Create `crates/core/src/ledger/category.rs` with the test at the bottom:

```rust
//! Category DAL — CRUD for transaction categories.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub emoji: String,
    pub is_income: bool,
    pub sort_order: i32,
    pub is_default: bool,
    pub deleted_at: Option<i64>,
}

impl Category {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            emoji: row.get("emoji")?,
            is_income: row.get::<_, i64>("is_income")? != 0,
            sort_order: row.get("sort_order")?,
            is_default: row.get::<_, i64>("is_default")? != 0,
            deleted_at: row.get("deleted_at")?,
        })
    }
}

pub fn list(conn: &Connection) -> Result<Vec<Category>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, emoji, is_income, sort_order, is_default, deleted_at
         FROM category
         WHERE deleted_at IS NULL
         ORDER BY sort_order, id",
    )?;
    let rows = stmt
        .query_map([], Category::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn insert(
    conn: &Connection,
    name: &str,
    emoji: &str,
    is_income: bool,
) -> Result<Category> {
    let max_order: i32 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) FROM category WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO category (name, emoji, is_income, sort_order, is_default)
         VALUES (?1, ?2, ?3, ?4, 0)",
        params![name, emoji, is_income as i64, max_order + 1],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, name: &str, emoji: &str) -> Result<Category> {
    conn.execute(
        "UPDATE category SET name = ?1, emoji = ?2 WHERE id = ?3",
        params![name, emoji, id],
    )?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let is_default: i64 = conn.query_row(
        "SELECT is_default FROM category WHERE id = ?1",
        [id],
        |r| r.get(0),
    )?;
    anyhow::ensure!(is_default == 0, "cannot delete a default category");
    // Reassign orphaned transactions to Other (id = 9)
    conn.execute(
        "UPDATE ledger_transaction SET category_id = 9
         WHERE category_id = ?1 AND deleted_at IS NULL",
        [id],
    )?;
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE category SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

fn get(conn: &Connection, id: i64) -> Result<Category> {
    let mut stmt = conn.prepare(
        "SELECT id, name, emoji, is_income, sort_order, is_default, deleted_at
         FROM category WHERE id = ?1",
    )?;
    Ok(stmt.query_row([id], Category::from_row)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn seeds_ten_default_categories() {
        let (_d, conn) = fresh_conn();
        let cats = list(&conn).unwrap();
        assert_eq!(cats.len(), 10, "expected 10 seeded categories");
        assert!(cats.iter().any(|c| c.name == "Groceries"));
        assert!(cats.iter().any(|c| c.name == "Income" && c.is_income));
    }

    #[test]
    fn insert_custom_category_appended_at_end() {
        let (_d, conn) = fresh_conn();
        let cat = insert(&conn, "Pets", "🐶", false).unwrap();
        assert_eq!(cat.name, "Pets");
        assert!(!cat.is_default);
        let all = list(&conn).unwrap();
        assert_eq!(all.last().unwrap().name, "Pets");
    }

    #[test]
    fn update_renames_category() {
        let (_d, conn) = fresh_conn();
        let cat = insert(&conn, "Pets", "🐶", false).unwrap();
        let updated = update(&conn, cat.id, "Animals", "🐾").unwrap();
        assert_eq!(updated.name, "Animals");
        assert_eq!(updated.emoji, "🐾");
    }

    #[test]
    fn cannot_delete_default_category() {
        let (_d, conn) = fresh_conn();
        let err = delete(&conn, 1).unwrap_err(); // id=1 is Groceries (default)
        assert!(err.to_string().contains("cannot delete a default category"));
    }

    #[test]
    fn delete_custom_category_soft_deletes_it() {
        let (_d, conn) = fresh_conn();
        let cat = insert(&conn, "Hobbies", "🎸", false).unwrap();
        delete(&conn, cat.id).unwrap();
        let all = list(&conn).unwrap();
        assert!(!all.iter().any(|c| c.id == cat.id));
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/hanamori/life-assistant
cargo test -p manor-core ledger::category 2>&1 | tail -20
```

Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/ledger/ crates/core/src/lib.rs
git commit -m "feat(ledger): category DAL with default seeds and CRUD"
```

---

### Task 3: Transaction DAL

**Files:**
- Create: `crates/core/src/ledger/transaction.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/core/src/ledger/transaction.rs`:

```rust
//! Ledger transaction DAL — manual and synced entries.

use anyhow::Result;
use chrono::{Datelike, TimeZone, Utc};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transaction {
    pub id: i64,
    pub bank_account_id: Option<i64>,
    pub amount_pence: i64,
    pub currency: String,
    pub description: String,
    pub merchant: Option<String>,
    pub category_id: Option<i64>,
    pub date: i64,
    pub source: String,
    pub note: Option<String>,
    pub created_at: i64,
}

impl Transaction {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
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
            created_at: row.get("created_at")?,
        })
    }
}

/// Insert a manual transaction. Returns the inserted row.
pub fn insert(
    conn: &Connection,
    amount_pence: i64,
    currency: &str,
    description: &str,
    merchant: Option<&str>,
    category_id: Option<i64>,
    date: i64,
    note: Option<&str>,
) -> Result<Transaction> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO ledger_transaction
         (bank_account_id, amount_pence, currency, description, merchant,
          category_id, date, source, note, created_at)
         VALUES (NULL, ?1, ?2, ?3, ?4, ?5, ?6, 'manual', ?7, ?8)",
        params![amount_pence, currency, description, merchant, category_id, date, note, now],
    )?;
    get(conn, conn.last_insert_rowid())
}

/// Update description, merchant, category and note on an existing transaction.
pub fn update(
    conn: &Connection,
    id: i64,
    description: &str,
    merchant: Option<&str>,
    category_id: Option<i64>,
    note: Option<&str>,
) -> Result<Transaction> {
    conn.execute(
        "UPDATE ledger_transaction
         SET description = ?1, merchant = ?2, category_id = ?3, note = ?4
         WHERE id = ?5 AND deleted_at IS NULL",
        params![description, merchant, category_id, note, id],
    )?;
    get(conn, id)
}

/// Soft-delete a transaction.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE ledger_transaction SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// All non-deleted transactions for a given month, newest first.
pub fn list_by_month(conn: &Connection, year: i32, month: u32) -> Result<Vec<Transaction>> {
    let start = month_start_ts(year, month);
    let end = month_end_ts(year, month);
    let mut stmt = conn.prepare(
        "SELECT id, bank_account_id, amount_pence, currency, description, merchant,
                category_id, date, source, note, created_at
         FROM ledger_transaction
         WHERE deleted_at IS NULL AND date >= ?1 AND date < ?2
         ORDER BY date DESC, id DESC",
    )?;
    let rows = stmt
        .query_map(params![start, end], Transaction::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Unix timestamp for the first second of the given month (UTC).
pub(crate) fn month_start_ts(year: i32, month: u32) -> i64 {
    Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .unwrap()
        .timestamp()
}

/// Unix timestamp for the first second of the month *after* the given month (exclusive end).
pub(crate) fn month_end_ts(year: i32, month: u32) -> i64 {
    let start = Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0).unwrap();
    let next_month = start + chrono::Duration::days(32);
    Utc.with_ymd_and_hms(next_month.year(), next_month.month(), 1, 0, 0, 0)
        .unwrap()
        .timestamp()
}

fn get(conn: &Connection, id: i64) -> Result<Transaction> {
    let mut stmt = conn.prepare(
        "SELECT id, bank_account_id, amount_pence, currency, description, merchant,
                category_id, date, source, note, created_at
         FROM ledger_transaction WHERE id = ?1",
    )?;
    Ok(stmt.query_row([id], Transaction::from_row)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn april_ts(day: u32) -> i64 {
        Utc.with_ymd_and_hms(2026, 4, day, 12, 0, 0).unwrap().timestamp()
    }

    #[test]
    fn insert_and_retrieve_manual_transaction() {
        let (_d, conn) = fresh_conn();
        let tx = insert(&conn, -1240, "GBP", "Tesco Express", Some("Tesco"), Some(1), april_ts(16), None).unwrap();
        assert_eq!(tx.amount_pence, -1240);
        assert_eq!(tx.description, "Tesco Express");
        assert_eq!(tx.merchant, Some("Tesco".to_string()));
        assert_eq!(tx.category_id, Some(1));
        assert_eq!(tx.source, "manual");
        assert!(tx.bank_account_id.is_none());
    }

    #[test]
    fn list_by_month_filters_correctly() {
        let (_d, conn) = fresh_conn();
        // April transaction
        insert(&conn, -500, "GBP", "April spend", None, None, april_ts(10), None).unwrap();
        // March transaction (should not appear)
        let march_ts = Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap().timestamp();
        insert(&conn, -300, "GBP", "March spend", None, None, march_ts, None).unwrap();

        let txns = list_by_month(&conn, 2026, 4).unwrap();
        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].description, "April spend");
    }

    #[test]
    fn update_edits_description_and_category() {
        let (_d, conn) = fresh_conn();
        let tx = insert(&conn, -800, "GBP", "Old desc", None, None, april_ts(15), None).unwrap();
        let updated = update(&conn, tx.id, "New desc", Some("Deliveroo"), Some(2), Some("note")).unwrap();
        assert_eq!(updated.description, "New desc");
        assert_eq!(updated.merchant, Some("Deliveroo".to_string()));
        assert_eq!(updated.category_id, Some(2));
        assert_eq!(updated.note, Some("note".to_string()));
    }

    #[test]
    fn delete_soft_deletes_transaction() {
        let (_d, conn) = fresh_conn();
        let tx = insert(&conn, -100, "GBP", "Gone", None, None, april_ts(1), None).unwrap();
        delete(&conn, tx.id).unwrap();
        let txns = list_by_month(&conn, 2026, 4).unwrap();
        assert!(txns.is_empty());
    }

    #[test]
    fn month_boundary_tests() {
        // March end should equal April start
        assert_eq!(month_end_ts(2026, 3), month_start_ts(2026, 4));
        // December wraps to January
        assert_eq!(month_end_ts(2026, 12), month_start_ts(2027, 1));
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cd /Users/hanamori/life-assistant
cargo test -p manor-core ledger::transaction 2>&1 | tail -20
```

Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/ledger/transaction.rs crates/core/src/ledger/mod.rs
git commit -m "feat(ledger): transaction DAL with insert/update/delete/list_by_month"
```

---

### Task 4: Budget DAL + monthly summary

**Files:**
- Create: `crates/core/src/ledger/budget.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/core/src/ledger/budget.rs`:

```rust
//! Budget DAL — monthly limits per category + spend rollup.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use crate::ledger::transaction::{month_end_ts, month_start_ts};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Budget {
    pub id: i64,
    pub category_id: i64,
    pub amount_pence: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySpend {
    pub category_id: i64,
    pub category_name: String,
    pub category_emoji: String,
    pub spent_pence: i64,
    pub budget_pence: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlySummary {
    pub total_in_pence: i64,
    pub total_out_pence: i64,
    pub by_category: Vec<CategorySpend>,
}

impl Budget {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            category_id: row.get("category_id")?,
            amount_pence: row.get("amount_pence")?,
            created_at: row.get("created_at")?,
        })
    }
}

pub fn list(conn: &Connection) -> Result<Vec<Budget>> {
    let mut stmt = conn.prepare(
        "SELECT id, category_id, amount_pence, created_at
         FROM budget WHERE deleted_at IS NULL",
    )?;
    let rows = stmt
        .query_map([], Budget::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Insert or update the monthly budget for a category.
/// If the category had a soft-deleted budget, it is restored.
pub fn upsert(conn: &Connection, category_id: i64, amount_pence: i64) -> Result<Budget> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO budget (category_id, amount_pence, created_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(category_id) DO UPDATE
           SET amount_pence = excluded.amount_pence,
               deleted_at   = NULL",
        params![category_id, amount_pence, now],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM budget WHERE category_id = ?1",
        [category_id],
        |r| r.get(0),
    )?;
    Ok(conn.query_row(
        "SELECT id, category_id, amount_pence, created_at FROM budget WHERE id = ?1",
        [id],
        Budget::from_row,
    )?)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE budget SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Aggregate total in/out and per-category spend vs budget for a month.
pub fn monthly_summary(conn: &Connection, year: i32, month: u32) -> Result<MonthlySummary> {
    let start = month_start_ts(year, month);
    let end = month_end_ts(year, month);

    let (total_in, total_out): (i64, i64) = conn.query_row(
        "SELECT
           COALESCE(SUM(CASE WHEN amount_pence > 0 THEN amount_pence ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN amount_pence < 0 THEN -amount_pence ELSE 0 END), 0)
         FROM ledger_transaction
         WHERE deleted_at IS NULL AND date >= ?1 AND date < ?2",
        params![start, end],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.emoji,
                COALESCE(SUM(-t.amount_pence), 0) AS spent,
                b.amount_pence AS budget_pence
         FROM category c
         LEFT JOIN ledger_transaction t
           ON t.category_id = c.id
          AND t.deleted_at IS NULL
          AND t.date >= ?1 AND t.date < ?2
          AND t.amount_pence < 0
         LEFT JOIN budget b ON b.category_id = c.id AND b.deleted_at IS NULL
         WHERE c.deleted_at IS NULL AND c.is_income = 0
         GROUP BY c.id
         ORDER BY spent DESC",
    )?;
    let by_category = stmt
        .query_map(params![start, end], |row| {
            Ok(CategorySpend {
                category_id: row.get(0)?,
                category_name: row.get(1)?,
                category_emoji: row.get(2)?,
                spent_pence: row.get(3)?,
                budget_pence: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(MonthlySummary {
        total_in_pence: total_in,
        total_out_pence: total_out,
        by_category,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::transaction;
    use manor_core::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn april_ts(day: u32) -> i64 {
        use chrono::{TimeZone, Utc};
        Utc.with_ymd_and_hms(2026, 4, day, 12, 0, 0).unwrap().timestamp()
    }

    #[test]
    fn upsert_creates_budget() {
        let (_d, conn) = fresh_conn();
        let b = upsert(&conn, 1, 40000).unwrap(); // £400 for Groceries
        assert_eq!(b.category_id, 1);
        assert_eq!(b.amount_pence, 40000);
    }

    #[test]
    fn upsert_updates_existing_budget() {
        let (_d, conn) = fresh_conn();
        upsert(&conn, 1, 40000).unwrap();
        let updated = upsert(&conn, 1, 50000).unwrap();
        assert_eq!(updated.amount_pence, 50000);
        assert_eq!(list(&conn).unwrap().len(), 1); // still only one row
    }

    #[test]
    fn delete_removes_budget_from_list() {
        let (_d, conn) = fresh_conn();
        let b = upsert(&conn, 1, 40000).unwrap();
        delete(&conn, b.id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }

    #[test]
    fn upsert_restores_soft_deleted_budget() {
        let (_d, conn) = fresh_conn();
        let b = upsert(&conn, 1, 40000).unwrap();
        delete(&conn, b.id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
        let restored = upsert(&conn, 1, 60000).unwrap();
        assert_eq!(restored.amount_pence, 60000);
        assert_eq!(list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn monthly_summary_totals_in_and_out() {
        let (_d, conn) = fresh_conn();
        transaction::insert(&conn, 320000, "GBP", "Salary", None, Some(10), april_ts(1), None).unwrap();
        transaction::insert(&conn, -3420, "GBP", "Tesco", None, Some(1), april_ts(5), None).unwrap();
        transaction::insert(&conn, -1850, "GBP", "Deliveroo", None, Some(2), april_ts(10), None).unwrap();

        let s = monthly_summary(&conn, 2026, 4).unwrap();
        assert_eq!(s.total_in_pence, 320000);
        assert_eq!(s.total_out_pence, 5270);
    }

    #[test]
    fn monthly_summary_category_spend_and_budget() {
        let (_d, conn) = fresh_conn();
        upsert(&conn, 1, 40000).unwrap(); // £400 grocery budget
        transaction::insert(&conn, -3420, "GBP", "Tesco", None, Some(1), april_ts(5), None).unwrap();

        let s = monthly_summary(&conn, 2026, 4).unwrap();
        let groceries = s.by_category.iter().find(|c| c.category_id == 1).unwrap();
        assert_eq!(groceries.spent_pence, 3420);
        assert_eq!(groceries.budget_pence, Some(40000));

        let eating_out = s.by_category.iter().find(|c| c.category_id == 2).unwrap();
        assert_eq!(eating_out.spent_pence, 0);
        assert_eq!(eating_out.budget_pence, None);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cd /Users/hanamori/life-assistant
cargo test -p manor-core ledger::budget 2>&1 | tail -20
```

Expected: 6 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/ledger/budget.rs
git commit -m "feat(ledger): budget DAL with upsert/delete/monthly_summary"
```

---

### Task 5: App ledger module + Tauri commands

**Files:**
- Create: `crates/app/src/ledger/mod.rs`
- Create: `crates/app/src/ledger/commands.rs`
- Modify: `crates/app/src/lib.rs`

- [ ] **Step 1: Write the failing test (compile check)**

Create `crates/app/src/ledger/mod.rs`:

```rust
//! Tauri command glue for the Ledger feature.

pub mod commands;
```

Create `crates/app/src/ledger/commands.rs`:

```rust
//! Tauri commands for Ledger — categories, transactions, budgets.

use crate::assistant::commands::Db;
use chrono::{Datelike, Local};
use manor_core::ledger::{budget, category, transaction};
use tauri::State;

fn current_year_month() -> (i32, u32) {
    let now = Local::now();
    (now.year(), now.month())
}

// ── Categories ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn ledger_list_categories(state: State<'_, Db>) -> Result<Vec<category::Category>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    category::list(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct UpsertCategoryArgs {
    pub id: Option<i64>,
    pub name: String,
    pub emoji: String,
    #[serde(rename = "isIncome")]
    pub is_income: bool,
}

#[tauri::command]
pub fn ledger_upsert_category(
    state: State<'_, Db>,
    args: UpsertCategoryArgs,
) -> Result<category::Category, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(id) = args.id {
        category::update(&conn, id, &args.name, &args.emoji).map_err(|e| e.to_string())
    } else {
        category::insert(&conn, &args.name, &args.emoji, args.is_income)
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn ledger_delete_category(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    category::delete(&conn, id).map_err(|e| e.to_string())
}

// ── Transactions ──────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct AddTransactionArgs {
    #[serde(rename = "amountPence")]
    pub amount_pence: i64,
    pub currency: String,
    pub description: String,
    pub merchant: Option<String>,
    #[serde(rename = "categoryId")]
    pub category_id: Option<i64>,
    pub date: i64,
    pub note: Option<String>,
}

#[tauri::command]
pub fn ledger_add_transaction(
    state: State<'_, Db>,
    args: AddTransactionArgs,
) -> Result<transaction::Transaction, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    transaction::insert(
        &conn,
        args.amount_pence,
        &args.currency,
        &args.description,
        args.merchant.as_deref(),
        args.category_id,
        args.date,
        args.note.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct UpdateTransactionArgs {
    pub id: i64,
    pub description: String,
    pub merchant: Option<String>,
    #[serde(rename = "categoryId")]
    pub category_id: Option<i64>,
    pub note: Option<String>,
}

#[tauri::command]
pub fn ledger_update_transaction(
    state: State<'_, Db>,
    args: UpdateTransactionArgs,
) -> Result<transaction::Transaction, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    transaction::update(
        &conn,
        args.id,
        &args.description,
        args.merchant.as_deref(),
        args.category_id,
        args.note.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_delete_transaction(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    transaction::delete(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_list_transactions(
    state: State<'_, Db>,
    year: i32,
    month: u32,
) -> Result<Vec<transaction::Transaction>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    transaction::list_by_month(&conn, year, month).map_err(|e| e.to_string())
}

// ── Budgets ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn ledger_list_budgets(state: State<'_, Db>) -> Result<Vec<budget::Budget>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    budget::list(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct UpsertBudgetArgs {
    #[serde(rename = "categoryId")]
    pub category_id: i64,
    #[serde(rename = "amountPence")]
    pub amount_pence: i64,
}

#[tauri::command]
pub fn ledger_upsert_budget(
    state: State<'_, Db>,
    args: UpsertBudgetArgs,
) -> Result<budget::Budget, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    budget::upsert(&conn, args.category_id, args.amount_pence).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_delete_budget(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    budget::delete(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_monthly_summary(
    state: State<'_, Db>,
    year: i32,
    month: u32,
) -> Result<budget::MonthlySummary, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    budget::monthly_summary(&conn, year, month).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Add `pub mod ledger;` to `crates/app/src/lib.rs`**

In `crates/app/src/lib.rs`, add after `pub mod sync;`:

```rust
pub mod ledger;
```

- [ ] **Step 3: Register the 9 commands in the invoke_handler in `crates/app/src/lib.rs`**

In the `invoke_handler` macro call, add after `rhythm::commands::add_person,`:

```rust
ledger::commands::ledger_list_categories,
ledger::commands::ledger_upsert_category,
ledger::commands::ledger_delete_category,
ledger::commands::ledger_add_transaction,
ledger::commands::ledger_update_transaction,
ledger::commands::ledger_delete_transaction,
ledger::commands::ledger_list_transactions,
ledger::commands::ledger_list_budgets,
ledger::commands::ledger_upsert_budget,
ledger::commands::ledger_delete_budget,
ledger::commands::ledger_monthly_summary,
```

- [ ] **Step 4: Verify compilation**

```bash
cd /Users/hanamori/life-assistant
cargo build -p manor-app 2>&1 | tail -10
```

Expected: compiles with no errors.

- [ ] **Step 5: Run all Rust tests**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/ledger/ crates/app/src/lib.rs
git commit -m "feat(ledger): app ledger module + 11 Tauri commands wired"
```

---

### Task 6: Frontend IPC + Zustand store

**Files:**
- Create: `apps/desktop/src/lib/ledger/ipc.ts`
- Create: `apps/desktop/src/lib/ledger/state.ts`

- [ ] **Step 1: Create the IPC wrapper**

Create `apps/desktop/src/lib/ledger/ipc.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";

export interface Category {
  id: number;
  name: string;
  emoji: string;
  is_income: boolean;
  sort_order: number;
  is_default: boolean;
  deleted_at: number | null;
}

export interface Transaction {
  id: number;
  bank_account_id: number | null;
  amount_pence: number;
  currency: string;
  description: string;
  merchant: string | null;
  category_id: number | null;
  date: number;
  source: "manual" | "sync";
  note: string | null;
  created_at: number;
}

export interface Budget {
  id: number;
  category_id: number;
  amount_pence: number;
  created_at: number;
}

export interface CategorySpend {
  category_id: number;
  category_name: string;
  category_emoji: string;
  spent_pence: number;
  budget_pence: number | null;
}

export interface MonthlySummary {
  total_in_pence: number;
  total_out_pence: number;
  by_category: CategorySpend[];
}

// Categories
export async function listCategories(): Promise<Category[]> {
  return invoke<Category[]>("ledger_list_categories");
}
export async function upsertCategory(args: {
  id?: number;
  name: string;
  emoji: string;
  isIncome: boolean;
}): Promise<Category> {
  return invoke<Category>("ledger_upsert_category", { args });
}
export async function deleteCategory(id: number): Promise<void> {
  return invoke<void>("ledger_delete_category", { id });
}

// Transactions
export async function listTransactions(year: number, month: number): Promise<Transaction[]> {
  return invoke<Transaction[]>("ledger_list_transactions", { year, month });
}
export async function addTransaction(args: {
  amountPence: number;
  currency: string;
  description: string;
  merchant?: string;
  categoryId?: number;
  date: number;
  note?: string;
}): Promise<Transaction> {
  return invoke<Transaction>("ledger_add_transaction", { args });
}
export async function updateTransaction(args: {
  id: number;
  description: string;
  merchant?: string;
  categoryId?: number;
  note?: string;
}): Promise<Transaction> {
  return invoke<Transaction>("ledger_update_transaction", { args });
}
export async function deleteTransaction(id: number): Promise<void> {
  return invoke<void>("ledger_delete_transaction", { id });
}

// Budgets
export async function listBudgets(): Promise<Budget[]> {
  return invoke<Budget[]>("ledger_list_budgets");
}
export async function upsertBudget(args: {
  categoryId: number;
  amountPence: number;
}): Promise<Budget> {
  return invoke<Budget>("ledger_upsert_budget", { args });
}
export async function deleteBudget(id: number): Promise<void> {
  return invoke<void>("ledger_delete_budget", { id });
}
export async function getMonthlySummary(
  year: number,
  month: number
): Promise<MonthlySummary> {
  return invoke<MonthlySummary>("ledger_monthly_summary", { year, month });
}
```

- [ ] **Step 2: Create the Zustand store**

Create `apps/desktop/src/lib/ledger/state.ts`:

```typescript
import { create } from "zustand";
import type { Budget, Category, MonthlySummary, Transaction } from "./ipc";

interface LedgerStore {
  categories: Category[];
  transactions: Transaction[];
  budgets: Budget[];
  summary: MonthlySummary | null;
  currentYear: number;
  currentMonth: number;

  setCategories: (c: Category[]) => void;
  setTransactions: (t: Transaction[]) => void;
  setBudgets: (b: Budget[]) => void;
  setSummary: (s: MonthlySummary) => void;
  upsertTransaction: (t: Transaction) => void;
  removeTransaction: (id: number) => void;
  upsertCategory: (c: Category) => void;
  removeCategory: (id: number) => void;
  upsertBudget: (b: Budget) => void;
  removeBudget: (id: number) => void;
}

const now = new Date();

export const useLedgerStore = create<LedgerStore>((set) => ({
  categories: [],
  transactions: [],
  budgets: [],
  summary: null,
  currentYear: now.getFullYear(),
  currentMonth: now.getMonth() + 1,

  setCategories: (c) => set({ categories: c }),
  setTransactions: (t) => set({ transactions: t }),
  setBudgets: (b) => set({ budgets: b }),
  setSummary: (s) => set({ summary: s }),

  upsertTransaction: (t) =>
    set((st) => {
      const idx = st.transactions.findIndex((x) => x.id === t.id);
      if (idx === -1) return { transactions: [t, ...st.transactions] };
      const next = st.transactions.slice();
      next[idx] = t;
      return { transactions: next };
    }),

  removeTransaction: (id) =>
    set((st) => ({ transactions: st.transactions.filter((x) => x.id !== id) })),

  upsertCategory: (c) =>
    set((st) => {
      const idx = st.categories.findIndex((x) => x.id === c.id);
      if (idx === -1) return { categories: [...st.categories, c] };
      const next = st.categories.slice();
      next[idx] = c;
      return { categories: next };
    }),

  removeCategory: (id) =>
    set((st) => ({ categories: st.categories.filter((x) => x.id !== id) })),

  upsertBudget: (b) =>
    set((st) => {
      const idx = st.budgets.findIndex((x) => x.id === b.id);
      if (idx === -1) return { budgets: [...st.budgets, b] };
      const next = st.budgets.slice();
      next[idx] = b;
      return { budgets: next };
    }),

  removeBudget: (id) =>
    set((st) => ({ budgets: st.budgets.filter((x) => x.id !== id) })),
}));
```

- [ ] **Step 3: Type-check**

```bash
cd /Users/hanamori/life-assistant/apps/desktop && pnpm exec tsc --noEmit 2>&1
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant
git add apps/desktop/src/lib/ledger/
git commit -m "feat(ledger): frontend IPC wrappers and Zustand store"
```

---

### Task 7: Nav wiring

**Files:**
- Modify: `apps/desktop/src/lib/nav.ts`
- Modify: `apps/desktop/src/components/Nav/Sidebar.tsx`
- Modify: `apps/desktop/src/App.tsx`

- [ ] **Step 1: Add `"ledger"` to the View type in `nav.ts`**

In `apps/desktop/src/lib/nav.ts`, change:

```typescript
export type View = "today" | "chores" | "timeblocks";
```

to:

```typescript
export type View = "today" | "chores" | "timeblocks" | "ledger";
```

- [ ] **Step 2: Add the Ledger nav icon in `Sidebar.tsx`**

In `apps/desktop/src/components/Nav/Sidebar.tsx`, after `<NavIcon view="timeblocks" icon="⏱" title="Time Blocks" />` add:

```tsx
<NavIcon view="ledger" icon="💰" title="Ledger" />
```

- [ ] **Step 3: Wire LedgerView in `App.tsx`**

In `apps/desktop/src/App.tsx`, add the import after the `TimeBlocksView` import:

```tsx
import LedgerView from "./components/Ledger/LedgerView";
```

Add the view render after `{view === "timeblocks" && <TimeBlocksView />}`:

```tsx
{view === "ledger" && <LedgerView />}
```

At this point `LedgerView.tsx` doesn't exist yet — the app won't compile until Task 8. That's fine; commit the nav wiring now and the view in the next task.

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant
git add apps/desktop/src/lib/nav.ts \
        apps/desktop/src/components/Nav/Sidebar.tsx \
        apps/desktop/src/App.tsx
git commit -m "feat(ledger): add Ledger nav icon and view slot"
```

---

### Task 8: LedgerView + SummaryCard

**Files:**
- Create: `apps/desktop/src/components/Ledger/LedgerView.tsx`
- Create: `apps/desktop/src/components/Ledger/SummaryCard.tsx`

- [ ] **Step 1: Create SummaryCard**

Create `apps/desktop/src/components/Ledger/SummaryCard.tsx`:

```tsx
import type { MonthlySummary } from "../../lib/ledger/ipc";

const MONTH_NAMES = [
  "January","February","March","April","May","June",
  "July","August","September","October","November","December",
];

function gradientForSpend(totalOut: number, totalBudget: number | null): string {
  if (totalBudget === null || totalBudget === 0) return "linear-gradient(135deg, #1a1a2e 0%, #16213e 100%)";
  const pct = totalOut / totalBudget;
  if (pct >= 1) return "linear-gradient(135deg, #2d0000 0%, #3d0a0a 100%)";
  if (pct >= 0.75) return "linear-gradient(135deg, #2d1f00 0%, #3d2900 100%)";
  return "linear-gradient(135deg, #1a1a2e 0%, #16213e 100%)";
}

function progressColor(pct: number): string {
  if (pct >= 1) return "#FF3B30";
  if (pct >= 0.75) return "#FFB347";
  return "white";
}

interface Props {
  summary: MonthlySummary;
  year: number;
  month: number;
  totalBudget: number | null; // sum of all category budgets, null if none set
  onBudgetPress: () => void;
}

export default function SummaryCard({ summary, year, month, totalBudget, onBudgetPress }: Props) {
  const pct = totalBudget ? Math.min(summary.total_out_pence / totalBudget, 1.1) : 0;
  const remaining = totalBudget ? totalBudget - summary.total_out_pence : null;

  const alertCategories = summary.by_category.filter(
    (c) => c.budget_pence !== null && c.budget_pence > 0 && c.spent_pence / c.budget_pence >= 0.75
  ).slice(0, 3);

  function formatPounds(pence: number): string {
    return `£${(Math.abs(pence) / 100).toFixed(0)}`;
  }

  return (
    <div
      onClick={onBudgetPress}
      role="button"
      tabIndex={0}
      style={{
        background: gradientForSpend(summary.total_out_pence, totalBudget),
        borderRadius: 14,
        padding: "18px 20px",
        color: "white",
        cursor: "pointer",
        marginBottom: 8,
      }}
    >
      <div style={{ fontSize: 11, opacity: 0.5, letterSpacing: 0.6, marginBottom: 8 }}>
        {MONTH_NAMES[month - 1].toUpperCase()} {year}
      </div>

      <div style={{ fontSize: 28, fontWeight: 700, marginBottom: 4 }}>
        {formatPounds(summary.total_out_pence)}
      </div>

      {totalBudget !== null && (
        <div style={{ fontSize: 12, opacity: 0.5, marginBottom: 12 }}>
          of {formatPounds(totalBudget)} budget
          {remaining !== null && remaining >= 0
            ? ` · ${formatPounds(remaining)} remaining`
            : ` · ${formatPounds(Math.abs(remaining ?? 0))} over`}
        </div>
      )}

      {totalBudget !== null && (
        <div
          style={{
            background: "rgba(255,255,255,0.12)",
            borderRadius: 6,
            height: 6,
            marginBottom: alertCategories.length > 0 ? 14 : 0,
          }}
        >
          <div
            style={{
              background: progressColor(pct),
              width: `${Math.min(pct * 100, 100)}%`,
              height: 6,
              borderRadius: 6,
              transition: "width 0.3s",
            }}
          />
        </div>
      )}

      {alertCategories.length > 0 && (
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {alertCategories.map((c) => {
            const catPct = c.budget_pence! > 0 ? c.spent_pence / c.budget_pence! : 0;
            const over = catPct >= 1;
            return (
              <div
                key={c.category_id}
                style={{
                  background: over
                    ? "rgba(255,59,48,0.25)"
                    : "rgba(255,179,71,0.25)",
                  border: `1px solid ${over ? "rgba(255,59,48,0.5)" : "rgba(255,179,71,0.5)"}`,
                  borderRadius: 20,
                  padding: "4px 10px",
                  fontSize: 11,
                }}
              >
                {over ? "🔴" : "⚠️"} {c.category_name} {Math.round(catPct * 100)}%
              </div>
            );
          })}
        </div>
      )}

      {totalBudget === null && (
        <div style={{ fontSize: 12, opacity: 0.4 }}>
          Tap to set budgets →
        </div>
      )}

      {summary.total_in_pence > 0 && (
        <div style={{ fontSize: 12, opacity: 0.5, marginTop: 8 }}>
          +{formatPounds(summary.total_in_pence)} income
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create LedgerView (loads data, renders SummaryCard + placeholder feed)**

Create `apps/desktop/src/components/Ledger/LedgerView.tsx`:

```tsx
import { useEffect } from "react";
import { useLedgerStore } from "../../lib/ledger/state";
import {
  listCategories,
  listTransactions,
  listBudgets,
  getMonthlySummary,
} from "../../lib/ledger/ipc";
import { AVATAR_FOOTPRINT_PX } from "../../lib/layout";
import SummaryCard from "./SummaryCard";
import TransactionFeed from "./TransactionFeed";
import AddTransactionForm from "./AddTransactionForm";
import BudgetSheet from "./BudgetSheet";
import { useState } from "react";

export default function LedgerView() {
  const { categories, transactions, budgets, summary, currentYear, currentMonth,
          setCategories, setTransactions, setBudgets, setSummary } = useLedgerStore();
  const [showAdd, setShowAdd] = useState(false);
  const [showBudgets, setShowBudgets] = useState(false);

  useEffect(() => {
    void listCategories().then(setCategories);
    void listBudgets().then(setBudgets);
    void listTransactions(currentYear, currentMonth).then(setTransactions);
    void getMonthlySummary(currentYear, currentMonth).then(setSummary);
  }, [currentYear, currentMonth, setCategories, setBudgets, setTransactions, setSummary]);

  const totalBudget = budgets.length > 0
    ? budgets.reduce((sum, b) => sum + b.amount_pence, 0)
    : null;

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
        {summary && (
          <SummaryCard
            summary={summary}
            year={currentYear}
            month={currentMonth}
            totalBudget={totalBudget}
            onBudgetPress={() => setShowBudgets(true)}
          />
        )}
        <TransactionFeed
          transactions={transactions}
          categories={categories}
          onAdd={() => setShowAdd(true)}
        />
      </main>

      {showAdd && (
        <AddTransactionForm
          categories={categories}
          onClose={() => setShowAdd(false)}
          onSaved={async () => {
            setShowAdd(false);
            const [txns, s] = await Promise.all([
              listTransactions(currentYear, currentMonth),
              getMonthlySummary(currentYear, currentMonth),
            ]);
            setTransactions(txns);
            setSummary(s);
          }}
        />
      )}

      {showBudgets && (
        <BudgetSheet
          categories={categories}
          budgets={budgets}
          onClose={() => setShowBudgets(false)}
          onChanged={async () => {
            const [bs, s] = await Promise.all([
              listBudgets(),
              getMonthlySummary(currentYear, currentMonth),
            ]);
            setBudgets(bs);
            setSummary(s);
          }}
        />
      )}
    </>
  );
}
```

- [ ] **Step 3: Type-check**

```bash
cd /Users/hanamori/life-assistant/apps/desktop && pnpm exec tsc --noEmit 2>&1
```

This will fail because `TransactionFeed`, `AddTransactionForm`, and `BudgetSheet` don't exist yet. That's expected — proceed to the commit after creating stub files.

Create stub `apps/desktop/src/components/Ledger/TransactionFeed.tsx`:

```tsx
import type { Category, Transaction } from "../../lib/ledger/ipc";

interface Props {
  transactions: Transaction[];
  categories: Category[];
  onAdd: () => void;
}

export default function TransactionFeed(_props: Props) {
  return <div>Transaction feed — coming in next task</div>;
}
```

Create stub `apps/desktop/src/components/Ledger/AddTransactionForm.tsx`:

```tsx
import type { Category } from "../../lib/ledger/ipc";

interface Props {
  categories: Category[];
  onClose: () => void;
  onSaved: () => Promise<void>;
}

export default function AddTransactionForm(_props: Props) {
  return null;
}
```

Create stub `apps/desktop/src/components/Ledger/BudgetSheet.tsx`:

```tsx
import type { Budget, Category } from "../../lib/ledger/ipc";

interface Props {
  categories: Category[];
  budgets: Budget[];
  onClose: () => void;
  onChanged: () => Promise<void>;
}

export default function BudgetSheet(_props: Props) {
  return null;
}
```

- [ ] **Step 4: Re-run type-check — expect clean**

```bash
cd /Users/hanamori/life-assistant/apps/desktop && pnpm exec tsc --noEmit 2>&1
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
cd /Users/hanamori/life-assistant
git add apps/desktop/src/components/Ledger/
git commit -m "feat(ledger): LedgerView + SummaryCard with budget health badges"
```

---

### Task 9: TransactionFeed + TransactionRow

**Files:**
- Modify: `apps/desktop/src/components/Ledger/TransactionFeed.tsx` (replace stub)
- Create: `apps/desktop/src/components/Ledger/TransactionRow.tsx`

- [ ] **Step 1: Create TransactionRow**

Create `apps/desktop/src/components/Ledger/TransactionRow.tsx`:

```tsx
import type { Category, Transaction } from "../../lib/ledger/ipc";

// Category → pastel background colour for the emoji icon
const CATEGORY_COLORS: Record<number, string> = {
  1: "#E8F4FD",  // Groceries — light blue
  2: "#FFF0F0",  // Eating Out — light red
  3: "#F0F0FF",  // Transport — light purple
  4: "#F5F0FF",  // Utilities — light violet
  5: "#FFF8E6",  // Subscriptions — light amber
  6: "#F0FFF4",  // Health — light green
  7: "#FFF0F8",  // Shopping — light pink
  8: "#F0FAFF",  // Entertainment — light cyan
  9: "#F5F5F5",  // Other — neutral
  10: "#E8FDF0", // Income — green
};

function iconBg(categoryId: number | null): string {
  if (categoryId === null) return "#F5F5F5";
  return CATEGORY_COLORS[categoryId] ?? "#F5F5F5";
}

function formatAmount(pence: number, currency: string): string {
  const symbol = currency === "GBP" ? "£" : currency === "USD" ? "$" : "€";
  const abs = Math.abs(pence) / 100;
  const formatted = abs % 1 === 0 ? abs.toFixed(0) : abs.toFixed(2);
  return `${pence < 0 ? "-" : "+"}${symbol}${formatted}`;
}

interface Props {
  tx: Transaction;
  category: Category | undefined;
  onClick: () => void;
}

export default function TransactionRow({ tx, category, onClick }: Props) {
  const isIncome = tx.amount_pence > 0;

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "10px 12px",
        background: "#fafafa",
        borderRadius: 12,
        cursor: "pointer",
        gap: 10,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 10, minWidth: 0 }}>
        <div
          style={{
            width: 32,
            height: 32,
            borderRadius: 9,
            background: iconBg(tx.category_id),
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 16,
            flexShrink: 0,
          }}
        >
          {category?.emoji ?? "💳"}
        </div>
        <div style={{ minWidth: 0 }}>
          <div
            style={{
              fontSize: 13,
              fontWeight: 600,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {tx.merchant ?? tx.description}
          </div>
          <div style={{ fontSize: 11, color: "#bbb", marginTop: 1 }}>
            {category?.name ?? "Uncategorised"}
            {tx.source === "sync" && " · Synced"}
          </div>
        </div>
      </div>

      <div
        style={{
          fontSize: 13,
          fontWeight: 600,
          color: isIncome ? "#2BB94A" : "inherit",
          flexShrink: 0,
        }}
      >
        {formatAmount(tx.amount_pence, tx.currency)}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Replace TransactionFeed stub with real implementation**

Replace the entire content of `apps/desktop/src/components/Ledger/TransactionFeed.tsx`:

```tsx
import TransactionRow from "./TransactionRow";
import type { Category, Transaction } from "../../lib/ledger/ipc";

const MONTH_SHORT = ["Jan","Feb","Mar","Apr","May","Jun",
                     "Jul","Aug","Sep","Oct","Nov","Dec"];

function dayLabel(dateTs: number): string {
  const d = new Date(dateTs * 1000);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);

  if (d.toDateString() === today.toDateString()) return "TODAY";
  if (d.toDateString() === yesterday.toDateString()) return "YESTERDAY";
  return `${d.getDate()} ${MONTH_SHORT[d.getMonth()]}`.toUpperCase();
}

function groupByDay(txns: Transaction[]): [string, Transaction[]][] {
  const groups = new Map<string, Transaction[]>();
  for (const tx of txns) {
    const label = dayLabel(tx.date);
    if (!groups.has(label)) groups.set(label, []);
    groups.get(label)!.push(tx);
  }
  return Array.from(groups.entries());
}

interface Props {
  transactions: Transaction[];
  categories: Category[];
  onAdd: () => void;
}

export default function TransactionFeed({ transactions, categories, onAdd }: Props) {
  const catMap = new Map(categories.map((c) => [c.id, c]));
  const groups = groupByDay(transactions);

  return (
    <div
      style={{
        background: "var(--paper)",
        border: "1px solid var(--hairline)",
        borderRadius: "var(--radius-lg)",
        boxShadow: "var(--shadow-sm)",
        padding: "16px 18px",
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 12,
        }}
      >
        <div
          style={{
            fontSize: 11,
            fontWeight: 700,
            textTransform: "uppercase",
            letterSpacing: 0.6,
            color: "rgba(0,0,0,0.55)",
          }}
        >
          Transactions
        </div>
        <button
          onClick={onAdd}
          style={{
            background: "var(--imessage-blue)",
            color: "white",
            border: "none",
            borderRadius: 20,
            padding: "5px 14px",
            fontSize: 12,
            fontWeight: 600,
            cursor: "pointer",
            fontFamily: "inherit",
          }}
        >
          + Add
        </button>
      </div>

      {groups.length === 0 && (
        <div
          style={{
            textAlign: "center",
            padding: "24px 0",
            fontSize: 13,
            color: "rgba(0,0,0,0.35)",
          }}
        >
          No transactions this month.
          <br />
          <span style={{ fontSize: 11 }}>Add one manually or connect a bank in v0.3b.</span>
        </div>
      )}

      {groups.map(([label, txns]) => (
        <div key={label} style={{ marginBottom: 16 }}>
          <div
            style={{
              fontSize: 10,
              color: "#aaa",
              fontWeight: 700,
              letterSpacing: 0.6,
              padding: "0 4px",
              marginBottom: 6,
            }}
          >
            {label}
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 3 }}>
            {txns.map((tx) => (
              <TransactionRow
                key={tx.id}
                tx={tx}
                category={catMap.get(tx.category_id ?? -1)}
                onClick={() => {/* edit drawer in future task */}}
              />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Type-check**

```bash
cd /Users/hanamori/life-assistant/apps/desktop && pnpm exec tsc --noEmit 2>&1
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant
git add apps/desktop/src/components/Ledger/TransactionFeed.tsx \
        apps/desktop/src/components/Ledger/TransactionRow.tsx
git commit -m "feat(ledger): TransactionFeed with day groups and TransactionRow"
```

---

### Task 10: AddTransactionForm drawer

**Files:**
- Modify: `apps/desktop/src/components/Ledger/AddTransactionForm.tsx` (replace stub)

- [ ] **Step 1: Replace the stub with the real form**

Replace the entire content of `apps/desktop/src/components/Ledger/AddTransactionForm.tsx`:

```tsx
import { useState } from "react";
import { addTransaction } from "../../lib/ledger/ipc";
import type { Category } from "../../lib/ledger/ipc";

interface Props {
  categories: Category[];
  onClose: () => void;
  onSaved: () => Promise<void>;
}

function todayTs(): number {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return Math.floor(d.getTime() / 1000);
}

function toDateInputValue(ts: number): string {
  return new Date(ts * 1000).toISOString().slice(0, 10);
}

function parsePence(raw: string): number | null {
  // Accept "12.50", "12", "£12.50", "-12.50"
  const cleaned = raw.replace(/[£$€,\s]/g, "");
  const n = parseFloat(cleaned);
  if (isNaN(n)) return null;
  return Math.round(n * 100);
}

export default function AddTransactionForm({ categories, onClose, onSaved }: Props) {
  const [amountRaw, setAmountRaw] = useState("");
  const [description, setDescription] = useState("");
  const [categoryId, setCategoryId] = useState<number | "">("");
  const [date, setDate] = useState(toDateInputValue(todayTs()));
  const [note, setNote] = useState("");
  const [isIncome, setIsIncome] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const expenseCategories = categories.filter((c) => !c.is_income);
  const incomeCategories = categories.filter((c) => c.is_income);
  const visibleCategories = isIncome ? incomeCategories : expenseCategories;

  async function handleSave() {
    const pence = parsePence(amountRaw);
    if (pence === null || pence === 0) {
      setError("Enter a valid amount");
      return;
    }
    if (!description.trim()) {
      setError("Enter a description");
      return;
    }
    const dateTs = Math.floor(new Date(date).getTime() / 1000);
    const signedPence = isIncome ? Math.abs(pence) : -Math.abs(pence);

    setSaving(true);
    setError(null);
    try {
      await addTransaction({
        amountPence: signedPence,
        currency: "GBP",
        description: description.trim(),
        categoryId: categoryId !== "" ? categoryId : undefined,
        date: dateTs,
        note: note.trim() || undefined,
      });
      await onSaved();
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  }

  const inputStyle: React.CSSProperties = {
    width: "100%",
    padding: "9px 12px",
    fontSize: 14,
    border: "1px solid var(--hairline)",
    borderRadius: 10,
    background: "#fafafa",
    fontFamily: "inherit",
    boxSizing: "border-box",
  };

  const labelStyle: React.CSSProperties = {
    fontSize: 11,
    fontWeight: 700,
    textTransform: "uppercase",
    letterSpacing: 0.5,
    color: "rgba(0,0,0,0.5)",
    marginBottom: 5,
    display: "block",
  };

  return (
    <>
      <div
        onClick={onClose}
        style={{
          position: "fixed",
          inset: 0,
          background: "rgba(0,0,0,0.25)",
          zIndex: 700,
        }}
      />
      <div
        style={{
          position: "fixed",
          right: 0,
          top: 0,
          bottom: 0,
          width: 420,
          background: "var(--paper)",
          boxShadow: "-4px 0 24px rgba(0,0,0,0.12)",
          zIndex: 800,
          display: "flex",
          flexDirection: "column",
          animation: "drawerIn 200ms ease-out",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            padding: "18px 20px 14px",
            borderBottom: "1px solid var(--hairline)",
          }}
        >
          <div style={{ fontSize: 16, fontWeight: 700 }}>Add Transaction</div>
          <button
            onClick={onClose}
            style={{
              background: "none",
              border: "none",
              fontSize: 20,
              cursor: "pointer",
              color: "rgba(0,0,0,0.4)",
              lineHeight: 1,
              padding: 0,
            }}
          >
            ✕
          </button>
        </div>

        {/* Form body */}
        <div style={{ flex: 1, overflow: "auto", padding: "20px" }}>
          {/* Income / Expense toggle */}
          <div style={{ display: "flex", gap: 8, marginBottom: 20 }}>
            {(["expense", "income"] as const).map((type) => (
              <button
                key={type}
                onClick={() => {
                  setIsIncome(type === "income");
                  setCategoryId("");
                }}
                style={{
                  flex: 1,
                  padding: "8px 0",
                  borderRadius: 10,
                  border: "1px solid var(--hairline)",
                  background:
                    (type === "income") === isIncome
                      ? type === "income"
                        ? "#2BB94A"
                        : "#0866EF"
                      : "transparent",
                  color: (type === "income") === isIncome ? "white" : "rgba(0,0,0,0.5)",
                  fontWeight: 600,
                  fontSize: 13,
                  cursor: "pointer",
                  fontFamily: "inherit",
                }}
              >
                {type === "income" ? "Income" : "Expense"}
              </button>
            ))}
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div>
              <label style={labelStyle}>Amount</label>
              <input
                style={inputStyle}
                type="text"
                inputMode="decimal"
                placeholder="£0.00"
                value={amountRaw}
                onChange={(e) => setAmountRaw(e.target.value)}
              />
            </div>

            <div>
              <label style={labelStyle}>Description</label>
              <input
                style={inputStyle}
                type="text"
                placeholder="e.g. Tesco Express"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>

            <div>
              <label style={labelStyle}>Category</label>
              <select
                style={{ ...inputStyle, appearance: "none" }}
                value={categoryId}
                onChange={(e) =>
                  setCategoryId(e.target.value === "" ? "" : Number(e.target.value))
                }
              >
                <option value="">Uncategorised</option>
                {visibleCategories.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.emoji} {c.name}
                  </option>
                ))}
              </select>
            </div>

            <div>
              <label style={labelStyle}>Date</label>
              <input
                style={inputStyle}
                type="date"
                value={date}
                onChange={(e) => setDate(e.target.value)}
              />
            </div>

            <div>
              <label style={labelStyle}>Note (optional)</label>
              <input
                style={inputStyle}
                type="text"
                placeholder="Optional note"
                value={note}
                onChange={(e) => setNote(e.target.value)}
              />
            </div>
          </div>

          {error && (
            <div
              style={{
                marginTop: 16,
                padding: "10px 12px",
                background: "rgba(255,59,48,0.08)",
                border: "1px solid rgba(255,59,48,0.3)",
                borderRadius: 10,
                fontSize: 13,
                color: "var(--imessage-red)",
              }}
            >
              {error}
            </div>
          )}
        </div>

        {/* Footer */}
        <div
          style={{
            padding: "14px 20px",
            borderTop: "1px solid var(--hairline)",
          }}
        >
          <button
            onClick={handleSave}
            disabled={saving}
            style={{
              width: "100%",
              padding: "12px 0",
              background: "var(--imessage-blue)",
              color: "white",
              border: "none",
              borderRadius: 12,
              fontSize: 15,
              fontWeight: 700,
              cursor: saving ? "default" : "pointer",
              opacity: saving ? 0.6 : 1,
              fontFamily: "inherit",
            }}
          >
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </>
  );
}
```

- [ ] **Step 2: Type-check**

```bash
cd /Users/hanamori/life-assistant/apps/desktop && pnpm exec tsc --noEmit 2>&1
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
cd /Users/hanamori/life-assistant
git add apps/desktop/src/components/Ledger/AddTransactionForm.tsx
git commit -m "feat(ledger): AddTransactionForm drawer with income/expense toggle"
```

---

### Task 11: BudgetSheet drawer

**Files:**
- Modify: `apps/desktop/src/components/Ledger/BudgetSheet.tsx` (replace stub)

- [ ] **Step 1: Replace the stub with the real component**

Replace the entire content of `apps/desktop/src/components/Ledger/BudgetSheet.tsx`:

```tsx
import { useState } from "react";
import { deleteBudget, upsertBudget } from "../../lib/ledger/ipc";
import type { Budget, Category } from "../../lib/ledger/ipc";

interface Props {
  categories: Category[];
  budgets: Budget[];
  onClose: () => void;
  onChanged: () => Promise<void>;
}

function toPounds(pence: number): string {
  return (pence / 100).toFixed(0);
}

function parsePence(raw: string): number | null {
  const n = parseFloat(raw.replace(/[£,\s]/g, ""));
  if (isNaN(n) || n < 0) return null;
  return Math.round(n * 100);
}

export default function BudgetSheet({ categories, budgets, onClose, onChanged }: Props) {
  const budgetMap = new Map(budgets.map((b) => [b.category_id, b]));
  const [drafts, setDrafts] = useState<Record<number, string>>(() => {
    const init: Record<number, string> = {};
    budgets.forEach((b) => {
      init[b.category_id] = toPounds(b.amount_pence);
    });
    return init;
  });
  const [saving, setSaving] = useState(false);

  const expenseCategories = categories.filter((c) => !c.is_income);

  async function handleSave() {
    setSaving(true);
    try {
      for (const cat of expenseCategories) {
        const raw = drafts[cat.id] ?? "";
        const pence = raw.trim() === "" ? null : parsePence(raw);
        const existing = budgetMap.get(cat.id);

        if (pence === null || pence === 0) {
          // Clear budget if one existed
          if (existing) await deleteBudget(existing.id);
        } else {
          await upsertBudget({ categoryId: cat.id, amountPence: pence });
        }
      }
      await onChanged();
    } catch (e) {
      console.error("BudgetSheet save error:", e);
      setSaving(false);
    }
  }

  const inputStyle: React.CSSProperties = {
    width: 100,
    padding: "7px 10px",
    fontSize: 14,
    border: "1px solid var(--hairline)",
    borderRadius: 8,
    background: "#fafafa",
    fontFamily: "inherit",
    textAlign: "right",
  };

  return (
    <>
      <div
        onClick={onClose}
        style={{
          position: "fixed",
          inset: 0,
          background: "rgba(0,0,0,0.25)",
          zIndex: 700,
        }}
      />
      <div
        style={{
          position: "fixed",
          right: 0,
          top: 0,
          bottom: 0,
          width: 420,
          background: "var(--paper)",
          boxShadow: "-4px 0 24px rgba(0,0,0,0.12)",
          zIndex: 800,
          display: "flex",
          flexDirection: "column",
          animation: "drawerIn 200ms ease-out",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            padding: "18px 20px 14px",
            borderBottom: "1px solid var(--hairline)",
          }}
        >
          <div>
            <div style={{ fontSize: 16, fontWeight: 700 }}>Monthly Budgets</div>
            <div style={{ fontSize: 12, color: "rgba(0,0,0,0.4)", marginTop: 2 }}>
              Leave blank to skip tracking a category
            </div>
          </div>
          <button
            onClick={onClose}
            style={{
              background: "none",
              border: "none",
              fontSize: 20,
              cursor: "pointer",
              color: "rgba(0,0,0,0.4)",
              lineHeight: 1,
              padding: 0,
            }}
          >
            ✕
          </button>
        </div>

        {/* Category list */}
        <div style={{ flex: 1, overflow: "auto", padding: "12px 20px" }}>
          {expenseCategories.map((cat) => (
            <div
              key={cat.id}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "10px 0",
                borderBottom: "1px solid var(--hairline)",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <span style={{ fontSize: 18 }}>{cat.emoji}</span>
                <span style={{ fontSize: 14, fontWeight: 500 }}>{cat.name}</span>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span style={{ fontSize: 13, color: "rgba(0,0,0,0.4)" }}>£</span>
                <input
                  style={inputStyle}
                  type="number"
                  min="0"
                  placeholder="—"
                  value={drafts[cat.id] ?? ""}
                  onChange={(e) =>
                    setDrafts((d) => ({ ...d, [cat.id]: e.target.value }))
                  }
                />
                <span style={{ fontSize: 12, color: "rgba(0,0,0,0.35)" }}>/mo</span>
              </div>
            </div>
          ))}
        </div>

        {/* Footer */}
        <div
          style={{
            padding: "14px 20px",
            borderTop: "1px solid var(--hairline)",
          }}
        >
          <button
            onClick={handleSave}
            disabled={saving}
            style={{
              width: "100%",
              padding: "12px 0",
              background: "var(--imessage-blue)",
              color: "white",
              border: "none",
              borderRadius: 12,
              fontSize: 15,
              fontWeight: 700,
              cursor: saving ? "default" : "pointer",
              opacity: saving ? 0.6 : 1,
              fontFamily: "inherit",
            }}
          >
            {saving ? "Saving…" : "Save Budgets"}
          </button>
        </div>
      </div>
    </>
  );
}
```

- [ ] **Step 2: Type-check**

```bash
cd /Users/hanamori/life-assistant/apps/desktop && pnpm exec tsc --noEmit 2>&1
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
cd /Users/hanamori/life-assistant
git add apps/desktop/src/components/Ledger/BudgetSheet.tsx
git commit -m "feat(ledger): BudgetSheet drawer for managing monthly category limits"
```

---

### Task 12: /spent slash command

**Files:**
- Modify: `apps/desktop/src/lib/today/slash.ts`
- Modify: `apps/desktop/src/components/Assistant/Assistant.tsx`

The `/spent £12.50 coffee` command creates a manual transaction and confirms via toast.

- [ ] **Step 1: Add `SpentCommand` to `slash.ts`**

Replace the content of `apps/desktop/src/lib/today/slash.ts`:

```typescript
export type SlashCommand =
  | { type: "task"; title: string }
  | { type: "spent"; amountPence: number; description: string }
  | { type: "unknown"; raw: string };

/**
 * Parse a submitted message for slash-command syntax.
 *
 * /task <title>          — add a task
 * /spent <amount> <desc> — add a manual expense (e.g. /spent £12.50 coffee)
 */
export function parseSlash(input: string): SlashCommand | null {
  if (!input.startsWith("/")) return null;

  // /task <title>
  const taskMatch = input.match(/^\/task\s+(.+?)\s*$/);
  if (taskMatch) {
    const title = taskMatch[1].trim();
    if (!title) return null;
    return { type: "task", title };
  }
  if (/^\/task\s*$/.test(input)) return null;

  // /spent <amount> <description>
  // Accepts: /spent 12.50 coffee, /spent £12.50 coffee, /spent $8 lunch
  const spentMatch = input.match(/^\/spent\s+[£$€]?(\d+(?:\.\d{1,2})?)\s+(.+?)\s*$/);
  if (spentMatch) {
    const pence = Math.round(parseFloat(spentMatch[1]) * 100);
    const description = spentMatch[2].trim();
    if (pence === 0 || !description) return null;
    return { type: "spent", amountPence: -pence, description }; // negative = expense
  }
  if (/^\/spent\s*$/.test(input)) return null;

  return { type: "unknown", raw: input };
}
```

- [ ] **Step 2: Handle the `spent` command in `Assistant.tsx`**

In `apps/desktop/src/components/Assistant/Assistant.tsx`, add the ledger import at the top:

```typescript
import { addTransaction, listCategories } from "../../lib/ledger/ipc";
```

In `handleSubmit`, after the `slash?.type === "task"` block (around line 72), add:

```typescript
    if (slash?.type === "spent") {
      try {
        const now = new Date();
        now.setHours(0, 0, 0, 0);
        await addTransaction({
          amountPence: slash.amountPence,
          currency: "GBP",
          description: slash.description,
          date: Math.floor(now.getTime() / 1000),
        });
        enqueueBubble({
          id: newBubbleId(),
          kind: "assistant",
          content: `Added: ${slash.description} (${(Math.abs(slash.amountPence) / 100).toFixed(2)})`,
          messageId: null,
          ttlMs: 6000,
        });
        return;
      } catch (e) {
        setAvatarState("confused");
        enqueueBubble({
          id: newBubbleId(),
          kind: "error",
          content: `Couldn't add transaction: ${String(e)}`,
          messageId: null,
          ttlMs: 7000,
        });
        return;
      }
    }
```

- [ ] **Step 3: Type-check**

```bash
cd /Users/hanamori/life-assistant/apps/desktop && pnpm exec tsc --noEmit 2>&1
```

Expected: no errors.

- [ ] **Step 4: Run all Rust tests to confirm nothing regressed**

```bash
cd /Users/hanamori/life-assistant && cargo test 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/hanamori/life-assistant
git add apps/desktop/src/lib/today/slash.ts \
        apps/desktop/src/components/Assistant/Assistant.tsx
git commit -m "feat(ledger): /spent slash command adds manual expense via pill"
```

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Task |
|---|---|
| DB schema — category, bank_account stub, ledger_transaction, budget | Task 1 |
| Default category seeds (10 rows) | Task 1 |
| Category DAL — list, insert, update, delete (cannot delete default) | Task 2 |
| Transaction DAL — insert, update, delete, list_by_month | Task 3 |
| Month boundary helpers (`month_start_ts`, `month_end_ts`) | Task 3 |
| Budget DAL — list, upsert, delete, monthly_summary | Task 4 |
| Budget upsert restores soft-deleted row | Task 4 |
| Tauri commands for all DAL operations | Task 5 |
| Frontend IPC typed wrappers | Task 6 |
| Zustand store (categories, transactions, budgets, summary) | Task 6 |
| `"ledger"` view in nav + 💰 icon in sidebar | Task 7 |
| LedgerView loads data + renders cards | Task 8 |
| SummaryCard — dark gradient, budget progress bar, health badges | Task 8 |
| Budget badge colours: green <75%, amber 75–99%, red ≥100% | Task 8 |
| Gradient shifts: normal → amber → red based on overspend | Task 8 |
| TransactionFeed — day-grouped rows, empty state | Task 9 |
| TransactionRow — emoji icon, merchant, category, amount in green for income | Task 9 |
| AddTransactionForm — income/expense toggle, amount, description, category, date, note | Task 10 |
| BudgetSheet — per-category monthly limit editor, blank = no tracking | Task 11 |
| `/spent £x description` slash command | Task 12 |

All spec requirements covered. No placeholders. Type names are consistent across all tasks.

**Out of scope for this plan (Phase 5b):** GoCardless + Plaid OAuth, sync engine, token monitor, Settings bank accounts section, avatar budget proposal nudges.
