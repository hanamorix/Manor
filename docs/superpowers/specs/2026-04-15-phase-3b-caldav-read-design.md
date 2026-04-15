# Phase 3b — CalDAV Read

**Date:** 2026-04-15
**Phase:** v0.1 Heartbeat, Phase 3b (second sub-phase of Phase 3; honors the `event` schema contract from the 3c spec §5.2)
**Status:** Spec — ready to plan

---

## 1. Summary

Manor can read your calendar. The EventsCard on the Today view — currently empty with "No calendar connected. Coming next phase." — finally fills with today's events. Sync is CalDAV-only (iCloud + any Basic Auth server), flat (all calendars under an account merge into one stream), and triggered at app start / on account creation / on manual refresh.

Setup happens through a proper **Settings modal** (the first Settings surface in Manor), triggered by a cog icon on HeaderCard and the ⌘, macOS convention. The modal uses a tabbed layout with "Calendars" active in 3b and "AI (soon)" / "About (soon)" placeholder tabs so the shape scales to Phase 4+ without redesign. Accounts are added via an inline form (URL + username + password), password is stored in macOS Keychain.

Build position: 3b lands between 3a (shipped) and 3c (spec written, not yet planned). The `event` table schema was contracted in 3c spec §5.2 — 3b honors it verbatim so 3c's eventual `compose_today_context` query works against the populated table without re-migration.

---

## 2. Goals & non-goals

### 2.1 Goals

- `calendar_account` table (new) + `event` table (per 3c spec §5.2, verbatim) + migration `V3__calendar.sql`
- CalDAV discovery + event fetch via `reqwest` (reused from Phase 2)
- iCal parsing (RFC 5545) via the `ical` crate
- RRULE expansion via the `rrule` crate in a 22-day window (today − 7 days → today + 14 days) — each occurrence becomes its own `event` row with a deterministic `external_id`
- Credentials stored in **macOS Keychain** via the `keyring` crate (username + URL in DB, password in Keychain under key `manor/caldav-{account_id}`)
- **Settings modal** triggered by a cog icon on HeaderCard + `⌘,`. Tabbed layout with "Calendars" active, "AI (soon)" / "About (soon)" placeholder tabs.
- Add-account form inside Calendars tab: server URL, username, password (`type="password"`), "Connect" button → runs discovery + kicks off first sync
- Account row shows: display name, username, sync status (`synced 3m ago` / `syncing…` / `error: <reason>`), per-account Sync + Remove buttons
- Three sync triggers: app start (all accounts), new-account create (the new one), Settings per-account Sync button
- **EventsCard populates:** today's events from `event` table, via a new `list_events_today` IPC command, ordered by `start_at`, formatted as `HH:MM — title`

### 2.2 Non-goals (explicit YAGNI)

| Item | Where it lives instead |
|---|---|
| Writing events to CalDAV | Phase 4+ "Manor edits calendar" proposal-kind |
| Google / OAuth | Dedicated later phase "3b.1 OAuth providers" |
| Per-calendar filtering or colour | Phase 4+ Rhythm (where category signals become useful) |
| Background periodic sync | Phase 4+ Ambient Manor |
| Push notifications on event | Phase 4+ Ambient Manor |
| Event detail view / edit / delete UI | Phase 4+ when writes become real |
| Attendees, location, description rendering | Schema carries only what 3c reads (title + time); richer fields deferred |
| Incremental sync (sync-token / ETag) | Full resync-per-account for v0.1 — simple + correct; optimise only if it's actually slow |
| Timezone metadata on events beyond UTC seconds | Store UTC, render local. TZ string used during parse but not persisted. |
| All-day event visual distinction | Follows 3c spec behaviour: `start_at = midnight local`, `end_at = midnight next day`, shown in UI as `00:00 — Title`. Revisit if painful. |
| Recurrence exceptions (EXDATE, individual VEVENT overrides) | EXDATE handled by `rrule` crate natively. Individual `RECURRENCE-ID` overrides: out of scope (treat as separate one-off events if the parser exposes them). |
| Functional Settings surfaces beyond Calendars | "AI" and "About" tabs ship as disabled stubs only |

---

## 3. Architecture

### 3.1 Rust backend (touched)

| Crate / module | Change |
|---|---|
| `manor-core/migrations/V3__calendar.sql` | New: `calendar_account` table + `event` table (per 3c §5.2) |
| `manor-core/src/assistant/calendar_account.rs` | New module: `CalendarAccount` type + CRUD (insert, list, get, update_sync_state, delete) |
| `manor-core/src/assistant/event.rs` | New module: `Event` + `NewEvent` types + CRUD (insert_many, list_today, delete_for_account) |
| `manor-app/src/sync/mod.rs` | New top-level module tree for the sync engine |
| `manor-app/src/sync/caldav.rs` | HTTP client: PROPFIND for principal/home-set/calendars, REPORT for events |
| `manor-app/src/sync/ical.rs` | Parse iCal → intermediate `ParsedEvent` shape |
| `manor-app/src/sync/expand.rs` | Run `rrule` on RRULE-bearing events, return flattened occurrences |
| `manor-app/src/sync/keychain.rs` | Thin wrapper around `keyring::Entry` for `manor/caldav-{id}` |
| `manor-app/src/sync/engine.rs` | Orchestrator: `sync_account(id) → SyncResult` (fetch + parse + expand + wipe-and-reinsert) |
| `manor-app/src/assistant/commands.rs` (extend) | New commands: `list_calendar_accounts`, `add_calendar_account`, `remove_calendar_account`, `sync_account`, `sync_all_accounts`, `list_events_today` |
| `manor-app/src/lib.rs` | `register()` — wire new commands into `invoke_handler`; `setup()` closure kicks off `sync_all_accounts()` in a background `tokio::task` (not blocking window open) |

### 3.2 New workspace dependencies

| Crate | Purpose |
|---|---|
| `ical` | RFC 5545 iCalendar parser |
| `rrule` | Recurrence rule expander with EXDATE support |
| `keyring` | Cross-platform secure credential store (uses macOS Keychain on Mac) |
| `chrono-tz` | Named-zone → UTC conversion for `DTSTART;TZID=...` values |
| `quick-xml` | CalDAV REPORT / PROPFIND responses are XML |

### 3.3 Frontend structure (under `apps/desktop/src/`)

```
components/Settings/
  SettingsModal.tsx       # the modal shell: tabs, close button, Esc dismiss, focus trap
  Tabs.tsx                # tab bar (Calendars active, AI/About disabled with "(soon)")
  CalendarsTab.tsx        # account list + add-account form, reads useSettingsStore
  AccountRow.tsx          # one row: avatar badge, name, sync status, Sync + Remove buttons
  AddAccountForm.tsx      # URL/username/password inputs + Connect button
  SettingsCog.tsx         # the cog icon that lives on HeaderCard
components/Today/
  EventsCard.tsx          # MODIFIED: no longer empty — renders from useTodayStore.events
lib/settings/
  state.ts                # useSettingsStore zustand slice
  ipc.ts                  # typed wrappers for calendar-account + sync commands
lib/today/
  state.ts                # MODIFIED: add events[] slice + setEvents mutation
  ipc.ts                  # MODIFIED: add listEventsToday wrapper
```

Modified:
- `App.tsx` — mount `<SettingsModal />` at the app root (it handles its own visibility via the store)
- `HeaderCard.tsx` — add the `<SettingsCog />` beside the live-clock badge
- `styles.css` — add `@keyframes settingsIn` (fade + scale from 0.97)

### 3.4 Tauri IPC contract additions

| Command | Shape | Purpose |
|---|---|---|
| `list_calendar_accounts() -> Vec<CalendarAccount>` | query | Settings modal hydrates from this |
| `add_calendar_account(server_url, username, password) -> CalendarAccount` | mutation | stores account + password-in-Keychain, spawns first sync in background |
| `remove_calendar_account(id) -> ()` | mutation | deletes account + its events (CASCADE) + wipes Keychain entry |
| `sync_account(id) -> SyncResult` | mutation | manual per-account refresh |
| `sync_all_accounts() -> Vec<SyncResult>` | mutation | startup trigger and Settings "Sync all" button |
| `list_events_today() -> Vec<Event>` | query | EventsCard + eventually 3c |

### 3.5 State management

New `useSettingsStore` separate from today/assistant stores. Clean boundaries. `useTodayStore` gains an `events[]` slice.

### 3.6 No changes to Phase 2 Assistant plumbing

Manor's tool-use still only exposes `add_task` (no new calendar tools in 3b). The `StreamChunk` enum is unchanged.

---

## 4. Settings modal — UX detail

### 4.1 Trigger

- **Cog icon** (⚙) on the right side of HeaderCard, beside the live-clock badge. 18×18px, `opacity: 0.6`, hover → 1.0
- **Keyboard shortcut** `⌘,` (macOS Preferences convention). Top-level keydown listener in `App.tsx`
- Both call `useSettingsStore.setModalOpen(true)`

### 4.2 Modal shell

- Centered overlay, 540×440px
- Backdrop: `rgba(0,0,0,0.25)` with `backdrop-filter: blur(2px)`
- Background: `var(--paper)`, `border-radius: 14px`, `var(--shadow-lg)`
- Header row (12×16 padding): title `Settings`, close `×` button on the right
- Body: tab bar + tab content
- Footer: none (actions live inline per tab)
- Animation: `settingsIn 200ms ease-out` — fade + scale from 0.97
- Close behaviour: Esc, backdrop click, or × button
- Focus trap while open; restore focus to the cog on close

### 4.3 Tab bar

- Three tabs: **Calendars** (active, clickable), **AI** (disabled, label `AI (soon)`), **About** (disabled, label `About (soon)`)
- Active tab: bold, 2px underline in `var(--imessage-blue)`
- Disabled tabs: `opacity: 0.35`, `cursor: default`, no onClick handler

### 4.4 Calendars tab content

Three zones, top to bottom:

1. **Section header:** `Your calendar accounts` (11px uppercase tracked)
2. **Account list:** zero-or-more `<AccountRow>`, gap 6px
3. **Add button + form:** `+ Add calendar account` link. Click reveals the `<AddAccountForm>` inline below the list.

### 4.5 `<AccountRow>` layout

```
[iC]  iCloud                          [Sync]   [Remove]
      hanamorix@icloud.com · <status-line>
```

- Left: 28×28 rounded square with provider shorthand (`iC` for `caldav.icloud.com`, `FM` for `mail.fastmail.com`, generic dot for unknown hosts)
- Middle: display name (bold, 13px) + meta (11px muted)
- Right: two pill buttons (`Sync` secondary grey, `Remove` secondary grey)

Status-line content per state:

| State | Text |
|---|---|
| Idle | `synced 3m ago` (relative time from `last_synced_at`) |
| Syncing | `syncing…` + a subtle pulsing grey dot ⬤ |
| Error | `error: <short-message>` in `var(--imessage-red)`; hover shows the full `last_error` as a tooltip |
| Never synced | `not synced yet` |

`Sync` button disabled while `syncingAccountIds.has(id)`.

### 4.6 `<AddAccountForm>`

```
Server URL         [ https://caldav.icloud.com/        ]
Username           [ hanamorix@icloud.com              ]
App-specific pwd   [ •••••••••••••                     ]  ← type="password"
                   [ Cancel ]  [ Connect ]
```

- Three inputs, vertically stacked
- URL placeholder hints `https://caldav.icloud.com` (iCloud users can copy-paste verbatim)
- Password field uses `type="password"`
- Small "?" icon next to the password label links to a tooltip: *"For iCloud: go to appleid.apple.com → Sign-In and Security → App-Specific Passwords"*
- Connect button disabled until all three fields are non-empty

On Connect:

1. Disable form, show "Connecting…" overlay
2. Call `add_calendar_account(url, user, pass)` — backend stores account, saves password to Keychain, runs discovery + first sync in a spawned tokio task
3. **Success:** form collapses, new row appears in the list with `syncing…` status; row polls the account list via `list_calendar_accounts` every 1.5s until its `last_synced_at` or `last_error` changes
4. **Error:** form stays open with the error in red above the buttons (`Connection failed: <reason>`). Password stays in the field; user can correct URL/user and retry.

### 4.7 Per-account Sync button

- Click → calls `sync_account(id)` → row status flips to `syncing…` (optimistic)
- On return: row updates from the `SyncResult` (new `last_synced_at` or `last_error`)

### 4.8 Per-account Remove button

- Click → row shows a soft-confirm: `Really remove?` with an inline `Yes / No` pair (3s auto-dismiss if neither clicked)
- Yes → calls `remove_calendar_account(id)` → optimistic: row disappears from the store → backend CASCADE-deletes events + wipes Keychain
- EventsCard refreshes after removal (some of its events vanish)

---

## 5. Data model

### 5.1 Migration `V3__calendar.sql`

New file at `crates/core/migrations/V3__calendar.sql`:

```sql
CREATE TABLE calendar_account (
  id                INTEGER PRIMARY KEY,
  display_name      TEXT    NOT NULL,
  server_url        TEXT    NOT NULL,
  username          TEXT    NOT NULL,
  last_synced_at    INTEGER NULL,       -- unix seconds, NULL = never synced
  last_error        TEXT    NULL,       -- short reason, NULL = last sync succeeded (or never attempted)
  created_at        INTEGER NOT NULL,   -- unix ms
  UNIQUE (server_url, username)
);

CREATE TABLE event (
  id                  INTEGER PRIMARY KEY,
  calendar_account_id INTEGER NOT NULL REFERENCES calendar_account(id) ON DELETE CASCADE,
  external_id         TEXT    NOT NULL,
  title               TEXT    NOT NULL,
  start_at            INTEGER NOT NULL,    -- unix seconds, UTC
  end_at              INTEGER NOT NULL,    -- unix seconds, UTC
  created_at          INTEGER NOT NULL,
  UNIQUE (calendar_account_id, external_id)
);

CREATE INDEX idx_event_start_at ON event (start_at);
```

The `event` table is verbatim from **3c spec §5.2**. `calendar_account` adds the minimum Manor needs. **Password is NOT in this table** — it lives in macOS Keychain.

### 5.2 Password in Keychain (via `keyring` crate)

| Key | Value |
|---|---|
| service: `"manor"`, account: `"caldav-{id}"` | the plaintext CalDAV password |

`display_name` is either user-provided or derived by the backend from the URL host (e.g., `caldav.icloud.com` → `"iCloud"`, `mail.fastmail.com` → `"Fastmail"`, else the bare host).

### 5.3 Rust data-access modules

`crates/core/src/assistant/calendar_account.rs`:

| Function | Signature | Purpose |
|---|---|---|
| `insert(conn, display_name, server_url, username) -> Result<i64>` | create | DB row only (password → Keychain via manor-app) |
| `list(conn) -> Result<Vec<CalendarAccount>>` | list | ordered by `created_at` |
| `get(conn, id) -> Result<Option<CalendarAccount>>` | single | for sync engine |
| `update_sync_state(conn, id, last_synced_at, last_error) -> Result<()>` | write | called after every sync |
| `delete(conn, id) -> Result<()>` | delete | CASCADE drops events via FK |

`crates/core/src/assistant/event.rs`:

| Function | Signature | Purpose |
|---|---|---|
| `insert_many(conn, events: &[NewEvent]) -> Result<()>` | batch insert | sync engine calls this after expanding |
| `list_today(conn, start_utc, end_utc) -> Result<Vec<Event>>` | query | EventsCard + eventually 3c |
| `delete_for_account(conn, account_id) -> Result<()>` | cascade cleanup | called at the start of every sync (before re-inserting) |

### 5.4 `CalendarAccount` struct (Rust)

```rust
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
```

**No `password` field on the struct.** Password is fetched from Keychain when the sync engine needs it.

### 5.5 Frontend types (`lib/settings/ipc.ts`)

```ts
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
```

### 5.6 `useSettingsStore` zustand slice

```ts
interface SettingsStore {
  modalOpen: boolean;
  activeTab: "calendars" | "ai" | "about";  // 3b only uses "calendars"
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
```

### 5.7 `useTodayStore` gains an events slice

```ts
events: Event[];
setEvents: (e: Event[]) => void;
```

where `Event` matches the Rust struct: `{ id, calendar_account_id, external_id, title, start_at, end_at, created_at }`.

---

## 6. CalDAV protocol layer

### 6.1 Discovery flow

Runs once on `add_calendar_account`, before first sync:

**Step 1 — Principal discovery.** `PROPFIND` on the user-provided server URL:

```xml
<D:propfind xmlns:D="DAV:">
  <D:prop><D:current-user-principal/></D:prop>
</D:propfind>
```

Returns the principal URL (e.g., `https://caldav.icloud.com/12345/principal/`).

**Step 2 — Home-set discovery.** `PROPFIND` on the principal URL:

```xml
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop><C:calendar-home-set/></D:prop>
</D:propfind>
```

Returns the calendar-home-set URL.

**Step 3 — Calendar enumeration.** `PROPFIND` on the home-set URL with `Depth: 1`. Response lists calendar collection URLs + their display names. Held in memory for the duration of the sync (not persisted).

### 6.2 Event fetch per calendar

`REPORT` on each calendar URL with a time-range filter (today − 7 days → today + 14 days):

```xml
<C:calendar-query xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop xmlns:D="DAV:">
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VEVENT">
        <C:time-range start="20260408T000000Z" end="20260429T000000Z"/>
      </C:comp-filter>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>
```

Response is a multistatus XML document (parsed with `quick-xml`); we extract each `calendar-data` block's inline iCal string.

### 6.3 Auth

HTTP Basic (`Authorization: Basic base64(username:password)`) on every request. Password pulled from Keychain per-request via `keyring::Entry::get_password()`.

### 6.4 iCal parsing (`crates/app/src/sync/ical.rs`)

Uses the `ical` crate to parse each `VCALENDAR` block. For each `VEVENT` inside:

- **Required:** `UID`, `DTSTART`, `DTEND` (or derived from `DURATION`), `SUMMARY`
- **Handled:** `RRULE`, `EXDATE`, `RECURRENCE-ID`
- **Ignored:** `LOCATION`, `DESCRIPTION`, `ATTENDEE`, etc. — not stored per the 3c spec; skipped to save memory

### 6.5 Time-zone handling

| DTSTART form | Manor's treatment |
|---|---|
| `DTSTART:20260415T093000Z` (UTC) | stored as-is, parse as UTC seconds |
| `DTSTART;TZID=Europe/London:20260415T093000` | look up TZID via `chrono-tz`, convert to UTC seconds |
| `DTSTART;VALUE=DATE:20260415` (all-day) | `chrono_tz::Tz` from system local TZ → midnight local of that date → UTC seconds; `DTEND` becomes midnight next day |
| Unknown TZID that isn't in `chrono-tz`'s db | treat as UTC (Manor emits `tracing::warn!`); rare in practice for iCloud/Fastmail sources |

### 6.6 RRULE expansion (`crates/app/src/sync/expand.rs`)

Uses the `rrule` crate. For each VEVENT with an RRULE:

1. Build an `RRuleSet` from the parent DTSTART + RRULE + any EXDATEs
2. Call `.all_between(window_start, window_end, true)` to get occurrences within `today − 7 days` and `today + 14 days`
3. For each occurrence, produce a `NewEvent` with:
   - `external_id = format!("{}::{}", parent_uid, start.to_rfc3339())` — deterministic, so re-sync finds the same row (though 3b wipes-and-reinserts anyway)
   - `start_at` = occurrence's UTC seconds
   - `end_at` = start + (original DTEND − original DTSTART)
   - `title` = parent SUMMARY

Non-recurring events just produce a single `NewEvent` with `external_id = UID`.

### 6.7 Sync order per account

```
1. BEGIN TRANSACTION
2. event::delete_for_account(account_id)
3. discover() → list of calendar URLs
4. for each calendar URL:
      REPORT with time-range filter
      parse VCALENDAR → Vec<ParsedEvent>
      expand RRULEs → Vec<NewEvent>
      (accumulate into one Vec)
5. event::insert_many(all accumulated events)
6. calendar_account::update_sync_state(account_id, now, None)
7. COMMIT
```

On failure at any step: ROLLBACK, then `update_sync_state(account_id, last_synced_at_unchanged, Some(error_message))`.

---

## 7. Sync engine

### 7.1 Three triggers, one code path

| Trigger | Invocation | UI feedback |
|---|---|---|
| App start | `register()` `setup()` closure spawns `tokio::task` calling `sync_all_accounts()` | EventsCard shows prior cached events immediately; silently refreshes when sync finishes |
| `add_calendar_account` | Backend spawns `sync_account(new_id)` after insert | Settings row shows `syncing…` until result |
| Manual Sync button | Frontend calls `invoke("sync_account", {id})` | Row shows `syncing…`; on return updates status-line |

`sync_all_accounts()` iterates accounts **serially** (one at a time) — avoids hammering multiple iCloud endpoints simultaneously and keeps DB lock contention low. For 1–3 accounts in practice, serial is fast enough.

### 7.2 Concurrency guard

The sync engine keeps an in-memory `HashSet<i64>` of accounts currently being synced, behind a `Mutex`. `sync_account(id)`:

1. If `id` is already in the set: return immediately with `SyncResult { account_id, events_added: 0, error: Some("already syncing"), synced_at: prior }` — no double sync
2. Otherwise: insert `id`, run the sync, remove `id` on completion (success or error)

Held in a new `SyncState` struct managed by Tauri alongside `Db`.

### 7.3 `SyncResult`

```rust
pub struct SyncResult {
    pub account_id: i64,
    pub events_added: u32,
    pub error: Option<String>,   // None on success
    pub synced_at: i64,           // unix seconds, always current time
}
```

### 7.4 Error taxonomy

Every error maps to a short user-facing string for the status line (full text stored in `calendar_account.last_error` for tooltip):

| Error class | User-facing | Full/dev message |
|---|---|---|
| Network unreachable | `server unreachable` | `connect error: <reason>` |
| 401 / 403 from CalDAV | `bad credentials` | full HTTP response body |
| 404 from CalDAV | `URL not found` | the URL that 404'd |
| Discovery response malformed | `discovery failed` | parse error + URL |
| iCal parse failure on a specific event | silently skipped — not a sync-level error | `tracing::warn!` with UID |
| RRULE parse failure on a specific event | silently skipped — not a sync-level error | `tracing::warn!` with UID |
| DB error mid-transaction | `database error` | full error |
| Keychain lookup failure | `password missing from keychain` | key that was searched |

Individual per-event parse failures never fail the whole sync — 95%-good data beats all-or-nothing.

### 7.5 UI reflection of sync state

- Immediate optimistic: `markSyncing(id)` called client-side on button click → row shows `syncing…`
- Authoritative: when IPC returns, `upsertAccount(updated)` replaces the row with the DB's new `last_synced_at` / `last_error`

After a successful sync, the **EventsCard refreshes**: the frontend calls `listEventsToday()` and updates `useTodayStore.events`. This is wired in the sync callback in `SettingsModal.tsx` — one extra refresh per sync.

### 7.6 Removal cleanup

`remove_calendar_account(id)`:

1. `DELETE FROM calendar_account WHERE id = ?1` — CASCADE wipes `event` rows
2. Delete Keychain entry `manor/caldav-{id}` (ignore `not found` errors)
3. Return

No background undo. If you remove an account by mistake, re-add it.

---

## 8. Testing strategy

### 8.1 Rust unit tests

| Module | Tests |
|---|---|
| `manor-core::assistant::calendar_account` | `insert_returns_id`; `list_orders_by_created_at`; `update_sync_state_persists_both_timestamp_and_error`; `delete_cascades_events` |
| `manor-core::assistant::event` | `insert_many_persists_batch`; `list_today_filters_by_utc_bounds`; `list_today_excludes_future_days`; `delete_for_account_scoped_to_account` |
| `manor-app::sync::ical` | `parses_utc_dtstart`; `parses_tzid_dtstart_via_chrono_tz`; `parses_all_day_as_midnight_local_pair`; `skips_vevent_missing_dtend_or_duration_with_warn`; `extracts_uid_summary_rrule_exdate_only` |
| `manor-app::sync::expand` | `expands_weekly_rrule_to_14_day_window`; `applies_exdate_exclusions`; `deterministic_external_id_format`; `non_recurring_event_yields_one_newevent_with_uid_as_external_id` |

### 8.2 Rust integration test with `wiremock`

| Test | Shape |
|---|---|
| `sync_account_happy_path_with_mock_caldav` | Mock returns: PROPFIND responses for principal + home-set + one calendar; REPORT response with 3 VEVENTs (2 one-offs, 1 weekly recurring). Assert `events_added` matches expected expansion, `last_synced_at` set, `last_error` None, `event` rows correct. |
| `sync_account_401_sets_bad_credentials_error` | Mock returns 401 on principal PROPFIND. Assert `SyncResult.error == Some("bad credentials")`, `last_synced_at` unchanged. |
| `sync_account_network_unreachable_sets_server_unreachable` | Point client at 127.0.0.1:1. Assert error. |
| `double_sync_same_account_second_returns_already_syncing` | Spawn two concurrent `sync_account(id)` tasks. Assert one success + one `already syncing`. |
| `malformed_event_skipped_not_sync_failure` | Mock returns 1 valid event + 1 malformed VEVENT. Assert `events_added == 1`, `error == None`. |

### 8.3 Frontend vitest

| File | Tests |
|---|---|
| `lib/settings/state.test.ts` | `setModalOpen`; `setAccounts_replaces`; `upsertAccount_replaces_or_appends`; `removeAccount`; `markSyncing_and_markSynced_update_set` |
| `lib/today/state.test.ts` (extend existing) | `setEvents_replaces_array` |

### 8.4 No explicit keychain tests in 3b

`keyring::Entry` calls are thin wrappers; verified via manual smoke. If flakiness appears later, introduce a trait + mock.

### 8.5 No explicit settings-modal component tests

Visual components verified in manual smoke (consistent with Phases 2 and 3a).

### 8.6 Manual smoke (end-to-end, last task in the plan)

1. Launch Manor (Today view empty, Events card still says "No calendar connected")
2. Click the cog icon on HeaderCard → Settings modal opens with "Calendars" tab active, empty list, `+ Add calendar account` link
3. Click `+ Add calendar account` → form appears
4. Enter URL `https://caldav.icloud.com`, your Apple ID email, and a real iCloud app-specific password → click Connect
5. Form collapses, new row appears with `syncing…` status
6. ~5–15 seconds later (iCloud network): row flips to `synced 0s ago`; EventsCard behind the modal now shows today's events ordered by time
7. Close the modal (× or Esc): EventsCard stays populated
8. Reopen modal, click per-account Sync — row flips to `syncing…` → `synced 0s ago`
9. Remove the account, re-add with wrong password → form shows `Connection failed: bad credentials`; retry with the real one
10. Click Remove on the account → soft-confirm → Yes → row disappears, EventsCard returns to empty
11. Kill Manor + relaunch → account persists in DB, password persists in Keychain → app-start sync fires silently, EventsCard populates after a few seconds
12. Create a recurring event in iCloud (e.g., daily standup 10am) via another client, wait for iCloud propagation (~30s–2min), click Sync in Manor → event now appears in today's EventsCard

---

## 9. Phase 3b completion criteria

- [ ] `cargo test --workspace --all-targets` green (existing 31 + Phase 3b's ~20 new, total ~51)
- [ ] `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `pnpm tsc` clean
- [ ] `pnpm --filter manor-desktop test` green (existing 20 + Phase 3b's ~7 new, total ~27)
- [ ] CI on the feature branch's PR is green
- [ ] Manual smoke (12-step list in §8.6) — every step works against real iCloud
- [ ] PR merged to main
- [ ] Tag `phase-3b-caldav-read-complete` on the merge commit

---

## 10. Open questions

None. Every behaviour is specified.

---

*End of spec. Implementation plan via `superpowers:writing-plans` once Hana approves.*
