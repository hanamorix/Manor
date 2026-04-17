# Phase 5c Ledger Completions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the v0.3 Ledger — recurring payments with auto-insertion on app open, contracts with renewal alerts surfaced in Today, CSV import from named bank presets, and a persistent month-in-review panel with on-demand Ollama narrative.

**Architecture:** Two new DAL modules (`recurring`, `contract`) under `crates/core/src/ledger/`. A CSV import module under `crates/app/src/ledger/csv_import.rs` with hard-coded bank preset schemas. Auto-insert + renewal scan run in `lib.rs` `setup()` as a single `spawn_blocking`, after the existing calendar-sync spawn. AI month review reuses the existing `OllamaClient` + `StreamChunk` channel pattern from the assistant. Today view gains a new `RenewalAlertsCard` component. Ledger view gains `RecurringSection`, `ContractsSection`, `MonthReviewPanel`, and a CSV import drawer.

**Tech Stack:** Rust (anyhow, chrono, rusqlite, csv, reqwest, tokio), Refinery migrations, Tauri 2 IPC + Channel streaming, React 18, TypeScript, Zustand, Ollama.

---

## File Map

| Path | Status | What it does |
|---|---|---|
| `crates/core/migrations/V12__ledger_completions.sql` | Create | Schema — `recurring_payment` + `contract` + `ledger_transaction.recurring_payment_id` |
| `crates/core/src/ledger/recurring.rs` | Create | `RecurringPayment` DAL + `auto_insert_due` |
| `crates/core/src/ledger/contract.rs` | Create | `Contract` DAL + `check_renewals` |
| `crates/core/src/ledger/transaction.rs` | Modify | Add `recurring_payment_id` field; `insert_recurring` helper |
| `crates/core/src/ledger/mod.rs` | Modify | `pub mod recurring; pub mod contract;` |
| `crates/app/src/ledger/csv_import.rs` | Create | Bank preset definitions, CSV parser, dup check, keyword categorizer |
| `crates/app/src/ledger/ai_review.rs` | Create | Prompt builder + streaming runner for month-in-review |
| `crates/app/src/ledger/mod.rs` | Modify | `pub mod csv_import; pub mod ai_review;` |
| `crates/app/src/ledger/commands.rs` | Modify | Add 11 new Tauri commands |
| `crates/app/src/lib.rs` | Modify | Register commands, schedule auto-insert + renewal scan on setup |
| `apps/desktop/src/lib/ledger/ipc.ts` | Modify | Types + IPC wrappers for recurring / contracts / CSV / review |
| `apps/desktop/src/lib/ledger/state.ts` | Modify | Add `recurring`, `contracts`, `renewalAlerts` slices |
| `apps/desktop/src/lib/today/ipc.ts` | Modify | Add `getRenewalAlerts` + `RenewalAlert` type |
| `apps/desktop/src/lib/today/state.ts` | Modify | Add `renewalAlerts` slice |
| `apps/desktop/src/components/Ledger/RecurringSection.tsx` | Create | Collapsible list, pause/resume, add |
| `apps/desktop/src/components/Ledger/AddRecurringDrawer.tsx` | Create | Drawer form for recurring payment |
| `apps/desktop/src/components/Ledger/ContractsSection.tsx` | Create | Collapsible list, countdown pills |
| `apps/desktop/src/components/Ledger/AddContractDrawer.tsx` | Create | Drawer form for contract |
| `apps/desktop/src/components/Ledger/CsvImportDrawer.tsx` | Create | Preset picker, file, preview, confirm |
| `apps/desktop/src/components/Ledger/MonthReviewPanel.tsx` | Create | Persistent summary + Review-with-AI narrative |
| `apps/desktop/src/components/Ledger/LedgerView.tsx` | Modify | Mount new sections, import button, review panel |
| `apps/desktop/src/components/Today/RenewalAlertsCard.tsx` | Create | Contract renewals surfaced above EventsCard |
| `apps/desktop/src/components/Today/Today.tsx` | Modify | Mount RenewalAlertsCard |

---

## Task 1: V12 Schema Migration

**Files:**
- Create: `crates/core/migrations/V12__ledger_completions.sql`
- Test: existing migration runner — `crates/core/src/assistant/db.rs`

- [ ] **Step 1: Write the migration**

```sql
-- V12__ledger_completions.sql
-- Recurring payment templates — auto-insert logic runs on app open.
CREATE TABLE recurring_payment (
    id           INTEGER PRIMARY KEY,
    description  TEXT    NOT NULL,
    amount_pence INTEGER NOT NULL,
    currency     TEXT    NOT NULL DEFAULT 'GBP',
    category_id  INTEGER REFERENCES category(id),
    day_of_month INTEGER NOT NULL CHECK (day_of_month BETWEEN 1 AND 28),
    active       INTEGER NOT NULL DEFAULT 1,
    note         TEXT,
    created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at   INTEGER
);

-- Trace which recurring template produced a transaction.
ALTER TABLE ledger_transaction ADD COLUMN recurring_payment_id INTEGER REFERENCES recurring_payment(id);

CREATE INDEX idx_ledger_transaction_recurring ON ledger_transaction(recurring_payment_id);

-- Contracts with renewal alerts.
CREATE TABLE contract (
    id                   INTEGER PRIMARY KEY,
    provider             TEXT    NOT NULL,
    kind                 TEXT    NOT NULL DEFAULT 'other',
    description          TEXT,
    monthly_cost_pence   INTEGER NOT NULL,
    term_start           INTEGER NOT NULL,
    term_end             INTEGER NOT NULL,
    exit_fee_pence       INTEGER,
    renewal_alert_days   INTEGER NOT NULL DEFAULT 30,
    recurring_payment_id INTEGER REFERENCES recurring_payment(id),
    note                 TEXT,
    created_at           INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at           INTEGER
);

CREATE INDEX idx_contract_term_end ON contract(term_end) WHERE deleted_at IS NULL;
```

- [ ] **Step 2: Verify migration runs**

Run: `cd /Users/hanamori/life-assistant && cargo test -p manor-core --lib ledger 2>&1 | tail -20`
Expected: existing ledger tests still pass; no migration errors.

- [ ] **Step 3: Commit**

```bash
git add crates/core/migrations/V12__ledger_completions.sql
git commit -m "feat(core): V12 migration — recurring_payment, contract, recurring_payment_id"
```

---

## Task 2: RecurringPayment struct + basic DAL

**Files:**
- Create: `crates/core/src/ledger/recurring.rs`
- Modify: `crates/core/src/ledger/mod.rs:1-6`

- [ ] **Step 1: Write failing tests first**

Create `crates/core/src/ledger/recurring.rs`:

```rust
//! RecurringPayment DAL — templates that auto-insert transactions monthly.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecurringPayment {
    pub id: i64,
    pub description: String,
    pub amount_pence: i64,
    pub currency: String,
    pub category_id: Option<i64>,
    pub day_of_month: i64,
    pub active: bool,
    pub note: Option<String>,
    pub created_at: i64,
}

impl RecurringPayment {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let active: i64 = row.get("active")?;
        Ok(Self {
            id: row.get("id")?,
            description: row.get("description")?,
            amount_pence: row.get("amount_pence")?,
            currency: row.get("currency")?,
            category_id: row.get("category_id")?,
            day_of_month: row.get("day_of_month")?,
            active: active != 0,
            note: row.get("note")?,
            created_at: row.get("created_at")?,
        })
    }
}

pub fn insert(
    conn: &Connection,
    description: &str,
    amount_pence: i64,
    currency: &str,
    category_id: Option<i64>,
    day_of_month: i64,
    note: Option<&str>,
) -> Result<RecurringPayment> {
    anyhow::ensure!(
        (1..=28).contains(&day_of_month),
        "day_of_month must be 1..=28, got {day_of_month}"
    );
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO recurring_payment
         (description, amount_pence, currency, category_id, day_of_month, active, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)",
        params![description, amount_pence, currency, category_id, day_of_month, note, now],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn list(conn: &Connection) -> Result<Vec<RecurringPayment>> {
    let mut stmt = conn.prepare(
        "SELECT id, description, amount_pence, currency, category_id, day_of_month,
                active, note, created_at
         FROM recurring_payment
         WHERE deleted_at IS NULL
         ORDER BY day_of_month ASC, id ASC",
    )?;
    Ok(stmt
        .query_map([], RecurringPayment::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn update(
    conn: &Connection,
    id: i64,
    description: &str,
    amount_pence: i64,
    category_id: Option<i64>,
    day_of_month: i64,
    active: bool,
    note: Option<&str>,
) -> Result<RecurringPayment> {
    anyhow::ensure!(
        (1..=28).contains(&day_of_month),
        "day_of_month must be 1..=28, got {day_of_month}"
    );
    let rows = conn.execute(
        "UPDATE recurring_payment
         SET description = ?1, amount_pence = ?2, category_id = ?3,
             day_of_month = ?4, active = ?5, note = ?6
         WHERE id = ?7 AND deleted_at IS NULL",
        params![description, amount_pence, category_id, day_of_month, active as i64, note, id],
    )?;
    anyhow::ensure!(rows > 0, "recurring_payment id={id} not found");
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE recurring_payment SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

fn get(conn: &Connection, id: i64) -> Result<RecurringPayment> {
    let mut stmt = conn.prepare(
        "SELECT id, description, amount_pence, currency, category_id, day_of_month,
                active, note, created_at
         FROM recurring_payment WHERE id = ?1",
    )?;
    Ok(stmt.query_row([id], RecurringPayment::from_row)?)
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
    fn insert_creates_row() {
        let (_d, conn) = fresh_conn();
        let r = insert(&conn, "Netflix", 1299, "GBP", Some(5), 15, None).unwrap();
        assert_eq!(r.description, "Netflix");
        assert_eq!(r.amount_pence, 1299);
        assert_eq!(r.day_of_month, 15);
        assert!(r.active);
    }

    #[test]
    fn insert_rejects_invalid_day_of_month() {
        let (_d, conn) = fresh_conn();
        assert!(insert(&conn, "Bad", 100, "GBP", None, 29, None).is_err());
        assert!(insert(&conn, "Bad", 100, "GBP", None, 0, None).is_err());
    }

    #[test]
    fn list_orders_by_day_of_month() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "Rent", 100000, "GBP", None, 1, None).unwrap();
        insert(&conn, "Netflix", 1299, "GBP", None, 15, None).unwrap();
        insert(&conn, "Gym", 3000, "GBP", None, 7, None).unwrap();
        let rows = list(&conn).unwrap();
        let days: Vec<_> = rows.iter().map(|r| r.day_of_month).collect();
        assert_eq!(days, vec![1, 7, 15]);
    }

    #[test]
    fn update_changes_fields() {
        let (_d, conn) = fresh_conn();
        let r = insert(&conn, "Old", 500, "GBP", None, 10, None).unwrap();
        let u = update(&conn, r.id, "New", 800, Some(2), 20, false, Some("paused")).unwrap();
        assert_eq!(u.description, "New");
        assert_eq!(u.amount_pence, 800);
        assert!(!u.active);
        assert_eq!(u.note, Some("paused".to_string()));
    }

    #[test]
    fn delete_soft_deletes() {
        let (_d, conn) = fresh_conn();
        let r = insert(&conn, "Gone", 100, "GBP", None, 1, None).unwrap();
        delete(&conn, r.id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }
}
```

- [ ] **Step 2: Wire module in**

Edit `crates/core/src/ledger/mod.rs`:

```rust
//! Ledger subsystem: categories, transactions, budgets, recurring payments, contracts.

pub mod budget;
pub mod category;
pub mod contract;
pub mod recurring;
pub mod transaction;
```

(Task 4 adds the `contract` module; keep the line now so both modules land together in mod.rs.)

- [ ] **Step 3: Run tests**

Run: `cd /Users/hanamori/life-assistant && cargo test -p manor-core ledger::recurring -- --nocapture 2>&1 | tail -20`
Expected: 5 tests pass. `contract` module will fail to compile until Task 4 — temporarily remove `pub mod contract;` if you want to test task-by-task; re-add in Task 4.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/ledger/recurring.rs crates/core/src/ledger/mod.rs
git commit -m "feat(ledger): RecurringPayment DAL with CRUD + day_of_month validation"
```

---

## Task 3: `auto_insert_due` — the core monthly insertion

**Files:**
- Modify: `crates/core/src/ledger/recurring.rs`
- Modify: `crates/core/src/ledger/transaction.rs` (add `insert_recurring` helper)

- [ ] **Step 1: Add `recurring_payment_id` to Transaction struct**

Edit `crates/core/src/ledger/transaction.rs`. Add field to struct (after `source`):

```rust
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
    pub recurring_payment_id: Option<i64>,
    pub created_at: i64,
}
```

Update `from_row`:

```rust
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
            recurring_payment_id: row.get("recurring_payment_id")?,
            created_at: row.get("created_at")?,
        })
    }
```

Update all four SELECT statements in this file to include `recurring_payment_id` (in `list_by_month`, `get`, and anywhere else that selects columns explicitly). Run `grep -n 'SELECT id, bank_account_id' crates/core/src/ledger/transaction.rs` — update every hit.

Add helper below `insert`:

```rust
/// Insert a transaction generated by a recurring_payment template.
pub fn insert_recurring(
    conn: &Connection,
    recurring_payment_id: i64,
    amount_pence: i64,
    currency: &str,
    description: &str,
    category_id: Option<i64>,
    date: i64,
) -> Result<Transaction> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO ledger_transaction
         (bank_account_id, amount_pence, currency, description, merchant,
          category_id, date, source, note, recurring_payment_id, created_at)
         VALUES (NULL, ?1, ?2, ?3, NULL, ?4, ?5, 'recurring', NULL, ?6, ?7)",
        params![amount_pence, currency, description, category_id, date, recurring_payment_id, now],
    )?;
    get(conn, conn.last_insert_rowid())
}
```

- [ ] **Step 2: Write failing test for `auto_insert_due`**

Append to `crates/core/src/ledger/recurring.rs` (above the existing `#[cfg(test)]` block, add public fn; then extend tests):

```rust
use chrono::{DateTime, Datelike, TimeZone};

/// Run auto-insert for all active recurring payments. For each payment whose
/// `day_of_month <= today.day` and which has no transaction in the current
/// calendar month yet, insert one. All-or-nothing transaction.
///
/// Returns the number of transactions inserted.
pub fn auto_insert_due(conn: &mut Connection, now: DateTime<Utc>) -> Result<usize> {
    let tx = conn.transaction()?;
    let year = now.year();
    let month = now.month();
    let today_dom = now.day() as i64;
    let month_start = crate::ledger::transaction::month_start_ts(year, month);
    let month_end = crate::ledger::transaction::month_end_ts(year, month);
    let today_midnight = Utc
        .with_ymd_and_hms(year, month, now.day(), 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid ymd"))?
        .timestamp();

    let due: Vec<(i64, String, i64, String, Option<i64>)> = {
        let mut stmt = tx.prepare(
            "SELECT r.id, r.description, r.amount_pence, r.currency, r.category_id
             FROM recurring_payment r
             WHERE r.deleted_at IS NULL
               AND r.active = 1
               AND r.day_of_month <= ?1
               AND NOT EXISTS (
                   SELECT 1 FROM ledger_transaction t
                   WHERE t.recurring_payment_id = r.id
                     AND t.deleted_at IS NULL
                     AND t.date >= ?2 AND t.date < ?3
               )",
        )?;
        stmt.query_map(
            params![today_dom, month_start, month_end],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let inserted = due.len();
    for (id, description, amount_pence, currency, category_id) in due {
        // Debit: store as negative pence.
        let signed = -amount_pence.abs();
        tx.execute(
            "INSERT INTO ledger_transaction
             (bank_account_id, amount_pence, currency, description, merchant,
              category_id, date, source, note, recurring_payment_id, created_at)
             VALUES (NULL, ?1, ?2, ?3, NULL, ?4, ?5, 'recurring', NULL, ?6, ?7)",
            params![
                signed,
                currency,
                description,
                category_id,
                today_midnight,
                id,
                Utc::now().timestamp()
            ],
        )?;
    }
    tx.commit()?;
    Ok(inserted)
}
```

Add tests at the bottom of the `tests` module:

```rust
    use crate::ledger::transaction;

    fn utc_day(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 12, 0, 0).unwrap()
    }

    #[test]
    fn auto_insert_inserts_when_day_reached_and_no_existing() {
        let (_d, mut conn) = fresh_conn();
        insert(&conn, "Rent", 100000, "GBP", None, 1, None).unwrap();
        let count = auto_insert_due(&mut conn, utc_day(2026, 4, 16)).unwrap();
        assert_eq!(count, 1);
        let txns = transaction::list_by_month(&conn, 2026, 4).unwrap();
        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].source, "recurring");
        assert_eq!(txns[0].amount_pence, -100000);
        assert!(txns[0].recurring_payment_id.is_some());
    }

    #[test]
    fn auto_insert_skips_when_day_not_reached() {
        let (_d, mut conn) = fresh_conn();
        insert(&conn, "Rent", 100000, "GBP", None, 20, None).unwrap();
        let count = auto_insert_due(&mut conn, utc_day(2026, 4, 10)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn auto_insert_is_idempotent_within_month() {
        let (_d, mut conn) = fresh_conn();
        insert(&conn, "Rent", 100000, "GBP", None, 1, None).unwrap();
        auto_insert_due(&mut conn, utc_day(2026, 4, 16)).unwrap();
        let count = auto_insert_due(&mut conn, utc_day(2026, 4, 20)).unwrap();
        assert_eq!(count, 0);
        let txns = transaction::list_by_month(&conn, 2026, 4).unwrap();
        assert_eq!(txns.len(), 1);
    }

    #[test]
    fn auto_insert_re_runs_next_month() {
        let (_d, mut conn) = fresh_conn();
        insert(&conn, "Rent", 100000, "GBP", None, 1, None).unwrap();
        auto_insert_due(&mut conn, utc_day(2026, 4, 16)).unwrap();
        let count = auto_insert_due(&mut conn, utc_day(2026, 5, 2)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn auto_insert_skips_inactive() {
        let (_d, mut conn) = fresh_conn();
        let r = insert(&conn, "Paused", 500, "GBP", None, 1, None).unwrap();
        update(&conn, r.id, "Paused", 500, None, 1, false, None).unwrap();
        let count = auto_insert_due(&mut conn, utc_day(2026, 4, 16)).unwrap();
        assert_eq!(count, 0);
    }
```

- [ ] **Step 3: Run**

Run: `cd /Users/hanamori/life-assistant && cargo test -p manor-core ledger::recurring 2>&1 | tail -20`
Expected: all recurring tests pass (10 total). Run `cargo test -p manor-core ledger::transaction` too — expected: still green after adding the new column.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/ledger/recurring.rs crates/core/src/ledger/transaction.rs
git commit -m "feat(ledger): auto_insert_due for recurring payments + Transaction.recurring_payment_id"
```

---

## Task 4: Contract struct + DAL

**Files:**
- Create: `crates/core/src/ledger/contract.rs`

- [ ] **Step 1: Write module with tests**

```rust
//! Contract DAL — service contracts with renewal alerts.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Contract {
    pub id: i64,
    pub provider: String,
    pub kind: String,
    pub description: Option<String>,
    pub monthly_cost_pence: i64,
    pub term_start: i64,
    pub term_end: i64,
    pub exit_fee_pence: Option<i64>,
    pub renewal_alert_days: i64,
    pub recurring_payment_id: Option<i64>,
    pub note: Option<String>,
    pub created_at: i64,
}

impl Contract {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            provider: row.get("provider")?,
            kind: row.get("kind")?,
            description: row.get("description")?,
            monthly_cost_pence: row.get("monthly_cost_pence")?,
            term_start: row.get("term_start")?,
            term_end: row.get("term_end")?,
            exit_fee_pence: row.get("exit_fee_pence")?,
            renewal_alert_days: row.get("renewal_alert_days")?,
            recurring_payment_id: row.get("recurring_payment_id")?,
            note: row.get("note")?,
            created_at: row.get("created_at")?,
        })
    }
}

pub struct NewContract<'a> {
    pub provider: &'a str,
    pub kind: &'a str,
    pub description: Option<&'a str>,
    pub monthly_cost_pence: i64,
    pub term_start: i64,
    pub term_end: i64,
    pub exit_fee_pence: Option<i64>,
    pub renewal_alert_days: i64,
    pub recurring_payment_id: Option<i64>,
    pub note: Option<&'a str>,
}

pub fn insert(conn: &Connection, new: NewContract<'_>) -> Result<Contract> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO contract
         (provider, kind, description, monthly_cost_pence, term_start, term_end,
          exit_fee_pence, renewal_alert_days, recurring_payment_id, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            new.provider, new.kind, new.description, new.monthly_cost_pence,
            new.term_start, new.term_end, new.exit_fee_pence,
            new.renewal_alert_days, new.recurring_payment_id, new.note, now
        ],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn list(conn: &Connection) -> Result<Vec<Contract>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider, kind, description, monthly_cost_pence, term_start,
                term_end, exit_fee_pence, renewal_alert_days, recurring_payment_id,
                note, created_at
         FROM contract
         WHERE deleted_at IS NULL
         ORDER BY term_end ASC",
    )?;
    Ok(stmt.query_map([], Contract::from_row)?.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn update(conn: &Connection, id: i64, new: NewContract<'_>) -> Result<Contract> {
    let rows = conn.execute(
        "UPDATE contract
         SET provider = ?1, kind = ?2, description = ?3, monthly_cost_pence = ?4,
             term_start = ?5, term_end = ?6, exit_fee_pence = ?7,
             renewal_alert_days = ?8, recurring_payment_id = ?9, note = ?10
         WHERE id = ?11 AND deleted_at IS NULL",
        params![
            new.provider, new.kind, new.description, new.monthly_cost_pence,
            new.term_start, new.term_end, new.exit_fee_pence,
            new.renewal_alert_days, new.recurring_payment_id, new.note, id
        ],
    )?;
    anyhow::ensure!(rows > 0, "contract id={id} not found");
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE contract SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

fn get(conn: &Connection, id: i64) -> Result<Contract> {
    let mut stmt = conn.prepare(
        "SELECT id, provider, kind, description, monthly_cost_pence, term_start,
                term_end, exit_fee_pence, renewal_alert_days, recurring_payment_id,
                note, created_at
         FROM contract WHERE id = ?1",
    )?;
    Ok(stmt.query_row([id], Contract::from_row)?)
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

    fn ts(year: i32, month: u32, day: u32) -> i64 {
        Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap().timestamp()
    }

    fn sample(provider: &'static str, term_end: i64) -> NewContract<'static> {
        NewContract {
            provider,
            kind: "phone",
            description: None,
            monthly_cost_pence: 2500,
            term_start: ts(2025, 1, 1),
            term_end,
            exit_fee_pence: None,
            renewal_alert_days: 30,
            recurring_payment_id: None,
            note: None,
        }
    }

    #[test]
    fn insert_and_list() {
        let (_d, conn) = fresh_conn();
        insert(&conn, sample("O2", ts(2027, 1, 1))).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn list_orders_by_term_end() {
        let (_d, conn) = fresh_conn();
        insert(&conn, sample("Later", ts(2027, 6, 1))).unwrap();
        insert(&conn, sample("Sooner", ts(2026, 6, 1))).unwrap();
        let rows = list(&conn).unwrap();
        assert_eq!(rows[0].provider, "Sooner");
        assert_eq!(rows[1].provider, "Later");
    }

    #[test]
    fn update_changes_fields() {
        let (_d, conn) = fresh_conn();
        let c = insert(&conn, sample("O2", ts(2027, 1, 1))).unwrap();
        let changed = update(
            &conn, c.id,
            NewContract { provider: "EE", kind: "phone", description: None,
                monthly_cost_pence: 3000, term_start: c.term_start,
                term_end: ts(2028, 1, 1), exit_fee_pence: Some(5000),
                renewal_alert_days: 60, recurring_payment_id: None, note: None },
        ).unwrap();
        assert_eq!(changed.provider, "EE");
        assert_eq!(changed.monthly_cost_pence, 3000);
        assert_eq!(changed.exit_fee_pence, Some(5000));
    }

    #[test]
    fn delete_soft_deletes() {
        let (_d, conn) = fresh_conn();
        let c = insert(&conn, sample("O2", ts(2027, 1, 1))).unwrap();
        delete(&conn, c.id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd /Users/hanamori/life-assistant && cargo test -p manor-core ledger::contract 2>&1 | tail -20`
Expected: 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/ledger/contract.rs
git commit -m "feat(ledger): Contract DAL with term tracking + renewal_alert_days"
```

---

## Task 5: `check_renewals` — renewal alert query

**Files:**
- Modify: `crates/core/src/ledger/contract.rs`

- [ ] **Step 1: Add `RenewalAlert` struct + `check_renewals` fn**

Append below `delete`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenewalAlert {
    pub contract_id: i64,
    pub provider: String,
    pub kind: String,
    pub term_end: i64,
    pub days_remaining: i64,
    pub exit_fee_pence: Option<i64>,
    pub severity: String, // "amber" or "red"
}

/// Return contracts whose `term_end` is within their `renewal_alert_days` window.
/// `severity = "red"` if ≤7 days remaining, else `"amber"`.
pub fn check_renewals(conn: &Connection, now_ts: i64) -> Result<Vec<RenewalAlert>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider, kind, term_end, renewal_alert_days, exit_fee_pence
         FROM contract
         WHERE deleted_at IS NULL
           AND term_end - ?1 <= renewal_alert_days * 86400
           AND term_end > ?1
         ORDER BY term_end ASC",
    )?;
    let rows = stmt
        .query_map(params![now_ts], |row| {
            let id: i64 = row.get(0)?;
            let provider: String = row.get(1)?;
            let kind: String = row.get(2)?;
            let term_end: i64 = row.get(3)?;
            let _alert_days: i64 = row.get(4)?;
            let exit_fee: Option<i64> = row.get(5)?;
            let days_remaining = ((term_end - now_ts).max(0)) / 86400;
            let severity = if days_remaining <= 7 { "red" } else { "amber" };
            Ok(RenewalAlert {
                contract_id: id,
                provider,
                kind,
                term_end,
                days_remaining,
                exit_fee_pence: exit_fee,
                severity: severity.to_string(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
```

- [ ] **Step 2: Add tests**

Append to `tests` module:

```rust
    fn within(new_term_end: i64, alert_days: i64) -> NewContract<'static> {
        NewContract {
            provider: "O2", kind: "phone", description: None,
            monthly_cost_pence: 2500, term_start: ts(2025, 1, 1),
            term_end: new_term_end, exit_fee_pence: None,
            renewal_alert_days: alert_days,
            recurring_payment_id: None, note: None,
        }
    }

    #[test]
    fn check_renewals_includes_contracts_inside_alert_window() {
        let (_d, conn) = fresh_conn();
        let now = ts(2026, 4, 16);
        insert(&conn, within(now + 20 * 86400, 30)).unwrap(); // 20 days away, 30-day window
        insert(&conn, within(now + 90 * 86400, 30)).unwrap(); // 90 days away — out of window
        let alerts = check_renewals(&conn, now).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].days_remaining, 20);
        assert_eq!(alerts[0].severity, "amber");
    }

    #[test]
    fn check_renewals_marks_red_under_seven_days() {
        let (_d, conn) = fresh_conn();
        let now = ts(2026, 4, 16);
        insert(&conn, within(now + 3 * 86400, 30)).unwrap();
        let alerts = check_renewals(&conn, now).unwrap();
        assert_eq!(alerts[0].severity, "red");
    }

    #[test]
    fn check_renewals_excludes_already_expired() {
        let (_d, conn) = fresh_conn();
        let now = ts(2026, 4, 16);
        insert(&conn, within(now - 5 * 86400, 30)).unwrap();
        assert!(check_renewals(&conn, now).unwrap().is_empty());
    }

    #[test]
    fn check_renewals_excludes_deleted() {
        let (_d, conn) = fresh_conn();
        let now = ts(2026, 4, 16);
        let c = insert(&conn, within(now + 5 * 86400, 30)).unwrap();
        delete(&conn, c.id).unwrap();
        assert!(check_renewals(&conn, now).unwrap().is_empty());
    }
```

- [ ] **Step 3: Run**

Run: `cd /Users/hanamori/life-assistant && cargo test -p manor-core ledger::contract 2>&1 | tail -15`
Expected: 8 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/ledger/contract.rs
git commit -m "feat(ledger): check_renewals — amber/red severity by days_remaining"
```

---

## Task 6: Tauri commands — recurring

**Files:**
- Modify: `crates/app/src/ledger/commands.rs`

- [ ] **Step 1: Add command block**

Append to `crates/app/src/ledger/commands.rs` (after the existing budgets section):

```rust
// ── Recurring payments ────────────────────────────────────────────────────────

use manor_core::ledger::{contract, recurring};

#[tauri::command]
pub fn ledger_list_recurring(state: State<'_, Db>) -> Result<Vec<recurring::RecurringPayment>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    recurring::list(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct AddRecurringArgs {
    pub description: String,
    #[serde(rename = "amountPence")]
    pub amount_pence: i64,
    pub currency: String,
    #[serde(rename = "categoryId")]
    pub category_id: Option<i64>,
    #[serde(rename = "dayOfMonth")]
    pub day_of_month: i64,
    pub note: Option<String>,
}

#[tauri::command]
pub fn ledger_add_recurring(
    state: State<'_, Db>,
    args: AddRecurringArgs,
) -> Result<recurring::RecurringPayment, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    recurring::insert(
        &conn,
        &args.description,
        args.amount_pence,
        &args.currency,
        args.category_id,
        args.day_of_month,
        args.note.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct UpdateRecurringArgs {
    pub id: i64,
    pub description: String,
    #[serde(rename = "amountPence")]
    pub amount_pence: i64,
    #[serde(rename = "categoryId")]
    pub category_id: Option<i64>,
    #[serde(rename = "dayOfMonth")]
    pub day_of_month: i64,
    pub active: bool,
    pub note: Option<String>,
}

#[tauri::command]
pub fn ledger_update_recurring(
    state: State<'_, Db>,
    args: UpdateRecurringArgs,
) -> Result<recurring::RecurringPayment, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    recurring::update(
        &conn, args.id, &args.description, args.amount_pence,
        args.category_id, args.day_of_month, args.active, args.note.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_delete_recurring(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    recurring::delete(&conn, id).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Build**

Run: `cd /Users/hanamori/life-assistant && cargo check -p manor-app 2>&1 | tail -20`
Expected: no errors. (Commands aren't registered yet — that's Task 9.)

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/ledger/commands.rs
git commit -m "feat(app): 4 recurring-payment Tauri commands"
```

---

## Task 7: Tauri commands — contracts + renewal alerts

**Files:**
- Modify: `crates/app/src/ledger/commands.rs`

- [ ] **Step 1: Add commands**

Append:

```rust
// ── Contracts ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn ledger_list_contracts(state: State<'_, Db>) -> Result<Vec<contract::Contract>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    contract::list(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct ContractArgs {
    pub provider: String,
    pub kind: String,
    pub description: Option<String>,
    #[serde(rename = "monthlyCostPence")]
    pub monthly_cost_pence: i64,
    #[serde(rename = "termStart")]
    pub term_start: i64,
    #[serde(rename = "termEnd")]
    pub term_end: i64,
    #[serde(rename = "exitFeePence")]
    pub exit_fee_pence: Option<i64>,
    #[serde(rename = "renewalAlertDays")]
    pub renewal_alert_days: i64,
    #[serde(rename = "recurringPaymentId")]
    pub recurring_payment_id: Option<i64>,
    pub note: Option<String>,
}

impl<'a> ContractArgs {
    fn as_new(&'a self) -> contract::NewContract<'a> {
        contract::NewContract {
            provider: &self.provider,
            kind: &self.kind,
            description: self.description.as_deref(),
            monthly_cost_pence: self.monthly_cost_pence,
            term_start: self.term_start,
            term_end: self.term_end,
            exit_fee_pence: self.exit_fee_pence,
            renewal_alert_days: self.renewal_alert_days,
            recurring_payment_id: self.recurring_payment_id,
            note: self.note.as_deref(),
        }
    }
}

#[tauri::command]
pub fn ledger_add_contract(
    state: State<'_, Db>,
    args: ContractArgs,
) -> Result<contract::Contract, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    contract::insert(&conn, args.as_new()).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct UpdateContractArgs {
    pub id: i64,
    #[serde(flatten)]
    pub fields: ContractArgs,
}

#[tauri::command]
pub fn ledger_update_contract(
    state: State<'_, Db>,
    args: UpdateContractArgs,
) -> Result<contract::Contract, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    contract::update(&conn, args.id, args.fields.as_new()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_delete_contract(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    contract::delete(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_get_renewal_alerts(
    state: State<'_, Db>,
) -> Result<Vec<contract::RenewalAlert>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    contract::check_renewals(&conn, now).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Build**

Run: `cd /Users/hanamori/life-assistant && cargo check -p manor-app 2>&1 | tail -20`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/ledger/commands.rs
git commit -m "feat(app): 5 contract + renewal-alert Tauri commands"
```

---

## Task 8: CSV import module (parser + presets + dup check)

**Files:**
- Create: `crates/app/src/ledger/csv_import.rs`
- Modify: `crates/app/src/ledger/mod.rs`
- Modify: `crates/app/Cargo.toml` (add `csv` crate if missing)

- [ ] **Step 1: Verify `csv` crate is in Cargo.toml**

Run: `cd /Users/hanamori/life-assistant && grep -n '^csv' crates/app/Cargo.toml || echo MISSING`

If MISSING: add under `[dependencies]` in `crates/app/Cargo.toml`:

```toml
csv = "1.3"
```

Run: `cargo check -p manor-app 2>&1 | tail -5` — expected: builds.

- [ ] **Step 2: Write `csv_import.rs`**

```rust
//! CSV import with named bank presets.
//!
//! Each preset maps its bank's CSV schema to the canonical Manor row
//! (date, amount, description). Amounts end up in pence, signed (negative = debit).

use anyhow::{anyhow, Context, Result};
use chrono::{NaiveDate, TimeZone, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BankPreset {
    Monzo,
    Starling,
    Barclays,
    Hsbc,
    Natwest,
    Generic,
}

impl BankPreset {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "monzo" => Some(Self::Monzo),
            "starling" => Some(Self::Starling),
            "barclays" => Some(Self::Barclays),
            "hsbc" => Some(Self::Hsbc),
            "natwest" => Some(Self::Natwest),
            "generic" => Some(Self::Generic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewRow {
    pub date: i64,
    pub amount_pence: i64,
    pub description: String,
    pub suggested_category_id: Option<i64>,
    pub duplicate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub inserted: usize,
    pub skipped_duplicates: usize,
    pub skipped_errors: usize,
}

/// Generic CSV parser dispatch. Returns rows with suggested categories + dup flags.
pub fn parse_preview(
    conn: &Connection,
    preset: BankPreset,
    csv_bytes: &[u8],
    generic_cols: Option<GenericCols>,
) -> Result<Vec<PreviewRow>> {
    let raw = match preset {
        BankPreset::Monzo => parse_signed_amount(csv_bytes, "Date", "Amount", &["Name", "Description"])?,
        BankPreset::Starling => parse_signed_amount(csv_bytes, "Date", "Amount (GBP)", &["Counter Party"])?,
        BankPreset::Barclays => parse_signed_amount(csv_bytes, "Date", "Amount", &["Memo"])?,
        BankPreset::Hsbc => parse_debit_credit(csv_bytes, "Date", "Debit Amount", "Credit Amount", &["Transaction Description"])?,
        BankPreset::Natwest => parse_signed_amount(csv_bytes, "Date", "Value", &["Transaction type", "Description"])?,
        BankPreset::Generic => {
            let c = generic_cols.ok_or_else(|| anyhow!("generic preset requires column indices"))?;
            parse_generic(csv_bytes, c)?
        }
    };

    let categories = manor_core::ledger::category::list(conn).unwrap_or_default();
    let out = raw.into_iter().map(|r| {
        let suggested = categorize(&r.description, &categories);
        let duplicate = is_duplicate(conn, r.date, r.amount_pence, &r.description).unwrap_or(false);
        PreviewRow {
            date: r.date, amount_pence: r.amount_pence, description: r.description,
            suggested_category_id: suggested, duplicate,
        }
    }).collect();
    Ok(out)
}

/// Insert all non-duplicate rows. Uses a single sqlite transaction.
pub fn do_import(conn: &mut Connection, rows: Vec<PreviewRow>) -> Result<ImportResult> {
    let mut inserted = 0usize;
    let mut skipped_duplicates = 0usize;
    let mut skipped_errors = 0usize;
    let now = Utc::now().timestamp();
    let tx = conn.transaction()?;
    for row in rows {
        if row.duplicate {
            skipped_duplicates += 1;
            continue;
        }
        let res = tx.execute(
            "INSERT INTO ledger_transaction
             (bank_account_id, amount_pence, currency, description, merchant,
              category_id, date, source, note, created_at)
             VALUES (NULL, ?1, 'GBP', ?2, NULL, ?3, ?4, 'csv_import', NULL, ?5)",
            params![row.amount_pence, row.description, row.suggested_category_id, row.date, now],
        );
        match res {
            Ok(_) => inserted += 1,
            Err(_) => skipped_errors += 1,
        }
    }
    tx.commit()?;
    Ok(ImportResult { inserted, skipped_duplicates, skipped_errors })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GenericCols {
    pub date: usize,
    pub amount: usize,
    pub description: usize,
}

#[derive(Debug)]
struct RawRow {
    date: i64,
    amount_pence: i64,
    description: String,
}

fn parse_signed_amount(
    csv_bytes: &[u8],
    date_col: &str,
    amount_col: &str,
    desc_cols: &[&str],
) -> Result<Vec<RawRow>> {
    let mut rdr = csv::Reader::from_reader(csv_bytes);
    let headers = rdr.headers()?.clone();
    let date_idx = header_index(&headers, date_col)?;
    let amount_idx = header_index(&headers, amount_col)?;
    let desc_idxs: Vec<usize> = desc_cols.iter()
        .filter_map(|c| header_index(&headers, c).ok())
        .collect();
    if desc_idxs.is_empty() {
        return Err(anyhow!("no description columns found ({:?})", desc_cols));
    }

    let mut out = Vec::new();
    for rec in rdr.records() {
        let Ok(rec) = rec else { continue };
        let Ok(date) = parse_date(rec.get(date_idx).unwrap_or("")) else { continue };
        let Ok(amt) = parse_amount_pence(rec.get(amount_idx).unwrap_or("")) else { continue };
        let description = desc_idxs.iter()
            .filter_map(|i| rec.get(*i))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>().join(" ");
        out.push(RawRow { date, amount_pence: amt, description });
    }
    Ok(out)
}

fn parse_debit_credit(
    csv_bytes: &[u8],
    date_col: &str,
    debit_col: &str,
    credit_col: &str,
    desc_cols: &[&str],
) -> Result<Vec<RawRow>> {
    let mut rdr = csv::Reader::from_reader(csv_bytes);
    let headers = rdr.headers()?.clone();
    let date_idx = header_index(&headers, date_col)?;
    let debit_idx = header_index(&headers, debit_col)?;
    let credit_idx = header_index(&headers, credit_col)?;
    let desc_idxs: Vec<usize> = desc_cols.iter()
        .filter_map(|c| header_index(&headers, c).ok())
        .collect();

    let mut out = Vec::new();
    for rec in rdr.records() {
        let Ok(rec) = rec else { continue };
        let Ok(date) = parse_date(rec.get(date_idx).unwrap_or("")) else { continue };
        let debit = parse_amount_pence(rec.get(debit_idx).unwrap_or("")).unwrap_or(0);
        let credit = parse_amount_pence(rec.get(credit_idx).unwrap_or("")).unwrap_or(0);
        let amount = if debit != 0 { -debit.abs() } else if credit != 0 { credit.abs() } else { continue };
        let description = desc_idxs.iter()
            .filter_map(|i| rec.get(*i))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>().join(" ");
        out.push(RawRow { date, amount_pence: amount, description });
    }
    Ok(out)
}

fn parse_generic(csv_bytes: &[u8], cols: GenericCols) -> Result<Vec<RawRow>> {
    let mut rdr = csv::ReaderBuilder::new().has_headers(false).from_reader(csv_bytes);
    let mut out = Vec::new();
    for rec in rdr.records().skip(1) {
        let Ok(rec) = rec else { continue };
        let Ok(date) = parse_date(rec.get(cols.date).unwrap_or("")) else { continue };
        let Ok(amt) = parse_amount_pence(rec.get(cols.amount).unwrap_or("")) else { continue };
        let description = rec.get(cols.description).unwrap_or("").to_string();
        out.push(RawRow { date, amount_pence: amt, description });
    }
    Ok(out)
}

fn header_index(headers: &csv::StringRecord, col: &str) -> Result<usize> {
    headers.iter()
        .position(|h| h.eq_ignore_ascii_case(col))
        .ok_or_else(|| anyhow!("missing header '{col}'"))
}

fn parse_date(s: &str) -> Result<i64> {
    let s = s.trim();
    // Try ISO (YYYY-MM-DD), then UK (DD/MM/YYYY), then slash-ISO (YYYY/MM/DD).
    for fmt in &["%Y-%m-%d", "%d/%m/%Y", "%Y/%m/%d", "%d-%m-%Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Ok(Utc.with_ymd_and_hms(d.year_ce().1 as i32 * if d.year_ce().0 { 1 } else { -1 }, d.month(), d.day(), 0, 0, 0)
                .single().context("ymd out of range")?.timestamp());
        }
    }
    Err(anyhow!("unknown date format: {s}"))
}

fn parse_amount_pence(s: &str) -> Result<i64> {
    let cleaned: String = s.chars()
        .filter(|c| !matches!(*c, ' ' | '£' | ',' | '\''))
        .collect();
    if cleaned.is_empty() { return Err(anyhow!("empty amount")); }
    let f: f64 = cleaned.parse().map_err(|e| anyhow!("bad amount '{s}': {e}"))?;
    Ok((f * 100.0).round() as i64)
}

fn is_duplicate(conn: &Connection, date: i64, amount_pence: i64, description: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ledger_transaction
         WHERE date = ?1 AND amount_pence = ?2 AND LOWER(description) = LOWER(?3)
           AND deleted_at IS NULL",
        params![date, amount_pence, description],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

fn categorize(description: &str, categories: &[manor_core::ledger::category::Category]) -> Option<i64> {
    let up = description.to_uppercase();
    let find = |name: &str| categories.iter().find(|c| c.name.eq_ignore_ascii_case(name)).map(|c| c.id);
    const GROCERIES: &[&str] = &["TESCO", "SAINSBURY", "WAITROSE", "ALDI", "LIDL", "ASDA", "MORRISONS"];
    const EATING: &[&str] = &["UBER EATS", "DELIVEROO", "JUST EAT", "MCDONALD", "KFC", "NANDO"];
    const TRANSPORT: &[&str] = &["TFL", "UBER", "NATIONAL RAIL", "TRAINLINE"];
    const SUBS: &[&str] = &["NETFLIX", "SPOTIFY", "AMAZON PRIME", "DISNEY", "APPLE", "O2", "EE", "VODAFONE", "THREE", "SKY", "BT", "VIRGIN"];
    const HEALTH: &[&str] = &["BOOTS", "PHARMACY", "NHS", "DENTIST"];
    const INCOME: &[&str] = &["PAYROLL", "SALARY", "WAGES"];

    if GROCERIES.iter().any(|k| up.contains(k)) { return find("Groceries"); }
    if EATING.iter().any(|k| up.contains(k)) { return find("Eating Out"); }
    if TRANSPORT.iter().any(|k| up.contains(k)) { return find("Transport"); }
    if SUBS.iter().any(|k| up.contains(k)) { return find("Subscriptions"); }
    if HEALTH.iter().any(|k| up.contains(k)) { return find("Health"); }
    if INCOME.iter().any(|k| up.contains(k)) { return find("Income"); }
    None
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
    fn parses_monzo_signed_amount() {
        let csv = b"Date,Amount,Name,Description\n2026-04-10,-12.50,Tesco,Express\n2026-04-11,1500.00,Payroll Acme,\n";
        let (_d, conn) = fresh_conn();
        let rows = parse_preview(&conn, BankPreset::Monzo, csv, None).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].amount_pence, -1250);
        assert!(rows[0].description.contains("Tesco"));
        assert_eq!(rows[1].amount_pence, 150000);
    }

    #[test]
    fn parses_hsbc_split_debit_credit() {
        let csv = b"Date,Debit Amount,Credit Amount,Transaction Description\n10/04/2026,12.50,,TESCO\n11/04/2026,,1500.00,SALARY\n";
        let (_d, conn) = fresh_conn();
        let rows = parse_preview(&conn, BankPreset::Hsbc, csv, None).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].amount_pence, -1250);
        assert_eq!(rows[1].amount_pence, 150000);
    }

    #[test]
    fn suggests_category_by_keyword() {
        let csv = b"Date,Amount,Name,Description\n2026-04-10,-12.50,TESCO EXPRESS,Groceries\n";
        let (_d, conn) = fresh_conn();
        let rows = parse_preview(&conn, BankPreset::Monzo, csv, None).unwrap();
        assert_eq!(rows[0].suggested_category_id, Some(1)); // Groceries (id=1 from seeds)
    }

    #[test]
    fn flags_duplicates() {
        let (_d, mut conn) = fresh_conn();
        let now = Utc.with_ymd_and_hms(2026, 4, 10, 0, 0, 0).unwrap().timestamp();
        manor_core::ledger::transaction::insert(&conn, -1250, "GBP", "Tesco Express", None, None, now, None).unwrap();
        let csv = b"Date,Amount,Name,Description\n2026-04-10,-12.50,Tesco,Express\n";
        let rows = parse_preview(&conn, BankPreset::Monzo, csv, None).unwrap();
        assert!(rows[0].duplicate);

        let result = do_import(&mut conn, rows).unwrap();
        assert_eq!(result.inserted, 0);
        assert_eq!(result.skipped_duplicates, 1);
    }

    #[test]
    fn do_import_inserts_non_duplicates() {
        let (_d, mut conn) = fresh_conn();
        let csv = b"Date,Amount,Name,Description\n2026-04-10,-12.50,Tesco,\n2026-04-11,-5.00,Uber,\n";
        let rows = parse_preview(&conn, BankPreset::Monzo, csv, None).unwrap();
        let r = do_import(&mut conn, rows).unwrap();
        assert_eq!(r.inserted, 2);
        let txns = manor_core::ledger::transaction::list_by_month(&conn, 2026, 4).unwrap();
        assert_eq!(txns.len(), 2);
        assert_eq!(txns[0].source, "csv_import");
    }
}
```

- [ ] **Step 3: Update mod.rs**

Edit `crates/app/src/ledger/mod.rs`:

```rust
//! Tauri command glue for the Ledger feature.

pub mod ai_review;
pub mod commands;
pub mod csv_import;
```

(`ai_review` module is added in Task 10 — comment out for now if you want to compile at this step.)

- [ ] **Step 4: Run tests**

Run: `cd /Users/hanamori/life-assistant && cargo test -p manor-app ledger::csv_import 2>&1 | tail -20`
Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/ledger/csv_import.rs crates/app/src/ledger/mod.rs crates/app/Cargo.toml
git commit -m "feat(ledger): CSV import with 6 bank presets, dup detection, keyword categorizer"
```

---

## Task 9: `ledger_import_csv` command

**Files:**
- Modify: `crates/app/src/ledger/commands.rs`

- [ ] **Step 1: Add command**

Append:

```rust
// ── CSV Import ────────────────────────────────────────────────────────────────

use crate::ledger::csv_import::{self, BankPreset, GenericCols, ImportResult, PreviewRow};

#[derive(serde::Deserialize)]
pub struct ImportCsvArgs {
    pub preset: String,
    #[serde(rename = "csvBytes")]
    pub csv_bytes: Vec<u8>,
    #[serde(rename = "genericCols")]
    pub generic_cols: Option<GenericCols>,
}

#[derive(serde::Serialize)]
pub struct PreviewResponse {
    pub rows: Vec<PreviewRow>,
}

#[tauri::command]
pub fn ledger_preview_csv(
    state: State<'_, Db>,
    args: ImportCsvArgs,
) -> Result<PreviewResponse, String> {
    let preset = BankPreset::from_str(&args.preset).ok_or("unknown preset")?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let rows = csv_import::parse_preview(&conn, preset, &args.csv_bytes, args.generic_cols)
        .map_err(|e| e.to_string())?;
    Ok(PreviewResponse { rows })
}

#[derive(serde::Deserialize)]
pub struct DoImportArgs {
    pub rows: Vec<PreviewRow>,
}

#[tauri::command]
pub fn ledger_import_csv(
    state: State<'_, Db>,
    args: DoImportArgs,
) -> Result<ImportResult, String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    csv_import::do_import(&mut conn, args.rows).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Build**

Run: `cd /Users/hanamori/life-assistant && cargo check -p manor-app 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/ledger/commands.rs
git commit -m "feat(app): ledger_preview_csv + ledger_import_csv commands"
```

---

## Task 10: AI month-in-review streaming command

**Files:**
- Create: `crates/app/src/ledger/ai_review.rs`
- Modify: `crates/app/src/ledger/mod.rs` (ensure `pub mod ai_review;` uncommented)
- Modify: `crates/app/src/ledger/commands.rs`

- [ ] **Step 1: Write `ai_review.rs`**

```rust
//! Ollama-backed month-in-review narrative for the Ledger view.

use crate::assistant::ollama::{ChatMessage, ChatRole, OllamaClient, StreamChunk};
use manor_core::ledger::{budget::MonthlySummary, contract::RenewalAlert};
use tokio::sync::mpsc;

pub const REVIEW_MODEL: &str = "qwen2.5:7b-instruct";
pub const REVIEW_ENDPOINT: &str = "http://127.0.0.1:11434";

pub fn build_prompt(
    year: i32,
    month: u32,
    summary: &MonthlySummary,
    renewals: &[RenewalAlert],
) -> String {
    let month_name = month_name(month);
    let mut s = format!(
        "You are a calm personal finance assistant. The user's spending for {month_name} {year}:\n\n\
         Total in: £{:.2}\n\
         Total out: £{:.2}\n\
         Net: £{:.2}\n\n\
         By category:\n",
        summary.total_in_pence as f64 / 100.0,
        summary.total_out_pence as f64 / 100.0,
        (summary.total_in_pence - summary.total_out_pence) as f64 / 100.0,
    );
    for c in &summary.by_category {
        let spent = c.spent_pence as f64 / 100.0;
        if let Some(bp) = c.budget_pence {
            let budget = bp as f64 / 100.0;
            let diff = (c.spent_pence - bp).abs() as f64 / 100.0;
            let status = if c.spent_pence > bp { "over" } else { "under" };
            s.push_str(&format!(
                "  - {} {}: £{:.2} spent, budget £{:.2}, {status} by £{:.2}\n",
                c.category_emoji, c.category_name, spent, budget, diff
            ));
        } else {
            s.push_str(&format!(
                "  - {} {}: £{:.2} spent\n",
                c.category_emoji, c.category_name, spent
            ));
        }
    }
    if !renewals.is_empty() {
        s.push_str("\nUpcoming contract renewals: ");
        let list: Vec<String> = renewals.iter()
            .map(|r| format!("{} in {} days", r.provider, r.days_remaining))
            .collect();
        s.push_str(&list.join(", "));
        s.push('\n');
    }
    s.push_str("\nWrite 2-3 sentences summarising what happened this month in plain English. \
                Be specific about notable categories. Do not give financial advice. No bullet points.");
    s
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January", 2 => "February", 3 => "March", 4 => "April",
        5 => "May", 6 => "June", 7 => "July", 8 => "August",
        9 => "September", 10 => "October", 11 => "November", 12 => "December",
        _ => "Unknown",
    }
}

/// Run the review stream. Emits Token chunks on `out`; final Done is the caller's job.
pub async fn stream_review(prompt: String, out: mpsc::Sender<StreamChunk>) {
    let client = OllamaClient::new(REVIEW_ENDPOINT, REVIEW_MODEL);
    let msgs = vec![ChatMessage { role: ChatRole::User, content: prompt }];
    let _ = client.chat(&msgs, &[], &out).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::ledger::budget::CategorySpend;

    fn fixture_summary() -> MonthlySummary {
        MonthlySummary {
            total_in_pence: 320000,
            total_out_pence: 125000,
            by_category: vec![
                CategorySpend { category_id: 1, category_name: "Groceries".into(),
                    category_emoji: "🛒".into(), spent_pence: 45000, budget_pence: Some(40000) },
                CategorySpend { category_id: 2, category_name: "Eating Out".into(),
                    category_emoji: "🍕".into(), spent_pence: 12000, budget_pence: None },
            ],
        }
    }

    #[test]
    fn prompt_includes_totals_and_categories() {
        let p = build_prompt(2026, 4, &fixture_summary(), &[]);
        assert!(p.contains("April 2026"));
        assert!(p.contains("Total in: £3200.00"));
        assert!(p.contains("Total out: £1250.00"));
        assert!(p.contains("Net: £1950.00"));
        assert!(p.contains("🛒 Groceries: £450.00 spent, budget £400.00, over by £50.00"));
        assert!(p.contains("🍕 Eating Out: £120.00 spent"));
    }

    #[test]
    fn prompt_appends_renewals_when_present() {
        let renewals = vec![RenewalAlert {
            contract_id: 1, provider: "O2".into(), kind: "phone".into(),
            term_end: 0, days_remaining: 14, exit_fee_pence: None,
            severity: "amber".into(),
        }];
        let p = build_prompt(2026, 4, &fixture_summary(), &renewals);
        assert!(p.contains("Upcoming contract renewals: O2 in 14 days"));
    }
}
```

- [ ] **Step 2: Add the streaming command**

**NOTE (Landmark 2 update)**: The command routes through `crate::remote::orchestrator::remote_chat` when the user has enabled Claude for reviews (`ai.remote.enabled_for_review == "1"` AND a Claude key is set). Otherwise it falls through to local Ollama via `ai_review::stream_review`. This makes the toggle in Settings → AI actually work.

Append to `crates/app/src/ledger/commands.rs`:

```rust
// ── AI Month-in-Review ────────────────────────────────────────────────────────

use crate::assistant::ollama::StreamChunk;
use crate::ledger::ai_review;
use crate::remote::{keychain as remote_keychain, orchestrator as remote_orch,
                    PROVIDER_CLAUDE, REMOTE_ENABLED_FOR_REVIEW_KEY};
use tauri::ipc::Channel;
use tokio::sync::mpsc;

#[derive(serde::Deserialize)]
pub struct AiReviewArgs {
    pub year: i32,
    pub month: u32,
}

fn should_use_remote(conn: &rusqlite::Connection) -> bool {
    let enabled = manor_core::setting::get(conn, REMOTE_ENABLED_FOR_REVIEW_KEY)
        .ok()
        .flatten()
        .as_deref()
        == Some("1");
    enabled && remote_keychain::has_key(PROVIDER_CLAUDE)
}

#[tauri::command]
pub async fn ledger_ai_month_review(
    state: State<'_, Db>,
    args: AiReviewArgs,
    on_event: Channel<StreamChunk>,
) -> Result<(), String> {
    let (summary, renewals, use_remote) = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        let s = manor_core::ledger::budget::monthly_summary(&conn, args.year, args.month)
            .map_err(|e| e.to_string())?;
        let r = manor_core::ledger::contract::check_renewals(
            &conn, chrono::Utc::now().timestamp(),
        ).map_err(|e| e.to_string())?;
        let use_remote = should_use_remote(&conn);
        (s, r, use_remote)
    };
    let prompt = ai_review::build_prompt(args.year, args.month, &summary, &renewals);

    if use_remote {
        // Route through the Landmark 2 orchestrator — redacts, checks budget,
        // calls Claude, logs to remote_call_log. Non-streaming; we emit the
        // whole response as a single token chunk.
        let db_arc = state.inner().clone_arc();
        match remote_orch::remote_chat(db_arc, remote_orch::RemoteChatRequest {
            skill: "ledger_review",
            user_visible_reason: "Month-in-review narrative (ledger)",
            system_prompt: Some("You are a calm personal finance assistant. \
                Write 2-3 sentences in plain English. No bullet points. \
                Do not give financial advice."),
            user_prompt: &prompt,
            max_tokens: 400,
        }).await {
            Ok(outcome) => {
                on_event.send(StreamChunk::Token(outcome.text)).map_err(|e| e.to_string())?;
                on_event.send(StreamChunk::Done).map_err(|e| e.to_string())?;
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("remote review failed, falling back to local: {e}");
                // Fall through to local path below.
            }
        }
    }

    // Local Ollama streaming path.
    let (tx, mut rx) = mpsc::channel::<StreamChunk>(64);
    let stream_task = tokio::spawn(async move {
        ai_review::stream_review(prompt, tx).await;
    });
    while let Some(chunk) = rx.recv().await {
        on_event.send(chunk).map_err(|e| e.to_string())?;
    }
    let _ = stream_task.await;
    on_event.send(StreamChunk::Done).map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 3: Run tests**

Run: `cd /Users/hanamori/life-assistant && cargo test -p manor-app ledger::ai_review 2>&1 | tail -10`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/ledger/ai_review.rs crates/app/src/ledger/mod.rs crates/app/src/ledger/commands.rs
git commit -m "feat(ledger): ai_review prompt builder + streaming Tauri command"
```

---

## Task 11: Register commands + wire setup() side-effects

**Files:**
- Modify: `crates/app/src/lib.rs`

- [ ] **Step 1: Register 11 new commands**

In `crates/app/src/lib.rs`, find `.invoke_handler(tauri::generate_handler![...])` and append these entries inside the macro (after the last `ledger::commands::ledger_monthly_summary` line):

```rust
            ledger::commands::ledger_list_recurring,
            ledger::commands::ledger_add_recurring,
            ledger::commands::ledger_update_recurring,
            ledger::commands::ledger_delete_recurring,
            ledger::commands::ledger_list_contracts,
            ledger::commands::ledger_add_contract,
            ledger::commands::ledger_update_contract,
            ledger::commands::ledger_delete_contract,
            ledger::commands::ledger_get_renewal_alerts,
            ledger::commands::ledger_preview_csv,
            ledger::commands::ledger_import_csv,
            ledger::commands::ledger_ai_month_review,
```

- [ ] **Step 2: Schedule auto_insert_due + renewal log on startup**

In the same file, inside the `.setup(|app| { ... })` closure, after the existing `tauri::async_runtime::spawn(...)` block for calendar sync and before `Ok(())`, add:

```rust
            // Ledger: auto-insert due recurring payments + log renewal alerts.
            let db_arc_ledger = app.state::<assistant::commands::Db>().inner().clone_arc();
            tauri::async_runtime::spawn_blocking(move || {
                let mut conn = db_arc_ledger.lock().unwrap();
                let now = chrono::Utc::now();
                match manor_core::ledger::recurring::auto_insert_due(&mut conn, now) {
                    Ok(n) if n > 0 => tracing::info!("ledger: auto-inserted {n} recurring transaction(s)"),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("ledger: auto_insert_due failed: {e}"),
                }
                match manor_core::ledger::contract::check_renewals(&conn, now.timestamp()) {
                    Ok(alerts) if !alerts.is_empty() => {
                        tracing::info!("ledger: {} contract renewal alert(s) active", alerts.len());
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!("ledger: check_renewals failed: {e}"),
                }
            });
```

- [ ] **Step 3: Build + test full app**

Run: `cd /Users/hanamori/life-assistant && cargo build -p manor-app 2>&1 | tail -20`
Expected: clean build.

Run: `cargo test -p manor-app 2>&1 | tail -5`
Expected: all existing + new tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/lib.rs
git commit -m "feat(app): register 12 ledger commands + auto-insert/renewal scan on startup"
```

---

## Task 12: Frontend IPC + types (ledger)

**Files:**
- Modify: `apps/desktop/src/lib/ledger/ipc.ts`

- [ ] **Step 1: Append types + IPC functions**

Append to `apps/desktop/src/lib/ledger/ipc.ts`:

```ts
// Recurring payments
export interface RecurringPayment {
  id: number;
  description: string;
  amount_pence: number;
  currency: string;
  category_id: number | null;
  day_of_month: number;
  active: boolean;
  note: string | null;
  created_at: number;
}

export async function listRecurring(): Promise<RecurringPayment[]> {
  return invoke<RecurringPayment[]>("ledger_list_recurring");
}
export async function addRecurring(args: {
  description: string;
  amountPence: number;
  currency: string;
  categoryId?: number;
  dayOfMonth: number;
  note?: string;
}): Promise<RecurringPayment> {
  return invoke<RecurringPayment>("ledger_add_recurring", { args });
}
export async function updateRecurring(args: {
  id: number;
  description: string;
  amountPence: number;
  categoryId?: number;
  dayOfMonth: number;
  active: boolean;
  note?: string;
}): Promise<RecurringPayment> {
  return invoke<RecurringPayment>("ledger_update_recurring", { args });
}
export async function deleteRecurring(id: number): Promise<void> {
  return invoke<void>("ledger_delete_recurring", { id });
}

// Contracts
export interface Contract {
  id: number;
  provider: string;
  kind: "phone" | "broadband" | "insurance" | "energy" | "other";
  description: string | null;
  monthly_cost_pence: number;
  term_start: number;
  term_end: number;
  exit_fee_pence: number | null;
  renewal_alert_days: number;
  recurring_payment_id: number | null;
  note: string | null;
  created_at: number;
}

export interface RenewalAlert {
  contract_id: number;
  provider: string;
  kind: string;
  term_end: number;
  days_remaining: number;
  exit_fee_pence: number | null;
  severity: "amber" | "red";
}

export interface ContractArgs {
  provider: string;
  kind: string;
  description?: string;
  monthlyCostPence: number;
  termStart: number;
  termEnd: number;
  exitFeePence?: number;
  renewalAlertDays: number;
  recurringPaymentId?: number;
  note?: string;
}

export async function listContracts(): Promise<Contract[]> {
  return invoke<Contract[]>("ledger_list_contracts");
}
export async function addContract(args: ContractArgs): Promise<Contract> {
  return invoke<Contract>("ledger_add_contract", { args });
}
export async function updateContract(args: ContractArgs & { id: number }): Promise<Contract> {
  const { id, ...fields } = args;
  return invoke<Contract>("ledger_update_contract", { args: { id, fields } });
}
export async function deleteContract(id: number): Promise<void> {
  return invoke<void>("ledger_delete_contract", { id });
}
export async function getRenewalAlerts(): Promise<RenewalAlert[]> {
  return invoke<RenewalAlert[]>("ledger_get_renewal_alerts");
}

// CSV Import
export interface PreviewRow {
  date: number;
  amount_pence: number;
  description: string;
  suggested_category_id: number | null;
  duplicate: boolean;
}
export interface ImportResult {
  inserted: number;
  skipped_duplicates: number;
  skipped_errors: number;
}
export interface GenericCols { date: number; amount: number; description: number; }

export async function previewCsv(args: {
  preset: string;
  csvBytes: number[];
  genericCols?: GenericCols;
}): Promise<{ rows: PreviewRow[] }> {
  return invoke("ledger_preview_csv", { args });
}
export async function importCsv(rows: PreviewRow[]): Promise<ImportResult> {
  return invoke<ImportResult>("ledger_import_csv", { args: { rows } });
}

// AI Month Review — streams via Channel
import { Channel } from "@tauri-apps/api/core";
export type StreamChunk =
  | { type: "Token"; data: string }
  | { type: "Started"; data: number }
  | { type: "Done" }
  | { type: "Error"; data: string }
  | { type: "Proposal"; data: number };

export function aiMonthReview(
  args: { year: number; month: number },
  onEvent: (c: StreamChunk) => void
): Promise<void> {
  const ch = new Channel<StreamChunk>();
  ch.onmessage = onEvent;
  return invoke<void>("ledger_ai_month_review", { args, onEvent: ch });
}
```

- [ ] **Step 2: Typecheck**

Run: `cd /Users/hanamori/life-assistant/apps/desktop && pnpm tsc --noEmit 2>&1 | tail -20`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/lib/ledger/ipc.ts
git commit -m "feat(frontend): ledger IPC — recurring, contracts, CSV, AI review"
```

---

## Task 13: Frontend state slices

**Files:**
- Modify: `apps/desktop/src/lib/ledger/state.ts`
- Modify: `apps/desktop/src/lib/today/state.ts`

- [ ] **Step 1: Extend LedgerStore**

Replace `apps/desktop/src/lib/ledger/state.ts` with:

```ts
import { create } from "zustand";
import type {
  Budget, Category, Contract, MonthlySummary,
  RecurringPayment, RenewalAlert, Transaction
} from "./ipc";

interface LedgerStore {
  categories: Category[];
  transactions: Transaction[];
  budgets: Budget[];
  summary: MonthlySummary | null;
  recurring: RecurringPayment[];
  contracts: Contract[];
  renewalAlerts: RenewalAlert[];
  currentYear: number;
  currentMonth: number;

  setCategories: (c: Category[]) => void;
  setTransactions: (t: Transaction[]) => void;
  setBudgets: (b: Budget[]) => void;
  setSummary: (s: MonthlySummary) => void;
  setRecurring: (r: RecurringPayment[]) => void;
  setContracts: (c: Contract[]) => void;
  setRenewalAlerts: (a: RenewalAlert[]) => void;
  upsertTransaction: (t: Transaction) => void;
  removeTransaction: (id: number) => void;
  upsertCategory: (c: Category) => void;
  removeCategory: (id: number) => void;
  upsertBudget: (b: Budget) => void;
  removeBudget: (id: number) => void;
  upsertRecurring: (r: RecurringPayment) => void;
  removeRecurring: (id: number) => void;
  upsertContract: (c: Contract) => void;
  removeContract: (id: number) => void;
}

const now = new Date();

export const useLedgerStore = create<LedgerStore>((set) => ({
  categories: [],
  transactions: [],
  budgets: [],
  summary: null,
  recurring: [],
  contracts: [],
  renewalAlerts: [],
  currentYear: now.getFullYear(),
  currentMonth: now.getMonth() + 1,

  setCategories: (c) => set({ categories: c }),
  setTransactions: (t) => set({ transactions: t }),
  setBudgets: (b) => set({ budgets: b }),
  setSummary: (s) => set({ summary: s }),
  setRecurring: (r) => set({ recurring: r }),
  setContracts: (c) => set({ contracts: c }),
  setRenewalAlerts: (a) => set({ renewalAlerts: a }),

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
      const next = st.categories.slice(); next[idx] = c;
      return { categories: next };
    }),
  removeCategory: (id) =>
    set((st) => ({ categories: st.categories.filter((x) => x.id !== id) })),

  upsertBudget: (b) =>
    set((st) => {
      const idx = st.budgets.findIndex((x) => x.id === b.id);
      if (idx === -1) return { budgets: [...st.budgets, b] };
      const next = st.budgets.slice(); next[idx] = b;
      return { budgets: next };
    }),
  removeBudget: (id) =>
    set((st) => ({ budgets: st.budgets.filter((x) => x.id !== id) })),

  upsertRecurring: (r) =>
    set((st) => {
      const idx = st.recurring.findIndex((x) => x.id === r.id);
      if (idx === -1) return { recurring: [...st.recurring, r] };
      const next = st.recurring.slice(); next[idx] = r;
      return { recurring: next };
    }),
  removeRecurring: (id) =>
    set((st) => ({ recurring: st.recurring.filter((x) => x.id !== id) })),

  upsertContract: (c) =>
    set((st) => {
      const idx = st.contracts.findIndex((x) => x.id === c.id);
      if (idx === -1) return { contracts: [...st.contracts, c] };
      const next = st.contracts.slice(); next[idx] = c;
      return { contracts: next };
    }),
  removeContract: (id) =>
    set((st) => ({ contracts: st.contracts.filter((x) => x.id !== id) })),
}));
```

- [ ] **Step 2: Add `renewalAlerts` to Today store**

Open `apps/desktop/src/lib/today/state.ts`. Add to the store interface:

```ts
renewalAlerts: RenewalAlert[];
setRenewalAlerts: (a: RenewalAlert[]) => void;
```

Import `RenewalAlert` from `../ledger/ipc`. Add the initial value `renewalAlerts: []` and the setter `setRenewalAlerts: (a) => set({ renewalAlerts: a })` following existing patterns. (Grep the file to copy the existing style exactly.)

- [ ] **Step 3: Typecheck**

Run: `cd /Users/hanamori/life-assistant/apps/desktop && pnpm tsc --noEmit 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/lib/ledger/state.ts apps/desktop/src/lib/today/state.ts
git commit -m "feat(frontend): Zustand slices — recurring, contracts, renewalAlerts"
```

---

## Task 14: `RecurringSection` + `AddRecurringDrawer`

**Files:**
- Create: `apps/desktop/src/components/Ledger/RecurringSection.tsx`
- Create: `apps/desktop/src/components/Ledger/AddRecurringDrawer.tsx`

- [ ] **Step 1: Write `AddRecurringDrawer.tsx`**

Mirror the visual language of existing drawers (`AddTransactionForm.tsx`, `BudgetSheet.tsx`). Minimal fields: description (text), amount (pence input), category (select), day-of-month (1–28 number), active toggle.

```tsx
import { useState } from "react";
import { addRecurring, updateRecurring, type Category, type RecurringPayment } from "../../lib/ledger/ipc";

interface Props {
  categories: Category[];
  existing?: RecurringPayment;
  onClose: () => void;
  onSaved: () => void;
}

export default function AddRecurringDrawer({ categories, existing, onClose, onSaved }: Props) {
  const [description, setDescription] = useState(existing?.description ?? "");
  const [amount, setAmount] = useState(existing ? (existing.amount_pence / 100).toFixed(2) : "");
  const [categoryId, setCategoryId] = useState<number | null>(existing?.category_id ?? null);
  const [dayOfMonth, setDayOfMonth] = useState(existing?.day_of_month ?? 1);
  const [active, setActive] = useState(existing?.active ?? true);
  const [note, setNote] = useState(existing?.note ?? "");
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    setSaving(true);
    try {
      const amountPence = Math.round(parseFloat(amount) * 100);
      if (existing) {
        await updateRecurring({
          id: existing.id, description, amountPence,
          categoryId: categoryId ?? undefined, dayOfMonth, active,
          note: note || undefined,
        });
      } else {
        await addRecurring({
          description, amountPence, currency: "GBP",
          categoryId: categoryId ?? undefined, dayOfMonth,
          note: note || undefined,
        });
      }
      onSaved();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", zIndex: 100 }}
         onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()}
           style={{
             position: "absolute", right: 0, top: 0, bottom: 0, width: 420,
             background: "#111", color: "#eee", padding: 24, overflowY: "auto",
             display: "flex", flexDirection: "column", gap: 12,
           }}>
        <h2 style={{ margin: 0 }}>{existing ? "Edit" : "Add"} recurring payment</h2>
        <label>Description
          <input value={description} onChange={(e) => setDescription(e.target.value)} />
        </label>
        <label>Amount (£)
          <input type="number" step="0.01" value={amount} onChange={(e) => setAmount(e.target.value)} />
        </label>
        <label>Day of month (1–28)
          <input type="number" min={1} max={28} value={dayOfMonth}
                 onChange={(e) => setDayOfMonth(parseInt(e.target.value) || 1)} />
        </label>
        <label>Category
          <select value={categoryId ?? ""} onChange={(e) => setCategoryId(e.target.value ? parseInt(e.target.value) : null)}>
            <option value="">—</option>
            {categories.filter((c) => !c.is_income).map((c) => (
              <option key={c.id} value={c.id}>{c.emoji} {c.name}</option>
            ))}
          </select>
        </label>
        <label>Note (optional)
          <input value={note} onChange={(e) => setNote(e.target.value)} />
        </label>
        <label><input type="checkbox" checked={active} onChange={(e) => setActive(e.target.checked)} /> Active</label>
        <div style={{ display: "flex", gap: 8, marginTop: "auto" }}>
          <button onClick={onClose} disabled={saving}>Cancel</button>
          <button onClick={handleSave} disabled={saving || !description || !amount}>
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Write `RecurringSection.tsx`**

```tsx
import { useEffect, useState } from "react";
import { useLedgerStore } from "../../lib/ledger/state";
import { listRecurring, deleteRecurring, type RecurringPayment } from "../../lib/ledger/ipc";
import AddRecurringDrawer from "./AddRecurringDrawer";

export default function RecurringSection() {
  const { categories, recurring, setRecurring } = useLedgerStore();
  const [expanded, setExpanded] = useState(true);
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<RecurringPayment | null>(null);

  useEffect(() => { void listRecurring().then(setRecurring); }, [setRecurring]);

  const refresh = async () => {
    const r = await listRecurring();
    setRecurring(r);
  };

  return (
    <section style={{ background: "#151515", border: "1px solid #2a2a2a", borderRadius: 12, padding: 12 }}>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", cursor: "pointer" }}
              onClick={() => setExpanded((x) => !x)}>
        <h3 style={{ margin: 0 }}>Recurring ({recurring.filter((r) => r.active).length} active)</h3>
        <button onClick={(e) => { e.stopPropagation(); setAdding(true); }}>+ Add</button>
      </header>
      {expanded && (
        <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 4 }}>
          {recurring.length === 0 && <div style={{ color: "#666" }}>No recurring payments yet.</div>}
          {recurring.map((r) => {
            const cat = categories.find((c) => c.id === r.category_id);
            return (
              <div key={r.id}
                   style={{ display: "flex", justifyContent: "space-between", padding: 8,
                            borderRadius: 6, background: r.active ? "transparent" : "#1a1a1a", opacity: r.active ? 1 : 0.6 }}>
                <div>
                  <div>{cat?.emoji ?? "💳"} {r.description}</div>
                  <div style={{ fontSize: 12, color: "#888" }}>day {r.day_of_month} · £{(r.amount_pence / 100).toFixed(2)}</div>
                </div>
                <div style={{ display: "flex", gap: 4 }}>
                  <button onClick={() => setEditing(r)}>Edit</button>
                  <button onClick={async () => { await deleteRecurring(r.id); await refresh(); }}>Delete</button>
                </div>
              </div>
            );
          })}
        </div>
      )}
      {adding && (
        <AddRecurringDrawer categories={categories}
                            onClose={() => setAdding(false)}
                            onSaved={async () => { setAdding(false); await refresh(); }} />
      )}
      {editing && (
        <AddRecurringDrawer categories={categories} existing={editing}
                            onClose={() => setEditing(null)}
                            onSaved={async () => { setEditing(null); await refresh(); }} />
      )}
    </section>
  );
}
```

- [ ] **Step 3: Typecheck**

Run: `pnpm tsc --noEmit 2>&1 | tail -5` (from `apps/desktop`)
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Ledger/RecurringSection.tsx apps/desktop/src/components/Ledger/AddRecurringDrawer.tsx
git commit -m "feat(ledger): RecurringSection + AddRecurringDrawer"
```

---

## Task 15: `ContractsSection` + `AddContractDrawer`

**Files:**
- Create: `apps/desktop/src/components/Ledger/AddContractDrawer.tsx`
- Create: `apps/desktop/src/components/Ledger/ContractsSection.tsx`

- [ ] **Step 1: `AddContractDrawer.tsx`**

```tsx
import { useState } from "react";
import { addContract, updateContract, type Contract, type ContractArgs } from "../../lib/ledger/ipc";

const KINDS = ["phone", "broadband", "insurance", "energy", "other"] as const;

function toUnix(yyyymmdd: string): number {
  return Math.floor(new Date(`${yyyymmdd}T00:00:00Z`).getTime() / 1000);
}
function toDateInput(unix: number): string {
  return new Date(unix * 1000).toISOString().slice(0, 10);
}

interface Props {
  existing?: Contract;
  onClose: () => void;
  onSaved: () => void;
}

export default function AddContractDrawer({ existing, onClose, onSaved }: Props) {
  const [provider, setProvider] = useState(existing?.provider ?? "");
  const [kind, setKind] = useState<string>(existing?.kind ?? "other");
  const [description, setDescription] = useState(existing?.description ?? "");
  const [monthlyCost, setMonthlyCost] = useState(existing ? (existing.monthly_cost_pence / 100).toFixed(2) : "");
  const [termStart, setTermStart] = useState(existing ? toDateInput(existing.term_start) : "");
  const [termEnd, setTermEnd] = useState(existing ? toDateInput(existing.term_end) : "");
  const [exitFee, setExitFee] = useState(existing?.exit_fee_pence ? (existing.exit_fee_pence / 100).toFixed(2) : "");
  const [alertDays, setAlertDays] = useState(existing?.renewal_alert_days ?? 30);
  const [note, setNote] = useState(existing?.note ?? "");
  const [saving, setSaving] = useState(false);

  const save = async () => {
    setSaving(true);
    try {
      const args: ContractArgs = {
        provider, kind, description: description || undefined,
        monthlyCostPence: Math.round(parseFloat(monthlyCost) * 100),
        termStart: toUnix(termStart), termEnd: toUnix(termEnd),
        exitFeePence: exitFee ? Math.round(parseFloat(exitFee) * 100) : undefined,
        renewalAlertDays: alertDays, note: note || undefined,
      };
      if (existing) await updateContract({ ...args, id: existing.id });
      else await addContract(args);
      onSaved();
    } finally { setSaving(false); }
  };

  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", zIndex: 100 }} onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()}
           style={{ position: "absolute", right: 0, top: 0, bottom: 0, width: 420,
                    background: "#111", color: "#eee", padding: 24, overflowY: "auto",
                    display: "flex", flexDirection: "column", gap: 12 }}>
        <h2 style={{ margin: 0 }}>{existing ? "Edit" : "Add"} contract</h2>
        <label>Provider <input value={provider} onChange={(e) => setProvider(e.target.value)} /></label>
        <label>Kind
          <select value={kind} onChange={(e) => setKind(e.target.value)}>
            {KINDS.map((k) => <option key={k} value={k}>{k}</option>)}
          </select>
        </label>
        <label>Description <input value={description} onChange={(e) => setDescription(e.target.value)} /></label>
        <label>Monthly cost (£) <input type="number" step="0.01" value={monthlyCost} onChange={(e) => setMonthlyCost(e.target.value)} /></label>
        <label>Term start <input type="date" value={termStart} onChange={(e) => setTermStart(e.target.value)} /></label>
        <label>Term end <input type="date" value={termEnd} onChange={(e) => setTermEnd(e.target.value)} /></label>
        <label>Exit fee (£) <input type="number" step="0.01" value={exitFee} onChange={(e) => setExitFee(e.target.value)} /></label>
        <label>Alert days <input type="number" min={1} value={alertDays} onChange={(e) => setAlertDays(parseInt(e.target.value) || 30)} /></label>
        <label>Note <input value={note} onChange={(e) => setNote(e.target.value)} /></label>
        <div style={{ display: "flex", gap: 8, marginTop: "auto" }}>
          <button onClick={onClose} disabled={saving}>Cancel</button>
          <button onClick={save} disabled={saving || !provider || !monthlyCost || !termStart || !termEnd}>
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: `ContractsSection.tsx`**

```tsx
import { useEffect, useState } from "react";
import { useLedgerStore } from "../../lib/ledger/state";
import { listContracts, deleteContract, type Contract } from "../../lib/ledger/ipc";
import AddContractDrawer from "./AddContractDrawer";

function daysUntil(unix: number): number {
  return Math.floor((unix - Date.now() / 1000) / 86400);
}

export default function ContractsSection() {
  const { contracts, setContracts } = useLedgerStore();
  const [expanded, setExpanded] = useState(true);
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<Contract | null>(null);

  useEffect(() => { void listContracts().then(setContracts); }, [setContracts]);

  const refresh = async () => { setContracts(await listContracts()); };

  return (
    <section style={{ background: "#151515", border: "1px solid #2a2a2a", borderRadius: 12, padding: 12 }}>
      <header style={{ display: "flex", justifyContent: "space-between", cursor: "pointer" }}
              onClick={() => setExpanded((x) => !x)}>
        <h3 style={{ margin: 0 }}>Contracts ({contracts.length})</h3>
        <button onClick={(e) => { e.stopPropagation(); setAdding(true); }}>+ Add</button>
      </header>
      {expanded && (
        <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 4 }}>
          {contracts.length === 0 && <div style={{ color: "#666" }}>No contracts yet.</div>}
          {contracts.map((c) => {
            const days = daysUntil(c.term_end);
            const pillColor = days < 0 ? "#666" : days <= 7 ? "#c33" : days <= c.renewal_alert_days ? "#d90" : "#333";
            return (
              <div key={c.id}
                   style={{ display: "flex", justifyContent: "space-between", padding: 8, borderRadius: 6 }}>
                <div>
                  <div>{c.provider} <span style={{ fontSize: 11, color: "#888" }}>({c.kind})</span></div>
                  <div style={{ fontSize: 12, color: "#888" }}>£{(c.monthly_cost_pence / 100).toFixed(2)}/mo</div>
                </div>
                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <span style={{ padding: "2px 8px", borderRadius: 12, background: pillColor, fontSize: 11 }}>
                    {days < 0 ? "expired" : `${days}d`}
                  </span>
                  <button onClick={() => setEditing(c)}>Edit</button>
                  <button onClick={async () => { await deleteContract(c.id); await refresh(); }}>Delete</button>
                </div>
              </div>
            );
          })}
        </div>
      )}
      {adding && <AddContractDrawer onClose={() => setAdding(false)} onSaved={async () => { setAdding(false); await refresh(); }} />}
      {editing && <AddContractDrawer existing={editing} onClose={() => setEditing(null)} onSaved={async () => { setEditing(null); await refresh(); }} />}
    </section>
  );
}
```

- [ ] **Step 3: Typecheck + commit**

```bash
cd apps/desktop && pnpm tsc --noEmit 2>&1 | tail -5
# expected: clean
cd ../..
git add apps/desktop/src/components/Ledger/AddContractDrawer.tsx apps/desktop/src/components/Ledger/ContractsSection.tsx
git commit -m "feat(ledger): ContractsSection + AddContractDrawer with countdown pills"
```

---

## Task 16: `CsvImportDrawer`

**Files:**
- Create: `apps/desktop/src/components/Ledger/CsvImportDrawer.tsx`

- [ ] **Step 1: Write component**

```tsx
import { useState } from "react";
import { previewCsv, importCsv, type PreviewRow, type ImportResult } from "../../lib/ledger/ipc";
import { useLedgerStore } from "../../lib/ledger/state";

const PRESETS = [
  { id: "monzo", label: "Monzo" },
  { id: "starling", label: "Starling" },
  { id: "barclays", label: "Barclays" },
  { id: "hsbc", label: "HSBC" },
  { id: "natwest", label: "Natwest" },
  { id: "generic", label: "Generic (pick columns)" },
];

interface Props {
  onClose: () => void;
  onImported: (result: ImportResult) => void;
}

export default function CsvImportDrawer({ onClose, onImported }: Props) {
  const { categories } = useLedgerStore();
  const [preset, setPreset] = useState("monzo");
  const [rows, setRows] = useState<PreviewRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);

  const handleFile = async (f: File | null) => {
    setError(null);
    if (!f) return;
    const bytes = new Uint8Array(await f.arrayBuffer());
    try {
      const resp = await previewCsv({ preset, csvBytes: Array.from(bytes) });
      setRows(resp.rows);
    } catch (e) {
      setError(String(e));
    }
  };

  const doImport = async () => {
    setImporting(true);
    try {
      const result = await importCsv(rows);
      onImported(result);
    } finally {
      setImporting(false);
    }
  };

  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", zIndex: 100 }} onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()}
           style={{ position: "absolute", right: 0, top: 0, bottom: 0, width: 600,
                    background: "#111", color: "#eee", padding: 24, overflowY: "auto",
                    display: "flex", flexDirection: "column", gap: 12 }}>
        <h2 style={{ margin: 0 }}>Import CSV</h2>
        <label>Bank
          <select value={preset} onChange={(e) => { setPreset(e.target.value); setRows([]); }}>
            {PRESETS.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
          </select>
        </label>
        <input type="file" accept=".csv,text/csv" onChange={(e) => handleFile(e.target.files?.[0] ?? null)} />
        {error && <div style={{ color: "#f66" }}>{error}</div>}
        {rows.length > 0 && (
          <>
            <div style={{ fontSize: 12, color: "#888" }}>
              {rows.length} rows · {rows.filter((r) => r.duplicate).length} duplicate(s)
            </div>
            <table style={{ width: "100%", fontSize: 12, borderCollapse: "collapse" }}>
              <thead>
                <tr style={{ textAlign: "left", borderBottom: "1px solid #333" }}>
                  <th>Date</th><th>Amount</th><th>Description</th><th>Category</th>
                </tr>
              </thead>
              <tbody>
                {rows.slice(0, 20).map((r, i) => {
                  const cat = categories.find((c) => c.id === r.suggested_category_id);
                  return (
                    <tr key={i} style={{ opacity: r.duplicate ? 0.4 : 1, borderBottom: "1px solid #222" }}>
                      <td>{new Date(r.date * 1000).toISOString().slice(0, 10)}</td>
                      <td style={{ color: r.amount_pence < 0 ? "#f66" : "#6f6" }}>
                        £{(r.amount_pence / 100).toFixed(2)}
                      </td>
                      <td>{r.description}</td>
                      <td>{cat ? `${cat.emoji} ${cat.name}` : "—"}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            {rows.length > 20 && <div style={{ fontSize: 11, color: "#666" }}>(showing first 20 of {rows.length})</div>}
          </>
        )}
        <div style={{ display: "flex", gap: 8, marginTop: "auto" }}>
          <button onClick={onClose}>Cancel</button>
          <button onClick={doImport} disabled={rows.length === 0 || importing}>
            {importing ? "Importing…" : `Import ${rows.filter((r) => !r.duplicate).length}`}
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd apps/desktop && pnpm tsc --noEmit 2>&1 | tail -5
cd ../..
git add apps/desktop/src/components/Ledger/CsvImportDrawer.tsx
git commit -m "feat(ledger): CsvImportDrawer with preview + duplicate flagging"
```

---

## Task 17: `MonthReviewPanel`

**Files:**
- Create: `apps/desktop/src/components/Ledger/MonthReviewPanel.tsx`

- [ ] **Step 1: Write component**

```tsx
import { useState } from "react";
import { aiMonthReview, type MonthlySummary, type StreamChunk } from "../../lib/ledger/ipc";

interface Props {
  year: number;
  month: number;
  summary: MonthlySummary;
}

export default function MonthReviewPanel({ year, month, summary }: Props) {
  const [text, setText] = useState("");
  const [running, setRunning] = useState(false);
  const [refreshedAt, setRefreshedAt] = useState<Date | null>(null);
  const [error, setError] = useState<string | null>(null);

  const net = summary.total_in_pence - summary.total_out_pence;

  const run = async () => {
    setRunning(true);
    setText("");
    setError(null);
    try {
      await aiMonthReview({ year, month }, (c: StreamChunk) => {
        if (c.type === "Token") setText((t) => t + c.data);
        else if (c.type === "Error") setError(c.data);
      });
      setRefreshedAt(new Date());
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <section style={{ background: "#151515", border: "1px solid #2a2a2a", borderRadius: 12, padding: 16 }}>
      <div style={{ display: "flex", gap: 24, marginBottom: 12 }}>
        <div>
          <div style={{ fontSize: 11, color: "#888" }}>In</div>
          <div style={{ fontSize: 18, color: "#6f6" }}>£{(summary.total_in_pence / 100).toFixed(2)}</div>
        </div>
        <div>
          <div style={{ fontSize: 11, color: "#888" }}>Out</div>
          <div style={{ fontSize: 18, color: "#f66" }}>£{(summary.total_out_pence / 100).toFixed(2)}</div>
        </div>
        <div>
          <div style={{ fontSize: 11, color: "#888" }}>Net</div>
          <div style={{ fontSize: 18, color: net >= 0 ? "#6f6" : "#f66" }}>£{(net / 100).toFixed(2)}</div>
        </div>
      </div>
      {text ? (
        <>
          <div style={{ whiteSpace: "pre-wrap", fontSize: 14, color: "#ddd" }}>{text}</div>
          {refreshedAt && (
            <div style={{ fontSize: 11, color: "#666", marginTop: 6 }}>
              Refreshed {refreshedAt.toLocaleTimeString()} ·{" "}
              <a href="#" onClick={(e) => { e.preventDefault(); void run(); }}>Refresh</a>
            </div>
          )}
        </>
      ) : (
        <button onClick={run} disabled={running}>{running ? "Thinking…" : "Review with AI"}</button>
      )}
      {error && (
        <div style={{ marginTop: 8, fontSize: 12, color: "#f66" }}>
          AI unavailable — start Ollama to use this feature.{" "}
          <button onClick={() => setError(null)} style={{ background: "none", border: "none", color: "#888" }}>dismiss</button>
        </div>
      )}
    </section>
  );
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd apps/desktop && pnpm tsc --noEmit 2>&1 | tail -5
cd ../..
git add apps/desktop/src/components/Ledger/MonthReviewPanel.tsx
git commit -m "feat(ledger): MonthReviewPanel — persistent summary + streaming AI narrative"
```

---

## Task 18: `RenewalAlertsCard` in Today view

**Files:**
- Create: `apps/desktop/src/components/Today/RenewalAlertsCard.tsx`
- Modify: `apps/desktop/src/components/Today/Today.tsx`

- [ ] **Step 1: Write the card**

```tsx
import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";
import { getRenewalAlerts } from "../../lib/ledger/ipc";

export default function RenewalAlertsCard() {
  const { renewalAlerts, setRenewalAlerts } = useTodayStore();

  useEffect(() => { void getRenewalAlerts().then(setRenewalAlerts); }, [setRenewalAlerts]);

  if (renewalAlerts.length === 0) return null;

  return (
    <section style={{ background: "#1a1410", border: "1px solid #4a2f15", borderRadius: 12, padding: 12 }}>
      <header style={{ fontSize: 13, color: "#d90", marginBottom: 6 }}>What matters</header>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        {renewalAlerts.map((a) => (
          <div key={a.contract_id}
               style={{ display: "flex", justifyContent: "space-between",
                        padding: 6, borderRadius: 6, background: a.severity === "red" ? "#2a0e0e" : "#241805" }}>
            <div>{a.provider} ({a.kind}) renewing soon</div>
            <span style={{ padding: "2px 8px", borderRadius: 12, fontSize: 11,
                           background: a.severity === "red" ? "#c33" : "#d90" }}>
              {a.days_remaining}d
            </span>
          </div>
        ))}
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Mount in Today**

Edit `apps/desktop/src/components/Today/Today.tsx`. Add import and mount it below `ProposalBanner`:

```tsx
import RenewalAlertsCard from "./RenewalAlertsCard";

// inside the <main> JSX, after <ProposalBanner />:
<RenewalAlertsCard />
```

- [ ] **Step 3: Typecheck + commit**

```bash
cd apps/desktop && pnpm tsc --noEmit 2>&1 | tail -5
cd ../..
git add apps/desktop/src/components/Today/RenewalAlertsCard.tsx apps/desktop/src/components/Today/Today.tsx
git commit -m "feat(today): RenewalAlertsCard — amber/red contract pills"
```

---

## Task 19: Wire everything into `LedgerView`

**Files:**
- Modify: `apps/desktop/src/components/Ledger/LedgerView.tsx`

- [ ] **Step 1: Replace LedgerView**

```tsx
import { useEffect, useState } from "react";
import { useLedgerStore } from "../../lib/ledger/state";
import {
  listCategories, listTransactions, listBudgets, getMonthlySummary,
} from "../../lib/ledger/ipc";
import { AVATAR_FOOTPRINT_PX } from "../../lib/layout";
import MonthReviewPanel from "./MonthReviewPanel";
import RecurringSection from "./RecurringSection";
import ContractsSection from "./ContractsSection";
import TransactionFeed from "./TransactionFeed";
import AddTransactionForm from "./AddTransactionForm";
import BudgetSheet from "./BudgetSheet";
import CsvImportDrawer from "./CsvImportDrawer";

export default function LedgerView() {
  const { categories, transactions, budgets, summary, currentYear, currentMonth,
          setCategories, setTransactions, setBudgets, setSummary } = useLedgerStore();
  const [showAdd, setShowAdd] = useState(false);
  const [showBudgets, setShowBudgets] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [importToast, setImportToast] = useState<string | null>(null);

  useEffect(() => {
    void listCategories().then(setCategories);
    void listBudgets().then(setBudgets);
    void listTransactions(currentYear, currentMonth).then(setTransactions);
    void getMonthlySummary(currentYear, currentMonth).then(setSummary);
  }, [currentYear, currentMonth, setCategories, setBudgets, setTransactions, setSummary]);

  const refreshAfterChange = async () => {
    const [txns, s, bs] = await Promise.all([
      listTransactions(currentYear, currentMonth),
      getMonthlySummary(currentYear, currentMonth),
      listBudgets(),
    ]);
    setTransactions(txns);
    setSummary(s);
    setBudgets(bs);
  };

  return (
    <>
      <main style={{ maxWidth: 760, margin: "0 auto",
                     padding: `24px 24px ${AVATAR_FOOTPRINT_PX}px 24px`,
                     display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h2 style={{ margin: 0 }}>Ledger</h2>
          <div style={{ display: "flex", gap: 8 }}>
            <button onClick={() => setShowBudgets(true)}>Budgets</button>
            <button onClick={() => setShowImport(true)}>Import CSV</button>
          </div>
        </div>
        {summary && <MonthReviewPanel year={currentYear} month={currentMonth} summary={summary} />}
        <RecurringSection />
        <ContractsSection />
        <TransactionFeed transactions={transactions} categories={categories}
                         onAdd={() => setShowAdd(true)} />
        {importToast && <div style={{ fontSize: 12, color: "#6f6" }}>{importToast}</div>}
      </main>

      {showAdd && (
        <AddTransactionForm categories={categories}
                            onClose={() => setShowAdd(false)}
                            onSaved={async () => { setShowAdd(false); await refreshAfterChange(); }} />
      )}
      {showBudgets && (
        <BudgetSheet categories={categories} budgets={budgets}
                     onClose={() => setShowBudgets(false)}
                     onChanged={async () => { await refreshAfterChange(); }} />
      )}
      {showImport && (
        <CsvImportDrawer onClose={() => setShowImport(false)}
                         onImported={async (r) => {
                           setShowImport(false);
                           setImportToast(`Imported ${r.inserted} · skipped ${r.skipped_duplicates} duplicate(s)`);
                           await refreshAfterChange();
                           setTimeout(() => setImportToast(null), 4000);
                         }} />
      )}
    </>
  );
}
```

- [ ] **Step 2: Final typecheck + full build**

```bash
cd apps/desktop && pnpm tsc --noEmit 2>&1 | tail -10
cd ../..
cargo build -p manor-app 2>&1 | tail -10
cargo test 2>&1 | tail -10
```
Expected: all green.

- [ ] **Step 3: Manual smoke test**

Run: `cd /Users/hanamori/life-assistant && pnpm tauri dev` (or however manor-app runs — check `package.json` scripts).

Manually verify:
1. Ledger tab opens without console errors.
2. Add a recurring payment with day-of-month = today's day. Close/reopen app. A new transaction with `source=recurring` appears in the feed.
3. Add a contract with `term_end` 5 days from now. Verify Today view shows a red pill.
4. Import a small Monzo CSV. Preview shows rows; confirm imports them.
5. With Ollama running, click "Review with AI" — text streams in.
6. Stop Ollama, click Refresh — dismissable "AI unavailable" error appears, summary numbers still visible.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Ledger/LedgerView.tsx
git commit -m "feat(ledger): wire MonthReviewPanel, Recurring, Contracts, CSV into LedgerView"
```

---

## Post-Plan Verification

Before declaring this plan complete, run:

```bash
cd /Users/hanamori/life-assistant
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cd apps/desktop && pnpm tsc --noEmit && pnpm lint
```

All must pass. If clippy or fmt fails, fix in the offending task's commit via `--amend` only if the branch hasn't been pushed.
