# Phase 5b CalDAV Write Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add CalDAV write capability to Manor — create, update, delete events from the Today view, with full support for per-occurrence editing of recurring events.

**Architecture:** Thin write layer on top of existing CalDAV client; iCal generation lives in a dedicated `ical_write.rs` module. Rust commands follow the `spawn_blocking` pattern from existing calendar commands. Frontend changes are additive — EventsCard gains `+` and edit affordances; two new drawer components handle create and edit flows.

**Tech Stack:** Rust (anyhow, reqwest, rusqlite, quick_xml), RFC 5545 iCal, Tauri 2 IPC, React 18, TypeScript, Zustand, Refinery migrations

---

## File Map

| Path | Status | What it does |
|---|---|---|
| `crates/core/migrations/V6__calendar_write.sql` | Create | Schema — adds 9 columns to `event`, 1 to `calendar_account`, new `calendar` table |
| `crates/core/src/assistant/event.rs` | Modify | Add 8 new fields to `Event`/`NewEvent`; add `soft_delete`; update `list_today` |
| `crates/core/src/assistant/calendar_account.rs` | Modify | Add `default_calendar_url`; add `set_default_calendar` fn |
| `crates/core/src/assistant/calendar.rs` | Create | `Calendar` struct, `upsert`, `list` DAL functions |
| `crates/core/src/assistant/mod.rs` | Modify | `pub mod calendar;` |
| `crates/app/src/sync/caldav.rs` | Modify | `ReportItem` struct; `report_events` → `Vec<ReportItem>`; add `fetch_ical`, `put_event`, `delete_event` |
| `crates/app/src/sync/ical_write.rs` | Create | `generate_vcalendar`, `add_exdate`, `add_recurrence_override` |
| `crates/app/src/sync/expand.rs` | Modify | Add `event_url: &str` param; populate `event_url`, `is_recurring_occurrence`, `parent_event_url`, `occurrence_dtstart` on `NewEvent` |
| `crates/app/src/sync/engine.rs` | Modify | Use `ReportItem`; store calendars in DB; set default calendar; pass `href` to `expand` |
| `crates/app/src/assistant/commands.rs` | Modify | Add 5 new commands: `create_event`, `update_event`, `delete_event`, `set_default_calendar`, `list_calendars` |
| `crates/app/src/lib.rs` | Modify | Register 5 new commands in `invoke_handler![]` |
| `apps/desktop/src/lib/today/ipc.ts` | Modify | Expand `Event` interface; add `createEvent`, `updateEvent`, `deleteEvent` |
| `apps/desktop/src/lib/settings/ipc.ts` | Modify | Add `default_calendar_url` to `CalendarAccount`; add `CalendarInfo`, `listCalendars`, `setDefaultCalendar` |
| `apps/desktop/src/lib/today/state.ts` | Modify | Add `upsertEvent`, `removeEvent` mutations |
| `apps/desktop/src/lib/settings/state.ts` | Modify | Add `accountCalendars: Map<number, CalendarInfo[]>`, `setCalendars` mutation |
| `apps/desktop/src/components/Today/EventsCard.tsx` | Modify | `+` button in header; clickable rows; manage `showAdd`/`editingEvent` state |
| `apps/desktop/src/components/Today/AddEventDrawer.tsx` | Create | Slide-in drawer for event creation |
| `apps/desktop/src/components/Today/EditEventDrawer.tsx` | Create | Slide-in drawer for event editing/deletion |
| `apps/desktop/src/components/Settings/AccountRow.tsx` | Modify | Default calendar picker below sync status |

---

## Task 1: V6 Schema Migration

**Files:**
- Create: `crates/core/migrations/V6__calendar_write.sql`
- Test: `crates/core/src/assistant/db.rs` (existing migration runner test)

- [ ] **Step 1: Write the migration file**

```sql
-- V6__calendar_write.sql
-- Extend event table with write-support columns
ALTER TABLE event ADD COLUMN event_url               TEXT;
ALTER TABLE event ADD COLUMN etag                    TEXT;
ALTER TABLE event ADD COLUMN description             TEXT;
ALTER TABLE event ADD COLUMN location                TEXT;
ALTER TABLE event ADD COLUMN all_day                 INTEGER NOT NULL DEFAULT 0;
ALTER TABLE event ADD COLUMN is_recurring_occurrence INTEGER NOT NULL DEFAULT 0;
ALTER TABLE event ADD COLUMN parent_event_url        TEXT;
ALTER TABLE event ADD COLUMN occurrence_dtstart      TEXT;
ALTER TABLE event ADD COLUMN deleted_at              INTEGER;

-- Calendar account default calendar
ALTER TABLE calendar_account ADD COLUMN default_calendar_url TEXT;

-- Persisted calendar list (one row per calendar URL per account)
CREATE TABLE calendar (
  id                  INTEGER PRIMARY KEY,
  calendar_account_id INTEGER NOT NULL REFERENCES calendar_account(id) ON DELETE CASCADE,
  url                 TEXT    NOT NULL,
  display_name        TEXT,
  created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
  UNIQUE(calendar_account_id, url)
);
```

Save to `crates/core/migrations/V6__calendar_write.sql`.

- [ ] **Step 2: Verify migration runs**

```bash
cd /Users/hanamori/life-assistant
cargo test -p manor-core 2>&1 | grep -E "^(test|FAILED|error)"
```

Expected: all existing tests pass. The Refinery runner in `db.rs` applies V6 to every fresh `db::init()` call in tests.

- [ ] **Step 3: Commit**

```bash
git add crates/core/migrations/V6__calendar_write.sql
git commit -m "feat(core): V6 migration — event write columns + calendar table"
```

---

## Task 2: Extend Event DAL

**Files:**
- Modify: `crates/core/src/assistant/event.rs`

- [ ] **Step 1: Write failing tests first**

Add to the `#[cfg(test)]` block in `event.rs`:

```rust
#[test]
fn list_today_excludes_deleted_events() {
    let (_d, conn, aid) = fresh_conn_with_account();
    insert_many(
        &conn,
        &[NewEvent {
            calendar_account_id: aid,
            external_id: "ev1".into(),
            title: "Alive".into(),
            start_at: 150,
            end_at: 200,
            event_url: None,
            etag: None,
            description: None,
            location: None,
            all_day: false,
            is_recurring_occurrence: false,
            parent_event_url: None,
            occurrence_dtstart: None,
        }],
    )
    .unwrap();
    // Soft-delete the event
    let id = list_today(&conn, 0, 1000).unwrap()[0].id;
    soft_delete(&conn, id).unwrap();
    let rows = list_today(&conn, 0, 1000).unwrap();
    assert_eq!(rows.len(), 0, "deleted events must be hidden");
}

#[test]
fn soft_delete_sets_deleted_at() {
    let (_d, conn, aid) = fresh_conn_with_account();
    insert_many(
        &conn,
        &[NewEvent {
            calendar_account_id: aid,
            external_id: "del1".into(),
            title: "Gone".into(),
            start_at: 150,
            end_at: 200,
            event_url: Some("https://cal.example.com/del1.ics".into()),
            etag: Some("\"abc123\"".into()),
            description: None,
            location: None,
            all_day: false,
            is_recurring_occurrence: false,
            parent_event_url: None,
            occurrence_dtstart: None,
        }],
    )
    .unwrap();
    let id = list_today(&conn, 0, 1000).unwrap()[0].id;
    soft_delete(&conn, id).unwrap();
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM event WHERE id = ?1 AND deleted_at IS NOT NULL",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);
}
```

- [ ] **Step 2: Run — expect compile error (NewEvent missing fields)**

```bash
cargo test -p manor-core 2>&1 | head -40
```

Expected: `error[E0063]: missing fields` for NewEvent struct literal.

- [ ] **Step 3: Expand `Event` and `NewEvent` structs**

Replace the existing `Event` and `NewEvent` structs and their `from_row` impl in `event.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub id: i64,
    pub calendar_account_id: i64,
    pub external_id: String,
    pub title: String,
    pub start_at: i64,
    pub end_at: i64,
    pub created_at: i64,
    pub event_url: Option<String>,
    pub etag: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub all_day: bool,
    pub is_recurring_occurrence: bool,
    pub parent_event_url: Option<String>,
    pub occurrence_dtstart: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    pub calendar_account_id: i64,
    pub external_id: String,
    pub title: String,
    pub start_at: i64,
    pub end_at: i64,
    pub event_url: Option<String>,
    pub etag: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub all_day: bool,
    pub is_recurring_occurrence: bool,
    pub parent_event_url: Option<String>,
    pub occurrence_dtstart: Option<String>,
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
            event_url: row.get("event_url")?,
            etag: row.get("etag")?,
            description: row.get("description")?,
            location: row.get("location")?,
            all_day: row.get::<_, i64>("all_day").map(|v| v != 0)?,
            is_recurring_occurrence: row.get::<_, i64>("is_recurring_occurrence").map(|v| v != 0)?,
            parent_event_url: row.get("parent_event_url")?,
            occurrence_dtstart: row.get("occurrence_dtstart")?,
        })
    }
}
```

- [ ] **Step 4: Update `insert_many` to include new columns**

Replace the `insert_many` function:

```rust
pub fn insert_many(conn: &Connection, events: &[NewEvent]) -> Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    let now_ms = Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO event (
                calendar_account_id, external_id, title, start_at, end_at, created_at,
                event_url, etag, description, location, all_day,
                is_recurring_occurrence, parent_event_url, occurrence_dtstart
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?;
        for ev in events {
            stmt.execute(params![
                ev.calendar_account_id,
                ev.external_id,
                ev.title,
                ev.start_at,
                ev.end_at,
                now_ms,
                ev.event_url,
                ev.etag,
                ev.description,
                ev.location,
                ev.all_day as i64,
                ev.is_recurring_occurrence as i64,
                ev.parent_event_url,
                ev.occurrence_dtstart,
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}
```

- [ ] **Step 5: Update `list_today` SELECT and add `soft_delete`**

Replace `list_today` and add `soft_delete` after it:

```rust
pub fn list_today(conn: &Connection, start_utc: i64, end_utc: i64) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT id, calendar_account_id, external_id, title, start_at, end_at, created_at,
                event_url, etag, description, location, all_day,
                is_recurring_occurrence, parent_event_url, occurrence_dtstart
         FROM event
         WHERE start_at >= ?1 AND start_at < ?2 AND deleted_at IS NULL
         ORDER BY start_at",
    )?;
    let rows = stmt
        .query_map(params![start_utc, end_utc], Event::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn soft_delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE event SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}
```

- [ ] **Step 6: Fix existing test struct literals**

All existing `NewEvent { ... }` literals in the test block need the 8 new fields added with their zero values:

```rust
NewEvent {
    calendar_account_id: aid,
    external_id: "u1".into(),
    title: "A".into(),
    start_at: 100,
    end_at: 200,
    event_url: None,
    etag: None,
    description: None,
    location: None,
    all_day: false,
    is_recurring_occurrence: false,
    parent_event_url: None,
    occurrence_dtstart: None,
}
```

Apply this pattern to all four test helpers (`insert_many_persists_batch`, `list_today_filters_by_utc_bounds`, `delete_for_account_scoped_to_account`, and the inline SQL in `calendar_account.rs` tests that bypasses the DAL).

- [ ] **Step 7: Run tests — expect pass**

```bash
cargo test -p manor-core 2>&1 | grep -E "^(test|FAILED|error)"
```

Expected: all tests pass including the two new ones.

- [ ] **Step 8: Commit**

```bash
git add crates/core/src/assistant/event.rs
git commit -m "feat(core): expand Event/NewEvent structs for write support + soft_delete"
```

---

## Task 3: Calendar DAL + CalendarAccount Updates

**Files:**
- Create: `crates/core/src/assistant/calendar.rs`
- Modify: `crates/core/src/assistant/calendar_account.rs`
- Modify: `crates/core/src/assistant/mod.rs`

- [ ] **Step 1: Write failing tests for the new calendar module**

Create `crates/core/src/assistant/calendar.rs` with tests first:

```rust
//! Persisted calendar list — one row per calendar URL per account.

use anyhow::Result;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Calendar {
    pub id: i64,
    pub calendar_account_id: i64,
    pub url: String,
    pub display_name: Option<String>,
}

impl Calendar {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            calendar_account_id: row.get("calendar_account_id")?,
            url: row.get("url")?,
            display_name: row.get("display_name")?,
        })
    }
}

/// Upsert a calendar URL. INSERT OR IGNORE — never overwrites existing rows.
pub fn upsert(
    conn: &Connection,
    account_id: i64,
    url: &str,
    display_name: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO calendar (calendar_account_id, url, display_name)
         VALUES (?1, ?2, ?3)",
        params![account_id, url, display_name],
    )?;
    Ok(())
}

pub fn list(conn: &Connection, account_id: i64) -> Result<Vec<Calendar>> {
    let mut stmt = conn.prepare(
        "SELECT id, calendar_account_id, url, display_name
         FROM calendar WHERE calendar_account_id = ?1
         ORDER BY id",
    )?;
    let rows = stmt
        .query_map([account_id], Calendar::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::{calendar_account, db};
    use tempfile::tempdir;

    fn fresh(account: &str) -> (tempfile::TempDir, Connection, i64) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let id = calendar_account::insert(&conn, account, "https://cal.test", "u").unwrap();
        (dir, conn, id)
    }

    #[test]
    fn upsert_then_list() {
        let (_d, conn, aid) = fresh("A");
        upsert(&conn, aid, "https://cal.test/home/work/", Some("Work")).unwrap();
        upsert(&conn, aid, "https://cal.test/home/personal/", Some("Personal")).unwrap();
        let cals = list(&conn, aid).unwrap();
        assert_eq!(cals.len(), 2);
        assert_eq!(cals[0].display_name.as_deref(), Some("Work"));
    }

    #[test]
    fn upsert_is_idempotent() {
        let (_d, conn, aid) = fresh("A");
        upsert(&conn, aid, "https://cal.test/home/work/", Some("Work")).unwrap();
        upsert(&conn, aid, "https://cal.test/home/work/", Some("Work")).unwrap();
        let cals = list(&conn, aid).unwrap();
        assert_eq!(cals.len(), 1);
    }

    #[test]
    fn list_scoped_to_account() {
        let (_d, conn, aid) = fresh("A");
        let bid = calendar_account::insert(&conn, "B", "https://b.test", "u").unwrap();
        upsert(&conn, aid, "https://cal.test/home/work/", None).unwrap();
        upsert(&conn, bid, "https://b.test/home/cal/", None).unwrap();
        assert_eq!(list(&conn, aid).unwrap().len(), 1);
        assert_eq!(list(&conn, bid).unwrap().len(), 1);
    }

    #[test]
    fn cascade_delete_removes_calendars() {
        let (_d, conn, aid) = fresh("A");
        upsert(&conn, aid, "https://cal.test/home/work/", None).unwrap();
        calendar_account::delete(&conn, aid).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM calendar", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
```

- [ ] **Step 2: Run — expect fail (module not declared)**

```bash
cargo test -p manor-core 2>&1 | head -20
```

Expected: `error[E0583]: file not found for module 'calendar'` or similar.

- [ ] **Step 3: Add `pub mod calendar;` to mod.rs**

In `crates/core/src/assistant/mod.rs`, add:

```rust
pub mod calendar;
```

alongside the existing `pub mod calendar_account;`, `pub mod event;`, etc.

- [ ] **Step 4: Run tests — expect pass**

```bash
cargo test -p manor-core 2>&1 | grep -E "^(test|FAILED|error)"
```

Expected: 4 new calendar tests pass.

- [ ] **Step 5: Write failing test for `set_default_calendar`**

In `calendar_account.rs` tests, add:

```rust
#[test]
fn set_default_calendar_persists_url() {
    let (_d, conn) = fresh_conn();
    let id = insert(&conn, "iCloud", "https://caldav.icloud.com", "a@b.c").unwrap();
    set_default_calendar(&conn, id, "https://caldav.icloud.com/12345/calendars/home/").unwrap();
    let row = get(&conn, id).unwrap().unwrap();
    assert_eq!(
        row.default_calendar_url.as_deref(),
        Some("https://caldav.icloud.com/12345/calendars/home/")
    );
}
```

- [ ] **Step 6: Run — expect compile error (missing field + missing fn)**

```bash
cargo test -p manor-core 2>&1 | head -20
```

Expected: `error[E0609]: no field 'default_calendar_url'` on `CalendarAccount`.

- [ ] **Step 7: Update CalendarAccount struct + add set_default_calendar**

In `calendar_account.rs`, add the field to `CalendarAccount`:

```rust
pub struct CalendarAccount {
    pub id: i64,
    pub display_name: String,
    pub server_url: String,
    pub username: String,
    pub last_synced_at: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub default_calendar_url: Option<String>,
}
```

Update `from_row` to include the new field:

```rust
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
            default_calendar_url: row.get("default_calendar_url")?,
        })
    }
}
```

Update both SELECT strings in `list` and `get` to include `default_calendar_url`:

```rust
// in list():
"SELECT id, display_name, server_url, username, last_synced_at, last_error, created_at, default_calendar_url
 FROM calendar_account
 ORDER BY created_at"

// in get():
"SELECT id, display_name, server_url, username, last_synced_at, last_error, created_at, default_calendar_url
 FROM calendar_account WHERE id = ?1"
```

Add the new function after `update_sync_state`:

```rust
pub fn set_default_calendar(conn: &Connection, id: i64, url: &str) -> Result<()> {
    conn.execute(
        "UPDATE calendar_account SET default_calendar_url = ?1 WHERE id = ?2",
        params![url, id],
    )?;
    Ok(())
}
```

- [ ] **Step 8: Run tests — expect pass**

```bash
cargo test -p manor-core 2>&1 | grep -E "^(test|FAILED|error)"
```

Expected: all tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/core/src/assistant/calendar.rs crates/core/src/assistant/calendar_account.rs crates/core/src/assistant/mod.rs
git commit -m "feat(core): calendar DAL + default_calendar_url on CalendarAccount"
```

---

## Task 4: CalDAV Write Client Methods

**Files:**
- Modify: `crates/app/src/sync/caldav.rs`

- [ ] **Step 1: Write failing tests**

In the `#[cfg(test)]` block of `caldav.rs`, add:

```rust
#[tokio::test]
async fn report_events_returns_report_items_with_href_and_etag() {
    let server = MockServer::start().await;
    let body = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/home/cal1/event-1.ics</D:href>
    <D:propstat><D:prop>
      <D:getetag>"abc123"</D:getetag>
      <C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:u1
DTSTART:20260415T093000Z
DTEND:20260415T103000Z
SUMMARY:Standup
END:VEVENT
END:VCALENDAR</C:calendar-data>
    </D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#;
    Mock::given(method("REPORT"))
        .respond_with(ResponseTemplate::new(207).set_body_string(body))
        .mount(&server)
        .await;

    let client = CalDavClient::new("u", "p");
    let start = Utc::now() - chrono::Duration::days(7);
    let end = Utc::now() + chrono::Duration::days(14);
    let items = client
        .report_events(&format!("{}/home/cal1/", server.uri()), start, end)
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0].href.ends_with("/event-1.ics"));
    assert_eq!(items[0].etag, "\"abc123\"");
    assert!(items[0].ical.contains("UID:u1"));
}

#[tokio::test]
async fn put_event_returns_new_etag() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .respond_with(
            ResponseTemplate::new(201)
                .append_header("ETag", "\"newetag456\""),
        )
        .mount(&server)
        .await;

    let client = CalDavClient::new("u", "p");
    let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let etag = client
        .put_event(&format!("{}/home/cal/event.ics", server.uri()), ical, None)
        .await
        .unwrap();
    assert_eq!(etag, "\"newetag456\"");
}

#[tokio::test]
async fn put_event_412_returns_conflict_error() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(412))
        .mount(&server)
        .await;

    let client = CalDavClient::new("u", "p");
    let result = client
        .put_event(
            &format!("{}/home/cal/event.ics", server.uri()),
            "irrelevant",
            Some("\"old-etag\""),
        )
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("conflict"));
}

#[tokio::test]
async fn delete_event_sends_delete_with_if_match() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(header("If-Match", "\"abc\""))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = CalDavClient::new("u", "p");
    client
        .delete_event(&format!("{}/home/cal/event.ics", server.uri()), "\"abc\"")
        .await
        .unwrap();
}
```

- [ ] **Step 2: Run — expect compile errors**

```bash
cargo test -p manor-app 2>&1 | head -30
```

Expected: `error[E0277]: the trait...` or `no field 'href'` on `report_events` return type.

- [ ] **Step 3: Add `ReportItem` struct and update `report_events`**

Add `ReportItem` after `CalendarInfo`:

```rust
#[derive(Debug, Clone)]
pub struct ReportItem {
    pub ical: String,
    pub href: String,
    pub etag: String,
}
```

Replace `report_events` signature and body:

```rust
pub async fn report_events(
    &self,
    calendar_url: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<ReportItem>> {
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
    let xml = self
        .request_xml(REPORT, calendar_url, Some("1"), &body)
        .await?;
    Ok(extract_report_items(&xml, calendar_url))
}
```

- [ ] **Step 4: Replace `extract_calendar_data_blocks` with `extract_report_items`**

Remove `extract_calendar_data_blocks` entirely. Add `extract_report_items`:

```rust
/// Extract `<D:response>` blocks from a REPORT response into ReportItems.
/// Each response contains an href, a getetag, and calendar-data text.
fn extract_report_items(xml: &str, base_url: &str) -> Vec<ReportItem> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut in_response = false;
    let mut in_href = false;
    let mut in_etag = false;
    let mut in_data = false;
    let mut cur_href = String::new();
    let mut cur_etag = String::new();
    let mut cur_ical = String::new();
    let mut out = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "response" => {
                        in_response = true;
                        cur_href.clear();
                        cur_etag.clear();
                        cur_ical.clear();
                    }
                    "href" if in_response => {
                        in_href = true;
                        cur_href.clear();
                    }
                    "getetag" if in_response => {
                        in_etag = true;
                        cur_etag.clear();
                    }
                    "calendar-data" if in_response => {
                        in_data = true;
                        cur_ical.clear();
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Text(t)) => {
                let text = t.unescape().unwrap_or_default();
                if in_href { cur_href.push_str(&text); }
                if in_etag { cur_etag.push_str(&text); }
                if in_data { cur_ical.push_str(&text); }
            }
            Ok(XmlEvent::CData(t)) => {
                if in_data {
                    cur_ical.push_str(&String::from_utf8_lossy(&t));
                }
            }
            Ok(XmlEvent::End(e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "href" => in_href = false,
                    "getetag" => in_etag = false,
                    "calendar-data" => in_data = false,
                    "response" => {
                        if in_response && !cur_ical.trim().is_empty() {
                            out.push(ReportItem {
                                href: absolutize(base_url, cur_href.trim()),
                                etag: cur_etag.trim().to_string(),
                                ical: cur_ical.clone(),
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
```

- [ ] **Step 5: Add `fetch_ical`, `put_event`, `delete_event` methods to `CalDavClient`**

```rust
/// Fetch a single .ics resource. Returns (ical_body, etag).
pub async fn fetch_ical(&self, url: &str) -> Result<(String, String)> {
    let resp = self
        .http
        .get(url)
        .header(AUTHORIZATION, self.auth_header())
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        bail!("GET {url} returned {status}");
    }
    let etag = resp
        .headers()
        .get("ETag")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = resp.text().await?;
    Ok((body, etag))
}

/// PUT an iCal body to `url`. If `etag` is Some, sends `If-Match` for optimistic concurrency.
/// Returns the new ETag from the response headers (empty string if server doesn't return one).
/// Returns an error containing "conflict" if the server responds 412.
pub async fn put_event(&self, url: &str, body: &str, etag: Option<&str>) -> Result<String> {
    let mut req = self
        .http
        .put(url)
        .header(AUTHORIZATION, self.auth_header())
        .header(CONTENT_TYPE, HeaderValue::from_static("text/calendar; charset=utf-8"))
        .body(body.to_string());
    if let Some(e) = etag {
        req = req.header("If-Match", e);
    }
    let resp = req.send().await?;
    let status = resp.status();
    if status.as_u16() == 412 {
        bail!("conflict: event was modified by another client (412)");
    }
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        bail!("PUT {url} returned {status}: {text}");
    }
    let new_etag = resp
        .headers()
        .get("ETag")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    Ok(new_etag)
}

/// DELETE an event at `url` with `If-Match: etag`.
pub async fn delete_event(&self, url: &str, etag: &str) -> Result<()> {
    let resp = self
        .http
        .delete(url)
        .header(AUTHORIZATION, self.auth_header())
        .header("If-Match", etag)
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        bail!("DELETE {url} returned {status}");
    }
    Ok(())
}
```

- [ ] **Step 6: Fix the old test that called `report_events` and checked `ics[0]`**

The existing test `report_events_extracts_calendar_data_blocks` now uses `ReportItem`. Update it:

```rust
#[tokio::test]
async fn report_events_extracts_calendar_data_blocks() {
    let server = MockServer::start().await;
    let body = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/home/cal1/1.ics</D:href>
    <D:propstat><D:prop>
      <D:getetag>"etag1"</D:getetag>
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
        .mount(&server)
        .await;

    let client = CalDavClient::new("u", "p");
    let start = Utc::now() - chrono::Duration::days(7);
    let end = Utc::now() + chrono::Duration::days(14);
    let items = client
        .report_events(&format!("{}/home/cal1/", server.uri()), start, end)
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0].ical.contains("UID:u1"));
}
```

- [ ] **Step 7: Run tests — expect pass**

```bash
cargo test -p manor-app --lib 2>&1 | grep -E "^(test|FAILED|error)"
```

Expected: all tests pass including the 4 new ones.

- [ ] **Step 8: Commit**

```bash
git add crates/app/src/sync/caldav.rs
git commit -m "feat(sync): ReportItem return type + fetch_ical/put_event/delete_event"
```

---

## Task 5: iCal Write Module

**Files:**
- Create: `crates/app/src/sync/ical_write.rs`
- Modify: `crates/app/src/sync/mod.rs` (add `pub mod ical_write;`)

- [ ] **Step 1: Write failing tests first**

Create `crates/app/src/sync/ical_write.rs`:

```rust
//! iCal generation helpers for CalDAV write operations.
//! Produces RFC 5545-compliant VCALENDAR strings.

/// Format a UTC timestamp (unix seconds) as iCal DTSTART/DTEND value.
fn fmt_utc(ts: i64) -> String {
    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
        .unwrap_or_default();
    dt.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Fold a long iCal line at 75 octets per RFC 5545 §3.1.
/// Lines > 75 bytes are wrapped with CRLF + SP.
fn fold_line(line: &str) -> String {
    let bytes = line.as_bytes();
    if bytes.len() <= 75 {
        return line.to_string();
    }
    let mut out = String::new();
    let mut pos = 0;
    let mut first = true;
    while pos < bytes.len() {
        let limit = if first { 75 } else { 74 }; // first line 75, continuation 74 (1 for space)
        // find a safe split on a char boundary
        let end = (pos + limit).min(bytes.len());
        // walk back to char boundary if needed
        let mut safe = end;
        while safe > pos && !line.is_char_boundary(safe) {
            safe -= 1;
        }
        if !first {
            out.push(' ');
        }
        out.push_str(&line[pos..safe]);
        out.push_str("\r\n");
        pos = safe;
        first = false;
    }
    out
}

/// Generate a complete VCALENDAR string for a new VEVENT.
pub fn generate_vcalendar(
    uid: &str,
    summary: &str,
    dtstart_utc: i64,
    dtend_utc: i64,
    description: Option<&str>,
    location: Option<&str>,
    all_day: bool,
) -> String {
    let (dtstart_val, dtend_val) = if all_day {
        let start_dt = chrono::DateTime::<chrono::Utc>::from_timestamp(dtstart_utc, 0).unwrap_or_default();
        let end_dt = chrono::DateTime::<chrono::Utc>::from_timestamp(dtend_utc, 0).unwrap_or_default();
        (
            format!("VALUE=DATE:{}", start_dt.format("%Y%m%d")),
            format!("VALUE=DATE:{}", end_dt.format("%Y%m%d")),
        )
    } else {
        (fmt_utc(dtstart_utc), fmt_utc(dtend_utc))
    };

    let mut lines: Vec<String> = vec![
        "BEGIN:VCALENDAR".into(),
        "VERSION:2.0".into(),
        "PRODID:-//Manor//CalDAV Write//EN".into(),
        "BEGIN:VEVENT".into(),
        fold_line(&format!("UID:{uid}")),
        fold_line(&format!("SUMMARY:{summary}")),
    ];

    if all_day {
        lines.push(fold_line(&format!("DTSTART;{dtstart_val}")));
        lines.push(fold_line(&format!("DTEND;{dtend_val}")));
    } else {
        lines.push(fold_line(&format!("DTSTART:{dtstart_val}")));
        lines.push(fold_line(&format!("DTEND:{dtend_val}")));
    }

    if let Some(desc) = description {
        if !desc.is_empty() {
            lines.push(fold_line(&format!("DESCRIPTION:{desc}")));
        }
    }
    if let Some(loc) = location {
        if !loc.is_empty() {
            lines.push(fold_line(&format!("LOCATION:{loc}")));
        }
    }
    lines.push("END:VEVENT".into());
    lines.push("END:VCALENDAR".into());

    lines.join("\r\n") + "\r\n"
}

/// Add an EXDATE to a recurring parent event's iCal source to skip one occurrence.
/// `occurrence_dtstart_utc` is in `YYYYMMDDTHHMMSSz` format (iCal UTC notation).
pub fn add_exdate(ical: &str, occurrence_dtstart_utc: &str) -> String {
    // Insert EXDATE line immediately before END:VEVENT
    let exdate_line = format!("EXDATE:{occurrence_dtstart_utc}");
    ical.replacen(
        "END:VEVENT",
        &format!("{}\r\nEND:VEVENT", exdate_line),
        1,
    )
}

/// Add a RECURRENCE-ID override VEVENT to a parent iCal (edit one occurrence).
/// The override VEVENT is inserted before END:VCALENDAR.
pub fn add_recurrence_override(
    ical: &str,
    recurrence_id_utc: &str,
    summary: &str,
    dtstart_utc: i64,
    dtend_utc: i64,
    description: Option<&str>,
    location: Option<&str>,
) -> String {
    // Extract UID from parent
    let uid = ical
        .lines()
        .find(|l| l.trim_start_matches(' ').starts_with("UID:"))
        .map(|l| l.trim_start_matches(' ').trim_start_matches("UID:").trim())
        .unwrap_or("unknown");

    let mut override_lines: Vec<String> = vec![
        "BEGIN:VEVENT".into(),
        fold_line(&format!("UID:{uid}")),
        fold_line(&format!("RECURRENCE-ID:{recurrence_id_utc}")),
        fold_line(&format!("SUMMARY:{summary}")),
        fold_line(&format!("DTSTART:{}", fmt_utc(dtstart_utc))),
        fold_line(&format!("DTEND:{}", fmt_utc(dtend_utc))),
    ];
    if let Some(desc) = description {
        if !desc.is_empty() {
            override_lines.push(fold_line(&format!("DESCRIPTION:{desc}")));
        }
    }
    if let Some(loc) = location {
        if !loc.is_empty() {
            override_lines.push(fold_line(&format!("LOCATION:{loc}")));
        }
    }
    override_lines.push("END:VEVENT".into());

    let override_block = override_lines.join("\r\n");
    ical.replacen(
        "END:VCALENDAR",
        &format!("{}\r\nEND:VCALENDAR", override_block),
        1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_vcalendar_basic_event() {
        let ical = generate_vcalendar(
            "test-uid-1",
            "Team Lunch",
            1_745_000_000, // some unix timestamp
            1_745_003_600,
            None,
            None,
            false,
        );
        assert!(ical.contains("BEGIN:VCALENDAR"));
        assert!(ical.contains("UID:test-uid-1"));
        assert!(ical.contains("SUMMARY:Team Lunch"));
        assert!(ical.contains("BEGIN:VEVENT"));
        assert!(ical.contains("END:VEVENT"));
        assert!(ical.contains("END:VCALENDAR"));
        assert!(ical.ends_with("\r\n"), "must end with CRLF");
    }

    #[test]
    fn generate_vcalendar_all_day_uses_value_date() {
        // 2026-04-16 midnight UTC = 1744761600
        let ical = generate_vcalendar(
            "allday-1",
            "Holiday",
            1_744_761_600,
            1_744_848_000,
            None,
            None,
            true,
        );
        assert!(ical.contains("DTSTART;VALUE=DATE:20260416"));
        assert!(!ical.contains("DTSTART:20260416T"), "must not use time-based DTSTART for all-day");
    }

    #[test]
    fn generate_vcalendar_with_description_and_location() {
        let ical = generate_vcalendar(
            "ev-desc",
            "Meeting",
            1_745_000_000,
            1_745_003_600,
            Some("Quarterly planning"),
            Some("Conference Room A"),
            false,
        );
        assert!(ical.contains("DESCRIPTION:Quarterly planning"));
        assert!(ical.contains("LOCATION:Conference Room A"));
    }

    #[test]
    fn fold_line_wraps_at_75_bytes() {
        let long = "DESCRIPTION:".to_string() + &"x".repeat(80);
        let folded = fold_line(&long);
        // Every logical line after fold must be ≤ 75 octets (the continuation lines start with space)
        for physical_line in folded.split("\r\n").filter(|l| !l.is_empty()) {
            assert!(
                physical_line.len() <= 75,
                "line too long: {} bytes: {physical_line}",
                physical_line.len()
            );
        }
    }

    #[test]
    fn add_exdate_inserts_before_end_vevent() {
        let ical = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:rec-1\r\nRRULE:FREQ=WEEKLY\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let patched = add_exdate(ical, "20260422T090000Z");
        assert!(patched.contains("EXDATE:20260422T090000Z\r\nEND:VEVENT"));
    }

    #[test]
    fn add_recurrence_override_appends_vevent() {
        let parent = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:weekly-1\r\nRRULE:FREQ=WEEKLY\r\nDTSTART:20260415T090000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let patched = add_recurrence_override(
            parent,
            "20260422T090000Z",
            "Standup — renamed",
            1_745_600_000,
            1_745_603_600,
            None,
            None,
        );
        assert!(patched.contains("RECURRENCE-ID:20260422T090000Z"));
        assert!(patched.contains("SUMMARY:Standup — renamed"));
        // The original RRULE event must still be present
        assert!(patched.contains("RRULE:FREQ=WEEKLY"));
        // Two END:VEVENT (parent + override)
        assert_eq!(patched.matches("END:VEVENT").count(), 2);
    }
}
```

- [ ] **Step 2: Run — expect module not found error**

```bash
cargo test -p manor-app 2>&1 | head -20
```

Expected: `error[E0583]: file not found for module 'ical_write'`.

- [ ] **Step 3: Add module to sync/mod.rs**

In `crates/app/src/sync/mod.rs`, add:

```rust
pub mod ical_write;
```

- [ ] **Step 4: Run tests — expect pass**

```bash
cargo test -p manor-app --lib sync 2>&1 | grep -E "^(test|FAILED|error)"
```

Expected: 6 new ical_write tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/sync/ical_write.rs crates/app/src/sync/mod.rs
git commit -m "feat(sync): ical_write module — generate_vcalendar, add_exdate, add_recurrence_override"
```

---

## Task 6: Update expand.rs

**Files:**
- Modify: `crates/app/src/sync/expand.rs`

- [ ] **Step 1: Write failing test**

Add to the `#[cfg(test)]` block in `expand.rs`:

```rust
#[test]
fn non_recurring_event_gets_event_url() {
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
    let out = expand(&ev, 1, start, end, "https://cal.example.com/home/event.ics").unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].event_url.as_deref(), Some("https://cal.example.com/home/event.ics"));
    assert!(!out[0].is_recurring_occurrence);
}

#[test]
fn recurring_occurrences_get_parent_url_and_flag() {
    let ev = sample_weekly();
    let start = Utc.with_ymd_and_hms(2026, 4, 8, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 4, 29, 0, 0, 0).unwrap();
    let out = expand(&ev, 1, start, end, "https://cal.example.com/home/standup.ics").unwrap();
    assert!(out.len() >= 1);
    assert!(out[0].is_recurring_occurrence);
    assert_eq!(
        out[0].parent_event_url.as_deref(),
        Some("https://cal.example.com/home/standup.ics")
    );
    assert!(out[0].occurrence_dtstart.is_some());
}
```

- [ ] **Step 2: Run — expect compile error (extra arg)**

```bash
cargo test -p manor-app --lib sync::expand 2>&1 | head -20
```

Expected: argument count mismatch on `expand(...)` calls.

- [ ] **Step 3: Update `expand` signature and body**

Replace the `expand` function signature and add the new fields to `NewEvent` construction:

```rust
pub fn expand(
    ev: &ParsedEvent,
    account_id: i64,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    event_url: &str,
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
                event_url: Some(event_url.to_string()),
                etag: None,
                description: None,
                location: None,
                all_day: false,
                is_recurring_occurrence: false,
                parent_event_url: None,
                occurrence_dtstart: None,
            }]);
        }
        return Ok(vec![]);
    };

    // Recurring: build RRuleSet and enumerate.
    let parent_start_utc = DateTime::<Utc>::from_timestamp(ev.start_at, 0)
        .ok_or_else(|| anyhow::anyhow!("bad parent start_at"))?;

    let dtstart_line = format!("DTSTART:{}\n", parent_start_utc.format("%Y%m%dT%H%M%SZ"));
    let rule_block = format!("{dtstart_line}RRULE:{rrule_str}");
    let mut rset = RRuleSet::from_str(&rule_block)?;

    let window_start_rrule = window_start.with_timezone(&RruleTz::UTC);
    let window_end_rrule = window_end.with_timezone(&RruleTz::UTC);

    rset = rset.after(window_start_rrule).before(window_end_rrule);
    let result = rset.all(10_000);

    let exdate_set: std::collections::HashSet<String> = ev.exdates.iter().cloned().collect();

    let out = result
        .dates
        .into_iter()
        .filter_map(|occ| {
            let occ_utc = occ.with_timezone(&Utc);
            let rfc = occ_utc.to_rfc3339();
            if exdate_set.contains(&rfc) {
                return None;
            }
            let start = occ_utc.timestamp();
            let occ_dtstart = occ_utc.format("%Y%m%dT%H%M%SZ").to_string();
            Some(NewEvent {
                calendar_account_id: account_id,
                external_id: format!("{}::{}", ev.uid, rfc),
                title: ev.summary.clone(),
                start_at: start,
                end_at: start + duration,
                event_url: None,
                etag: None,
                description: None,
                location: None,
                all_day: false,
                is_recurring_occurrence: true,
                parent_event_url: Some(event_url.to_string()),
                occurrence_dtstart: Some(occ_dtstart),
            })
        })
        .collect();

    Ok(out)
}
```

- [ ] **Step 4: Fix existing test calls to `expand`**

All existing `expand(&ev, 1, start, end)` calls in expand.rs tests need the `event_url` argument. Add `"https://cal.example.com/event.ics"` as the fifth argument to each call.

- [ ] **Step 5: Run tests — expect pass**

```bash
cargo test -p manor-app --lib sync::expand 2>&1 | grep -E "^(test|FAILED|error)"
```

Expected: all tests pass including the 2 new ones.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/sync/expand.rs
git commit -m "feat(sync): pass event_url through expand — recurring occurrences get parent_event_url"
```

---

## Task 7: Update Sync Engine

**Files:**
- Modify: `crates/app/src/sync/engine.rs`

- [ ] **Step 1: Update engine imports**

At the top of `engine.rs`, add `calendar` to the `manor_core::assistant` import:

```rust
use manor_core::assistant::{calendar, calendar_account, event};
```

- [ ] **Step 2: Update `do_sync` — use ReportItem, store calendars, set default, pass href**

Replace the `do_sync` function body. Key changes:
1. Iterate `ReportItem` instead of raw ical strings
2. Upsert each calendar in DB
3. Set default calendar if not already set
4. Pass `item.href` to `expand`
5. Update inline INSERT to include all new columns

```rust
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

    // Persist discovered calendars.
    for cal in &calendars {
        calendar::upsert(conn, account_id, &cal.url, cal.display_name.as_deref())?;
    }

    // Set default calendar if not already set.
    if account.default_calendar_url.is_none() {
        let noise = ["shared", "subscribed", "birthdays", "holidays"];
        let default = calendars.iter().find(|c| {
            let name = c.display_name.as_deref().unwrap_or("").to_lowercase();
            !noise.iter().any(|n| name.contains(n))
        });
        if let Some(cal) = default {
            calendar_account::set_default_calendar(conn, account_id, &cal.url)?;
        }
    }

    let window_start = Utc::now() - Duration::days(7);
    let window_end = Utc::now() + Duration::days(14);

    let mut new_events: Vec<event::NewEvent> = Vec::new();
    for cal in &calendars {
        let items = client
            .report_events(&cal.url, window_start, window_end)
            .await?;
        for item in &items {
            for parsed in ical::parse_vcalendar(&item.ical, local_tz) {
                match expand::expand(&parsed, account_id, window_start, window_end, &item.href) {
                    Ok(mut occurrences) => new_events.append(&mut occurrences),
                    Err(e) => tracing::warn!("skipping expansion for uid {}: {e}", parsed.uid),
                }
            }
        }
    }

    let tx = conn.unchecked_transaction()?;
    event::delete_for_account(&tx, account_id)?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO event (
                calendar_account_id, external_id, title, start_at, end_at, created_at,
                event_url, etag, description, location, all_day,
                is_recurring_occurrence, parent_event_url, occurrence_dtstart
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?;
        let created_at = Utc::now().timestamp_millis();
        for ev in &new_events {
            let _ = stmt.execute(rusqlite::params![
                ev.calendar_account_id,
                ev.external_id,
                ev.title,
                ev.start_at,
                ev.end_at,
                created_at,
                ev.event_url,
                ev.etag,
                ev.description,
                ev.location,
                ev.all_day as i64,
                ev.is_recurring_occurrence as i64,
                ev.parent_event_url,
                ev.occurrence_dtstart,
            ]);
        }
    }
    tx.commit()?;

    Ok(new_events.len() as u32)
}
```

- [ ] **Step 3: Update engine tests — REPORT mock body needs etag + href**

The `report_body_three_events()` test helper needs `<D:getetag>` in each response. Update it:

```rust
fn report_body_three_events() -> String {
    r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response><D:href>/home/cal1/1.ics</D:href><D:propstat><D:prop>
    <D:getetag>"etag1"</D:getetag>
    <C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:one
DTSTART:20260415T100000Z
DTEND:20260415T110000Z
SUMMARY:One
END:VEVENT
END:VCALENDAR</C:calendar-data></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>
  <D:response><D:href>/home/cal1/2.ics</D:href><D:propstat><D:prop>
    <D:getetag>"etag2"</D:getetag>
    <C:calendar-data>BEGIN:VCALENDAR
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
```

- [ ] **Step 4: Run tests — expect pass**

```bash
cargo test -p manor-app 2>&1 | grep -E "^(test|FAILED|error)"
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/sync/engine.rs
git commit -m "feat(sync): store calendars + set default + pass href to expand in do_sync"
```

---

## Task 8: New Tauri Commands + lib.rs Registration

**Files:**
- Modify: `crates/app/src/assistant/commands.rs`
- Modify: `crates/app/src/lib.rs`

- [ ] **Step 1: Add imports to commands.rs**

At the top of `commands.rs`, add to the `manor_core::assistant` import:

```rust
use manor_core::assistant::{
    calendar::{self, Calendar},
    calendar_account::{self, CalendarAccount},
    // ... keep all existing imports
};
```

And add `uuid` generation for new event UIDs:

```rust
// At top of file, after other imports:
fn new_uid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("manor-{ts:x}")
}
```

- [ ] **Step 2: Add the 5 new commands**

Add after the `list_events_today` command:

```rust
// === Calendar list + default ===

#[tauri::command]
pub fn list_calendars(
    db: State<'_, Db>,
    account_id: i64,
) -> Result<Vec<Calendar>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    calendar::list(&conn, account_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_default_calendar(
    db: State<'_, Db>,
    account_id: i64,
    url: String,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    calendar_account::set_default_calendar(&conn, account_id, &url)
        .map_err(|e| e.to_string())
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
    let url = format!("{}{}.ics", args.calendar_url.trim_end_matches('/').to_string() + "/", uid);

    let account_id = args.account_id;
    let db_arc = db.clone_arc();
    let sync_state_arc = sync_state.inner().clone();

    let handle = tokio::runtime::Handle::current();
    tauri::async_runtime::spawn_blocking(move || {
        // PUT to CalDAV server
        let rt = tokio::runtime::Handle::current();
        let account = {
            let conn = db_arc.lock().unwrap();
            calendar_account::get(&conn, account_id)
                .ok()
                .flatten()
        }
        .ok_or_else(|| "account not found".to_string())?;

        let client = crate::sync::caldav::CalDavClient::new(&account.username, &password);
        handle.block_on(client.put_event(&url, &ical, None))
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
        let ev = events.iter().find(|e| e.id == args.event_id)
            .ok_or_else(|| "event not found".to_string())?
            .clone();
        let pw = crate::sync::keychain::get_password(ev.calendar_account_id)
            .map_err(|e| format!("keychain: {e}"))?;
        (
            ev.calendar_account_id,
            ev.event_url.clone().ok_or("event has no URL (manual event?)")?,
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
                .map(|l| l.trim_start_matches(' ').trim_start_matches("UID:").trim().to_string())
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
        let ev = events.iter().find(|e| e.id == args.event_id)
            .ok_or_else(|| "event not found".to_string())?
            .clone();
        let pw = crate::sync::keychain::get_password(ev.calendar_account_id)
            .map_err(|e| format!("keychain: {e}"))?;
        (
            ev.calendar_account_id,
            ev.event_url.clone().ok_or("event has no URL (manual event?)")?,
            ev.is_recurring_occurrence,
            ev.parent_event_url.clone(),
            ev.occurrence_dtstart.clone(),
            pw,
        )
    };

    // Optimistically soft-delete in DB so the UI updates immediately.
    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        event::soft_delete(&conn, args.event_id).map_err(|e| e.to_string())?;
    }

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
```

- [ ] **Step 3: Register 5 new commands in lib.rs**

In `crates/app/src/lib.rs`, add to `invoke_handler![]`:

```rust
assistant::commands::list_calendars,
assistant::commands::set_default_calendar,
assistant::commands::create_event,
assistant::commands::update_event,
assistant::commands::delete_event,
```

- [ ] **Step 4: Build to verify**

```bash
cd /Users/hanamori/life-assistant
cargo build -p manor-app 2>&1 | grep -E "^error"
```

Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/assistant/commands.rs crates/app/src/lib.rs
git commit -m "feat(app): create_event, update_event, delete_event, list_calendars, set_default_calendar commands"
```

---

## Task 9: Frontend Types, IPC, Store Updates

**Files:**
- Modify: `apps/desktop/src/lib/today/ipc.ts`
- Modify: `apps/desktop/src/lib/settings/ipc.ts`
- Modify: `apps/desktop/src/lib/today/state.ts`
- Modify: `apps/desktop/src/lib/settings/state.ts`

- [ ] **Step 1: Update `apps/desktop/src/lib/today/ipc.ts`**

Read the current file first, then expand the `Event` interface and add write functions:

```typescript
// Add to Event interface:
export interface Event {
  id: number;
  calendar_account_id: number;
  external_id: string;
  title: string;
  start_at: number;
  end_at: number;
  created_at: number;
  event_url: string | null;
  etag: string | null;
  description: string | null;
  location: string | null;
  all_day: boolean;
  is_recurring_occurrence: boolean;
  parent_event_url: string | null;
  occurrence_dtstart: string | null;
}

export interface CreateEventArgs {
  account_id: number;
  calendar_url: string;
  title: string;
  start_at: number;
  end_at: number;
  description?: string;
  location?: string;
  all_day: boolean;
}

export interface UpdateEventArgs {
  event_id: number;
  title: string;
  start_at: number;
  end_at: number;
  description?: string;
  location?: string;
  all_day: boolean;
  edit_occurrence_only: boolean;
}

export interface DeleteEventArgs {
  event_id: number;
  delete_occurrence_only: boolean;
}

export async function createEvent(args: CreateEventArgs): Promise<void> {
  return invoke("create_event", { args });
}

export async function updateEvent(args: UpdateEventArgs): Promise<void> {
  return invoke("update_event", { args });
}

export async function deleteEvent(args: DeleteEventArgs): Promise<void> {
  return invoke("delete_event", { args });
}
```

- [ ] **Step 2: Update `apps/desktop/src/lib/settings/ipc.ts`**

Add to `CalendarAccount` interface and add new functions:

```typescript
// Add to CalendarAccount:
default_calendar_url: string | null;

// New:
export interface CalendarInfo {
  id: number;
  calendar_account_id: number;
  url: string;
  display_name: string | null;
}

export async function listCalendars(accountId: number): Promise<CalendarInfo[]> {
  return invoke("list_calendars", { accountId });
}

export async function setDefaultCalendar(accountId: number, url: string): Promise<void> {
  return invoke("set_default_calendar", { accountId, url });
}
```

- [ ] **Step 3: Update `apps/desktop/src/lib/today/state.ts`**

Read the current file, then add `upsertEvent` and `removeEvent` mutations to the Zustand store:

```typescript
upsertEvent: (e: Event) => set((s) => {
  const idx = s.events.findIndex((x) => x.id === e.id);
  if (idx >= 0) {
    const next = [...s.events];
    next[idx] = e;
    return { events: next };
  }
  return { events: [...s.events, e].sort((a, b) => a.start_at - b.start_at) };
}),
removeEvent: (id: number) => set((s) => ({
  events: s.events.filter((e) => e.id !== id),
})),
```

- [ ] **Step 4: Update `apps/desktop/src/lib/settings/state.ts`**

Add `accountCalendars` map and `setCalendars` mutation to the settings store:

```typescript
accountCalendars: new Map<number, CalendarInfo[]>(),
setCalendars: (accountId: number, calendars: CalendarInfo[]) =>
  set((s) => {
    const next = new Map(s.accountCalendars);
    next.set(accountId, calendars);
    return { accountCalendars: next };
  }),
```

- [ ] **Step 5: Type-check**

```bash
cd /Users/hanamori/life-assistant/apps/desktop
npm run type-check 2>&1 | grep -E "^(error|Error)"
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/lib/today/ipc.ts apps/desktop/src/lib/settings/ipc.ts
git add apps/desktop/src/lib/today/state.ts apps/desktop/src/lib/settings/state.ts
git commit -m "feat(frontend): Event write types + IPC + store mutations"
```

---

## Task 10: EventsCard Updates

**Files:**
- Modify: `apps/desktop/src/components/Today/EventsCard.tsx`

- [ ] **Step 1: Read the current EventsCard**

```bash
cat apps/desktop/src/components/Today/EventsCard.tsx
```

- [ ] **Step 2: Add `+` button, row click state, import drawers**

The modified EventsCard should:
1. Import `AddEventDrawer` and `EditEventDrawer` (created in next tasks — import defensively)
2. Add `showAdd: boolean` and `editingEvent: Event | null` state
3. Render a `+` button in the card header
4. Make each event row clickable (sets `editingEvent`)
5. Render `{showAdd && <AddEventDrawer … />}` and `{editingEvent && <EditEventDrawer … />}`

Key additions to the header area:

```tsx
<div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
  <div style={{ fontSize: 13, fontWeight: 700, color: "rgba(0,0,0,0.5)", textTransform: "uppercase", letterSpacing: 0.5 }}>
    Today's Events
  </div>
  <button
    onClick={() => setShowAdd(true)}
    style={{
      background: "var(--imessage-blue)",
      color: "white",
      border: "none",
      borderRadius: 8,
      padding: "4px 10px",
      fontSize: 13,
      fontWeight: 700,
      cursor: "pointer",
      fontFamily: "inherit",
    }}
  >
    +
  </button>
</div>
```

Each event row gets `onClick={() => setEditingEvent(event)}` and a pointer cursor:

```tsx
style={{ cursor: "pointer" }}
onClick={() => setEditingEvent(ev)}
```

- [ ] **Step 3: Type-check**

```bash
cd /Users/hanamori/life-assistant/apps/desktop
npm run type-check 2>&1 | grep -E "^(error|Error)"
```

Expected: errors only for missing `AddEventDrawer`/`EditEventDrawer` (they don't exist yet — that's fine, comment them out temporarily or use a `TODO` placeholder until Task 11/12).

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Today/EventsCard.tsx
git commit -m "feat(Today): EventsCard add-button + clickable rows"
```

---

## Task 11: AddEventDrawer

**Files:**
- Create: `apps/desktop/src/components/Today/AddEventDrawer.tsx`

- [ ] **Step 1: Write the component**

The drawer follows the same pattern as `AddTransactionForm.tsx` (slide-in from right, modal overlay, footer save button).

```tsx
import { useState } from "react";
import { createEvent } from "../../lib/today/ipc";
import type { CalendarInfo } from "../../lib/settings/ipc";

interface Props {
  accountId: number;
  defaultCalendarUrl: string;
  calendars: CalendarInfo[];
  onClose: () => void;
  onSaved: () => Promise<void>;
}

function todayStartSecs(): number {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return Math.floor(d.getTime() / 1000);
}

function toDateInputValue(ts: number): string {
  return new Date(ts * 1000).toISOString().slice(0, 10);
}

function toTimeInputValue(ts: number): string {
  return new Date(ts * 1000).toISOString().slice(11, 16);
}

function combineDateTime(date: string, time: string): number {
  return Math.floor(new Date(`${date}T${time}:00`).getTime() / 1000);
}

export default function AddEventDrawer({ accountId, defaultCalendarUrl, calendars, onClose, onSaved }: Props) {
  const now = Math.floor(Date.now() / 1000);
  const todayDate = toDateInputValue(now);
  const [title, setTitle] = useState("");
  const [date, setDate] = useState(todayDate);
  const [startTime, setStartTime] = useState("09:00");
  const [endTime, setEndTime] = useState("10:00");
  const [allDay, setAllDay] = useState(false);
  const [description, setDescription] = useState("");
  const [location, setLocation] = useState("");
  const [calendarUrl, setCalendarUrl] = useState(defaultCalendarUrl);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  async function handleSave() {
    if (!title.trim()) { setError("Enter a title"); return; }
    const start_at = allDay
      ? Math.floor(new Date(date + "T00:00:00").getTime() / 1000)
      : combineDateTime(date, startTime);
    const end_at = allDay
      ? start_at + 86400
      : combineDateTime(date, endTime);
    if (end_at <= start_at) { setError("End must be after start"); return; }

    setSaving(true);
    setError(null);
    try {
      await createEvent({
        account_id: accountId,
        calendar_url: calendarUrl,
        title: title.trim(),
        start_at,
        end_at,
        description: description.trim() || undefined,
        location: location.trim() || undefined,
        all_day: allDay,
      });
      await onSaved();
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  }

  return (
    <>
      <div
        onClick={onClose}
        style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.25)", zIndex: 700 }}
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
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "18px 20px 14px", borderBottom: "1px solid var(--hairline)" }}>
          <div style={{ fontSize: 16, fontWeight: 700 }}>Add Event</div>
          <button onClick={onClose} style={{ background: "none", border: "none", fontSize: 20, cursor: "pointer", color: "rgba(0,0,0,0.4)", lineHeight: 1, padding: 0 }}>✕</button>
        </div>

        {/* Body */}
        <div style={{ flex: 1, overflow: "auto", padding: "20px" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div>
              <label style={labelStyle}>Title</label>
              <input style={inputStyle} type="text" placeholder="e.g. Team Lunch" value={title} onChange={(e) => setTitle(e.target.value)} />
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <label style={{ ...labelStyle, marginBottom: 0 }}>All Day</label>
              <input type="checkbox" checked={allDay} onChange={(e) => setAllDay(e.target.checked)} />
            </div>

            <div>
              <label style={labelStyle}>Date</label>
              <input style={inputStyle} type="date" value={date} onChange={(e) => setDate(e.target.value)} />
            </div>

            {!allDay && (
              <div style={{ display: "flex", gap: 12 }}>
                <div style={{ flex: 1 }}>
                  <label style={labelStyle}>Start</label>
                  <input style={inputStyle} type="time" value={startTime} onChange={(e) => setStartTime(e.target.value)} />
                </div>
                <div style={{ flex: 1 }}>
                  <label style={labelStyle}>End</label>
                  <input style={inputStyle} type="time" value={endTime} onChange={(e) => setEndTime(e.target.value)} />
                </div>
              </div>
            )}

            <div>
              <label style={labelStyle}>Description (optional)</label>
              <input style={inputStyle} type="text" placeholder="Optional description" value={description} onChange={(e) => setDescription(e.target.value)} />
            </div>

            <div>
              <label style={labelStyle}>Location (optional)</label>
              <input style={inputStyle} type="text" placeholder="Optional location" value={location} onChange={(e) => setLocation(e.target.value)} />
            </div>

            {calendars.length > 1 && (
              <div>
                <label style={labelStyle}>Calendar</label>
                <select style={{ ...inputStyle, appearance: "none" }} value={calendarUrl} onChange={(e) => setCalendarUrl(e.target.value)}>
                  {calendars.map((c) => (
                    <option key={c.id} value={c.url}>{c.display_name ?? c.url}</option>
                  ))}
                </select>
              </div>
            )}

            {error && (
              <div style={{ padding: "10px 12px", background: "rgba(255,59,48,0.08)", border: "1px solid rgba(255,59,48,0.3)", borderRadius: 10, fontSize: 13, color: "var(--imessage-red)" }}>
                {error}
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div style={{ padding: "14px 20px", borderTop: "1px solid var(--hairline)" }}>
          <button
            onClick={handleSave}
            disabled={saving}
            style={{ width: "100%", padding: "12px 0", background: "var(--imessage-blue)", color: "white", border: "none", borderRadius: 12, fontSize: 15, fontWeight: 700, cursor: saving ? "default" : "pointer", opacity: saving ? 0.6 : 1, fontFamily: "inherit" }}
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
cd /Users/hanamori/life-assistant/apps/desktop
npm run type-check 2>&1 | grep -E "^(error|Error)"
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/components/Today/AddEventDrawer.tsx
git commit -m "feat(Today): AddEventDrawer — create event slide-in form"
```

---

## Task 12: EditEventDrawer

**Files:**
- Create: `apps/desktop/src/components/Today/EditEventDrawer.tsx`

- [ ] **Step 1: Write the component**

```tsx
import { useState } from "react";
import { updateEvent, deleteEvent } from "../../lib/today/ipc";
import type { Event } from "../../lib/today/ipc";

interface Props {
  event: Event;
  onClose: () => void;
  onSaved: () => Promise<void>;
}

function toDateInputValue(ts: number): string {
  return new Date(ts * 1000).toISOString().slice(0, 10);
}

function toTimeInputValue(ts: number): string {
  return new Date(ts * 1000).toISOString().slice(11, 16);
}

function combineDateTime(date: string, time: string): number {
  return Math.floor(new Date(`${date}T${time}:00`).getTime() / 1000);
}

export default function EditEventDrawer({ event, onClose, onSaved }: Props) {
  const [title, setTitle] = useState(event.title);
  const [date, setDate] = useState(toDateInputValue(event.start_at));
  const [startTime, setStartTime] = useState(toTimeInputValue(event.start_at));
  const [endTime, setEndTime] = useState(toTimeInputValue(event.end_at));
  const [allDay, setAllDay] = useState(event.all_day);
  const [description, setDescription] = useState(event.description ?? "");
  const [location, setLocation] = useState(event.location ?? "");
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isManual = !event.event_url;

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

  async function handleSave() {
    if (!title.trim()) { setError("Enter a title"); return; }
    const start_at = allDay
      ? Math.floor(new Date(date + "T00:00:00").getTime() / 1000)
      : combineDateTime(date, startTime);
    const end_at = allDay ? start_at + 86400 : combineDateTime(date, endTime);
    if (end_at <= start_at) { setError("End must be after start"); return; }

    setSaving(true);
    setError(null);
    try {
      await updateEvent({
        event_id: event.id,
        title: title.trim(),
        start_at,
        end_at,
        description: description.trim() || undefined,
        location: location.trim() || undefined,
        all_day: allDay,
        edit_occurrence_only: event.is_recurring_occurrence,
      });
      await onSaved();
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  }

  async function handleDelete(occurrenceOnly: boolean) {
    setDeleting(true);
    setError(null);
    try {
      await deleteEvent({ event_id: event.id, delete_occurrence_only: occurrenceOnly });
      await onSaved();
    } catch (e) {
      setError(String(e));
      setDeleting(false);
    }
  }

  return (
    <>
      <div onClick={onClose} style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.25)", zIndex: 700 }} />
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
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "18px 20px 14px", borderBottom: "1px solid var(--hairline)" }}>
          <div style={{ fontSize: 16, fontWeight: 700 }}>Edit Event</div>
          <button onClick={onClose} style={{ background: "none", border: "none", fontSize: 20, cursor: "pointer", color: "rgba(0,0,0,0.4)", lineHeight: 1, padding: 0 }}>✕</button>
        </div>

        {/* Body */}
        <div style={{ flex: 1, overflow: "auto", padding: "20px" }}>
          {isManual && (
            <div style={{ marginBottom: 16, padding: "8px 12px", background: "rgba(0,0,0,0.04)", borderRadius: 8, fontSize: 12, color: "rgba(0,0,0,0.5)" }}>
              Manual event — changes are local only
            </div>
          )}

          {event.is_recurring_occurrence && (
            <div style={{ marginBottom: 16, padding: "8px 12px", background: "rgba(0,122,255,0.08)", borderRadius: 8, fontSize: 12, color: "var(--imessage-blue)" }}>
              Recurring event — editing this occurrence only
            </div>
          )}

          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div>
              <label style={labelStyle}>Title</label>
              <input style={inputStyle} type="text" value={title} onChange={(e) => setTitle(e.target.value)} />
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <label style={{ ...labelStyle, marginBottom: 0 }}>All Day</label>
              <input type="checkbox" checked={allDay} onChange={(e) => setAllDay(e.target.checked)} />
            </div>

            <div>
              <label style={labelStyle}>Date</label>
              <input style={inputStyle} type="date" value={date} onChange={(e) => setDate(e.target.value)} />
            </div>

            {!allDay && (
              <div style={{ display: "flex", gap: 12 }}>
                <div style={{ flex: 1 }}>
                  <label style={labelStyle}>Start</label>
                  <input style={inputStyle} type="time" value={startTime} onChange={(e) => setStartTime(e.target.value)} />
                </div>
                <div style={{ flex: 1 }}>
                  <label style={labelStyle}>End</label>
                  <input style={inputStyle} type="time" value={endTime} onChange={(e) => setEndTime(e.target.value)} />
                </div>
              </div>
            )}

            <div>
              <label style={labelStyle}>Description (optional)</label>
              <input style={inputStyle} type="text" value={description} onChange={(e) => setDescription(e.target.value)} />
            </div>

            <div>
              <label style={labelStyle}>Location (optional)</label>
              <input style={inputStyle} type="text" value={location} onChange={(e) => setLocation(e.target.value)} />
            </div>

            {error && (
              <div style={{ padding: "10px 12px", background: "rgba(255,59,48,0.08)", border: "1px solid rgba(255,59,48,0.3)", borderRadius: 10, fontSize: 13, color: "var(--imessage-red)" }}>
                {error}
              </div>
            )}

            {/* Delete section */}
            {!confirmDelete ? (
              <button
                onClick={() => setConfirmDelete(true)}
                style={{ marginTop: 8, background: "none", border: "1px solid rgba(255,59,48,0.4)", borderRadius: 10, padding: "10px 0", color: "var(--imessage-red)", fontSize: 13, fontWeight: 600, cursor: "pointer", fontFamily: "inherit" }}
              >
                Delete Event
              </button>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {event.is_recurring_occurrence && (
                  <button
                    onClick={() => handleDelete(true)}
                    disabled={deleting}
                    style={{ background: "rgba(255,59,48,0.08)", border: "1px solid rgba(255,59,48,0.4)", borderRadius: 10, padding: "10px 0", color: "var(--imessage-red)", fontSize: 13, fontWeight: 600, cursor: "pointer", fontFamily: "inherit" }}
                  >
                    Delete this occurrence only
                  </button>
                )}
                <button
                  onClick={() => handleDelete(false)}
                  disabled={deleting}
                  style={{ background: "rgba(255,59,48,0.15)", border: "1px solid rgba(255,59,48,0.6)", borderRadius: 10, padding: "10px 0", color: "var(--imessage-red)", fontSize: 14, fontWeight: 700, cursor: "pointer", fontFamily: "inherit" }}
                >
                  {event.is_recurring_occurrence ? "Delete all occurrences" : "Confirm Delete"}
                </button>
                <button onClick={() => setConfirmDelete(false)} style={{ background: "none", border: "none", fontSize: 13, color: "rgba(0,0,0,0.4)", cursor: "pointer", fontFamily: "inherit" }}>
                  Cancel
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div style={{ padding: "14px 20px", borderTop: "1px solid var(--hairline)" }}>
          <button
            onClick={handleSave}
            disabled={saving}
            style={{ width: "100%", padding: "12px 0", background: "var(--imessage-blue)", color: "white", border: "none", borderRadius: 12, fontSize: 15, fontWeight: 700, cursor: saving ? "default" : "pointer", opacity: saving ? 0.6 : 1, fontFamily: "inherit" }}
          >
            {saving ? "Saving…" : "Save Changes"}
          </button>
        </div>
      </div>
    </>
  );
}
```

- [ ] **Step 2: Type-check**

```bash
cd /Users/hanamori/life-assistant/apps/desktop
npm run type-check 2>&1 | grep -E "^(error|Error)"
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/components/Today/EditEventDrawer.tsx
git commit -m "feat(Today): EditEventDrawer — edit/delete event with recurring occurrence support"
```

---

## Task 13: Settings AccountRow — Default Calendar Picker

**Files:**
- Modify: `apps/desktop/src/components/Settings/AccountRow.tsx`

- [ ] **Step 1: Read the current AccountRow**

```bash
cat apps/desktop/src/components/Settings/AccountRow.tsx
```

- [ ] **Step 2: Add calendar picker below sync status**

In `AccountRow`, import the needed functions and add the picker. The component should:
1. Load calendars for the account on mount via `listCalendars(account.id)`
2. Render a `<select>` showing each calendar's `display_name ?? url`
3. Call `setDefaultCalendar(account.id, selectedUrl)` on change

```tsx
// Add to existing imports:
import { listCalendars, setDefaultCalendar } from "../../lib/settings/ipc";
import type { CalendarInfo } from "../../lib/settings/ipc";

// Add inside the component, after existing state:
const [calendars, setCalendars] = useState<CalendarInfo[]>([]);

useEffect(() => {
  listCalendars(account.id).then(setCalendars).catch(() => {});
}, [account.id]);

// Add below the sync status section:
{calendars.length > 0 && (
  <div style={{ marginTop: 10, display: "flex", alignItems: "center", gap: 8 }}>
    <span style={{ fontSize: 11, color: "rgba(0,0,0,0.5)", minWidth: 100 }}>Default calendar</span>
    <select
      value={account.default_calendar_url ?? ""}
      onChange={async (e) => {
        await setDefaultCalendar(account.id, e.target.value);
        onRefresh?.();
      }}
      style={{
        flex: 1,
        padding: "5px 8px",
        fontSize: 12,
        border: "1px solid var(--hairline)",
        borderRadius: 8,
        background: "#fafafa",
        fontFamily: "inherit",
      }}
    >
      <option value="">Auto-select</option>
      {calendars.map((c) => (
        <option key={c.id} value={c.url}>
          {c.display_name ?? c.url}
        </option>
      ))}
    </select>
  </div>
)}
```

Note: `onRefresh` must be part of the component's existing `Props` interface (check the current file — add it if missing: `onRefresh?: () => void`).

- [ ] **Step 3: Type-check the full project**

```bash
cd /Users/hanamori/life-assistant/apps/desktop
npm run type-check 2>&1 | grep -E "^(error|Error)"
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Settings/AccountRow.tsx
git commit -m "feat(Settings): default calendar picker in AccountRow"
```

---

## Self-Review Checklist

After writing this plan, checking spec coverage:

1. **V6 migration** — Task 1 covers all schema changes including the `calendar` table and `deleted_at` on `event`. ✓
2. **Event struct expansion** — Task 2 adds all 8 new fields. `soft_delete` added. `list_today` updated. ✓
3. **Calendar DAL** — Task 3 creates `calendar.rs` with `upsert` + `list`. `set_default_calendar` added to `calendar_account`. ✓
4. **ReportItem** — Task 4 changes `report_events` return type and updates the extractor. Old test updated. ✓
5. **CalDAV write methods** — Task 4 adds `fetch_ical`, `put_event`, `delete_event`. 412 → "conflict" error. ✓
6. **iCal generation** — Task 5 covers all 3 functions + line folding + all-day. ✓
7. **expand.rs** — Task 6 adds `event_url` param; non-recurring gets `event_url`; recurring gets `parent_event_url`, `is_recurring_occurrence`, `occurrence_dtstart`. ✓
8. **Sync engine** — Task 7 stores calendars, sets default (noise filter), passes href to expand. ✓
9. **5 new commands** — Task 8 covers all: `create_event`, `update_event`, `delete_event`, `set_default_calendar`, `list_calendars`. Registered in lib.rs. ✓
10. **Frontend types + IPC** — Task 9 expands Event interface and adds write functions. ✓
11. **Store mutations** — Task 9 adds `upsertEvent`, `removeEvent`, `accountCalendars`, `setCalendars`. ✓
12. **EventsCard** — Task 10 adds `+` button and clickable rows. ✓
13. **AddEventDrawer** — Task 11 with calendar picker when >1 calendar. ✓
14. **EditEventDrawer** — Task 12 with per-occurrence edit/delete confirmation. ✓
15. **AccountRow default calendar** — Task 13. ✓
16. **spawn_blocking pattern** — Tasks 8 write commands all use `spawn_blocking`. ✓
17. **Optimistic delete** — Task 8 `delete_event` soft-deletes in DB before CalDAV call. ✓
