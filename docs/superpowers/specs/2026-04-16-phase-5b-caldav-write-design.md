# Phase 5b — CalDAV Write

**Date:** 2026-04-16
**Phase:** v0.2 Rhythm gap-close (CalDAV write was listed in v0.2 scope but deferred from Phase 4)
**Status:** Spec — approved, ready to plan

---

## 1. Summary

Manor can read your calendar. This phase closes the gap by letting you write to it too — create, edit, and delete events directly from Manor, with changes pushed to CalDAV and synced back. Full CRUD on both one-time and recurring events. Editing or deleting a recurring occurrence modifies that occurrence only (EXDATE for delete, RECURRENCE-ID override for edit), leaving the rest of the series intact.

Architecture: synchronous push + re-sync. Every write operation calls CalDAV immediately; on success, a background re-sync pulls the canonical state back. On 412 conflict (another client modified the event), Manor surfaces an error and forces a re-sync so the user sees current truth.

---

## 2. Goals & non-goals

### 2.1 Goals

- **Full CRUD**: create, edit, delete — for both one-time and recurring events
- **Recurring occurrence editing** — delete adds EXDATE to parent; edit injects a RECURRENCE-ID VEVENT override into parent
- **Default write calendar** — per account, set during discovery, user-changeable in Settings
- **V6 schema migration** — extends `event` and `calendar_account` with write-supporting columns
- **Updated sync** — REPORT parser now extracts `event_url` and `etag` per event row
- **iCal write module** — generates and patches iCal strings (create, EXDATE, RECURRENCE-ID)
- **CalDAV PUT/DELETE client methods** — extends existing `CalDavClient`
- **UI**: `+` button on EventsCard, `AddEventDrawer`, `EditEventDrawer`, Settings default-calendar picker

### 2.2 Non-goals

| Item | Where it lives |
|---|---|
| Creating recurring events from Manor | v0.4+ — RRULE authoring is its own UX problem |
| Moving an event to a different calendar | Out of scope |
| Editing all occurrences in a series | Only this-occurrence supported; full-series edit deferred |
| Attendees, invites, RSVP | Out of scope |
| Google Calendar / OAuth providers | Separate future phase |
| Offline event queue | Not needed — Manor is a desktop app used when online |
| Conflict auto-merge | 412 → error + re-sync; user retries |

---

## 3. Architecture

Five things change or are added:

1. **V6 schema migration** — ALTER TABLE additions to `event` and `calendar_account`
2. **Sync engine update** — extract `event_url` + `etag` from REPORT responses; rrule expander marks occurrence rows with parent metadata
3. **CalDAV write layer** — three new methods on `CalDavClient`: `fetch_ical`, `put_event`, `delete_event`
4. **iCal write module** — new `crates/app/src/sync/ical_write.rs`: generate, patch-with-exdate, patch-with-recurrence-override
5. **Four new Tauri commands** — `create_event`, `update_event`, `delete_event`, `set_default_calendar`; plus UI in `AddEventDrawer`, `EditEventDrawer`, EventsCard updates, Settings update

### 3.1 File map

```
crates/core/migrations/V6__calendar_write.sql     — new migration
crates/core/src/assistant/event.rs                — MODIFIED: new fields on Event/NewEvent structs + soft_delete DAL
crates/core/src/assistant/calendar_account.rs     — MODIFIED: default_calendar_url field + set_default_calendar DAL
crates/core/src/assistant/calendar.rs             — NEW: Calendar struct + insert/list DAL
crates/app/src/sync/caldav.rs                     — MODIFIED: fetch_ical, put_event, delete_event + REPORT href/etag extraction
crates/app/src/sync/ical_write.rs                 — NEW: generate_vcalendar, add_exdate, add_recurrence_override
crates/app/src/sync/expand.rs                     — MODIFIED: set is_recurring_occurrence, parent_event_url, occurrence_dtstart
crates/app/src/assistant/commands.rs              — MODIFIED: 5 new commands (create_event, update_event, delete_event, set_default_calendar, list_calendars)
apps/desktop/src/components/Today/EventsCard.tsx  — MODIFIED: + button, clickable rows
apps/desktop/src/components/Today/AddEventDrawer.tsx   — NEW
apps/desktop/src/components/Today/EditEventDrawer.tsx  — NEW
apps/desktop/src/components/Settings/AccountRow.tsx    — MODIFIED: default-calendar picker
apps/desktop/src/lib/settings/ipc.ts              — MODIFIED: new command wrappers, updated types
apps/desktop/src/lib/today/ipc.ts                 — MODIFIED: new command wrappers
apps/desktop/src/lib/today/state.ts               — MODIFIED: upsertEvent, removeEvent mutations
```

---

## 4. Database Schema — V6 migration

```sql
-- crates/core/migrations/V6__calendar_write.sql

ALTER TABLE event ADD COLUMN event_url               TEXT;
ALTER TABLE event ADD COLUMN etag                    TEXT;
ALTER TABLE event ADD COLUMN description             TEXT;
ALTER TABLE event ADD COLUMN location                TEXT;
ALTER TABLE event ADD COLUMN all_day                 INTEGER NOT NULL DEFAULT 0;
ALTER TABLE event ADD COLUMN is_recurring_occurrence INTEGER NOT NULL DEFAULT 0;
ALTER TABLE event ADD COLUMN parent_event_url        TEXT;
ALTER TABLE event ADD COLUMN occurrence_dtstart      TEXT;

ALTER TABLE event ADD COLUMN deleted_at                INTEGER;

ALTER TABLE calendar_account ADD COLUMN default_calendar_url TEXT;
```

All new columns are nullable or have defaults — existing rows from V3 remain valid.

### Column semantics

| Column | Type | Meaning |
|---|---|---|
| `event_url` | TEXT NULL | CalDAV `href` for this event — used for PUT (non-recurring) / DELETE |
| `etag` | TEXT NULL | Current ETag from server — sent as `If-Match` on PUT/DELETE |
| `description` | TEXT NULL | DESCRIPTION property from iCal, or user-provided |
| `location` | TEXT NULL | LOCATION property from iCal, or user-provided |
| `all_day` | INTEGER (bool) | 1 if DTSTART was a DATE (not DATETIME) |
| `is_recurring_occurrence` | INTEGER (bool) | 1 for rows produced by rrule expansion |
| `parent_event_url` | TEXT NULL | CalDAV href of the parent VEVENT — used to fetch+patch for EXDATE / RECURRENCE-ID |
| `occurrence_dtstart` | TEXT NULL | UTC RFC3339 string of this occurrence's DTSTART — used as EXDATE / RECURRENCE-ID value |
| `default_calendar_url` | TEXT NULL (on calendar_account) | Where new events are PUT for this account |

---

## 5. CalDAV Write Layer

### 5.1 New methods on `CalDavClient` (`crates/app/src/sync/caldav.rs`)

```rust
/// Fetch raw iCal body + current ETag for an existing event.
pub async fn fetch_ical(&self, url: &str) -> Result<(String, String)>

/// Create (etag=None) or conditionally update (etag=Some) an event.
/// Uses If-Match: <etag> when etag is Some.
/// Returns the new ETag from the server's ETag response header.
/// 412 Precondition Failed → returns Err with message containing "conflict".
pub async fn put_event(&self, url: &str, body: &str, etag: Option<&str>) -> Result<String>

/// Conditionally delete an event.
/// Uses If-Match: <etag>.
/// 412 → Err with "conflict" message.
pub async fn delete_event(&self, url: &str, etag: &str) -> Result<()>
```

- PUT uses `Content-Type: text/calendar; charset=utf-8`
- Both PUT and DELETE include `If-Match: <etag>` when an etag is provided
- 412 response maps to `anyhow::bail!("conflict: event was modified by another client")`

### 5.2 Updated REPORT parser

The REPORT response parser currently extracts only `calendar-data`. It now also extracts `D:href` (→ `event_url`) and `D:getetag` (→ `etag`) from each `<D:response>` block. Both are stored on the `NewEvent` structs passed to `event::insert_many`.

### 5.3 Updated rrule expander (`expand.rs`)

For each expanded occurrence, the `NewEvent` gains:
- `is_recurring_occurrence = true`
- `parent_event_url = Some(parent_href)` — the href of the parent VEVENT
- `occurrence_dtstart = Some(occurrence_start.to_rfc3339())` — UTC RFC3339

---

## 6. iCal Write Module

New file: `crates/app/src/sync/ical_write.rs`

### 6.1 `generate_vcalendar`

```rust
pub fn generate_vcalendar(
    uid: &str,
    summary: &str,
    dtstart_utc: i64,   // unix seconds
    dtend_utc: i64,
    description: Option<&str>,
    location: Option<&str>,
    all_day: bool,
) -> String
```

Produces a minimal RFC 5545–compliant `VCALENDAR` string:

```
BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Manor//Manor Calendar//EN
BEGIN:VEVENT
UID:<uid>
DTSTART:<formatted>
DTEND:<formatted>
SUMMARY:<summary>
DESCRIPTION:<description>   ← omitted if None
LOCATION:<location>         ← omitted if None
END:VEVENT
END:VCALENDAR
```

Date format: `YYYYMMDDTHHMMSSZ` for datetime; `YYYYMMDD` (with `VALUE=DATE`) for all-day.
Lines longer than 75 octets are folded with `\r\n ` per RFC 5545 §3.1.

### 6.2 `add_exdate`

```rust
pub fn add_exdate(ical: &str, occurrence_dtstart_utc: &str) -> String
```

String-patches the raw iCal fetched from `parent_event_url`:
- Finds the last `END:VEVENT` in the string
- Inserts `EXDATE:<occurrence_dtstart_utc>` on the line before it
- If an `EXDATE` line already exists, appends a second `EXDATE` property (RFC 5545 permits multiple)

### 6.3 `add_recurrence_override`

```rust
pub fn add_recurrence_override(
    ical: &str,
    recurrence_id_utc: &str,   // = occurrence_dtstart (original start of the occurrence)
    summary: &str,
    dtstart_utc: i64,
    dtend_utc: i64,
    description: Option<&str>,
    location: Option<&str>,
) -> String
```

Finds `END:VCALENDAR` and injects a complete new VEVENT block before it:

```
BEGIN:VEVENT
UID:<same uid as parent>
RECURRENCE-ID:<recurrence_id_utc>
DTSTART:<new dtstart>
DTEND:<new dtend>
SUMMARY:<new summary>
DESCRIPTION:<new description>   ← omitted if None
LOCATION:<new location>         ← omitted if None
END:VEVENT
```

The UID is extracted from the parent iCal string (first `UID:` line).

---

## 7. Tauri Commands

Four new commands in `crates/app/src/assistant/commands.rs`:

### `create_event`

```rust
#[tauri::command]
async fn create_event(
    account_id: i64,
    title: String,
    start_ts: i64,
    end_ts: i64,
    description: Option<String>,
    location: Option<String>,
    all_day: bool,
    state: State<'_, Db>,
    sync_state: State<'_, SyncState>,
) -> Result<(), String>
```

1. Load `calendar_account` row → get `default_calendar_url` (error if NULL: *"No default calendar set for this account"*)
2. Generate UUID for UID
3. `ical_write::generate_vcalendar(uid, title, start_ts, end_ts, description, location, all_day)`
4. `caldav_client.put_event("<default_calendar_url>/<uid>.ics", body, None)` → new etag
5. Spawn `sync_account(account_id)` in background tokio task
6. Return `Ok(())`

### `update_event`

```rust
#[tauri::command]
async fn update_event(
    event_id: i64,
    title: String,
    start_ts: i64,
    end_ts: i64,
    description: Option<String>,
    location: Option<String>,
    state: State<'_, Db>,
    sync_state: State<'_, SyncState>,
) -> Result<(), String>
```

1. Load event row
2. **Non-recurring** (`is_recurring_occurrence = false`):
   - `fetch_ical(event_url)` → `(body, server_etag)`
   - Line-replace `SUMMARY:`, `DTSTART:`, `DTEND:`, `DESCRIPTION:`, `LOCATION:` in body
   - `put_event(event_url, patched, Some(server_etag))`
3. **Recurring occurrence** (`is_recurring_occurrence = true`):
   - `fetch_ical(parent_event_url)` → `(body, parent_etag)`
   - `ical_write::add_recurrence_override(body, occurrence_dtstart, title, start_ts, end_ts, description, location)`
   - `put_event(parent_event_url, patched, Some(parent_etag))`
4. Spawn `sync_account` in background
5. `"conflict"` in error string → frontend shows *"This event was changed elsewhere. Refreshing…"* and re-fetches

### `delete_event`

```rust
#[tauri::command]
async fn delete_event(
    event_id: i64,
    state: State<'_, Db>,
    sync_state: State<'_, SyncState>,
) -> Result<(), String>
```

1. Load event row
2. **Non-recurring**: `caldav_client.delete_event(event_url, etag)`
3. **Recurring occurrence**:
   - `fetch_ical(parent_event_url)` → `(body, parent_etag)`
   - `ical_write::add_exdate(body, occurrence_dtstart)`
   - `put_event(parent_event_url, patched, Some(parent_etag))`
4. Soft-delete local row immediately (`event::soft_delete(conn, event_id)`)
5. Spawn `sync_account` in background

### `set_default_calendar`

```rust
#[tauri::command]
async fn set_default_calendar(
    account_id: i64,
    calendar_url: String,
    state: State<'_, Db>,
) -> Result<(), String>
```

Updates `calendar_account.default_calendar_url`. No CalDAV call.

---

## 8. Frontend

### 8.1 EventsCard updates (`components/Today/EventsCard.tsx`)

- Header gains a `+` button (right-aligned, same style as other card `+` buttons)
- Each event row gets `cursor: pointer` + hover background (`var(--paper-muted)`)
- Clicking `+` → `setShowAdd(true)` → mounts `<AddEventDrawer>`
- Clicking a row → `setEditingEvent(event)` → mounts `<EditEventDrawer event={event}>`
- After add/update/delete success: call `listEventsToday()` + `setEvents(result)`

### 8.2 AddEventDrawer (`components/Today/AddEventDrawer.tsx`)

Slide-in drawer (same pattern as `AddTransactionForm`). Fields:

| Field | Type | Default | Required |
|---|---|---|---|
| Title | text input | — | yes |
| Date | date input | today | yes |
| Start time | time input | next round hour | yes |
| End time | time input | start + 1h | yes |
| All day | checkbox | unchecked | no |
| Description | textarea | — | no |
| Location | text input | — | no |
| Calendar | select (accounts) | first account | if >1 account |

When all-day is checked: hide start/end time inputs.
Submit → `invoke("create_event", {...})` → close on success, inline error on failure.
Conflict error string → show *"This event was changed elsewhere. Please try again."*

### 8.3 EditEventDrawer (`components/Today/EditEventDrawer.tsx`)

Same fields as Add, pre-populated from the `Event` row.

For recurring occurrences (`is_recurring_occurrence = true`): quiet notice at top — *"Editing this occurrence only."*

Delete button at drawer bottom:
- Non-recurring: inline confirm *"Delete this event?"* — Yes / No (3s auto-dismiss)
- Recurring occurrence: inline confirm *"Remove this occurrence?"* — Yes / No (3s auto-dismiss)

Submit → `invoke("update_event", {...})`. Delete → `invoke("delete_event", {id})`. Both close drawer on success.

### 8.4 Settings — AccountRow default calendar (`components/Settings/AccountRow.tsx`)

Below the sync status line, when `account.default_calendar_url` is not null (i.e. discovery ran):

```
New events → [Personal Calendar ▼]
```

A `<select>` whose options are the calendars discovered for this account. `useSettingsStore` gains `accountCalendars: Map<number, CalendarInfo[]>` and a `setCalendars(accountId, calendars)` mutation. The AccountRow calls `invoke("list_calendars", { accountId })` on mount and stores the result via `setCalendars`. Changing selection → `invoke("set_default_calendar", { accountId, calendarUrl })`.

Hidden if the account has only one calendar.

### 8.5 New command: `list_calendars`

```rust
#[tauri::command]
async fn list_calendars(account_id: i64, state: State<'_, Db>) -> Result<Vec<CalendarInfo>, String>
```

Returns the list of calendar display names + URLs discovered for an account. Requires a new `calendar` table (see §8.6) or storing them during discovery.

### 8.6 Calendar table (V6 addition)

To support the default-calendar picker, we need to persist discovered calendars. Add to `V6__calendar_write.sql`:

```sql
CREATE TABLE calendar (
  id                  INTEGER PRIMARY KEY,
  calendar_account_id INTEGER NOT NULL REFERENCES calendar_account(id) ON DELETE CASCADE,
  url                 TEXT    NOT NULL,
  display_name        TEXT,
  created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
  UNIQUE(calendar_account_id, url)
);
```

During `add_calendar_account` discovery (engine.rs): after enumerating calendars, insert each one into this table. `default_calendar_url` is set to the first non-shared calendar's URL (heuristic: display name does not contain "shared", "subscribed", "birthdays", "holidays", case-insensitive; fallback: first calendar).

`CalendarInfo` struct:
```rust
pub struct CalendarInfo {
    pub id: i64,
    pub url: String,
    pub display_name: Option<String>,
}
```

Frontend type:
```ts
export interface CalendarInfo {
  id: number;
  url: string;
  display_name: string | null;
}
```

---

## 9. Updated Rust Structs

### `Event` (crates/core/src/assistant/event.rs)

```rust
pub struct Event {
    pub id: i64,
    pub calendar_account_id: i64,
    pub external_id: String,
    pub title: String,
    pub start_at: i64,
    pub end_at: i64,
    pub created_at: i64,
    // New in V6:
    pub event_url: Option<String>,
    pub etag: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub all_day: bool,
    pub is_recurring_occurrence: bool,
    pub parent_event_url: Option<String>,
    pub occurrence_dtstart: Option<String>,
}
```

New DAL function:
```rust
pub fn soft_delete(conn: &Connection, id: i64) -> Result<()>
```
Sets `deleted_at = unixepoch()` on the row (requires adding `deleted_at INTEGER` to the event table — add to V6 migration).

### `NewEvent` gains the same new optional fields.

### `CalendarAccount` (crates/core/src/assistant/calendar_account.rs)

```rust
pub struct CalendarAccount {
    // existing fields...
    pub default_calendar_url: Option<String>,
}
```

New DAL function:
```rust
pub fn set_default_calendar(conn: &Connection, id: i64, url: &str) -> Result<()>
```

---

## 10. Testing

### 10.1 Rust unit tests — `ical_write.rs` (6 tests)

- `generate_vcalendar_produces_valid_structure` — output contains `BEGIN:VCALENDAR`, `BEGIN:VEVENT`, correct `DTSTART`/`DTEND`, `SUMMARY`, `END:VEVENT`, `END:VCALENDAR`
- `generate_vcalendar_all_day_uses_date_format` — `DTSTART;VALUE=DATE:YYYYMMDD`, no time component
- `add_exdate_inserts_before_end_vevent` — output has `EXDATE:<ts>` inside the parent VEVENT block
- `add_exdate_appends_second_when_one_already_exists` — two EXDATE properties both present
- `add_recurrence_override_injects_new_vevent_before_end_vcalendar` — output has two VEVENT blocks, second has `RECURRENCE-ID`
- `line_folding_at_75_octets` — long SUMMARY is folded with `\r\n ` per RFC 5545

### 10.2 Rust unit tests — sync update (2 new)

- `report_parser_extracts_href_and_etag` — given sample multistatus XML, assert `event_url` and `etag` correctly extracted per response item
- `rrule_expander_sets_recurring_fields` — expanded occurrences have `is_recurring_occurrence = true`, `parent_event_url` matches parent href, `occurrence_dtstart` matches occurrence UTC DTSTART

### 10.3 Rust integration tests with `wiremock` (4 new)

- `create_event_puts_ical_and_returns_new_etag` — mock returns 201 with ETag header; assert PUT body is valid VCALENDAR, command returns Ok
- `delete_nonrecurring_event_sends_delete_with_if_match` — assert DELETE request has `If-Match` header matching stored etag
- `delete_recurring_occurrence_patches_parent_with_exdate` — mock returns parent iCal on GET + 204 on PUT; assert PUT body contains `EXDATE`
- `update_event_412_returns_conflict_error` — mock returns 412; assert command Err string contains `"conflict"`

### 10.4 Frontend vitest (3 new)

- `upsertEvent_replaces_existing_by_id`
- `removeEvent_removes_by_id`
- `addEventDrawer_validates_required_fields`

### 10.5 Manual smoke

1. Open Manor → EventsCard rows have hover state; `+` button visible in header
2. Click `+` → AddEventDrawer opens; fill "Team sync", today, 2pm–3pm → Submit → drawer closes, event appears after re-sync
3. Click the new event row → EditEventDrawer opens pre-populated
4. Change title → Submit → event updates in feed
5. Open edit drawer → Delete → *"Delete this event?"* → Yes → event disappears
6. Sync a recurring event from iCloud (e.g. daily 10am standup)
7. Click one occurrence → EditEventDrawer shows *"Editing this occurrence only"*
8. Change title → Submit → after re-sync, only that occurrence shows new title; others unchanged
9. Open a recurring occurrence → Delete → *"Remove this occurrence?"* → Yes → after re-sync, that date is gone; others remain
10. Settings → Calendars → AccountRow shows *"New events → [calendar name]"* picker (if >1 calendar)
11. Create event with wrong credentials account → error shown inline in drawer

---

## 11. Completion criteria

- [ ] `cargo test --workspace --all-targets` green
- [ ] `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `pnpm tsc` clean
- [ ] `pnpm --filter manor-desktop test` green
- [ ] Manual smoke (11-step list in §10.5) passes against real iCloud

---

*End of spec. Next: implementation plan via `superpowers:writing-plans`.*
