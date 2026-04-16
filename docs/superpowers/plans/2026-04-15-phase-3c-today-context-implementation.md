# Phase 3c — Local LLM Aware of Today's State: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Augment Manor's system prompt on every `send_message` with a live "Today" block — current time, today's events, and all open tasks — so she gives context-aware responses.

**Architecture:** One pure function `compose_today_context(now, conn)` in a new `today.rs` module under `crates/app/src/assistant/`. It queries `task::list_open` and `event::list_today` from `manor_core`, formats a markdown block, and returns a `String`. `send_message` in `commands.rs` calls it once per turn and prepends the result to the system prompt. No new dependencies; no frontend changes.

**Tech Stack:** Rust, chrono (already in workspace), rusqlite (already in workspace), manor_core DAL functions (`task::list_open`, `event::list_today`, `calendar_account::insert`), tempfile (already in workspace dev-deps) for test fixtures.

---

## File map

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `crates/app/src/assistant/today.rs` | `compose_today_context()` + all 8 unit tests |
| Modify | `crates/app/src/assistant/mod.rs` | `pub mod today;` declaration |
| Modify | `crates/app/src/assistant/commands.rs` | Call `today::compose_today_context` in `send_message`, prepend to system prompt |

---

## Task 1: Scaffold `today.rs` + empty-day test

**Files:**
- Create: `crates/app/src/assistant/today.rs`
- Modify: `crates/app/src/assistant/mod.rs`

- [x] **Step 1: Add `pub mod today;` to `mod.rs`**

Open `crates/app/src/assistant/mod.rs` and add the module declaration:

```rust
//! Assistant glue: Ollama client + Tauri commands.

pub mod commands;
pub mod ollama;
pub mod prompts;
pub mod today;
pub mod tools;
```

- [x] **Step 2: Create `today.rs` with function stub + test skeleton**

Create `crates/app/src/assistant/today.rs`:

```rust
//! Composes the "Today" context block injected into every system prompt turn.

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use manor_core::assistant::{event, task};
use rusqlite::Connection;

/// Render a markdown "Today" block for Manor's system prompt.
///
/// Pure function: all inputs arrive as parameters; no global state reads.
/// The caller passes `Local::now()` so tests can inject a fixed instant.
pub fn compose_today_context(now: DateTime<Local>, conn: &Connection) -> Result<String> {
    // Day boundaries in local time → UTC epoch seconds for the event query.
    let today_str = now.date_naive().format("%Y-%m-%d").to_string();
    let day_start_local = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap();
    let day_end_local = day_start_local + chrono::Duration::days(1);
    let start_utc = day_start_local.with_timezone(&Utc).timestamp();
    let end_utc = day_end_local.with_timezone(&Utc).timestamp();

    // Query both data sources.
    let tasks = task::list_open(conn)?;
    let events = event::list_today(conn, start_utc, end_utc)?;

    let n_ev = events.len();
    let n_tk = tasks.len();

    // --- Header ---
    let weekday = now.format("%A").to_string();
    let day_month = now.format("%-d %B").to_string();
    let time_str = now.format("%H:%M").to_string();
    let tz_str = now.format("%Z").to_string();
    let mut out = format!("## Today — {weekday}, {day_month}\nNow: {time_str} {tz_str}\n\n");

    // --- Preamble ---
    let preamble = match (n_ev, n_tk) {
        (0, 0) => "Nothing scheduled and your task list is clear.".to_string(),
        (0, n) => format!("No events today, but {} on your list.", count_tasks(n)),
        (n, 0) => format!("{} today, no open tasks.", count_events(n)),
        (n, m) => {
            let shape = if n <= 1 && m <= 2 {
                "Quiet"
            } else if n <= 3 && m <= 5 {
                "Moderate"
            } else {
                "Full"
            };
            format!("{shape} day: {} and {}.", count_events(n), count_open_tasks(m))
        }
    };
    out.push_str(&preamble);
    out.push('\n');

    // --- Events section ---
    if n_ev > 0 {
        out.push_str("\nEvents:\n");
        for ev in &events {
            let start_local = DateTime::<Utc>::from_timestamp(ev.start_at, 0)
                .unwrap()
                .with_timezone(&Local);
            let end_local = DateTime::<Utc>::from_timestamp(ev.end_at, 0)
                .unwrap()
                .with_timezone(&Local);
            let t = start_local.format("%H:%M");
            let done = if end_local < now { " (done)" } else { "" };
            out.push_str(&format!("- {t} — {}{done}\n", ev.title));
        }
    }

    // --- Tasks section ---
    if n_tk > 0 {
        out.push_str("\nTasks (open):\n");
        for t in &tasks {
            let due_today = t.due_date.as_deref() == Some(today_str.as_str());
            let suffix = if due_today { " — due today" } else { "" };
            out.push_str(&format!("- {}{suffix}\n", t.title));
        }
    }

    Ok(out)
}

// --- private helpers ---

fn count_events(n: usize) -> String {
    if n == 1 {
        "1 event".into()
    } else {
        format!("{n} events")
    }
}

fn count_tasks(n: usize) -> String {
    if n == 1 {
        "1 task".into()
    } else {
        format!("{n} tasks")
    }
}

fn count_open_tasks(n: usize) -> String {
    if n == 1 {
        "1 open task".into()
    } else {
        format!("{n} open tasks")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone};
    use manor_core::assistant::{calendar_account, db, event::NewEvent};
    use tempfile::tempdir;

    // ── helpers ────────────────────────────────────────────────────────────

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    /// Build a `DateTime<Local>` from a date string + hour + minute.
    /// Uses the system local timezone — same as what the app uses at runtime.
    fn local_dt(date: &str, h: u32, m: u32) -> DateTime<Local> {
        let naive = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap();
        Local.from_local_datetime(&naive).unwrap()
    }

    /// Unix-second timestamp for a local datetime.
    fn local_ts(date: &str, h: u32, m: u32) -> i64 {
        local_dt(date, h, m).timestamp()
    }

    fn seed_account(conn: &Connection) -> i64 {
        calendar_account::insert(conn, "Test", "https://cal.example.com", "user").unwrap()
    }

    fn seed_event(conn: &Connection, acct: i64, title: &str, start: i64, end: i64) {
        event::insert_many(
            conn,
            &[NewEvent {
                calendar_account_id: acct,
                external_id: format!("uid-{title}"),
                title: title.to_string(),
                start_at: start,
                end_at: end,
            }],
        )
        .unwrap();
    }

    fn seed_task(conn: &Connection, title: &str, due: Option<&str>) {
        task::insert(conn, title, due, None).unwrap();
    }

    // ── test 1: empty day ──────────────────────────────────────────────────

    #[test]
    fn empty_day_returns_clear_preamble_no_sections() {
        let (_d, conn) = fresh_conn();
        let now = local_dt("2026-04-15", 9, 0);
        let result = compose_today_context(now, &conn).unwrap();

        assert!(
            result.contains("## Today — Tuesday, 15 April"),
            "header missing: {result}"
        );
        assert!(
            result.contains("Nothing scheduled and your task list is clear."),
            "preamble wrong: {result}"
        );
        assert!(!result.contains("Events:"), "events section should be absent: {result}");
        assert!(!result.contains("Tasks (open):"), "tasks section should be absent: {result}");
    }
}
```

- [x] **Step 3: Run test to verify it fails (function stub compiles but may fail on behaviour)**

```bash
cd ~/life-assistant && cargo test -p manor_app today::tests::empty_day 2>&1 | tail -20
```

Expected: either `FAILED` or compilation error. If it passes already, the test is wrong — review.

- [x] **Step 4: Fix any compilation issues and run until green**

```bash
cd ~/life-assistant && cargo test -p manor_app today::tests::empty_day 2>&1 | tail -20
```

Expected:
```
test assistant::today::tests::empty_day_returns_clear_preamble_no_sections ... ok
test result: ok. 1 passed; ...
```

- [x] **Step 5: Commit**

```bash
cd ~/life-assistant
git add crates/app/src/assistant/today.rs crates/app/src/assistant/mod.rs
git commit -m "feat(assistant): today.rs scaffold + empty-day test (TDD)"
```

---

## Task 2: Preamble variants + sections (tasks-only, events-only)

**Files:**
- Modify: `crates/app/src/assistant/today.rs` (add tests 2 and 3 inside the `tests` module)

The function implementation is already complete from Task 1. This task writes tests 2 and 3 to verify the tasks-only and events-only branches.

- [x] **Step 1: Add test 2 (tasks-only) inside the `tests` mod**

Append inside the `tests` module (after `empty_day_returns_clear_preamble_no_sections`):

```rust
    #[test]
    fn tasks_only_preamble_and_section() {
        let (_d, conn) = fresh_conn();
        seed_task(&conn, "Reply to Miriam", Some("2026-04-15"));
        seed_task(&conn, "Pick up prescription", None);

        let now = local_dt("2026-04-15", 9, 0);
        let result = compose_today_context(now, &conn).unwrap();

        assert!(
            result.contains("No events today, but 2 tasks on your list."),
            "preamble wrong: {result}"
        );
        assert!(!result.contains("Events:"), "events section should be absent: {result}");
        assert!(result.contains("Tasks (open):"), "tasks section missing: {result}");
        assert!(result.contains("- Reply to Miriam"), "task 1 missing: {result}");
        assert!(result.contains("- Pick up prescription"), "task 2 missing: {result}");
    }
```

- [x] **Step 2: Add test 3 (events-only) inside the `tests` mod**

```rust
    #[test]
    fn events_only_preamble_and_section() {
        let (_d, conn) = fresh_conn();
        let acct = seed_account(&conn);
        let start = local_ts("2026-04-15", 12, 30);
        let end = local_ts("2026-04-15", 13, 30);
        seed_event(&conn, acct, "Lunch with Sam", start, end);

        let now = local_dt("2026-04-15", 9, 0);
        let result = compose_today_context(now, &conn).unwrap();

        assert!(
            result.contains("1 event today, no open tasks."),
            "preamble wrong: {result}"
        );
        assert!(result.contains("Events:"), "events section missing: {result}");
        assert!(result.contains("- 12:30 — Lunch with Sam"), "event entry missing: {result}");
        assert!(!result.contains("Tasks (open):"), "tasks section should be absent: {result}");
    }
```

- [x] **Step 3: Run both new tests**

```bash
cd ~/life-assistant && cargo test -p manor_app today::tests 2>&1 | tail -20
```

Expected: 3 tests passing (including the Task 1 test).

- [x] **Step 4: Commit**

```bash
cd ~/life-assistant
git add crates/app/src/assistant/today.rs
git commit -m "test(assistant): tasks-only + events-only preamble tests (TDD)"
```

---

## Task 3: Done markers + due-today suffix

**Files:**
- Modify: `crates/app/src/assistant/today.rs` (add tests 4 and 5)

- [x] **Step 1: Add test 4 (done markers on past events)**

Append inside the `tests` module:

```rust
    #[test]
    fn past_events_get_done_marker_future_do_not() {
        let (_d, conn) = fresh_conn();
        let acct = seed_account(&conn);

        // Past event: ended at 11:00, now is 14:32
        let past_start = local_ts("2026-04-15", 10, 0);
        let past_end = local_ts("2026-04-15", 11, 0);
        seed_event(&conn, acct, "Boiler service", past_start, past_end);

        // Future event: starts at 16:00, now is 14:32
        let future_start = local_ts("2026-04-15", 16, 0);
        let future_end = local_ts("2026-04-15", 17, 0);
        seed_event(&conn, acct, "Dentist", future_start, future_end);

        let now = local_dt("2026-04-15", 14, 32);
        let result = compose_today_context(now, &conn).unwrap();

        assert!(
            result.contains("- 10:00 — Boiler service (done)"),
            "past event should have (done): {result}"
        );
        assert!(
            result.contains("- 16:00 — Dentist\n"),
            "future event should not have (done): {result}"
        );
    }
```

- [x] **Step 2: Add test 5 (due-today suffix + ordering)**

```rust
    #[test]
    fn due_today_tasks_get_suffix_others_do_not() {
        let (_d, conn) = fresh_conn();
        // due today
        seed_task(&conn, "Reply to Miriam", Some("2026-04-15"));
        // no due date (appears after due tasks, due_date IS NULL sorts last)
        seed_task(&conn, "Pick up prescription", None);
        // due in the future — still open, still appears
        seed_task(&conn, "Tax return", Some("2026-05-01"));

        let now = local_dt("2026-04-15", 9, 0);
        let result = compose_today_context(now, &conn).unwrap();

        assert!(
            result.contains("- Reply to Miriam — due today"),
            "today task missing suffix: {result}"
        );
        assert!(
            result.contains("- Pick up prescription\n"),
            "no-due task should have no suffix: {result}"
        );
        assert!(
            result.contains("- Tax return\n"),
            "future-due task should appear without suffix: {result}"
        );
    }
```

- [x] **Step 3: Run both new tests**

```bash
cd ~/life-assistant && cargo test -p manor_app today::tests 2>&1 | tail -20
```

Expected: 5 tests passing.

- [x] **Step 4: Commit**

```bash
cd ~/life-assistant
git add crates/app/src/assistant/today.rs
git commit -m "test(assistant): done markers + due-today suffix tests (TDD)"
```

---

## Task 4: Shape logic + timezone display + day-boundary

**Files:**
- Modify: `crates/app/src/assistant/today.rs` (add tests 6, 7, 8)

- [x] **Step 1: Add test 6 (shape templating)**

Append inside the `tests` module:

```rust
    /// Helper: insert N tasks with no due date.
    fn seed_n_tasks(conn: &Connection, n: usize) {
        for i in 0..n {
            seed_task(conn, &format!("Task {i}"), None);
        }
    }

    /// Helper: insert N events spread 1h apart starting at 08:00.
    fn seed_n_events(conn: &Connection, acct: i64, n: usize, date: &str) {
        for i in 0..n {
            let h = 8 + i as u32;
            let start = local_ts(date, h, 0);
            let end = local_ts(date, h, 59);
            seed_event(conn, acct, &format!("Event {i}"), start, end);
        }
    }

    #[test]
    fn shape_quiet_moderate_full() {
        // quiet: 1 event + 2 tasks
        {
            let (_d, conn) = fresh_conn();
            let acct = seed_account(&conn);
            seed_n_events(&conn, acct, 1, "2026-04-15");
            seed_n_tasks(&conn, 2);
            let now = local_dt("2026-04-15", 9, 0);
            let result = compose_today_context(now, &conn).unwrap();
            assert!(result.contains("Quiet day:"), "expected Quiet: {result}");
        }

        // moderate: 3 events + 5 tasks
        {
            let (_d, conn) = fresh_conn();
            let acct = seed_account(&conn);
            seed_n_events(&conn, acct, 3, "2026-04-15");
            seed_n_tasks(&conn, 5);
            let now = local_dt("2026-04-15", 9, 0);
            let result = compose_today_context(now, &conn).unwrap();
            assert!(result.contains("Moderate day:"), "expected Moderate: {result}");
        }

        // full: 5 events + 10 tasks
        {
            let (_d, conn) = fresh_conn();
            let acct = seed_account(&conn);
            seed_n_events(&conn, acct, 5, "2026-04-15");
            seed_n_tasks(&conn, 10);
            let now = local_dt("2026-04-15", 9, 0);
            let result = compose_today_context(now, &conn).unwrap();
            assert!(result.contains("Full day:"), "expected Full: {result}");
        }
    }
```

- [x] **Step 2: Add test 7 (timezone display in header)**

```rust
    #[test]
    fn header_contains_time_and_timezone() {
        let (_d, conn) = fresh_conn();
        let now = local_dt("2026-04-15", 14, 32);
        let result = compose_today_context(now, &conn).unwrap();

        // Time portion is always deterministic — 14:32
        assert!(
            result.contains("Now: 14:32"),
            "header time wrong: {result}"
        );
        // Timezone abbreviation is system-local; just assert it's present and non-empty.
        // We build the expected tz from the same `now` so the test is system-independent.
        let tz = now.format("%Z").to_string();
        assert!(
            !tz.is_empty(),
            "timezone abbreviation is empty — chrono problem"
        );
        assert!(
            result.contains(&format!("Now: 14:32 {tz}")),
            "tz not in header: {result}"
        );
    }
```

- [x] **Step 3: Add test 8 (day-boundary edges)**

```rust
    #[test]
    fn day_boundary_excludes_yesterday_includes_midnight_today() {
        let (_d, conn) = fresh_conn();
        let acct = seed_account(&conn);

        // Yesterday 23:59 — should NOT appear in today's set
        let yesterday_start = local_ts("2026-04-14", 23, 59);
        let yesterday_end = local_ts("2026-04-15", 0, 0); // ends exactly at midnight
        seed_event(&conn, acct, "Late yesterday", yesterday_start, yesterday_end);

        // Today 00:00 exactly — should appear
        let today_start = local_ts("2026-04-15", 0, 0);
        let today_end = local_ts("2026-04-15", 1, 0);
        seed_event(&conn, acct, "Midnight start", today_start, today_end);

        let now = local_dt("2026-04-15", 9, 0);
        let result = compose_today_context(now, &conn).unwrap();

        assert!(
            !result.contains("Late yesterday"),
            "event before today should be excluded: {result}"
        );
        assert!(
            result.contains("- 00:00 — Midnight start"),
            "midnight-start event should be included: {result}"
        );
    }
```

- [x] **Step 4: Run all 8 tests**

```bash
cd ~/life-assistant && cargo test -p manor_app today::tests 2>&1 | tail -25
```

Expected: 8 tests passing.

- [x] **Step 5: Run full workspace test suite to catch regressions**

```bash
cd ~/life-assistant && cargo test --workspace 2>&1 | grep -E "^test result|FAILED"
```

Expected: all `test result: ok.` lines, 0 FAILED.

- [x] **Step 6: Commit**

```bash
cd ~/life-assistant
git add crates/app/src/assistant/today.rs
git commit -m "test(assistant): shape + timezone + day-boundary tests (TDD)"
```

---

## Task 5: Wire `compose_today_context` into `send_message`

**Files:**
- Modify: `crates/app/src/assistant/commands.rs`

- [x] **Step 1: Locate the system prompt build in `send_message` (line ~100)**

In `crates/app/src/assistant/commands.rs`, find this block (currently around line 99–103):

```rust
    // 3. Build chat-message history (system prompt + recent turns).
    let mut chat_msgs: Vec<ChatMessage> = vec![ChatMessage {
        role: ChatRole::System,
        content: SYSTEM_PROMPT.into(),
    }];
```

- [x] **Step 2: Compose the today block and inject it**

Replace the block found above with:

```rust
    // 3. Build chat-message history (system prompt + today context + recent turns).
    let today_block = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        crate::assistant::today::compose_today_context(Local::now(), &conn)
            .map_err(|e| e.to_string())?
    };
    let mut chat_msgs: Vec<ChatMessage> = vec![ChatMessage {
        role: ChatRole::System,
        content: format!("{SYSTEM_PROMPT}\n\n{today_block}"),
    }];
```

`Local` is already imported at the top of `commands.rs` (`use chrono::Local;`), so no new imports are needed.

- [x] **Step 3: Build the workspace**

```bash
cd ~/life-assistant && cargo build --workspace 2>&1 | grep -E "^error|warning\[" | head -20
```

Expected: no `error` lines. Warnings about unused imports are acceptable.

- [x] **Step 4: Run full test suite**

```bash
cd ~/life-assistant && cargo test --workspace 2>&1 | grep -E "^test result|FAILED"
```

Expected: all `test result: ok.` lines, 42+ tests passing total (34 prior + 8 new).

- [x] **Step 5: Commit**

```bash
cd ~/life-assistant
git add crates/app/src/assistant/commands.rs
git commit -m "feat(assistant): inject today context into system prompt on every send_message"
```

---

## Deliverable summary

| File | Change |
|------|--------|
| `crates/app/src/assistant/today.rs` | New — `compose_today_context` + 8 unit tests |
| `crates/app/src/assistant/mod.rs` | `pub mod today;` added |
| `crates/app/src/assistant/commands.rs` | Today block composed + prepended to system prompt in `send_message` |

No new dependencies. No frontend changes. No new Tauri commands. Tests baseline: 34 Rust → 42 Rust.
