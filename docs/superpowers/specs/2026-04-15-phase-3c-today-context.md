# Phase 3c — Local LLM Aware of Today's State

**Date:** 2026-04-15
**Phase:** v0.1 Heartbeat, Phase 3c (third sub-phase of Phase 3; specced first to lock the schema contract for 3a + 3b)
**Status:** Spec — ready to plan, but cannot be implemented until Phases 3a (`task` table) and 3b (`event` table) ship

---

## 1. Summary

Manor's responses become specific to Hana's day. Every time `send_message` is invoked, the system prompt is augmented with a "Today" block — current local time, today's events (with past ones marked `(done)`), and today's open tasks. Manor stops giving generic answers; she knows what's on the plate.

The work itself is small (~6–8 tasks): one new module exposing one pure function, called from one place. The reason Phase 3c is **specced first** is that the rendered output dictates which columns the `task` and `event` tables must expose — letting Phases 3a and 3b land their schemas right the first time, with no migration churn.

---

## 2. Build order

This spec is written **before** Phases 3a and 3b. Implementation order is **3a → 3b → 3c** because `compose_today_context()` queries the `task` and `event` tables; those tables don't exist until 3a and 3b ship.

The ordering is intentional:

- **3c spec first** — defines the column-level contract Phases 3a + 3b must honor (see §5).
- **3a builds** — the `task` table + manual-task UI/IPC. Must include the columns specified here.
- **3b builds** — the `event` + `calendar_account` tables + CalDAV sync. Must include the columns specified here.
- **3c builds** — the module specced in this document. By the time it lands, both data sources exist; nothing here changes.

---

## 3. Goals

- Manor knows the **current time + date** every turn she responds.
- Manor knows today's **events** (past ones marked `(done)`, future ones with start times).
- Manor knows today's **open tasks** (those with `completed_at IS NULL`).
- Context block lives **inside the system prompt**, refreshed on every `send_message` invocation (no caching).
- Context format is a **mixture** of natural prose + structured markdown sections (see §4).
- The `task` and `event` table schemas defined here become the contract Phases 3a + 3b must honor.

## 4. Non-goals (explicit YAGNI)

| Item | Where it lives instead |
|---|---|
| Multi-day window (week-ahead, yesterday) | Future config change once Phase 4+ proves the need (max horizon: 1 week) |
| Completed tasks in context | Manor's conversation memory already covers this; can re-add if it bites |
| Weather / location / mood / music | Not in v0.1 Heartbeat at all |
| Tool-use to fetch context (model decides when) | Phase 5+ when context grows large |
| Notifications about upcoming events | Manor responds when asked; Phase 4+ Ambient Manor introduces proactive nudges |
| Time-block / focus-mode awareness | Phase 4 Rhythm |
| Token budget / overflow strategy | Today's data is small (~10 lines); revisit when proven a problem |
| Recurrence rules (RRULE) at the 3c layer | 3b ingests RRULEs and materialises individual `event` rows; 3c only sees today's materialised events |

---

## 5. Schema contract for Phases 3a and 3b

These are the **minimum** columns the `task` and `event` tables must expose for `compose_today_context` to query them. 3a and 3b can add more columns for their own views; the columns here are non-negotiable.

### 5.1 `task` table — required columns

```sql
CREATE TABLE task (
  id              INTEGER PRIMARY KEY,
  title           TEXT    NOT NULL,
  due_date        TEXT    NULL,        -- 'YYYY-MM-DD' in Hana's local TZ, or NULL
  completed_at    INTEGER NULL,        -- unix seconds, NULL = open
  created_at      INTEGER NOT NULL     -- unix ms (matches message.created_at convention)
);

CREATE INDEX idx_task_open_due ON task (completed_at, due_date);
```

**Why date-only string for `due_date`:** tasks are end-of-day commitments, not appointments. Storing as `'2026-04-15'` (TEXT) sidesteps timezone math, sorts naturally, and equality-checks in SQL with no math.

**Query 3c uses:**

```sql
SELECT id, title, due_date FROM task
 WHERE completed_at IS NULL
 ORDER BY (due_date IS NULL), due_date, created_at;
```

### 5.2 `event` table — required columns

```sql
CREATE TABLE event (
  id                  INTEGER PRIMARY KEY,
  calendar_account_id INTEGER NOT NULL REFERENCES calendar_account(id),
  external_id         TEXT    NOT NULL,    -- CalDAV UID, for sync upsert
  title               TEXT    NOT NULL,
  start_at            INTEGER NOT NULL,    -- unix seconds, UTC
  end_at              INTEGER NOT NULL,    -- unix seconds, UTC
  created_at          INTEGER NOT NULL,
  UNIQUE (calendar_account_id, external_id)
);

CREATE INDEX idx_event_start_at ON event (start_at);
```

**Why UTC seconds:** events are time-pinned moments. UTC keeps stored data unambiguous; convert to local at render time.

**Query 3c uses:**

```sql
SELECT title, start_at, end_at FROM event
 WHERE start_at >= ?1 AND start_at < ?2  -- today's UTC bounds
 ORDER BY start_at;
```

### 5.3 Explicitly NOT required by 3c

3a and 3b should not over-build for 3c. The following columns may exist for their own views but 3c does not read them:

| Table | Optional fields 3c doesn't read | When they'd land |
|---|---|---|
| `task` | `description`, `priority`, `tags`, `recurrence` | 3a if scope grows; otherwise Rhythm |
| `event` | `location`, `description`, `rrule`, `status`, `attendees`, `all_day` | 3b for event-detail view; later phases for richer rendering |

Both tables also gain a `proposal_id INTEGER NULL REFERENCES proposal(id)` column (matching `message`'s shape) so future AI-edited tasks/events trace back to a proposal. 3c does not need it but it should land in 3a/3b's initial schema rather than being migrated in later.

---

## 6. Architecture

### 6.1 Module layout

New module `crates/app/src/assistant/today.rs` exposes one pure function:

```rust
pub fn compose_today_context(
    now: chrono::DateTime<chrono::Local>,
    conn: &rusqlite::Connection,
) -> Result<String, anyhow::Error>
```

- Pure inputs → pure output.
- Tested by seeding an in-memory SQLite, fixing `now`, and asserting the rendered string.
- No global state, no side effects beyond the read query.

### 6.2 Call site

`send_message` in `crates/app/src/assistant/commands.rs`. Right after the placeholder assistant row is inserted and before chat history is loaded, compose the today block and prepend it to the system prompt:

```rust
let today_block = today::compose_today_context(Local::now(), &conn)?;
let mut chat_msgs: Vec<ChatMessage> = vec![ChatMessage {
    role: ChatRole::System,
    content: format!("{SYSTEM_PROMPT}\n\n{today_block}"),
}];
// ... then push history as before
```

### 6.3 Time zone discipline

Manor uses **system-local time** (`chrono::Local`) because "today" only makes sense in Hana's wall-clock day. All event/task timestamps in the DB are stored as Unix epoch (UTC); `today.rs` converts to local for display and for the day boundaries.

Day boundaries are computed as:

```rust
let today_start_local = now.date_naive().and_hms_opt(0, 0, 0).unwrap()
    .and_local_timezone(Local).unwrap();
let today_end_local = today_start_local + chrono::Duration::days(1);
let today_start_utc = today_start_local.with_timezone(&Utc).timestamp();
let today_end_utc = today_end_local.with_timezone(&Utc).timestamp();
```

These bounds are passed into the event query.

### 6.4 No new dependencies

`rusqlite`, `chrono`, `anyhow` are all already declared as workspace deps from Phase 2.

---

## 7. Context format — exact rendered output

`compose_today_context` returns this shape, with deterministic templating.

### 7.1 Header (always present)

```
## Today — {Weekday}, {DD Month}
Now: {HH:MM} {TZ}
```

### 7.2 Prose preamble (always present)

One short sentence summarising the day's shape. Template logic:

| Events | Open tasks | Sentence |
|---|---|---|
| 0 | 0 | `Nothing scheduled and your task list is clear.` |
| 0 | N | `No events today, but N task(s) on your list.` |
| N | 0 | `N event(s) today, no open tasks.` |
| N | M | `{shape} day: N event(s) and M open task(s).` |

`{shape}` is computed from the totals:

| Condition | Shape |
|---|---|
| `events ≤ 1 AND tasks ≤ 2` | `quiet` |
| `events ≤ 3 AND tasks ≤ 5` | `moderate` |
| otherwise | `full` |

### 7.3 Events section (only if at least one event today)

```
Events:
- {HH:MM} — {title}{ (done) if end_time < now}
```

- Sorted by `start_at` ascending.
- `(done)` marker appended if `end_at < now`.
- Times formatted as 24-hour `HH:MM` in local TZ.

### 7.4 Tasks section (only if at least one open task)

```
Tasks (open):
- {title}{ — due today if due_date == today}
```

- Sorted by `due_date` ascending (`NULL` last), then by `created_at` ascending.
- `— due today` suffix appended if `due_date == now.date_naive().format("%Y-%m-%d").to_string()` — string equality on the same `'YYYY-MM-DD'` shape.

### 7.5 Worked example — Tuesday 15 April 2026, 14:32 BST

```
## Today — Tuesday, 15 April
Now: 14:32 BST

Moderate day: 2 events and 2 open tasks.

Events:
- 10:00 — Boiler service (done)
- 12:30 — Lunch with Sam (done)

Tasks (open):
- Reply to Miriam — due today
- Pick up prescription
```

### 7.6 Empty-day example

```
## Today — Tuesday, 15 April
Now: 09:00 BST

Nothing scheduled and your task list is clear.
```

---

## 8. Failure modes & edge cases

| Scenario | Behaviour |
|---|---|
| `task` or `event` table missing | Cannot happen by build order — 3a + 3b ship first. The spec assumes both tables exist when 3c lands. |
| DB query fails (lock contention, corrupted file) | `compose_today_context` returns `Err`; `send_message` aborts the turn with a `StreamChunk::Error(Unknown)`. Manor shows the confused expression + "Something went wrong" bubble. |
| `chrono::Local::now()` panics | Effectively impossible on macOS (always has a local TZ). If it ever did, propagates as `Err` the same way. |
| Event with `end_at == start_at` (zero-duration) | Treated as "done if `end_at < now`". Edge case shrugs at minute-precision. |
| All-day events from CalDAV | 3b stores them as `start_at = midnight local`, `end_at = midnight next day`. Rendered as `00:00 — Title` with `(done)` flipping at end-of-day. Slightly clumsy display; revisit in 3b if it's painful (could add `all_day BOOLEAN` flag and special-case). |
| 50+ events on one day | Render them all — `qwen2.5:7b-instruct` has 32k context, our biggest realistic day fits. Token-budget management is a non-goal until proven a problem. |
| Future event whose `start_at` is later today but somehow `end_at` is in the past | Bad data. Render as `(done)` (it's done from `end_at` POV). 3b should validate at insert time. |
| Task with `due_date` in the past, still open | Appears in the list without a `— due today` suffix (since `due_date != today`). Manor sees the title and can ask about it; no automatic overdue marker in v0.1. |
| User is in airplane mode / Ollama unreachable | Unrelated to 3c — the today block is composed locally before the Ollama call. The Ollama-unreachable error path from Phase 2 still applies. |

---

## 9. Testing strategy

### 9.1 Unit tests in `today.rs`

Each test seeds an in-memory SQLite via `db::init`, inserts `task` + `event` rows directly, fixes `now` to a known `DateTime<Local>`, and asserts the rendered string matches expected markdown via `assert_eq!`.

| # | Scenario | Asserts |
|---|---|---|
| 1 | Empty day (no events, no tasks) | Preamble = `Nothing scheduled and your task list is clear.`, no Events/Tasks sections |
| 2 | Tasks only | `No events today, but N task(s) on your list.`, no Events section |
| 3 | Events only | `N event(s) today, no open tasks.`, no Tasks section |
| 4 | Mixed past + future events | Past events get `(done)` marker; future don't |
| 5 | Mixed today / no-due-date / past-due tasks | Today tasks get `— due today` suffix; ordering = due_date asc with NULL last |
| 6 | Shape templating | 1ev+2tk → `quiet`; 3ev+5tk → `moderate`; 5ev+10tk → `full` |
| 7 | Time zone display | Fixed `now` in BST (e.g. 2026-04-15 14:32 BST) renders `Now: 14:32 BST` correctly |
| 8 | Day-boundary edges | Event at 23:59 local yesterday: NOT in today's set; event at 00:00 local today: IS in today's set |

### 9.2 Integration

Phase 2's existing `register_returns_builder` test still passes (no command-surface changes). No new integration test needed; `compose_today_context` is fully exercised by the unit tests, and the call site in `send_message` is a one-line append that's verified manually during smoke testing.

---

## 10. Deliverable summary

**New files:**

- `crates/app/src/assistant/today.rs` — the module + `compose_today_context()` + tests

**Modified:**

- `crates/app/src/assistant/mod.rs` — `pub mod today;`
- `crates/app/src/assistant/commands.rs` — call `today::compose_today_context(Local::now(), &conn)` in `send_message`, prepend to `SYSTEM_PROMPT` in the system message

**No new dependencies. No frontend changes. ~6–8 tasks in the writing-plans output.**

---

## 11. Open questions

None. Every behaviour is specified.

---

*End of spec. The implementation plan for Phase 3c will be written via `superpowers:writing-plans` only **after** Phases 3a and 3b have shipped and the `task` + `event` tables exist.*
