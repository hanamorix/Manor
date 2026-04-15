# Phase 3b — CalDAV Read Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Populate Manor's EventsCard with real CalDAV-synced events. Adds the first Settings surface (tabbed modal triggered by a cog + ⌘,), CalDAV discovery + event fetch + iCal parsing + RRULE expansion, Keychain-stored credentials, and sync triggers on app start / add-account / manual refresh.

**Architecture:** New Rust modules — `manor-core::assistant::{calendar_account, event}` (DAL), `manor-app::sync::{caldav, ical, expand, keychain, engine}` (sync stack). 6 new Tauri commands. New frontend `useSettingsStore` + `<Settings*>` component tree. `useTodayStore` gains an `events[]` slice. Full resync-per-account (wipe + reinsert) in a ±7-day backward / 14-day forward window. Password in macOS Keychain via `keyring`.

**Tech Stack:** Rust (rusqlite, refinery, reqwest, tokio, serde, quick-xml, ical, rrule, chrono-tz, keyring, wiremock for tests); React 18 + TypeScript + zustand; Tauri 2.x.

**Worktree setup:** Before Task 1, create a worktree at `/Users/hanamori/life-assistant/.worktrees/phase-3b-caldav-read` on a new branch `feature/phase-3b-caldav-read` branched from `main`. All tasks run inside that worktree.

**Spec reference:** `docs/superpowers/specs/2026-04-15-phase-3b-caldav-read-design.md` — defer to it for exact layouts, wire-formats, and prose; the plan below drills down to the code.

---

## Task breakdown (17 tasks)

1. Workspace dependencies (ical, rrule, keyring, chrono-tz, quick-xml, base64)
2. V3 migration + `calendar_account` DAL (TDD)
3. `event` DAL (TDD)
4. iCal parser module (TDD with fixture strings)
5. RRULE expansion module (TDD)
6. Keychain wrapper
7. CalDAV HTTP client — PROPFIND + REPORT (TDD with wiremock)
8. Sync engine orchestrator (TDD with wiremock)
9. `SyncState` + 6 new Tauri commands + app-start sync
10. Frontend foundations — `useSettingsStore` + settings IPC + `useTodayStore.events` (TDD state tests)
11. `SettingsModal` shell + `Tabs` + `settingsIn` keyframe + `⌘,` hotkey + mount in App
12. `SettingsCog` + wire into HeaderCard
13. `AccountRow` component
14. `AddAccountForm` component
15. `CalendarsTab` composer
16. `EventsCard` — populate from store
17. Manual smoke + tag + PR

---

### Task 1: Workspace dependencies

**Files:**
- Modify: `Cargo.toml` (root) — `[workspace.dependencies]`
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/app/Cargo.toml`

- [ ] **Step 1: Extend `[workspace.dependencies]`**

Add these entries to the existing table in the root `Cargo.toml` (keep all existing entries intact):

```toml
ical = "0.11"
rrule = "0.13"
keyring = "3"
chrono-tz = { version = "0.9", default-features = false, features = ["std"] }
quick-xml = "0.36"
base64 = "0.22"
```

- [ ] **Step 2: Wire deps into `manor-core`**

Add to `crates/core/Cargo.toml` `[dependencies]`:

```toml
# (keep existing)
chrono-tz.workspace = true
```

(manor-core only needs `chrono-tz` for event day-boundary helpers — other CalDAV deps stay in `manor-app`.)

- [ ] **Step 3: Wire deps into `manor-app`**

Add to `crates/app/Cargo.toml` `[dependencies]`:

```toml
# (keep existing)
ical.workspace = true
rrule.workspace = true
keyring.workspace = true
chrono-tz.workspace = true
quick-xml.workspace = true
base64.workspace = true
```

- [ ] **Step 4: Confirm workspace builds**

Run: `cargo check --workspace`
Expected: clean (first run pulls ~50 new transitive crates; 3–5 min).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/core/Cargo.toml crates/app/Cargo.toml
git commit -m "chore(deps): add ical, rrule, keyring, chrono-tz, quick-xml, base64 for phase 3b"
```

---

### Task 2: V3 migration + `calendar_account` DAL (TDD)

**Files:**
- Create: `crates/core/migrations/V3__calendar.sql`
- Create: `crates/core/src/assistant/calendar_account.rs`
- Modify: `crates/core/src/assistant/mod.rs`

- [ ] **Step 1: Migration**

Create `crates/core/migrations/V3__calendar.sql`:

```sql
CREATE TABLE calendar_account (
  id                INTEGER PRIMARY KEY,
  display_name      TEXT    NOT NULL,
  server_url        TEXT    NOT NULL,
  username          TEXT    NOT NULL,
  last_synced_at    INTEGER NULL,
  last_error        TEXT    NULL,
  created_at        INTEGER NOT NULL,
  UNIQUE (server_url, username)
);

CREATE TABLE event (
  id                  INTEGER PRIMARY KEY,
  calendar_account_id INTEGER NOT NULL REFERENCES calendar_account(id) ON DELETE CASCADE,
  external_id         TEXT    NOT NULL,
  title               TEXT    NOT NULL,
  start_at            INTEGER NOT NULL,
  end_at              INTEGER NOT NULL,
  created_at          INTEGER NOT NULL,
  UNIQUE (calendar_account_id, external_id)
);

CREATE INDEX idx_event_start_at ON event (start_at);
```

**IMPORTANT:** enable SQLite foreign keys at connection open. Update `crates/core/src/assistant/db.rs` `init()` to execute `PRAGMA foreign_keys = ON;` right after `Connection::open`. If this isn't set, the `ON DELETE CASCADE` on `event.calendar_account_id` is silently ignored. Read the current `db.rs` first, add the pragma call between `Connection::open(path)?` and `migrations::runner()...`.

- [ ] **Step 2: Expose the new submodule**

Edit `crates/core/src/assistant/mod.rs` — add `calendar_account` (alphabetical position) and `event`. Final shape:

```rust
//! Assistant substrate: SQLite persistence for conversations, messages, proposals, tasks, calendar accounts, and events.

pub mod calendar_account;
pub mod conversation;
pub mod db;
pub mod event;     // will land in Task 3
pub mod message;
pub mod proposal;
pub mod task;
```

(You'll create `event.rs` empty here — just `//! stub` — so this compiles. Task 3 fills it.)

Create `crates/core/src/assistant/event.rs` with just:

```rust
//! Event data access — implemented in Task 3.
```

- [ ] **Step 3: Write `calendar_account.rs`**

Create `crates/core/src/assistant/calendar_account.rs`:

```rust
//! Calendar account metadata (CalDAV). Password is NOT stored here —
//! it lives in macOS Keychain, keyed by the row id.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalendarAccount {
    pub id: i64,
    pub display_name: String,
    pub server_url: String,
    pub username: String,
    pub last_synced_at: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: i64,
}

impl CalendarAccount {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            display_name: row.get("display_name")?,
            server_url: row.get("server_url")?,
            username: row.get("username")?,
            last_synced_at: row.get("last_synced_at")?,
            last_error: row.get("last_error")?,
            created_at: row.get("created_at")?,
        })
    }
}

pub fn insert(
    conn: &Connection,
    display_name: &str,
    server_url: &str,
    username: &str,
) -> Result<i64> {
    let now_ms = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO calendar_account (display_name, server_url, username, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![display_name, server_url, username, now_ms],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list(conn: &Connection) -> Result<Vec<CalendarAccount>> {
    let mut stmt = conn.prepare(
        "SELECT id, display_name, server_url, username, last_synced_at, last_error, created_at
         FROM calendar_account
         ORDER BY created_at",
    )?;
    let rows = stmt
        .query_map([], CalendarAccount::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<CalendarAccount>> {
    let row = conn
        .query_row(
            "SELECT id, display_name, server_url, username, last_synced_at, last_error, created_at
             FROM calendar_account WHERE id = ?1",
            [id],
            CalendarAccount::from_row,
        )
        .optional()?;
    Ok(row)
}

pub fn update_sync_state(
    conn: &Connection,
    id: i64,
    last_synced_at: Option<i64>,
    last_error: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE calendar_account SET last_synced_at = ?1, last_error = ?2 WHERE id = ?3",
        params![last_synced_at, last_error, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM calendar_account WHERE id = ?1", [id])?;
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
    fn insert_returns_id() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "iCloud", "https://caldav.icloud.com", "a@b.c").unwrap();
        assert!(id > 0);
    }

    #[test]
    fn list_orders_by_created_at() {
        let (_d, conn) = fresh_conn();
        let a = insert(&conn, "A", "https://a.test", "a").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = insert(&conn, "B", "https://b.test", "b").unwrap();

        let rows = list(&conn).unwrap();
        let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![a, b]);
    }

    #[test]
    fn update_sync_state_persists_both_timestamp_and_error() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "iCloud", "https://caldav.icloud.com", "a@b.c").unwrap();

        update_sync_state(&conn, id, Some(1_700_000_000), None).unwrap();
        let row = get(&conn, id).unwrap().unwrap();
        assert_eq!(row.last_synced_at, Some(1_700_000_000));
        assert_eq!(row.last_error, None);

        update_sync_state(&conn, id, None, Some("bad credentials")).unwrap();
        let row = get(&conn, id).unwrap().unwrap();
        assert_eq!(row.last_synced_at, None);
        assert_eq!(row.last_error.as_deref(), Some("bad credentials"));
    }

    #[test]
    fn delete_cascades_events() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "iCloud", "https://caldav.icloud.com", "a@b.c").unwrap();
        // Insert an event directly (event DAL arrives in Task 3 — inline SQL here).
        conn.execute(
            "INSERT INTO event (calendar_account_id, external_id, title, start_at, end_at, created_at)
             VALUES (?1, 'uid-1', 'Test', 1, 2, 3)",
            [id],
        )
        .unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM event", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);

        delete(&conn, id).unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM event", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "CASCADE should have wiped the event");
    }
}
```

- [ ] **Step 4: Run tests + clippy + fmt**

```
cargo test -p manor-core --all-targets        # expect 25 + 4 new = 29 passing
cargo clippy -p manor-core --all-targets -- -D warnings
cargo fmt --all --check   # run `cargo fmt --all` if it surfaces fixes
```

- [ ] **Step 5: Commit**

```bash
git add crates/core/migrations/V3__calendar.sql crates/core/src/assistant/calendar_account.rs crates/core/src/assistant/event.rs crates/core/src/assistant/mod.rs crates/core/src/assistant/db.rs
git commit -m "feat(core): V3 migration + calendar_account DAL + FK pragma (TDD)"
```

---

### Task 3: `event` DAL (TDD)

**Files:**
- Modify: `crates/core/src/assistant/event.rs` (currently a stub from Task 2)

- [ ] **Step 1: Write the full module**

Replace `crates/core/src/assistant/event.rs` with:

```rust
//! Events — calendar entries synced from CalDAV.
//!
//! Sync strategy is wipe-and-reinsert per account (no incremental in v0.1),
//! so `insert_many` is batched and `delete_for_account` is called at the
//! start of every sync.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub id: i64,
    pub calendar_account_id: i64,
    pub external_id: String,
    pub title: String,
    pub start_at: i64,
    pub end_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    pub calendar_account_id: i64,
    pub external_id: String,
    pub title: String,
    pub start_at: i64,
    pub end_at: i64,
}

impl Event {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            calendar_account_id: row.get("calendar_account_id")?,
            external_id: row.get("external_id")?,
            title: row.get("title")?,
            start_at: row.get("start_at")?,
            end_at: row.get("end_at")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// Batch-insert events. Single transaction — all-or-nothing.
pub fn insert_many(conn: &Connection, events: &[NewEvent]) -> Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    let now_ms = Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO event (calendar_account_id, external_id, title, start_at, end_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for ev in events {
            stmt.execute(params![
                ev.calendar_account_id,
                ev.external_id,
                ev.title,
                ev.start_at,
                ev.end_at,
                now_ms,
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Events whose start_at falls in `[start_utc, end_utc)`. Ordered by start_at.
pub fn list_today(conn: &Connection, start_utc: i64, end_utc: i64) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT id, calendar_account_id, external_id, title, start_at, end_at, created_at
         FROM event
         WHERE start_at >= ?1 AND start_at < ?2
         ORDER BY start_at",
    )?;
    let rows = stmt
        .query_map(params![start_utc, end_utc], Event::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn delete_for_account(conn: &Connection, account_id: i64) -> Result<()> {
    conn.execute("DELETE FROM event WHERE calendar_account_id = ?1", [account_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::{calendar_account, db};
    use tempfile::tempdir;

    fn fresh_conn_with_account() -> (tempfile::TempDir, Connection, i64) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let id = calendar_account::insert(&conn, "iCloud", "https://caldav.icloud.com", "a@b.c").unwrap();
        (dir, conn, id)
    }

    #[test]
    fn insert_many_persists_batch() {
        let (_d, conn, aid) = fresh_conn_with_account();
        let batch = vec![
            NewEvent { calendar_account_id: aid, external_id: "u1".into(), title: "A".into(), start_at: 100, end_at: 200 },
            NewEvent { calendar_account_id: aid, external_id: "u2".into(), title: "B".into(), start_at: 300, end_at: 400 },
        ];
        insert_many(&conn, &batch).unwrap();
        let rows = list_today(&conn, 0, 1000).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn list_today_filters_by_utc_bounds() {
        let (_d, conn, aid) = fresh_conn_with_account();
        insert_many(&conn, &[
            NewEvent { calendar_account_id: aid, external_id: "u1".into(), title: "yesterday".into(), start_at: 50, end_at: 99 },
            NewEvent { calendar_account_id: aid, external_id: "u2".into(), title: "today".into(), start_at: 150, end_at: 200 },
            NewEvent { calendar_account_id: aid, external_id: "u3".into(), title: "tomorrow".into(), start_at: 500, end_at: 600 },
        ]).unwrap();
        let rows = list_today(&conn, 100, 300).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "today");
    }

    #[test]
    fn delete_for_account_scoped_to_account() {
        let (_d, conn, aid) = fresh_conn_with_account();
        let other = calendar_account::insert(&conn, "Other", "https://x.test", "x").unwrap();
        insert_many(&conn, &[
            NewEvent { calendar_account_id: aid, external_id: "a1".into(), title: "A".into(), start_at: 1, end_at: 2 },
            NewEvent { calendar_account_id: other, external_id: "o1".into(), title: "O".into(), start_at: 1, end_at: 2 },
        ]).unwrap();

        delete_for_account(&conn, aid).unwrap();

        let rows = list_today(&conn, 0, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "O");
    }

    #[test]
    fn insert_many_empty_is_noop() {
        let (_d, conn, _aid) = fresh_conn_with_account();
        insert_many(&conn, &[]).unwrap();
        let rows = list_today(&conn, 0, 1_000_000).unwrap();
        assert_eq!(rows.len(), 0);
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p manor-core --all-targets      # expect 29 + 4 new = 33 passing
cargo clippy -p manor-core --all-targets -- -D warnings
cargo fmt --all --check
```

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/assistant/event.rs
git commit -m "feat(core): event DAL — insert_many, list_today, delete_for_account (TDD)"
```

---

### Task 4: iCal parser module (TDD with fixture strings)

**Files:**
- Create: `crates/app/src/sync/mod.rs`
- Create: `crates/app/src/sync/ical.rs`
- Modify: `crates/app/src/lib.rs` (add `pub mod sync;`)

- [ ] **Step 1: Create sync module root**

Create `crates/app/src/sync/mod.rs`:

```rust
//! CalDAV + iCal sync stack.

pub mod caldav;
pub mod engine;
pub mod expand;
pub mod ical;
pub mod keychain;
```

Create empty stubs for the not-yet-written modules so this compiles:

- `crates/app/src/sync/caldav.rs`: `//! CalDAV HTTP — Task 7.`
- `crates/app/src/sync/engine.rs`: `//! Sync engine — Task 8.`
- `crates/app/src/sync/expand.rs`: `//! RRULE expansion — Task 5.`
- `crates/app/src/sync/keychain.rs`: `//! Keychain wrapper — Task 6.`

Modify `crates/app/src/lib.rs` to add `pub mod sync;` near the top (after `pub mod assistant;`).

- [ ] **Step 2: Write `ical.rs`**

Create `crates/app/src/sync/ical.rs`:

```rust
//! iCal (RFC 5545) VEVENT parsing.
//!
//! We only extract what Manor actually stores: UID, DTSTART, DTEND (or DURATION),
//! SUMMARY, and — for later recurrence expansion — RRULE and EXDATE. Everything
//! else (LOCATION, DESCRIPTION, ATTENDEE, ALARM, …) is ignored.

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use ical::parser::ical::component::IcalEvent;

/// Intermediate shape after parsing, before RRULE expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEvent {
    pub uid: String,
    pub summary: String,
    /// Start instant in UTC seconds.
    pub start_at: i64,
    /// End instant in UTC seconds.
    pub end_at: i64,
    /// RRULE string as it appears in the iCal (e.g. `FREQ=WEEKLY;BYDAY=MO`), or None.
    pub rrule: Option<String>,
    /// EXDATE values as RFC3339 UTC strings (already converted), or empty.
    pub exdates: Vec<String>,
    /// The original DTSTART string (preserved — rrule crate needs the raw context).
    pub dtstart_raw: String,
}

/// Parse a single VEVENT into a ParsedEvent.
///
/// Errors out only for events missing required properties; malformed values
/// within an otherwise-intact VEVENT produce a best-effort result.
pub fn parse_vevent(ev: &IcalEvent, local_tz: Tz) -> Result<ParsedEvent> {
    let uid = prop_value(ev, "UID").ok_or_else(|| anyhow!("VEVENT missing UID"))?;
    let summary = prop_value(ev, "SUMMARY").unwrap_or_else(|| "(no title)".to_string());

    let dtstart_prop = ev.properties.iter().find(|p| p.name == "DTSTART")
        .ok_or_else(|| anyhow!("VEVENT missing DTSTART"))?;
    let dtstart_raw = dtstart_prop.value.clone().unwrap_or_default();
    let start_at = parse_dt(dtstart_prop, local_tz)?;

    let end_at = if let Some(dtend_prop) = ev.properties.iter().find(|p| p.name == "DTEND") {
        parse_dt(dtend_prop, local_tz)?
    } else if let Some(dur) = prop_value(ev, "DURATION") {
        start_at + parse_duration_seconds(&dur)?
    } else {
        bail!("VEVENT {uid} missing both DTEND and DURATION");
    };

    let rrule = prop_value(ev, "RRULE");

    let exdates: Vec<String> = ev.properties.iter()
        .filter(|p| p.name == "EXDATE")
        .filter_map(|p| {
            let v = p.value.as_ref()?;
            parse_dt(p, local_tz).ok().map(|secs| {
                chrono::DateTime::<Utc>::from_timestamp(secs, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| v.clone())
            })
        })
        .collect();

    Ok(ParsedEvent { uid, summary, start_at, end_at, rrule, exdates, dtstart_raw })
}

fn prop_value(ev: &IcalEvent, name: &str) -> Option<String> {
    ev.properties.iter().find(|p| p.name == name).and_then(|p| p.value.clone())
}

/// Parse a DTSTART / DTEND / EXDATE value using its parameters (VALUE=DATE, TZID=...).
/// Returns the instant as UTC seconds.
fn parse_dt(prop: &ical::property::Property, local_tz: Tz) -> Result<i64> {
    let value = prop.value.as_ref().ok_or_else(|| anyhow!("{} missing value", prop.name))?;
    let params = prop.params.as_deref().unwrap_or(&[]);

    let is_date_only = params.iter().any(|(k, vals)| k == "VALUE" && vals.iter().any(|v| v == "DATE"));
    let tzid = params.iter().find_map(|(k, vals)| {
        if k == "TZID" { vals.first().cloned() } else { None }
    });

    if is_date_only {
        // YYYYMMDD — all-day, anchored to system-local midnight.
        let d = NaiveDate::parse_from_str(value, "%Y%m%d")
            .map_err(|e| anyhow!("bad DATE value {value}: {e}"))?;
        let naive = d.and_hms_opt(0, 0, 0).unwrap();
        let local_dt = local_tz.from_local_datetime(&naive).single()
            .ok_or_else(|| anyhow!("ambiguous local datetime for {value}"))?;
        return Ok(local_dt.with_timezone(&Utc).timestamp());
    }

    if value.ends_with('Z') {
        // YYYYMMDDTHHMMSSZ — UTC.
        let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ")
            .map_err(|e| anyhow!("bad UTC datetime {value}: {e}"))?;
        return Ok(Utc.from_utc_datetime(&naive).timestamp());
    }

    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
        .map_err(|e| anyhow!("bad naive datetime {value}: {e}"))?;

    let tz: Tz = match tzid {
        Some(name) => name.parse().unwrap_or(chrono_tz::UTC),
        None => local_tz,
    };
    let local_dt = tz.from_local_datetime(&naive).single()
        .ok_or_else(|| anyhow!("ambiguous local datetime for {value}"))?;
    Ok(local_dt.with_timezone(&Utc).timestamp())
}

fn parse_duration_seconds(s: &str) -> Result<i64> {
    // ISO-8601-ish: P[nD]T[nH][nM][nS]. We only handle hours/minutes/days; weeks/months rare in VEVENT.
    let mut secs: i64 = 0;
    let mut num = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() { num.push(ch); continue; }
        let n: i64 = if num.is_empty() { 0 } else { num.parse().map_err(|_| anyhow!("bad number in duration {s}"))? };
        num.clear();
        match ch {
            'W' => secs += n * 7 * 86_400,
            'D' => secs += n * 86_400,
            'H' => secs += n * 3600,
            'M' => secs += n * 60,
            'S' => secs += n,
            'P' | 'T' | '+' | '-' => {},
            _ => bail!("unexpected character {ch:?} in duration {s}"),
        }
    }
    Ok(secs)
}

/// Parse a full VCALENDAR string and return all VEVENTs that parse successfully.
/// Events that fail to parse individually are logged and skipped.
pub fn parse_vcalendar(ics: &str, local_tz: Tz) -> Vec<ParsedEvent> {
    let reader = ical::IcalParser::new(ics.as_bytes());
    let mut out = Vec::new();
    for cal_result in reader {
        let cal = match cal_result {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("skipping malformed VCALENDAR: {e}");
                continue;
            }
        };
        for ev in cal.events {
            match parse_vevent(&ev, local_tz) {
                Ok(parsed) => out.push(parsed),
                Err(e) => tracing::warn!("skipping malformed VEVENT: {e}"),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ldn() -> Tz { chrono_tz::Europe::London }

    #[test]
    fn parses_utc_dtstart() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:u1\r\nDTSTART:20260415T093000Z\r\nDTEND:20260415T103000Z\r\nSUMMARY:Boiler\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].uid, "u1");
        assert_eq!(events[0].summary, "Boiler");
        // 2026-04-15T09:30:00Z
        let expected = chrono::DateTime::parse_from_rfc3339("2026-04-15T09:30:00+00:00").unwrap().timestamp();
        assert_eq!(events[0].start_at, expected);
    }

    #[test]
    fn parses_tzid_dtstart_via_chrono_tz() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:u2\r\nDTSTART;TZID=Europe/London:20260415T093000\r\nDTEND;TZID=Europe/London:20260415T103000\r\nSUMMARY:Meeting\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        // 09:30 London in mid-April = UTC 08:30 (BST, +1)
        let expected = chrono::DateTime::parse_from_rfc3339("2026-04-15T08:30:00+00:00").unwrap().timestamp();
        assert_eq!(events[0].start_at, expected);
    }

    #[test]
    fn parses_all_day_as_midnight_local_pair() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:u3\r\nDTSTART;VALUE=DATE:20260415\r\nDTEND;VALUE=DATE:20260416\r\nSUMMARY:Birthday\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        // Midnight London = UTC 23:00 previous day (BST, +1)
        let midnight_local = chrono::DateTime::parse_from_rfc3339("2026-04-14T23:00:00+00:00").unwrap().timestamp();
        assert_eq!(events[0].start_at, midnight_local);
        assert_eq!(events[0].end_at - events[0].start_at, 86_400);
    }

    #[test]
    fn skips_malformed_vevent_others_survive() {
        // First VEVENT missing UID → skipped. Second is valid.
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nSUMMARY:NoUID\r\nDTSTART:20260415T093000Z\r\nDTEND:20260415T103000Z\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:ok\r\nDTSTART:20260415T110000Z\r\nDTEND:20260415T120000Z\r\nSUMMARY:OK\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].uid, "ok");
    }

    #[test]
    fn extracts_rrule_and_exdate() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:recurring\r\nDTSTART:20260415T093000Z\r\nDTEND:20260415T103000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=WE\r\nEXDATE:20260422T093000Z\r\nSUMMARY:Standup\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].rrule.as_deref(), Some("FREQ=WEEKLY;BYDAY=WE"));
        assert_eq!(events[0].exdates.len(), 1);
    }

    #[test]
    fn uses_duration_when_dtend_absent() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:dur\r\nDTSTART:20260415T093000Z\r\nDURATION:PT1H30M\r\nSUMMARY:Dur\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].end_at - events[0].start_at, 5400);
    }
}
```

- [ ] **Step 3: Run tests**

```
cargo test -p manor-app --all-targets    # existing 6 + 6 new = 12 passing
cargo clippy -p manor-app --all-targets -- -D warnings
cargo fmt --all --check
```

If `ical::property::Property` import path or `ical::parser::ical::component::IcalEvent` differ by crate version, check the 0.11 API and adjust — the core logic doesn't change.

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/sync crates/app/src/lib.rs
git commit -m "feat(sync): iCal parser with TZID + all-day + DURATION + RRULE extraction (TDD)"
```

---

### Task 5: RRULE expansion module (TDD)

**Files:**
- Modify: `crates/app/src/sync/expand.rs`

- [ ] **Step 1: Write `expand.rs`**

Replace `crates/app/src/sync/expand.rs` with:

```rust
//! Expand a parent VEVENT's RRULE into individual NewEvent rows within a window.

use anyhow::Result;
use chrono::{DateTime, Utc};
use manor_core::assistant::event::NewEvent;
use rrule::{RRuleSet, Tz as RruleTz};
use std::str::FromStr;

use crate::sync::ical::ParsedEvent;

/// Expand `ev` over the [window_start, window_end) range.
///
/// Non-recurring events (rrule=None) yield exactly one NewEvent with `external_id = uid`.
/// Recurring events yield one NewEvent per occurrence within the window, with
/// `external_id = "{uid}::{RFC3339-start}"` for deterministic re-sync.
pub fn expand(
    ev: &ParsedEvent,
    account_id: i64,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> Result<Vec<NewEvent>> {
    let duration = ev.end_at - ev.start_at;

    // Non-recurring: pass through if it falls inside the window.
    let Some(rrule_str) = &ev.rrule else {
        if ev.start_at >= window_start.timestamp() && ev.start_at < window_end.timestamp() {
            return Ok(vec![NewEvent {
                calendar_account_id: account_id,
                external_id: ev.uid.clone(),
                title: ev.summary.clone(),
                start_at: ev.start_at,
                end_at: ev.end_at,
            }]);
        }
        return Ok(vec![]);
    };

    // Recurring: build RRuleSet and enumerate.
    let parent_start_utc = DateTime::<Utc>::from_timestamp(ev.start_at, 0)
        .ok_or_else(|| anyhow::anyhow!("bad parent start_at"))?;

    // rrule 0.13 expects DTSTART in its own format. Compose the string it wants.
    let dtstart_line = format!("DTSTART:{}\n", parent_start_utc.format("%Y%m%dT%H%M%SZ"));
    let rule_block = format!("{dtstart_line}RRULE:{rrule_str}");
    let rset = RRuleSet::from_str(&rule_block)?;

    let window_start_rrule = window_start.with_timezone(&RruleTz::UTC);
    let window_end_rrule = window_end.with_timezone(&RruleTz::UTC);

    let occurrences = rset.all_between(window_start_rrule, window_end_rrule, true);

    let exdate_set: std::collections::HashSet<String> = ev.exdates.iter().cloned().collect();

    let out = occurrences
        .into_iter()
        .filter_map(|occ| {
            let occ_utc = occ.with_timezone(&Utc);
            let rfc = occ_utc.to_rfc3339();
            if exdate_set.contains(&rfc) {
                return None;
            }
            let start = occ_utc.timestamp();
            Some(NewEvent {
                calendar_account_id: account_id,
                external_id: format!("{}::{}", ev.uid, rfc),
                title: ev.summary.clone(),
                start_at: start,
                end_at: start + duration,
            })
        })
        .collect();

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_weekly() -> ParsedEvent {
        // Weekly event every Wednesday starting 2026-04-15 09:30 UTC
        ParsedEvent {
            uid: "weekly-1".into(),
            summary: "Standup".into(),
            start_at: Utc.with_ymd_and_hms(2026, 4, 15, 9, 30, 0).unwrap().timestamp(),
            end_at: Utc.with_ymd_and_hms(2026, 4, 15, 10, 0, 0).unwrap().timestamp(),
            rrule: Some("FREQ=WEEKLY;BYDAY=WE".into()),
            exdates: vec![],
            dtstart_raw: "20260415T093000Z".into(),
        }
    }

    #[test]
    fn non_recurring_event_yields_one_newevent_with_uid_as_external_id() {
        let ev = ParsedEvent {
            uid: "once".into(),
            summary: "Boiler".into(),
            start_at: Utc.with_ymd_and_hms(2026, 4, 15, 10, 0, 0).unwrap().timestamp(),
            end_at: Utc.with_ymd_and_hms(2026, 4, 15, 11, 0, 0).unwrap().timestamp(),
            rrule: None,
            exdates: vec![],
            dtstart_raw: "20260415T100000Z".into(),
        };
        let start = Utc.with_ymd_and_hms(2026, 4, 14, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 20, 0, 0, 0).unwrap();
        let out = expand(&ev, 1, start, end).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].external_id, "once");
    }

    #[test]
    fn expands_weekly_rrule_in_window() {
        let ev = sample_weekly();
        let start = Utc.with_ymd_and_hms(2026, 4, 8, 0, 0, 0).unwrap();   // today-7
        let end = Utc.with_ymd_and_hms(2026, 4, 29, 0, 0, 0).unwrap();    // today+14
        let out = expand(&ev, 1, start, end).unwrap();
        // Wed 2026-04-15 and Wed 2026-04-22 — two occurrences
        assert_eq!(out.len(), 2);
        assert!(out[0].external_id.starts_with("weekly-1::"));
        assert!(out[0].external_id.contains("2026-04-15T09:30:00"));
    }

    #[test]
    fn applies_exdate_exclusions() {
        let mut ev = sample_weekly();
        ev.exdates = vec!["2026-04-22T09:30:00+00:00".into()];
        let start = Utc.with_ymd_and_hms(2026, 4, 8, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 29, 0, 0, 0).unwrap();
        let out = expand(&ev, 1, start, end).unwrap();
        // Only the 2026-04-15 occurrence remains; 2026-04-22 is excluded.
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn deterministic_external_id_format() {
        let ev = sample_weekly();
        let start = Utc.with_ymd_and_hms(2026, 4, 14, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 17, 0, 0, 0).unwrap();
        let a = expand(&ev, 1, start, end).unwrap();
        let b = expand(&ev, 1, start, end).unwrap();
        assert_eq!(a[0].external_id, b[0].external_id);
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p manor-app --all-targets     # 12 + 4 new = 16 passing
cargo clippy -p manor-app --all-targets -- -D warnings
cargo fmt --all --check
```

**Expected friction:** `rrule` 0.13 API changes between minor versions. If `RRuleSet::from_str`, `all_between`, or `rrule::Tz` disagree with the code, check `cargo doc -p rrule --open` and adjust; the logic stays the same (parse parent DTSTART + RRULE, enumerate between bounds, subtract EXDATEs).

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/sync/expand.rs
git commit -m "feat(sync): RRULE expansion with window + EXDATE support (TDD)"
```

---

### Task 6: Keychain wrapper

**Files:**
- Modify: `crates/app/src/sync/keychain.rs`

- [ ] **Step 1: Write the wrapper**

Replace `crates/app/src/sync/keychain.rs` with:

```rust
//! macOS Keychain wrapper for CalDAV passwords.
//!
//! `keyring` is cross-platform but defaults to the macOS keychain on Mac.
//! Entries are keyed by (service="manor", account="caldav-{account_id}").

use anyhow::Result;
use keyring::Entry;

const SERVICE: &str = "manor";

fn account_key(account_id: i64) -> String {
    format!("caldav-{account_id}")
}

pub fn set_password(account_id: i64, password: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, &account_key(account_id))?;
    entry.set_password(password)?;
    Ok(())
}

pub fn get_password(account_id: i64) -> Result<String> {
    let entry = Entry::new(SERVICE, &account_key(account_id))?;
    Ok(entry.get_password()?)
}

/// Delete the Keychain entry. Missing entries are not an error — they're reported
/// as `Ok(false)` so callers know whether an entry actually existed.
pub fn delete_password(account_id: i64) -> Result<bool> {
    let entry = Entry::new(SERVICE, &account_key(account_id))?;
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(e.into()),
    }
}
```

- [ ] **Step 2: Verify build**

```
cargo check -p manor-app
```

No tests — Keychain calls require the real OS service, verified end-to-end via manual smoke.

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/sync/keychain.rs
git commit -m "feat(sync): Keychain wrapper for CalDAV passwords"
```

---

### Task 7: CalDAV HTTP client — PROPFIND + REPORT (TDD with wiremock)

**Files:**
- Modify: `crates/app/src/sync/caldav.rs`

- [ ] **Step 1: Write the client**

Replace `crates/app/src/sync/caldav.rs` with:

```rust
//! CalDAV HTTP client — discovery (PROPFIND) + event fetch (REPORT).

use anyhow::{anyhow, bail, Result};
use base64::Engine;
use chrono::{DateTime, Utc};
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Method;

const PROPFIND: &str = "PROPFIND";
const REPORT: &str = "REPORT";

pub struct CalDavClient {
    http: reqwest::Client,
    username: String,
    password: String,
}

#[derive(Debug, Clone)]
pub struct CalendarInfo {
    pub url: String,
    pub display_name: Option<String>,
}

impl CalDavClient {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self { http: reqwest::Client::new(), username: username.into(), password: password.into() }
    }

    fn auth_header(&self) -> HeaderValue {
        let creds = format!("{}:{}", self.username, self.password);
        let encoded = base64::engine::general_purpose::STANDARD.encode(creds.as_bytes());
        HeaderValue::from_str(&format!("Basic {encoded}")).expect("header should never be bad")
    }

    async fn request_xml(&self, method: &str, url: &str, depth: Option<&str>, body: &str) -> Result<String> {
        let method = Method::from_bytes(method.as_bytes())?;
        let mut req = self.http.request(method, url);
        req = req.header(AUTHORIZATION, self.auth_header());
        req = req.header(CONTENT_TYPE, HeaderValue::from_static("application/xml; charset=utf-8"));
        if let Some(d) = depth { req = req.header("Depth", d); }
        req = req.body(body.to_string());

        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() && status.as_u16() != 207 {
            let body = resp.text().await.unwrap_or_default();
            bail!("CalDAV {method} {url} returned {status}: {body}");
        }
        Ok(resp.text().await?)
    }

    /// Returns the current-user-principal href.
    pub async fn discover_principal(&self, server_url: &str) -> Result<String> {
        let body = r#"<?xml version="1.0"?>
<D:propfind xmlns:D="DAV:">
  <D:prop><D:current-user-principal/></D:prop>
</D:propfind>"#;
        let xml = self.request_xml(PROPFIND, server_url, Some("0"), body).await?;
        extract_first_href(&xml, "current-user-principal")
            .ok_or_else(|| anyhow!("no current-user-principal in PROPFIND response"))
            .map(|href| absolutize(server_url, &href))
    }

    /// Returns the calendar-home-set href.
    pub async fn discover_home_set(&self, principal_url: &str) -> Result<String> {
        let body = r#"<?xml version="1.0"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop><C:calendar-home-set/></D:prop>
</D:propfind>"#;
        let xml = self.request_xml(PROPFIND, principal_url, Some("0"), body).await?;
        extract_first_href(&xml, "calendar-home-set")
            .ok_or_else(|| anyhow!("no calendar-home-set in PROPFIND response"))
            .map(|href| absolutize(principal_url, &href))
    }

    /// Lists calendar collections under the home-set.
    pub async fn list_calendars(&self, home_set_url: &str) -> Result<Vec<CalendarInfo>> {
        let body = r#"<?xml version="1.0"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:displayname/>
    <D:resourcetype/>
  </D:prop>
</D:propfind>"#;
        let xml = self.request_xml(PROPFIND, home_set_url, Some("1"), body).await?;
        Ok(extract_calendar_collections(&xml, home_set_url))
    }

    /// Fetches events from a calendar URL within `[start, end)`.
    pub async fn report_events(&self, calendar_url: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<String>> {
        let start_s = start.format("%Y%m%dT%H%M%SZ").to_string();
        let end_s = end.format("%Y%m%dT%H%M%SZ").to_string();
        let body = format!(
            r#"<?xml version="1.0"?>
<C:calendar-query xmlns:C="urn:ietf:params:xml:ns:caldav" xmlns:D="DAV:">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VEVENT">
        <C:time-range start="{start_s}" end="{end_s}"/>
      </C:comp-filter>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>"#
        );
        let xml = self.request_xml(REPORT, calendar_url, Some("1"), &body).await?;
        Ok(extract_calendar_data_blocks(&xml))
    }
}

/// Parse a PROPFIND response and return the first `<D:href>` under the named prop element.
fn extract_first_href(xml: &str, prop_local_name: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut inside_prop = false;
    let mut inside_href = false;
    let mut href = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) => {
                let name = local_name(e.name().as_ref());
                if name == prop_local_name { inside_prop = true; }
                if inside_prop && name == "href" { inside_href = true; href.clear(); }
            }
            Ok(XmlEvent::Text(t)) => {
                if inside_href { href.push_str(&t.unescape().unwrap_or_default()); }
            }
            Ok(XmlEvent::End(e)) => {
                let name = local_name(e.name().as_ref());
                if name == "href" && inside_href { return Some(href.trim().to_string()); }
                if name == prop_local_name { inside_prop = false; }
            }
            Ok(XmlEvent::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Find all `<D:response>` elements that look like calendar collections (resourcetype has
/// `<C:calendar/>`) and return their href + displayname.
fn extract_calendar_collections(xml: &str, base_url: &str) -> Vec<CalendarInfo> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut in_response = false;
    let mut in_href = false;
    let mut in_resourcetype = false;
    let mut in_displayname = false;
    let mut saw_calendar_type = false;
    let mut cur_href = String::new();
    let mut cur_display = String::new();

    let mut out = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "response" => { in_response = true; saw_calendar_type = false; cur_href.clear(); cur_display.clear(); }
                    "href" if in_response => { in_href = true; cur_href.clear(); }
                    "resourcetype" => in_resourcetype = true,
                    "displayname" if in_response => { in_displayname = true; cur_display.clear(); }
                    _ => {}
                }
            }
            Ok(XmlEvent::Empty(e)) => {
                let name = local_name(e.name().as_ref());
                if in_resourcetype && name == "calendar" { saw_calendar_type = true; }
            }
            Ok(XmlEvent::Text(t)) => {
                let text = t.unescape().unwrap_or_default();
                if in_href { cur_href.push_str(&text); }
                if in_displayname { cur_display.push_str(&text); }
            }
            Ok(XmlEvent::End(e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "href" => in_href = false,
                    "resourcetype" => in_resourcetype = false,
                    "displayname" => in_displayname = false,
                    "response" => {
                        if in_response && saw_calendar_type && !cur_href.is_empty() {
                            out.push(CalendarInfo {
                                url: absolutize(base_url, cur_href.trim()),
                                display_name: if cur_display.trim().is_empty() { None } else { Some(cur_display.trim().to_string()) },
                            });
                        }
                        in_response = false;
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    out
}

/// Extract every `<C:calendar-data>` text block from a REPORT response.
fn extract_calendar_data_blocks(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut in_data = false;
    let mut cur = String::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) => {
                if local_name(e.name().as_ref()) == "calendar-data" { in_data = true; cur.clear(); }
            }
            Ok(XmlEvent::Text(t)) => {
                if in_data { cur.push_str(&t.unescape().unwrap_or_default()); }
            }
            Ok(XmlEvent::CData(t)) => {
                if in_data {
                    let s = String::from_utf8_lossy(&t);
                    cur.push_str(&s);
                }
            }
            Ok(XmlEvent::End(e)) => {
                if local_name(e.name().as_ref()) == "calendar-data" {
                    in_data = false;
                    if !cur.trim().is_empty() { out.push(cur.clone()); }
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn local_name(full: &[u8]) -> String {
    match full.iter().position(|&b| b == b':') {
        Some(i) => String::from_utf8_lossy(&full[i + 1..]).to_string(),
        None => String::from_utf8_lossy(full).to_string(),
    }
}

/// Resolve `maybe_relative` against `base`. If `maybe_relative` is already absolute
/// (starts with `http://` or `https://`), return it unchanged. If it starts with `/`,
/// take the scheme+host from `base`. Otherwise return as-is (unusual; best effort).
fn absolutize(base: &str, maybe_relative: &str) -> String {
    if maybe_relative.starts_with("http://") || maybe_relative.starts_with("https://") {
        return maybe_relative.to_string();
    }
    if maybe_relative.starts_with('/') {
        if let Ok(parsed) = reqwest::Url::parse(base) {
            return format!("{}://{}{}", parsed.scheme(), parsed.host_str().unwrap_or(""), maybe_relative);
        }
    }
    maybe_relative.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn discover_principal_parses_href() {
        let server = MockServer::start().await;
        let body = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/</D:href>
    <D:propstat><D:prop><D:current-user-principal><D:href>/12345/principal/</D:href></D:current-user-principal></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#;
        Mock::given(method("PROPFIND"))
            .and(header("Depth", "0"))
            .respond_with(ResponseTemplate::new(207).set_body_string(body).append_header("Content-Type", "application/xml"))
            .mount(&server).await;

        let client = CalDavClient::new("u", "p");
        let principal = client.discover_principal(&server.uri()).await.unwrap();
        assert!(principal.ends_with("/12345/principal/"));
    }

    #[tokio::test]
    async fn list_calendars_filters_to_calendar_resourcetype() {
        let server = MockServer::start().await;
        let body = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/home/cal1/</D:href>
    <D:propstat><D:prop><D:displayname>Work</D:displayname><D:resourcetype><D:collection/><C:calendar/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
  <D:response>
    <D:href>/home/notcal/</D:href>
    <D:propstat><D:prop><D:displayname>Inbox</D:displayname><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#;
        Mock::given(method("PROPFIND"))
            .and(header("Depth", "1"))
            .respond_with(ResponseTemplate::new(207).set_body_string(body))
            .mount(&server).await;

        let client = CalDavClient::new("u", "p");
        let cals = client.list_calendars(&format!("{}/home/", server.uri())).await.unwrap();
        assert_eq!(cals.len(), 1);
        assert!(cals[0].url.ends_with("/home/cal1/"));
        assert_eq!(cals[0].display_name.as_deref(), Some("Work"));
    }

    #[tokio::test]
    async fn report_events_extracts_calendar_data_blocks() {
        let server = MockServer::start().await;
        let body = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/home/cal1/1.ics</D:href>
    <D:propstat><D:prop>
      <C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:u1
DTSTART:20260415T093000Z
DTEND:20260415T103000Z
SUMMARY:Boiler
END:VEVENT
END:VCALENDAR</C:calendar-data>
    </D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#;
        Mock::given(method("REPORT"))
            .respond_with(ResponseTemplate::new(207).set_body_string(body))
            .mount(&server).await;

        let client = CalDavClient::new("u", "p");
        let start = Utc::now() - chrono::Duration::days(7);
        let end = Utc::now() + chrono::Duration::days(14);
        let ics = client.report_events(&format!("{}/home/cal1/", server.uri()), start, end).await.unwrap();
        assert_eq!(ics.len(), 1);
        assert!(ics[0].contains("UID:u1"));
    }

    #[tokio::test]
    async fn authorization_header_present() {
        let server = MockServer::start().await;
        Mock::given(method("PROPFIND"))
            .and(header("Authorization", "Basic dTpw")) // base64("u:p") = "dTpw"
            .respond_with(ResponseTemplate::new(207).set_body_string(
                r#"<?xml version="1.0"?><D:multistatus xmlns:D="DAV:"><D:response><D:href>/</D:href><D:propstat><D:prop><D:current-user-principal><D:href>/p/</D:href></D:current-user-principal></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response></D:multistatus>"#))
            .mount(&server).await;

        let client = CalDavClient::new("u", "p");
        client.discover_principal(&server.uri()).await.unwrap();
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p manor-app --all-targets    # 16 + 4 new = 20 passing
cargo clippy -p manor-app --all-targets -- -D warnings
cargo fmt --all --check
```

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/sync/caldav.rs
git commit -m "feat(sync): CalDAV HTTP client — PROPFIND discovery + REPORT event fetch (TDD)"
```

---

### Task 8: Sync engine orchestrator (TDD with wiremock)

**Files:**
- Modify: `crates/app/src/sync/engine.rs`

- [ ] **Step 1: Write the engine**

Replace `crates/app/src/sync/engine.rs` with:

```rust
//! Sync engine: orchestrates CalDAV fetch → iCal parse → RRULE expand → DB wipe-and-reinsert.

use anyhow::Result;
use chrono::{Duration, Utc};
use chrono_tz::Tz;
use manor_core::assistant::{calendar_account, event};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;

use crate::sync::caldav::CalDavClient;
use crate::sync::ical;
use crate::sync::expand;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub account_id: i64,
    pub events_added: u32,
    pub error: Option<String>,
    pub synced_at: i64,
}

/// In-memory set of currently-syncing account ids, behind a Mutex. Owned by Tauri state.
pub struct SyncState {
    in_flight: Mutex<HashSet<i64>>,
}

impl SyncState {
    pub fn new() -> Self { Self { in_flight: Mutex::new(HashSet::new()) } }

    fn try_begin(&self, id: i64) -> bool {
        let mut set = self.in_flight.lock().unwrap();
        set.insert(id)
    }

    fn end(&self, id: i64) {
        let mut set = self.in_flight.lock().unwrap();
        set.remove(&id);
    }
}

impl Default for SyncState {
    fn default() -> Self { Self::new() }
}

/// Map a fetch/parse error to a short user-facing string + full context for last_error.
fn error_string(err: &anyhow::Error) -> String {
    let msg = err.to_string();
    if msg.contains("401") || msg.contains("403") { "bad credentials".into() }
    else if msg.contains("404") { "URL not found".into() }
    else if msg.contains("connect error") || msg.contains("error sending request") { "server unreachable".into() }
    else if msg.contains("no current-user-principal") || msg.contains("no calendar-home-set") { "discovery failed".into() }
    else { format!("sync failed: {msg}") }
}

pub async fn sync_account(
    conn: &mut Connection,
    sync_state: &SyncState,
    account_id: i64,
    password: &str,
    local_tz: Tz,
) -> SyncResult {
    let now_secs = Utc::now().timestamp();

    if !sync_state.try_begin(account_id) {
        return SyncResult {
            account_id,
            events_added: 0,
            error: Some("already syncing".into()),
            synced_at: now_secs,
        };
    }

    let result = do_sync(conn, account_id, password, local_tz).await;
    sync_state.end(account_id);

    match result {
        Ok(added) => {
            let _ = calendar_account::update_sync_state(conn, account_id, Some(now_secs), None);
            SyncResult { account_id, events_added: added, error: None, synced_at: now_secs }
        }
        Err(e) => {
            let short = error_string(&e);
            // Full message goes into last_error; the short form is returned to the frontend.
            let _ = calendar_account::update_sync_state(conn, account_id, None, Some(&short));
            SyncResult { account_id, events_added: 0, error: Some(short), synced_at: now_secs }
        }
    }
}

async fn do_sync(
    conn: &mut Connection,
    account_id: i64,
    password: &str,
    local_tz: Tz,
) -> Result<u32> {
    let account = calendar_account::get(conn, account_id)?
        .ok_or_else(|| anyhow::anyhow!("account {account_id} not found"))?;

    let client = CalDavClient::new(&account.username, password);
    let principal = client.discover_principal(&account.server_url).await?;
    let home_set = client.discover_home_set(&principal).await?;
    let calendars = client.list_calendars(&home_set).await?;

    let window_start = Utc::now() - Duration::days(7);
    let window_end = Utc::now() + Duration::days(14);

    let mut new_events: Vec<event::NewEvent> = Vec::new();
    for cal in &calendars {
        let blocks = client.report_events(&cal.url, window_start, window_end).await?;
        for ics in blocks {
            for parsed in ical::parse_vcalendar(&ics, local_tz) {
                match expand::expand(&parsed, account_id, window_start, window_end) {
                    Ok(mut occurrences) => new_events.append(&mut occurrences),
                    Err(e) => tracing::warn!("skipping expansion for uid {}: {e}", parsed.uid),
                }
            }
        }
    }

    let tx = conn.unchecked_transaction()?;
    event::delete_for_account(&tx, account_id)?;
    // insert_many uses its own transaction on `conn`, but here we commit via `tx` — inline the INSERTs.
    {
        let mut stmt = tx.prepare(
            "INSERT INTO event (calendar_account_id, external_id, title, start_at, end_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        let created_at = Utc::now().timestamp_millis();
        for ev in &new_events {
            // Skip duplicates silently (UNIQUE constraint protects us regardless).
            let _ = stmt.execute(rusqlite::params![
                ev.calendar_account_id,
                ev.external_id,
                ev.title,
                ev.start_at,
                ev.end_at,
                created_at,
            ]);
        }
    }
    tx.commit()?;

    Ok(new_events.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::assistant::db;
    use tempfile::tempdir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn propfind_body_principal(server_uri: &str) -> String {
        format!(r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/</D:href>
    <D:propstat><D:prop><D:current-user-principal><D:href>{server_uri}/principal/</D:href></D:current-user-principal></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#)
    }

    fn propfind_body_home_set(server_uri: &str) -> String {
        format!(r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>{server_uri}/principal/</D:href>
    <D:propstat><D:prop><C:calendar-home-set><D:href>{server_uri}/home/</D:href></C:calendar-home-set></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#)
    }

    fn propfind_body_calendars(server_uri: &str) -> String {
        format!(r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>{server_uri}/home/cal1/</D:href>
    <D:propstat><D:prop><D:displayname>Work</D:displayname><D:resourcetype><D:collection/><C:calendar/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#)
    }

    fn report_body_three_events() -> String {
        r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response><D:href>/home/cal1/1.ics</D:href><D:propstat><D:prop><C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:one
DTSTART:20260415T100000Z
DTEND:20260415T110000Z
SUMMARY:One
END:VEVENT
END:VCALENDAR</C:calendar-data></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>
  <D:response><D:href>/home/cal1/2.ics</D:href><D:propstat><D:prop><C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:two
DTSTART:20260416T120000Z
DTEND:20260416T130000Z
SUMMARY:Two
END:VEVENT
END:VCALENDAR</C:calendar-data></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>
</D:multistatus>"#.to_string()
    }

    async fn mount_happy_path(server: &MockServer) {
        let uri = server.uri();
        Mock::given(method("PROPFIND")).and(path("/"))
            .respond_with(ResponseTemplate::new(207).set_body_string(propfind_body_principal(&uri)))
            .mount(server).await;
        Mock::given(method("PROPFIND")).and(path("/principal/"))
            .respond_with(ResponseTemplate::new(207).set_body_string(propfind_body_home_set(&uri)))
            .mount(server).await;
        Mock::given(method("PROPFIND")).and(path("/home/"))
            .respond_with(ResponseTemplate::new(207).set_body_string(propfind_body_calendars(&uri)))
            .mount(server).await;
        Mock::given(method("REPORT")).and(path("/home/cal1/"))
            .respond_with(ResponseTemplate::new(207).set_body_string(report_body_three_events()))
            .mount(server).await;
    }

    #[tokio::test]
    async fn sync_account_happy_path_with_mock_caldav() {
        let server = MockServer::start().await;
        mount_happy_path(&server).await;

        let dir = tempdir().unwrap();
        let mut conn = db::init(&dir.path().join("t.db")).unwrap();
        let aid = calendar_account::insert(&conn, "Mock", &server.uri(), "u").unwrap();
        let state = SyncState::new();

        let result = sync_account(&mut conn, &state, aid, "p", chrono_tz::UTC).await;
        assert_eq!(result.error, None, "expected no error, got {:?}", result.error);
        assert_eq!(result.events_added, 2);

        let row = calendar_account::get(&conn, aid).unwrap().unwrap();
        assert!(row.last_synced_at.is_some());
        assert_eq!(row.last_error, None);
    }

    #[tokio::test]
    async fn sync_account_401_sets_bad_credentials() {
        let server = MockServer::start().await;
        Mock::given(method("PROPFIND"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server).await;

        let dir = tempdir().unwrap();
        let mut conn = db::init(&dir.path().join("t.db")).unwrap();
        let aid = calendar_account::insert(&conn, "Mock", &server.uri(), "u").unwrap();

        let result = sync_account(&mut conn, &SyncState::new(), aid, "p", chrono_tz::UTC).await;
        assert_eq!(result.error.as_deref(), Some("bad credentials"));

        let row = calendar_account::get(&conn, aid).unwrap().unwrap();
        assert_eq!(row.last_error.as_deref(), Some("bad credentials"));
        assert_eq!(row.last_synced_at, None);
    }

    #[tokio::test]
    async fn sync_account_network_unreachable() {
        let dir = tempdir().unwrap();
        let mut conn = db::init(&dir.path().join("t.db")).unwrap();
        let aid = calendar_account::insert(&conn, "Mock", "http://127.0.0.1:1", "u").unwrap();

        let result = sync_account(&mut conn, &SyncState::new(), aid, "p", chrono_tz::UTC).await;
        assert!(result.error.is_some());
        // Error should map to "server unreachable" — allow tolerance in string matching.
        let msg = result.error.unwrap();
        assert!(msg == "server unreachable" || msg.contains("unreachable") || msg.contains("connect"));
    }

    #[tokio::test]
    async fn double_sync_second_returns_already_syncing() {
        let server = MockServer::start().await;
        mount_happy_path(&server).await;

        let dir = tempdir().unwrap();
        let mut conn = db::init(&dir.path().join("t.db")).unwrap();
        let aid = calendar_account::insert(&conn, "Mock", &server.uri(), "u").unwrap();

        let state = SyncState::new();
        state.try_begin(aid);   // pre-mark as in-flight

        let result = sync_account(&mut conn, &state, aid, "p", chrono_tz::UTC).await;
        assert_eq!(result.error.as_deref(), Some("already syncing"));
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p manor-app --all-targets    # 20 + 4 new = 24 passing
cargo clippy -p manor-app --all-targets -- -D warnings
cargo fmt --all --check
```

The wiremock sync tests are slow-ish (each starts a local HTTP server); budget ~10–20s total for the task.

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/sync/engine.rs
git commit -m "feat(sync): engine — fetch/parse/expand orchestrator + SyncState concurrency guard (TDD)"
```

---

### Task 9: `SyncState` + 6 new Tauri commands + app-start sync

**Files:**
- Modify: `crates/app/src/assistant/commands.rs`
- Modify: `crates/app/src/lib.rs`

- [ ] **Step 1: Add commands in `commands.rs`**

Add the following block to `crates/app/src/assistant/commands.rs` (alongside existing commands). Imports to add at the top:

```rust
use crate::sync::engine::{SyncResult, SyncState};
use crate::sync::keychain;
use manor_core::assistant::{
    calendar_account::{self, CalendarAccount},
    event::{self, Event},
};
```

Commands to append at the end:

```rust
fn today_utc_bounds() -> (i64, i64) {
    let now = Local::now();
    let start_local = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).unwrap();
    let end_local = start_local + chrono::Duration::days(1);
    (
        start_local.with_timezone(&chrono::Utc).timestamp(),
        end_local.with_timezone(&chrono::Utc).timestamp(),
    )
}

fn display_name_for_url(url: &str) -> String {
    if url.contains("caldav.icloud.com") { "iCloud".into() }
    else if url.contains("fastmail") { "Fastmail".into() }
    else { reqwest::Url::parse(url).ok().and_then(|u| u.host_str().map(|s| s.to_string())).unwrap_or_else(|| url.to_string()) }
}

#[tauri::command]
pub fn list_calendar_accounts(state: tauri::State<'_, Db>) -> Result<Vec<CalendarAccount>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    calendar_account::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_calendar_account(
    db: tauri::State<'_, Db>,
    sync_state: tauri::State<'_, SyncState>,
    server_url: String,
    username: String,
    password: String,
) -> Result<CalendarAccount, String> {
    let display_name = display_name_for_url(&server_url);
    let account = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let id = calendar_account::insert(&conn, &display_name, &server_url, &username).map_err(|e| e.to_string())?;
        keychain::set_password(id, &password).map_err(|e| format!("keychain: {e}"))?;
        calendar_account::get(&conn, id).map_err(|e| e.to_string())?.ok_or_else(|| "just-inserted row not found".to_string())?
    };

    // Kick off first sync (don't block the command on it — spawn a task).
    let account_id = account.id;
    let password_for_sync = password;
    let db_arc = db.inner().clone_arc();
    let sync_state_arc = sync_state.inner().clone_arc();
    tokio::spawn(async move {
        let mut conn_guard = db_arc.0.lock().unwrap();
        let _ = crate::sync::engine::sync_account(&mut conn_guard, sync_state_arc.as_ref(), account_id, &password_for_sync, chrono_tz::UTC).await;
    });

    Ok(account)
}

#[tauri::command]
pub fn remove_calendar_account(
    db: tauri::State<'_, Db>,
    id: i64,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    calendar_account::delete(&conn, id).map_err(|e| e.to_string())?;
    let _ = keychain::delete_password(id);
    Ok(())
}

#[tauri::command]
pub async fn sync_account(
    db: tauri::State<'_, Db>,
    sync_state: tauri::State<'_, SyncState>,
    id: i64,
) -> Result<SyncResult, String> {
    let password = keychain::get_password(id).map_err(|e| format!("keychain: {e}"))?;
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::sync::engine::sync_account(&mut conn, sync_state.inner(), id, &password, chrono_tz::UTC).await)
}

#[tauri::command]
pub async fn sync_all_accounts(
    db: tauri::State<'_, Db>,
    sync_state: tauri::State<'_, SyncState>,
) -> Result<Vec<SyncResult>, String> {
    let ids: Vec<i64> = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        calendar_account::list(&conn).map_err(|e| e.to_string())?.into_iter().map(|a| a.id).collect()
    };
    let mut results = Vec::with_capacity(ids.len());
    for id in ids {
        let Ok(password) = keychain::get_password(id) else { continue; };
        let mut conn = db.0.lock().map_err(|e| e.to_string())?;
        results.push(crate::sync::engine::sync_account(&mut conn, sync_state.inner(), id, &password, chrono_tz::UTC).await);
    }
    Ok(results)
}

#[tauri::command]
pub fn list_events_today(state: tauri::State<'_, Db>) -> Result<Vec<Event>, String> {
    let (start, end) = today_utc_bounds();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    event::list_today(&conn, start, end).map_err(|e| e.to_string())
}
```

**NOTE — `Db` lifetime for the spawned task:** `tauri::State<'_, Db>` can't cross an `await` in a spawned task if `Db` holds a `Mutex<Connection>` directly. Wrap `Db` in an `Arc` — modify the `Db` struct definition in this same file:

```rust
pub struct Db(pub std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>);

impl Db {
    pub fn open(path: std::path::PathBuf) -> anyhow::Result<Self> {
        let conn = manor_core::assistant::db::init(&path)?;
        Ok(Self(std::sync::Arc::new(std::sync::Mutex::new(conn))))
    }
    pub fn clone_arc(&self) -> std::sync::Arc<std::sync::Mutex<rusqlite::Connection>> { self.0.clone() }
}
```

Every existing reference like `state.0.lock()` continues to work (`Arc<Mutex<_>>` implements `.lock()`). Add a `clone_arc` helper to `SyncState` too so it can be shared with the spawned task:

```rust
// in engine.rs, alongside SyncState:
impl SyncState {
    pub fn arc() -> std::sync::Arc<Self> { std::sync::Arc::new(Self::new()) }
}

// and in commands.rs, register SyncState via Arc so inner().clone_arc() works — see lib.rs changes below.
```

Actually simpler: make `SyncState` itself behind `Arc` in the Tauri managed state (register `Arc<SyncState>` instead of `SyncState`), then `.inner().clone()` gives a clone of the `Arc`. Replace the `clone_arc()` references in `add_calendar_account` accordingly — the pattern is:

```rust
let sync_state_arc: std::sync::Arc<SyncState> = sync_state.inner().clone();
let db_arc: std::sync::Arc<std::sync::Mutex<rusqlite::Connection>> = db.inner().clone_arc();
```

- [ ] **Step 2: Wire into `lib.rs`**

Read current `crates/app/src/lib.rs`, then:

1. Import `SyncState` near the top: `use crate::sync::engine::SyncState;`
2. In `register()`, inside the `setup()` closure, after `app.manage(db);` also add:
   ```rust
   let sync_state: std::sync::Arc<SyncState> = std::sync::Arc::new(SyncState::new());
   app.manage(sync_state.clone());

   // Kick off background app-start sync of all existing accounts.
   let sync_state_for_start = sync_state.clone();
   let db_arc = app.state::<assistant::commands::Db>().inner().clone_arc();
   tauri::async_runtime::spawn(async move {
       let ids: Vec<i64> = {
           let conn = db_arc.lock().unwrap();
           manor_core::assistant::calendar_account::list(&conn).unwrap_or_default().into_iter().map(|a| a.id).collect()
       };
       for id in ids {
           let Ok(password) = crate::sync::keychain::get_password(id) else { continue; };
           let mut conn = db_arc.lock().unwrap();
           let _ = crate::sync::engine::sync_account(&mut conn, sync_state_for_start.as_ref(), id, &password, chrono_tz::UTC).await;
       }
   });
   ```
3. Extend `tauri::generate_handler!` to include the 6 new commands:
   - `assistant::commands::list_calendar_accounts`
   - `assistant::commands::add_calendar_account`
   - `assistant::commands::remove_calendar_account`
   - `assistant::commands::sync_account`
   - `assistant::commands::sync_all_accounts`
   - `assistant::commands::list_events_today`

- [ ] **Step 3: Verify build + tests**

```
cargo test --workspace --all-targets     # all 33 core + 24 app = 57 passing
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo check -p manor-desktop
```

Expect friction: the Arc/Mutex ownership dance through Tauri's `State<'_, T>` may need a couple iterations. If the spawned-task lifetimes refuse to compile, the safe fallback is to make `add_calendar_account` NOT spawn a follow-up sync — the frontend can just call `sync_account(id)` right after a successful add. Drop the spawn, keep everything synchronous. Document this if you go that route.

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/assistant/commands.rs crates/app/src/lib.rs
git commit -m "feat(app): calendar-account + sync + events commands + app-start sync"
```

---

### Task 10: Frontend foundations — `useSettingsStore` + settings IPC + `useTodayStore.events` (TDD)

**Files:**
- Create: `apps/desktop/src/lib/settings/state.ts`
- Create: `apps/desktop/src/lib/settings/state.test.ts`
- Create: `apps/desktop/src/lib/settings/ipc.ts`
- Modify: `apps/desktop/src/lib/today/state.ts` (add events slice)
- Modify: `apps/desktop/src/lib/today/state.test.ts` (1 new test)
- Modify: `apps/desktop/src/lib/today/ipc.ts` (add `listEventsToday`, `Event` type)

- [ ] **Step 1: settings/ipc.ts**

Create `apps/desktop/src/lib/settings/ipc.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export interface CalendarAccount {
  id: number;
  display_name: string;
  server_url: string;
  username: string;
  last_synced_at: number | null;
  last_error: string | null;
  created_at: number;
}

export interface SyncResult {
  account_id: number;
  events_added: number;
  error: string | null;
  synced_at: number;
}

export async function listCalendarAccounts(): Promise<CalendarAccount[]> {
  return invoke<CalendarAccount[]>("list_calendar_accounts");
}

export async function addCalendarAccount(
  serverUrl: string,
  username: string,
  password: string,
): Promise<CalendarAccount> {
  return invoke<CalendarAccount>("add_calendar_account", { serverUrl, username, password });
}

export async function removeCalendarAccount(id: number): Promise<void> {
  return invoke<void>("remove_calendar_account", { id });
}

export async function syncAccount(id: number): Promise<SyncResult> {
  return invoke<SyncResult>("sync_account", { id });
}

export async function syncAllAccounts(): Promise<SyncResult[]> {
  return invoke<SyncResult[]>("sync_all_accounts");
}
```

- [ ] **Step 2: settings/state.test.ts**

Create `apps/desktop/src/lib/settings/state.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useSettingsStore } from "./state";
import type { CalendarAccount } from "./ipc";

const sampleAccount = (overrides: Partial<CalendarAccount> = {}): CalendarAccount => ({
  id: 1,
  display_name: "iCloud",
  server_url: "https://caldav.icloud.com",
  username: "a@b.c",
  last_synced_at: null,
  last_error: null,
  created_at: Date.now(),
  ...overrides,
});

describe("useSettingsStore", () => {
  beforeEach(() => useSettingsStore.setState(useSettingsStore.getInitialState(), true));

  it("modal is closed by default", () => {
    expect(useSettingsStore.getState().modalOpen).toBe(false);
  });

  it("setModalOpen toggles", () => {
    useSettingsStore.getState().setModalOpen(true);
    expect(useSettingsStore.getState().modalOpen).toBe(true);
    useSettingsStore.getState().setModalOpen(false);
    expect(useSettingsStore.getState().modalOpen).toBe(false);
  });

  it("setAccounts replaces", () => {
    const a = sampleAccount({ id: 1 });
    const b = sampleAccount({ id: 2 });
    useSettingsStore.getState().setAccounts([a, b]);
    expect(useSettingsStore.getState().accounts).toEqual([a, b]);
  });

  it("upsertAccount replaces existing or appends new", () => {
    const a = sampleAccount({ id: 1, display_name: "old" });
    const aPrime = sampleAccount({ id: 1, display_name: "new" });
    const b = sampleAccount({ id: 2 });
    useSettingsStore.getState().setAccounts([a]);
    useSettingsStore.getState().upsertAccount(aPrime);
    expect(useSettingsStore.getState().accounts[0].display_name).toBe("new");
    useSettingsStore.getState().upsertAccount(b);
    expect(useSettingsStore.getState().accounts).toHaveLength(2);
  });

  it("removeAccount drops by id", () => {
    const a = sampleAccount({ id: 1 });
    const b = sampleAccount({ id: 2 });
    useSettingsStore.getState().setAccounts([a, b]);
    useSettingsStore.getState().removeAccount(1);
    expect(useSettingsStore.getState().accounts).toEqual([b]);
  });

  it("markSyncing and markSynced update the set", () => {
    useSettingsStore.getState().markSyncing(42);
    expect(useSettingsStore.getState().syncingAccountIds.has(42)).toBe(true);
    useSettingsStore.getState().markSynced(42);
    expect(useSettingsStore.getState().syncingAccountIds.has(42)).toBe(false);
  });
});
```

- [ ] **Step 3: settings/state.ts**

Create `apps/desktop/src/lib/settings/state.ts`:

```ts
import { create } from "zustand";
import type { CalendarAccount } from "./ipc";

interface SettingsStore {
  modalOpen: boolean;
  activeTab: "calendars" | "ai" | "about";
  accounts: CalendarAccount[];
  syncingAccountIds: Set<number>;

  setModalOpen: (open: boolean) => void;
  setActiveTab: (t: SettingsStore["activeTab"]) => void;
  setAccounts: (a: CalendarAccount[]) => void;
  upsertAccount: (a: CalendarAccount) => void;
  removeAccount: (id: number) => void;
  markSyncing: (id: number) => void;
  markSynced: (id: number) => void;
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  modalOpen: false,
  activeTab: "calendars",
  accounts: [],
  syncingAccountIds: new Set<number>(),

  setModalOpen: (open) => set({ modalOpen: open }),
  setActiveTab: (t) => set({ activeTab: t }),
  setAccounts: (a) => set({ accounts: a }),

  upsertAccount: (a) =>
    set((st) => {
      const idx = st.accounts.findIndex((x) => x.id === a.id);
      if (idx === -1) return { accounts: [...st.accounts, a] };
      const next = st.accounts.slice();
      next[idx] = a;
      return { accounts: next };
    }),

  removeAccount: (id) => set((st) => ({ accounts: st.accounts.filter((x) => x.id !== id) })),

  markSyncing: (id) =>
    set((st) => {
      const next = new Set(st.syncingAccountIds);
      next.add(id);
      return { syncingAccountIds: next };
    }),

  markSynced: (id) =>
    set((st) => {
      const next = new Set(st.syncingAccountIds);
      next.delete(id);
      return { syncingAccountIds: next };
    }),
}));
```

- [ ] **Step 4: Extend today store with events**

Edit `apps/desktop/src/lib/today/state.ts`. Add these to the `TodayStore` interface between existing slices:

```ts
events: Event[];
setEvents: (e: Event[]) => void;
```

Import the `Event` type at the top:

```ts
import type { Task, Proposal, Event } from "./ipc";
```

And in the store implementation, add:

```ts
events: [],
setEvents: (e) => set({ events: e }),
```

Edit `apps/desktop/src/lib/today/ipc.ts` — add the `Event` type + `listEventsToday`:

```ts
export interface Event {
  id: number;
  calendar_account_id: number;
  external_id: string;
  title: string;
  start_at: number;
  end_at: number;
  created_at: number;
}

export async function listEventsToday(): Promise<Event[]> {
  return invoke<Event[]>("list_events_today");
}
```

Edit `apps/desktop/src/lib/today/state.test.ts` — add one test at the bottom of the describe block:

```ts
  it("setEvents replaces the array", () => {
    const e = { id: 1, calendar_account_id: 1, external_id: "u1", title: "Test", start_at: 1, end_at: 2, created_at: 3 };
    useTodayStore.getState().setEvents([e]);
    expect(useTodayStore.getState().events).toEqual([e]);
  });
```

- [ ] **Step 5: Run tests**

```
pnpm --filter manor-desktop test    # existing 20 + 6 settings + 1 today = 27 passing
pnpm tsc
```

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/lib/settings apps/desktop/src/lib/today
git commit -m "feat(desktop): settings store + today events slice (TDD)"
```

---

### Task 11: `SettingsModal` shell + `Tabs` + animation + `⌘,` hotkey + mount in App

**Files:**
- Create: `apps/desktop/src/components/Settings/SettingsModal.tsx`
- Create: `apps/desktop/src/components/Settings/Tabs.tsx`
- Modify: `apps/desktop/src/styles.css` (append `settingsIn` keyframe)
- Modify: `apps/desktop/src/App.tsx` (mount modal + ⌘, hotkey)

- [ ] **Step 1: Append `settingsIn` keyframe to styles.css**

```css
@keyframes settingsIn {
  from { opacity: 0; transform: scale(0.97); }
  to   { opacity: 1; transform: scale(1); }
}
```

- [ ] **Step 2: Create `Tabs.tsx`**

```tsx
import { useSettingsStore } from "../../lib/settings/state";

type Tab = { id: "calendars" | "ai" | "about"; label: string; disabled: boolean };

const TABS: Tab[] = [
  { id: "calendars", label: "Calendars", disabled: false },
  { id: "ai", label: "AI (soon)", disabled: true },
  { id: "about", label: "About (soon)", disabled: true },
];

export default function Tabs() {
  const activeTab = useSettingsStore((s) => s.activeTab);
  const setActiveTab = useSettingsStore((s) => s.setActiveTab);

  return (
    <div style={{ display: "flex", gap: 2, borderBottom: "1px solid var(--hairline)", padding: "0 14px" }}>
      {TABS.map((t) => {
        const active = activeTab === t.id;
        return (
          <button
            key={t.id}
            onClick={() => !t.disabled && setActiveTab(t.id)}
            disabled={t.disabled}
            style={{
              padding: "10px 14px",
              fontSize: 13,
              fontWeight: active ? 700 : 500,
              background: "transparent",
              border: "none",
              borderBottom: active ? "2px solid var(--imessage-blue)" : "2px solid transparent",
              color: t.disabled ? "rgba(0,0,0,0.3)" : "var(--ink)",
              fontStyle: t.disabled ? "italic" : "normal",
              cursor: t.disabled ? "default" : "pointer",
              fontFamily: "inherit",
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

- [ ] **Step 3: Create `SettingsModal.tsx`**

```tsx
import { useEffect, useRef } from "react";
import { useSettingsStore } from "../../lib/settings/state";
import Tabs from "./Tabs";
import CalendarsTab from "./CalendarsTab";

export default function SettingsModal() {
  const modalOpen = useSettingsStore((s) => s.modalOpen);
  const setModalOpen = useSettingsStore((s) => s.setModalOpen);
  const activeTab = useSettingsStore((s) => s.activeTab);
  const modalRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!modalOpen) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") setModalOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [modalOpen, setModalOpen]);

  useEffect(() => {
    if (modalOpen) modalRef.current?.focus();
  }, [modalOpen]);

  if (!modalOpen) return null;

  return (
    <>
      <div
        onClick={() => setModalOpen(false)}
        style={{
          position: "fixed", inset: 0, background: "rgba(0,0,0,0.25)",
          backdropFilter: "blur(2px)", zIndex: 1200,
        }}
      />
      <div
        ref={modalRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        style={{
          position: "fixed", left: "50%", top: "50%",
          transform: "translate(-50%, -50%)",
          width: 540, height: 440,
          background: "var(--paper)",
          borderRadius: 14,
          boxShadow: "var(--shadow-lg)",
          zIndex: 1201,
          display: "flex", flexDirection: "column",
          animation: "settingsIn 200ms ease-out",
          outline: "none",
        }}
      >
        <header
          style={{
            padding: "12px 16px",
            borderBottom: "1px solid var(--hairline)",
            display: "flex", alignItems: "center", justifyContent: "space-between",
            fontWeight: 700, fontSize: 14,
          }}
        >
          <span>Settings</span>
          <button
            onClick={() => setModalOpen(false)}
            aria-label="Close"
            style={{
              width: 22, height: 22, borderRadius: "50%",
              background: "rgba(0,0,0,0.06)",
              border: "none",
              fontSize: 14, lineHeight: 1, cursor: "pointer",
              color: "rgba(0,0,0,0.55)",
              display: "flex", alignItems: "center", justifyContent: "center",
            }}
          >
            ×
          </button>
        </header>
        <Tabs />
        <div style={{ flex: 1, overflowY: "auto" }}>
          {activeTab === "calendars" && <CalendarsTab />}
        </div>
      </div>
    </>
  );
}
```

- [ ] **Step 4: Modify App.tsx to add ⌘, hotkey + mount modal**

Edit `apps/desktop/src/App.tsx`:

```tsx
import { useEffect } from "react";
import Assistant from "./components/Assistant/Assistant";
import Today from "./components/Today/Today";
import SettingsModal from "./components/Settings/SettingsModal";
import { useSettingsStore } from "./lib/settings/state";

export default function App() {
  const setModalOpen = useSettingsStore((s) => s.setModalOpen);
  const modalOpen = useSettingsStore((s) => s.modalOpen);

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
      <Today />
      <Assistant />
      <SettingsModal />
    </>
  );
}
```

- [ ] **Step 5: Create `CalendarsTab.tsx` as a placeholder**

(Full content lands in Task 15; stub now so SettingsModal compiles.)

```tsx
export default function CalendarsTab() {
  return (
    <div style={{ padding: 16 }}>
      <p style={{ color: "rgba(0,0,0,0.5)", fontSize: 13 }}>Calendars tab — landing in Task 15.</p>
    </div>
  );
}
```

- [ ] **Step 6: Verify + commit**

```
pnpm tsc
pnpm --filter manor-desktop test
```

```bash
git add apps/desktop/src/components/Settings apps/desktop/src/styles.css apps/desktop/src/App.tsx
git commit -m "feat(settings): modal shell + tabs + keyframe + cmd-comma hotkey + CalendarsTab stub"
```

---

### Task 12: `SettingsCog` + wire into HeaderCard

**Files:**
- Create: `apps/desktop/src/components/Settings/SettingsCog.tsx`
- Modify: `apps/desktop/src/components/Today/HeaderCard.tsx`

- [ ] **Step 1: SettingsCog**

```tsx
import { useSettingsStore } from "../../lib/settings/state";

export default function SettingsCog() {
  const setModalOpen = useSettingsStore((s) => s.setModalOpen);

  return (
    <button
      onClick={() => setModalOpen(true)}
      aria-label="Settings"
      title="Settings (⌘,)"
      style={{
        width: 18, height: 18,
        padding: 0,
        background: "transparent",
        border: "none",
        cursor: "pointer",
        opacity: 0.6,
        transition: "opacity 100ms ease",
        fontSize: 15,
        lineHeight: 1,
      }}
      onMouseEnter={(e) => ((e.currentTarget as HTMLButtonElement).style.opacity = "1")}
      onMouseLeave={(e) => ((e.currentTarget as HTMLButtonElement).style.opacity = "0.6")}
    >
      ⚙
    </button>
  );
}
```

- [ ] **Step 2: Modify HeaderCard.tsx**

Read the current file, then add `import SettingsCog from "../Settings/SettingsCog";` at the top. Modify the right-side block (currently just the clock badge) to wrap the clock + cog in a horizontal flex:

```tsx
<div style={{ display: "flex", alignItems: "center", gap: 10 }}>
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
  <SettingsCog />
</div>
```

- [ ] **Step 3: Verify + commit**

```
pnpm tsc
```

```bash
git add apps/desktop/src/components/Settings/SettingsCog.tsx apps/desktop/src/components/Today/HeaderCard.tsx
git commit -m "feat(today): SettingsCog in HeaderCard next to the clock"
```

---

### Task 13: `AccountRow` component

**Files:**
- Create: `apps/desktop/src/components/Settings/AccountRow.tsx`

Full component — handles display, status-line states, Sync button, Remove button with soft-confirm:

```tsx
import { useEffect, useRef, useState } from "react";
import type { CalendarAccount } from "../../lib/settings/ipc";
import { syncAccount, removeCalendarAccount } from "../../lib/settings/ipc";
import { listEventsToday } from "../../lib/today/ipc";
import { useSettingsStore } from "../../lib/settings/state";
import { useTodayStore } from "../../lib/today/state";

interface AccountRowProps { account: CalendarAccount; }

function relativeTime(seconds: number): string {
  const delta = Date.now() / 1000 - seconds;
  if (delta < 60) return `${Math.floor(delta)}s ago`;
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
  return `${Math.floor(delta / 86400)}d ago`;
}

function providerBadge(url: string): string {
  if (url.includes("caldav.icloud.com")) return "iC";
  if (url.includes("fastmail")) return "FM";
  return "●";
}

export default function AccountRow({ account }: AccountRowProps) {
  const upsertAccount = useSettingsStore((s) => s.upsertAccount);
  const removeAccount = useSettingsStore((s) => s.removeAccount);
  const markSyncing = useSettingsStore((s) => s.markSyncing);
  const markSynced = useSettingsStore((s) => s.markSynced);
  const syncing = useSettingsStore((s) => s.syncingAccountIds.has(account.id));
  const setEvents = useTodayStore((s) => s.setEvents);

  const [removeArmed, setRemoveArmed] = useState(false);
  const removeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => { if (removeTimer.current) clearTimeout(removeTimer.current); }, []);

  const handleSync = async () => {
    markSyncing(account.id);
    try {
      const result = await syncAccount(account.id);
      upsertAccount({
        ...account,
        last_synced_at: result.synced_at,
        last_error: result.error,
      });
      const events = await listEventsToday();
      setEvents(events);
    } finally {
      markSynced(account.id);
    }
  };

  const handleRemoveClick = () => {
    if (removeArmed) {
      removeAccount(account.id);
      void removeCalendarAccount(account.id).then(() => listEventsToday().then(setEvents));
      return;
    }
    setRemoveArmed(true);
    removeTimer.current = setTimeout(() => setRemoveArmed(false), 3000);
  };

  const statusLine = (() => {
    if (syncing) return "syncing…";
    if (account.last_error) return `error: ${account.last_error}`;
    if (account.last_synced_at) return `synced ${relativeTime(account.last_synced_at)}`;
    return "not synced yet";
  })();

  return (
    <div
      style={{
        display: "flex", gap: 10, alignItems: "center",
        padding: "8px 10px",
        background: removeArmed ? "rgba(255, 59, 48, 0.06)" : "white",
        border: "1px solid var(--hairline)",
        borderRadius: 8,
        marginBottom: 6,
      }}
    >
      <div style={{
        width: 28, height: 28, borderRadius: 6,
        background: "var(--imessage-blue)", color: "white",
        display: "flex", alignItems: "center", justifyContent: "center",
        fontSize: 11, fontWeight: 700,
      }}>
        {providerBadge(account.server_url)}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontWeight: 600, fontSize: 13 }}>{account.display_name}</div>
        <div
          style={{
            fontSize: 11, color: account.last_error ? "var(--imessage-red)" : "rgba(0,0,0,0.5)",
            whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis",
          }}
          title={account.last_error ?? undefined}
        >
          {account.username} · {statusLine}
        </div>
      </div>
      <button
        onClick={handleSync}
        disabled={syncing}
        style={{
          padding: "5px 10px", borderRadius: 6, fontSize: 11, fontWeight: 600,
          border: "1px solid var(--hairline)", background: "white",
          cursor: syncing ? "default" : "pointer", opacity: syncing ? 0.5 : 1,
        }}
      >
        Sync
      </button>
      <button
        onClick={handleRemoveClick}
        style={{
          padding: "5px 10px", borderRadius: 6, fontSize: 11, fontWeight: 600,
          border: "1px solid var(--hairline)", background: "white",
          cursor: "pointer",
          color: removeArmed ? "var(--imessage-red)" : "inherit",
        }}
      >
        {removeArmed ? "Yes?" : "Remove"}
      </button>
    </div>
  );
}
```

- [ ] **Verify + commit**

```
pnpm tsc
```

```bash
git add apps/desktop/src/components/Settings/AccountRow.tsx
git commit -m "feat(settings): AccountRow with sync status + soft-confirm remove"
```

---

### Task 14: `AddAccountForm` component

**Files:**
- Create: `apps/desktop/src/components/Settings/AddAccountForm.tsx`

```tsx
import { useState } from "react";
import { addCalendarAccount } from "../../lib/settings/ipc";
import { useSettingsStore } from "../../lib/settings/state";

interface AddAccountFormProps { onClose: () => void; }

export default function AddAccountForm({ onClose }: AddAccountFormProps) {
  const upsertAccount = useSettingsStore((s) => s.upsertAccount);

  const [serverUrl, setServerUrl] = useState("https://caldav.icloud.com");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const canSubmit = serverUrl.trim() && username.trim() && password.trim() && !busy;

  const onConnect = async () => {
    setBusy(true);
    setError(null);
    try {
      const account = await addCalendarAccount(serverUrl.trim(), username.trim(), password);
      upsertAccount(account);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const labelStyle: React.CSSProperties = {
    display: "block", fontSize: 11, fontWeight: 700,
    textTransform: "uppercase", letterSpacing: 0.6,
    color: "rgba(0,0,0,0.55)", marginBottom: 4, marginTop: 10,
  };

  const inputStyle: React.CSSProperties = {
    width: "100%", padding: "6px 10px",
    border: "1px solid var(--hairline)", borderRadius: 6,
    fontSize: 13, fontFamily: "inherit", outline: "none",
  };

  return (
    <div
      style={{
        padding: 12,
        background: "rgba(0,0,0,0.02)",
        border: "1px dashed var(--hairline)",
        borderRadius: 8,
        marginTop: 8,
      }}
    >
      <label style={labelStyle}>Server URL</label>
      <input
        type="text" value={serverUrl}
        onChange={(e) => setServerUrl(e.target.value)}
        placeholder="https://caldav.icloud.com"
        style={inputStyle}
      />
      <label style={labelStyle}>Username</label>
      <input
        type="text" value={username}
        onChange={(e) => setUsername(e.target.value)}
        placeholder="your-apple-id@icloud.com"
        style={inputStyle}
      />
      <label style={labelStyle}>
        App-specific password
        <span title="For iCloud: appleid.apple.com → Sign-In and Security → App-Specific Passwords" style={{ marginLeft: 6, color: "rgba(0,0,0,0.4)", cursor: "help" }}>?</span>
      </label>
      <input
        type="password" value={password}
        onChange={(e) => setPassword(e.target.value)}
        style={inputStyle}
      />

      {error && (
        <div style={{ marginTop: 8, color: "var(--imessage-red)", fontSize: 12 }}>
          Connection failed: {error}
        </div>
      )}

      <div style={{ marginTop: 12, display: "flex", gap: 8, justifyContent: "flex-end" }}>
        <button onClick={onClose} style={{
          padding: "6px 12px", borderRadius: 6, fontSize: 12, fontWeight: 600,
          border: "1px solid var(--hairline)", background: "white", cursor: "pointer",
        }}>Cancel</button>
        <button
          onClick={onConnect}
          disabled={!canSubmit}
          style={{
            padding: "6px 12px", borderRadius: 6, fontSize: 12, fontWeight: 700,
            border: "none", background: canSubmit ? "var(--imessage-blue)" : "rgba(0,0,0,0.15)",
            color: "white", cursor: canSubmit ? "pointer" : "default",
          }}
        >
          {busy ? "Connecting…" : "Connect"}
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Verify + commit**

```
pnpm tsc
```

```bash
git add apps/desktop/src/components/Settings/AddAccountForm.tsx
git commit -m "feat(settings): AddAccountForm with password field + connect flow"
```

---

### Task 15: `CalendarsTab` composer

**Files:**
- Modify: `apps/desktop/src/components/Settings/CalendarsTab.tsx`

Replace the Task 11 stub with the real composer:

```tsx
import { useEffect, useState } from "react";
import { useSettingsStore } from "../../lib/settings/state";
import { listCalendarAccounts } from "../../lib/settings/ipc";
import AccountRow from "./AccountRow";
import AddAccountForm from "./AddAccountForm";

export default function CalendarsTab() {
  const accounts = useSettingsStore((s) => s.accounts);
  const setAccounts = useSettingsStore((s) => s.setAccounts);
  const [adding, setAdding] = useState(false);

  useEffect(() => {
    void listCalendarAccounts().then(setAccounts);
  }, [setAccounts]);

  return (
    <div style={{ padding: "14px 16px" }}>
      <p style={{
        fontSize: 11, textTransform: "uppercase", letterSpacing: 0.6,
        color: "rgba(0,0,0,0.55)", fontWeight: 700, margin: "0 0 10px",
      }}>Your calendar accounts</p>

      {accounts.length === 0 && !adding && (
        <p style={{ color: "rgba(0,0,0,0.5)", fontSize: 13, marginBottom: 12 }}>
          No accounts yet. Add one to start syncing events.
        </p>
      )}

      {accounts.map((a) => <AccountRow key={a.id} account={a} />)}

      {!adding && (
        <button
          onClick={() => setAdding(true)}
          style={{
            background: "transparent", border: "none",
            color: "var(--imessage-blue)", fontWeight: 700, fontSize: 12,
            padding: "6px 0", cursor: "pointer",
          }}
        >
          + Add calendar account
        </button>
      )}

      {adding && <AddAccountForm onClose={() => setAdding(false)} />}
    </div>
  );
}
```

- [ ] **Verify + commit**

```
pnpm tsc
pnpm --filter manor-desktop test
```

```bash
git add apps/desktop/src/components/Settings/CalendarsTab.tsx
git commit -m "feat(settings): CalendarsTab composes AccountRow + AddAccountForm"
```

---

### Task 16: `EventsCard` — populate from store

**Files:**
- Modify: `apps/desktop/src/components/Today/EventsCard.tsx`
- Modify: `apps/desktop/src/components/Today/Today.tsx` (hydrate events on mount)

- [ ] **Step 1: Replace EventsCard**

```tsx
import { useTodayStore } from "../../lib/today/state";

const cardStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
};

const sectionHeader: React.CSSProperties = {
  fontSize: 11, textTransform: "uppercase", letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)", fontWeight: 700,
  margin: 0, marginBottom: 8,
};

function formatTime(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

export default function EventsCard() {
  const events = useTodayStore((s) => s.events);

  return (
    <div style={cardStyle}>
      <p style={sectionHeader}>Events</p>
      {events.length === 0 ? (
        <p style={{ fontStyle: "italic", color: "rgba(0,0,0,0.5)", margin: 0, fontSize: 13 }}>
          No events today.
        </p>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          {events.map((e) => (
            <div key={e.id} style={{ display: "flex", gap: 10, padding: "4px 0", fontSize: 13 }}>
              <span style={{ fontWeight: 700, minWidth: 48, color: "var(--imessage-blue)" }}>
                {formatTime(e.start_at)}
              </span>
              <span>{e.title}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Hydrate events on Today mount**

Edit `apps/desktop/src/components/Today/Today.tsx`. In the existing `useEffect` that calls `listTasks()`, also hydrate events:

```tsx
import { listTasks, listEventsToday } from "../../lib/today/ipc";
// ...
useEffect(() => {
  void listTasks().then(setTasks);
  void listEventsToday().then(setEvents);
}, [setTasks, setEvents]);
```

Add `const setEvents = useTodayStore((s) => s.setEvents);` alongside the existing `setTasks` selector.

- [ ] **Verify + commit**

```
pnpm tsc
pnpm --filter manor-desktop test
```

```bash
git add apps/desktop/src/components/Today/EventsCard.tsx apps/desktop/src/components/Today/Today.tsx
git commit -m "feat(today): EventsCard renders from store; hydrate events on mount"
```

---

### Task 17: Manual smoke + tag + PR

**Files:** none (verification + git)

- [ ] **Step 1: Full verification suite**

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets    # expect ~57 total
pnpm tsc
pnpm --filter manor-desktop test        # expect 27
```

- [ ] **Step 2: Manual end-to-end smoke against real iCloud**

Ollama still running with `qwen2.5:7b-instruct`. From the worktree:

```
./scripts/dev.sh
```

Walk the 12-step list from **spec §8.6**:

1. Launch Manor — EventsCard says "No events today."
2. Cog icon on HeaderCard visible; click → Settings modal opens, Calendars active, empty list + `+ Add calendar account`
3. Click `+ Add calendar account` → form appears
4. `https://caldav.icloud.com` + your Apple ID email + iCloud **app-specific password** → Connect
5. Form closes, new row appears with `syncing…`
6. 5–15s later: row flips to `synced Ns ago`; EventsCard populates with today's events, ordered by time
7. Close modal (× or Esc) → EventsCard stays populated
8. Reopen, click Sync → row reflips `syncing…` → `synced 0s ago`
9. Remove account, re-add with wrong password → `Connection failed: bad credentials`; retry with real
10. Click Remove → soft-confirm → Yes → row disappears, EventsCard returns to "No events today."
11. Quit Manor + relaunch → account persists, password persists in Keychain, app-start sync fires, EventsCard populates a few seconds in
12. Add a recurring event in iCloud via Calendar.app (e.g., daily standup 10am), wait ~30s for iCloud propagation, click Sync in Manor → today's occurrence now shows in EventsCard

Document any failures here as bullets under the step number.

- [ ] **Step 3: Push + open PR**

```bash
git push -u origin feature/phase-3b-caldav-read
gh pr create --base main --head feature/phase-3b-caldav-read \
  --title "Phase 3b — CalDAV read (Settings modal, sync engine, events)" \
  --body "$(cat <<'EOF'
## Summary

First real calendar integration. EventsCard populates with today's events synced from iCloud (or any CalDAV Basic Auth server). Adds the first Settings surface — tabbed modal triggered by a cog on HeaderCard and \`⌘,\`. Credentials in macOS Keychain.

- **Backend:** new \`manor-core::assistant::{calendar_account, event}\`, new \`manor-app::sync::{caldav, ical, expand, keychain, engine}\`. 6 new Tauri commands. App-start background sync in setup().
- **Sync model:** full resync per account (wipe + reinsert) in a 7-day-back / 14-day-forward window. RRULE expansion via \`rrule\` crate with EXDATE support. Each materialised occurrence gets \`{uid}::{RFC3339}\` external_id.
- **Frontend:** new \`useSettingsStore\` + \`<Settings*>\` component tree. \`useTodayStore\` gains \`events[]\`. EventsCard renders from the store.
- **Auth:** HTTP Basic; password in macOS Keychain via \`keyring\` (key: \`manor/caldav-{id}\`).

## Test plan

- [x] \`cargo test --workspace --all-targets\` (~57 tests)
- [x] \`cargo fmt --check\` + \`cargo clippy --all-targets -- -D warnings\` clean
- [x] \`pnpm tsc\` clean
- [x] \`pnpm --filter manor-desktop test\` (~27 tests)
- [x] Manual smoke against real iCloud (12-step list per spec §8.6)
- [ ] CI green on this PR

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 4: After CI green, merge + tag + cleanup**

```bash
# Wait until gh pr checks <n> is green
gh pr merge <n> --merge --delete-branch

# From the primary repo (/Users/hanamori/life-assistant), not the worktree:
cd /Users/hanamori/life-assistant
git checkout main
git pull origin main
git fetch --prune origin
git tag -a phase-3b-caldav-read-complete -m "Phase 3b CalDAV read — settings + sync + events"
git push origin phase-3b-caldav-read-complete
git worktree remove .worktrees/phase-3b-caldav-read
git branch -d feature/phase-3b-caldav-read    # if still present
```

---

## Self-Review Checklist

**1. Spec coverage (3b spec § → task):**
- §3.1 Rust modules → Tasks 1–9 (all modules created/modified as listed)
- §3.2 deps → Task 1
- §3.3 frontend file structure → Tasks 10–16
- §3.4 IPC contract → Task 9 (commands) + Task 10 (typed wrappers)
- §4 Settings modal UX → Tasks 11 (shell/tabs) + 12 (cog) + 13 (AccountRow) + 14 (AddAccountForm) + 15 (composer)
- §5 Data model → Task 2 (migration + calendar_account DAL) + Task 3 (event DAL) + Task 10 (frontend types/stores)
- §6 CalDAV protocol → Tasks 4 (iCal) + 5 (RRULE) + 7 (HTTP)
- §7 Sync engine → Task 8
- §8 Testing → Tasks 2–10 each include their TDD; §8.6 manual smoke → Task 17
- §9 Completion criteria → Task 17

**2. Placeholder scan:** No TBD / implement later / add appropriate / empty test bodies.

**3. Type consistency:**
- `CalendarAccount` (Rust) ↔ `CalendarAccount` (TS): id, display_name, server_url, username, last_synced_at, last_error, created_at. Match.
- `Event` / `NewEvent` (Rust) ↔ `Event` (TS): frontend only reads, so `NewEvent` doesn't need a TS counterpart. `Event` fields match 1:1.
- `SyncResult` (Rust) ↔ `SyncResult` (TS): account_id, events_added, error, synced_at. Match.
- `StreamChunk` unchanged; no calendar tools in Phase 3b.

**4. Known friction:** Task 9's Arc/Mutex ownership for the spawned follow-up sync may need iteration — documented with a safe fallback (skip the spawn, let the frontend request sync).

---

## Plan deviations (appended by implementers)

*(None yet.)*
