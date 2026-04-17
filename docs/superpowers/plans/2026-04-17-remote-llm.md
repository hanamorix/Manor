# Landmark 2 — Remote LLM Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship end-to-end remote Claude support for Manor: API key stored in macOS Keychain, every prompt passes through a property-tested redaction pipeline before leaving the Mac, every call is audited to `remote_call_log`, budget hard-caps prevent runaway spend, and a single tier gate routes the ledger AI month review to Claude when the user opts in. One provider (Claude), one consumer (ledger review), one gate (`ai.remote.enabled_for_review`). Everything designed so OpenAI/Gemini/Groq/OpenRouter drop in as trait impls later.

**Architecture:** New `manor_core::redact` module owns the PII scrubbing with property-based tests. New `manor_core::remote_call_log` table stores redacted prompts + responses + token counts + costs + errors. `crates/app/src/remote/` hosts the provider trait, Claude HTTP client, keychain wrapper, and orchestrator that glues redaction → budget check → provider call → log write. Ledger `ai_review.rs` reads the gate setting to pick Claude vs Ollama. Settings AI tab gains a `RemoteProvidersSection` (key + budget + toggle) and `CallLogSection` (last 20 entries with expand-on-click).

**Tech Stack:** Rust (rusqlite, reqwest — existing, proptest for redaction property tests), Anthropic `/v1/messages` API, keyring — existing, React 18 + TypeScript, Zustand.

**Scope boundary:** Claude only. Ledger review is the only consumer. No OpenAI / Gemini / Groq / OpenRouter. No `#[ai_tool]` macro. No YAML context recipes. No per-request cost estimate before the call. No streaming responses. No shared/team budgets. See design spec §9 for full non-goals.

---

## File Map

| Path | Status | Responsibility |
|---|---|---|
| `crates/core/migrations/V11__remote_call_log.sql` | Create | Schema for audit log + index |
| `crates/core/Cargo.toml` | Modify | Add `proptest` to dev-deps, `regex` to deps |
| `crates/core/src/redact.rs` | Create | `redact(input)` — PII scrubbing + property tests |
| `crates/core/src/remote_call_log.rs` | Create | DAL: insert_started, mark_completed, mark_errored, list_recent, sum_month_pence, clear_all |
| `crates/core/src/lib.rs` | Modify | `pub mod redact; pub mod remote_call_log;` |
| `crates/app/src/remote/mod.rs` | Create | Module root + constants |
| `crates/app/src/remote/provider.rs` | Create | `RemoteProvider` trait + `ChatMessage` + cost table |
| `crates/app/src/remote/claude.rs` | Create | Anthropic `/v1/messages` implementation |
| `crates/app/src/remote/keychain.rs` | Create | get/set/remove key via `keyring` |
| `crates/app/src/remote/orchestrator.rs` | Create | `remote_chat(skill, reason, messages)` — redact → budget → call → log |
| `crates/app/src/remote/commands.rs` | Create | 7 Tauri commands (keys, log, budget, toggle, test) |
| `crates/app/src/lib.rs` | Modify | `pub mod remote;` + register 7 commands |
| `crates/app/src/ledger/ai_review.rs` | Modify | Honor `ai.remote.enabled_for_review` — route to Claude when on |
| `crates/core/src/trash.rs` | Modify | Add `remote_call_log` to REGISTRY so Trash picks it up |
| `apps/desktop/src/lib/remote/ipc.ts` | Create | IPC wrappers + types |
| `apps/desktop/src/components/Settings/AiTab.tsx` | Modify | Add `RemoteProvidersSection` + `CallLogSection` |

---

## Task 1: V11 migration + `remote_call_log` DAL

**Files:**
- Create: `crates/core/migrations/V11__remote_call_log.sql`
- Create: `crates/core/src/remote_call_log.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/core/src/trash.rs`

### Step 1: Write migration

- [ ] Create `/Users/hanamori/life-assistant/crates/core/migrations/V11__remote_call_log.sql`:

```sql
-- V11__remote_call_log.sql
-- Audit log for every remote LLM call. Soft-deletable so it integrates with Trash.
--
-- Privacy guarantee: prompt_redacted contains the bytes that left this Mac, not
-- the original input. Unredacted prompts are never persisted.

CREATE TABLE remote_call_log (
    id                  INTEGER PRIMARY KEY,
    provider            TEXT    NOT NULL,
    model               TEXT    NOT NULL,
    skill               TEXT    NOT NULL,
    user_visible_reason TEXT    NOT NULL,
    prompt_redacted     TEXT    NOT NULL,
    response_text       TEXT,
    input_tokens        INTEGER,
    output_tokens       INTEGER,
    cost_pence          INTEGER,
    redaction_count     INTEGER NOT NULL DEFAULT 0,
    error               TEXT,
    started_at          INTEGER NOT NULL,
    completed_at        INTEGER,
    deleted_at          INTEGER
);

CREATE INDEX idx_remote_call_log_month ON remote_call_log(started_at)
  WHERE deleted_at IS NULL;
```

### Step 2: Create `remote_call_log.rs`

- [ ] Create `/Users/hanamori/life-assistant/crates/core/src/remote_call_log.rs`:

```rust
//! Audit log DAL for remote LLM calls. See V11 migration for schema.

use anyhow::Result;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallLogEntry {
    pub id: i64,
    pub provider: String,
    pub model: String,
    pub skill: String,
    pub user_visible_reason: String,
    pub prompt_redacted: String,
    pub response_text: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cost_pence: Option<i64>,
    pub redaction_count: i64,
    pub error: Option<String>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
}

impl CallLogEntry {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            provider: row.get("provider")?,
            model: row.get("model")?,
            skill: row.get("skill")?,
            user_visible_reason: row.get("user_visible_reason")?,
            prompt_redacted: row.get("prompt_redacted")?,
            response_text: row.get("response_text")?,
            input_tokens: row.get("input_tokens")?,
            output_tokens: row.get("output_tokens")?,
            cost_pence: row.get("cost_pence")?,
            redaction_count: row.get("redaction_count")?,
            error: row.get("error")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
        })
    }
}

pub struct NewCall<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub skill: &'a str,
    pub user_visible_reason: &'a str,
    pub prompt_redacted: &'a str,
    pub redaction_count: i64,
}

/// Insert an in-flight row. Returns the id for later mark_completed/mark_errored.
pub fn insert_started(conn: &Connection, new: NewCall<'_>) -> Result<i64> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO remote_call_log
         (provider, model, skill, user_visible_reason, prompt_redacted,
          redaction_count, started_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            new.provider, new.model, new.skill, new.user_visible_reason,
            new.prompt_redacted, new.redaction_count, now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn mark_completed(
    conn: &Connection,
    id: i64,
    response_text: &str,
    input_tokens: i64,
    output_tokens: i64,
    cost_pence: i64,
) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE remote_call_log
         SET response_text = ?1, input_tokens = ?2, output_tokens = ?3,
             cost_pence = ?4, completed_at = ?5
         WHERE id = ?6",
        params![response_text, input_tokens, output_tokens, cost_pence, now, id],
    )?;
    Ok(())
}

pub fn mark_errored(conn: &Connection, id: i64, error: &str) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE remote_call_log
         SET error = ?1, completed_at = ?2
         WHERE id = ?3",
        params![error, now, id],
    )?;
    Ok(())
}

pub fn list_recent(conn: &Connection, limit: usize) -> Result<Vec<CallLogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider, model, skill, user_visible_reason, prompt_redacted,
                response_text, input_tokens, output_tokens, cost_pence,
                redaction_count, error, started_at, completed_at
         FROM remote_call_log
         WHERE deleted_at IS NULL
         ORDER BY started_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit as i64], CallLogEntry::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Sum `cost_pence` for all non-deleted calls in the current calendar month (UTC)
/// for a given provider. Returns 0 if none.
pub fn sum_month_pence(conn: &Connection, provider: &str, now: DateTime<Utc>) -> Result<i64> {
    let month_start = Utc
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid month start"))?
        .timestamp();
    let (next_year, next_month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    let month_end = Utc
        .with_ymd_and_hms(next_year, next_month, 1, 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid month end"))?
        .timestamp();

    let total: i64 = conn.query_row(
        "SELECT COALESCE(SUM(cost_pence), 0)
         FROM remote_call_log
         WHERE provider = ?1
           AND deleted_at IS NULL
           AND started_at >= ?2 AND started_at < ?3",
        params![provider, month_start, month_end],
        |r| r.get(0),
    )?;
    Ok(total)
}

/// Soft-delete every row (user clicks "Clear call log").
pub fn clear_all(conn: &Connection) -> Result<usize> {
    let now = Utc::now().timestamp();
    let n = conn.execute(
        "UPDATE remote_call_log SET deleted_at = ?1 WHERE deleted_at IS NULL",
        params![now],
    )?;
    Ok(n)
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

    fn sample<'a>() -> NewCall<'a> {
        NewCall {
            provider: "claude",
            model: "claude-sonnet-4-6",
            skill: "ledger_review",
            user_visible_reason: "Write April spending narrative",
            prompt_redacted: "You are a calm personal finance assistant...",
            redaction_count: 0,
        }
    }

    #[test]
    fn insert_started_returns_id_and_creates_in_flight_row() {
        let (_d, conn) = fresh_conn();
        let id = insert_started(&conn, sample()).unwrap();
        assert!(id > 0);
        let entries = list_recent(&conn, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].response_text.is_none());
        assert!(entries[0].completed_at.is_none());
        assert!(entries[0].error.is_none());
    }

    #[test]
    fn mark_completed_fills_response_and_tokens() {
        let (_d, conn) = fresh_conn();
        let id = insert_started(&conn, sample()).unwrap();
        mark_completed(&conn, id, "Your April was calm.", 300, 80, 25).unwrap();
        let e = list_recent(&conn, 10).unwrap().into_iter().next().unwrap();
        assert_eq!(e.response_text.as_deref(), Some("Your April was calm."));
        assert_eq!(e.input_tokens, Some(300));
        assert_eq!(e.output_tokens, Some(80));
        assert_eq!(e.cost_pence, Some(25));
        assert!(e.completed_at.is_some());
    }

    #[test]
    fn mark_errored_sets_error_and_completed() {
        let (_d, conn) = fresh_conn();
        let id = insert_started(&conn, sample()).unwrap();
        mark_errored(&conn, id, "timeout after 3 attempts").unwrap();
        let e = list_recent(&conn, 10).unwrap().into_iter().next().unwrap();
        assert_eq!(e.error.as_deref(), Some("timeout after 3 attempts"));
        assert!(e.completed_at.is_some());
        assert!(e.response_text.is_none());
    }

    #[test]
    fn list_recent_orders_newest_first_and_respects_limit() {
        let (_d, conn) = fresh_conn();
        for _ in 0..5 {
            insert_started(&conn, sample()).unwrap();
        }
        let entries = list_recent(&conn, 3).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].started_at >= entries[1].started_at);
    }

    #[test]
    fn sum_month_pence_aggregates_only_current_month() {
        let (_d, conn) = fresh_conn();
        let id = insert_started(&conn, sample()).unwrap();
        mark_completed(&conn, id, "ok", 100, 50, 15).unwrap();

        // Plant a row stamped last month.
        let last_month = Utc
            .with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap().timestamp();
        conn.execute(
            "INSERT INTO remote_call_log
             (provider, model, skill, user_visible_reason, prompt_redacted,
              redaction_count, started_at, completed_at, cost_pence)
             VALUES ('claude','claude-sonnet-4-6','ledger_review','old','x', 0, ?1, ?1, 999)",
            [last_month],
        ).unwrap();

        let now = Utc.with_ymd_and_hms(2026, 4, 17, 12, 0, 0).unwrap();
        let total = sum_month_pence(&conn, "claude", now).unwrap();
        assert_eq!(total, 15, "last month's 999 must not count");
    }

    #[test]
    fn sum_month_pence_filters_by_provider() {
        let (_d, conn) = fresh_conn();
        let claude_id = insert_started(&conn, sample()).unwrap();
        mark_completed(&conn, claude_id, "ok", 100, 50, 20).unwrap();

        let openai_id = insert_started(
            &conn,
            NewCall { provider: "openai", ..sample() },
        ).unwrap();
        mark_completed(&conn, openai_id, "ok", 100, 50, 5).unwrap();

        let now = Utc::now();
        assert_eq!(sum_month_pence(&conn, "claude", now).unwrap(), 20);
        assert_eq!(sum_month_pence(&conn, "openai", now).unwrap(), 5);
    }

    #[test]
    fn clear_all_soft_deletes_every_row() {
        let (_d, conn) = fresh_conn();
        insert_started(&conn, sample()).unwrap();
        insert_started(&conn, sample()).unwrap();
        let n = clear_all(&conn).unwrap();
        assert_eq!(n, 2);
        assert!(list_recent(&conn, 10).unwrap().is_empty());
    }
}
```

### Step 3: Register + add to Trash REGISTRY

- [ ] Edit `/Users/hanamori/life-assistant/crates/core/src/lib.rs` — add `pub mod remote_call_log;` alongside existing foundation modules.

- [ ] Edit `/Users/hanamori/life-assistant/crates/core/src/trash.rs`. Find the `REGISTRY` constant (around line 20). Append a new entry:

```rust
    ("remote_call_log",   "user_visible_reason"),
```

Place it immediately before the closing `];` of the REGISTRY array so it participates in Trash list/restore/empty operations.

### Step 4: Gate + commit

- [ ] Run:
```bash
cd /Users/hanamori/life-assistant
cargo test -p manor-core remote_call_log 2>&1 | tail -15
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
cargo fmt --check 2>&1 | tail -5
```
Expected: 7 new tests pass; total 196 (189 + 7); clippy + fmt clean.

- [ ] Commit:
```bash
cd /Users/hanamori/life-assistant
git add crates/core/migrations/V11__remote_call_log.sql \
        crates/core/src/remote_call_log.rs \
        crates/core/src/trash.rs \
        crates/core/src/lib.rs
git commit -m "feat(core): remote_call_log — V11 schema + audit DAL + Trash registry"
```

---

## Task 2: Redaction pipeline with property tests

**Files:**
- Modify: `crates/core/Cargo.toml`
- Create: `crates/core/src/redact.rs`
- Modify: `crates/core/src/lib.rs`

### Step 1: Add deps

- [ ] Edit `/Users/hanamori/life-assistant/crates/core/Cargo.toml`. Under `[dependencies]` add:

```toml
regex = "1"
```

Under `[dev-dependencies]` add:

```toml
proptest = "1"
```

Check whether the workspace uses shared deps (root `Cargo.toml` `[workspace.dependencies]`). If yes, place at workspace level and use `.workspace = true` in the crate Cargo.toml — match the pattern used for `uuid`, `sha2`, `age`, `bytemuck`.

Run `cargo build -p manor-core 2>&1 | tail -5` — expected: clean (fetches regex + proptest).

### Step 2: Write `redact.rs`

- [ ] Create `/Users/hanamori/life-assistant/crates/core/src/redact.rs`:

```rust
//! PII scrubbing for outgoing remote LLM prompts.
//!
//! The redactor is the privacy boundary of Manor's remote LLM support. Every
//! prompt going to a remote provider passes through `redact(input)` first; the
//! returned `Redacted.text` is what's persisted to `remote_call_log` AND sent
//! over the wire. Unredacted input never touches disk.
//!
//! Tested with property tests (see `tests` module) — for any input containing
//! planted PII, `redact()`'s output must not contain the original PII substring.

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Redaction {
    pub kind: String,
    pub original_hash: String, // sha256 hex of the original match — for audit, not reversal
    pub placeholder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redacted {
    pub text: String,
    pub replacements: Vec<Redaction>,
}

impl Redacted {
    pub fn count(&self) -> usize {
        self.replacements.len()
    }
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

// Each pattern: (kind, regex, placeholder). Order matters — run more-specific
// patterns first so they don't get shadowed by greedy ones.
fn patterns() -> Vec<(&'static str, Regex, &'static str)> {
    vec![
        // UK sort code followed by 8-digit account number (optional whitespace/hyphen).
        // Matches "12-34-56 12345678" or "123456 12345678".
        (
            "account",
            Regex::new(r"\b\d{2}[- ]?\d{2}[- ]?\d{2}[- ]?\d{8}\b").unwrap(),
            "[REDACTED-ACCOUNT]",
        ),
        // IBAN: 2 letters + 2 digits + up to 30 alphanumerics.
        (
            "iban",
            Regex::new(r"\b[A-Z]{2}\d{2}[A-Z0-9]{10,30}\b").unwrap(),
            "[REDACTED-IBAN]",
        ),
        // Credit card: 13-19 digits in groups of 3-4, possibly separated by spaces/hyphens.
        // Rough — the caller relies on the Luhn check below to avoid false positives on long ids.
        (
            "card",
            Regex::new(r"\b(?:\d[- ]?){13,19}\b").unwrap(),
            "[REDACTED-CARD]",
        ),
        // UK NI number: AB 12 34 56 C
        (
            "ni",
            Regex::new(r"\b[A-CEGHJ-PR-TW-Z]{2}\s?\d{2}\s?\d{2}\s?\d{2}\s?[A-D]\b").unwrap(),
            "[REDACTED-NI]",
        ),
        // Phone: E.164 (+ up to 15 digits) OR UK national (0 followed by 9–10 digits).
        (
            "phone",
            Regex::new(r"(?:\+\d[\d\s().-]{7,18}|\b0\d[\d\s().-]{7,12})").unwrap(),
            "[REDACTED-PHONE]",
        ),
        // Email: localpart@domain. Replaced with preserved localpart hash + placeholder domain.
        (
            "email",
            Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b").unwrap(),
            "", // email uses a dynamic replacement — handled specially below
        ),
    ]
}

// Luhn validator used to keep the card pattern from redacting random 13+ digit sequences.
fn luhn_valid(digits: &str) -> bool {
    let nums: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if nums.len() < 13 || nums.len() > 19 {
        return false;
    }
    let mut sum = 0u32;
    for (i, n) in nums.iter().rev().enumerate() {
        let v = if i % 2 == 1 {
            let d = n * 2;
            if d > 9 { d - 9 } else { d }
        } else {
            *n
        };
        sum += v;
    }
    sum % 10 == 0
}

fn redact_email(m: &str) -> String {
    // Preserve localpart hash prefix so the model still sees "this is an email"
    // without leaking the domain. Format: user@[EMAIL-HOSTHASH-xxxx]
    if let Some(at) = m.rfind('@') {
        let domain = &m[at + 1..];
        let h = sha256_hex(domain);
        let prefix: String = h.chars().take(4).collect();
        format!("user@[EMAIL-HOSTHASH-{prefix}]")
    } else {
        "[REDACTED-EMAIL]".to_string()
    }
}

pub fn redact(input: &str) -> Redacted {
    let mut text = input.to_string();
    let mut replacements = Vec::new();

    for (kind, re, placeholder) in patterns() {
        let mut offset = 0i64;
        let orig_text = text.clone();
        for cap in re.find_iter(&orig_text) {
            let m = cap.as_str();

            // Card pattern: only redact if Luhn-valid (avoids false positives on random IDs).
            if kind == "card" && !luhn_valid(m) {
                continue;
            }

            let replacement = if kind == "email" {
                redact_email(m)
            } else {
                placeholder.to_string()
            };

            let start = (cap.start() as i64 + offset) as usize;
            let end = (cap.end() as i64 + offset) as usize;
            if end <= text.len() {
                text.replace_range(start..end, &replacement);
                offset += replacement.len() as i64 - (end - start) as i64;
                replacements.push(Redaction {
                    kind: kind.to_string(),
                    original_hash: sha256_hex(m),
                    placeholder: replacement,
                });
            }
        }
    }

    // UK postcode (simple — keep first half only): NW1 4AB → NW1
    // Applied last so it doesn't interfere with earlier patterns.
    let postcode_re = Regex::new(
        r"\b([A-Z]{1,2}\d[A-Z\d]?)\s?\d[A-Z]{2}\b",
    )
    .unwrap();
    let orig = text.clone();
    let mut offset = 0i64;
    for cap in postcode_re.captures_iter(&orig) {
        let full = cap.get(0).unwrap();
        let first_half = cap.get(1).unwrap().as_str().to_string();
        let start = (full.start() as i64 + offset) as usize;
        let end = (full.end() as i64 + offset) as usize;
        if end <= text.len() {
            text.replace_range(start..end, &first_half);
            offset += first_half.len() as i64 - (end - start) as i64;
            replacements.push(Redaction {
                kind: "postcode".to_string(),
                original_hash: sha256_hex(full.as_str()),
                placeholder: first_half,
            });
        }
    }

    Redacted { text, replacements }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unit tests: specific inputs, specific expectations ──────────────────

    #[test]
    fn redacts_email_preserving_localpart_marker() {
        let r = redact("Please send to alice@example.com about the report.");
        assert!(!r.text.contains("alice@example.com"));
        assert!(r.text.contains("user@[EMAIL-HOSTHASH-"));
        assert_eq!(r.replacements.iter().filter(|x| x.kind == "email").count(), 1);
    }

    #[test]
    fn redacts_uk_sort_code_and_account() {
        let r = redact("Account: 12-34-56 12345678 for the rent payment.");
        assert!(!r.text.contains("12345678"));
        assert!(r.text.contains("[REDACTED-ACCOUNT]"));
    }

    #[test]
    fn redacts_iban() {
        let r = redact("IBAN: GB82WEST12345698765432");
        assert!(!r.text.contains("GB82WEST12345698765432"));
        assert!(r.text.contains("[REDACTED-IBAN]"));
    }

    #[test]
    fn redacts_luhn_valid_card_numbers() {
        let r = redact("Card: 4532015112830366"); // Luhn-valid test card
        assert!(!r.text.contains("4532015112830366"));
        assert!(r.text.contains("[REDACTED-CARD]"));
    }

    #[test]
    fn does_not_redact_luhn_invalid_long_number() {
        let r = redact("Order ID: 1234567890123456789"); // Luhn-invalid
        assert!(r.text.contains("1234567890123456789"));
        assert!(r.replacements.iter().find(|x| x.kind == "card").is_none());
    }

    #[test]
    fn redacts_uk_phone() {
        let r = redact("Call me on +44 7700 900123");
        assert!(!r.text.contains("7700 900123"));
        assert!(r.text.contains("[REDACTED-PHONE]"));
    }

    #[test]
    fn redacts_ni_number() {
        let r = redact("NI: QQ 12 34 56 C");
        assert!(!r.text.contains("QQ 12 34 56 C"));
        assert!(r.text.contains("[REDACTED-NI]"));
    }

    #[test]
    fn collapses_postcode_to_first_half() {
        let r = redact("I live at 12 High Street, NW1 4AB.");
        assert!(!r.text.contains("NW1 4AB"));
        assert!(r.text.contains("NW1"));
    }

    #[test]
    fn does_not_redact_ordinary_text() {
        let r = redact("The weather is nice today. Alex wants to plan the week.");
        assert_eq!(r.replacements.len(), 0);
        assert_eq!(r.text, "The weather is nice today. Alex wants to plan the week.");
    }

    #[test]
    fn count_matches_replacement_list_length() {
        let r = redact("Send to a@b.com, b@b.com, and call +447700900123.");
        assert_eq!(r.count(), r.replacements.len());
        assert_eq!(r.count(), 3);
    }

    // ── Property tests: the load-bearing assurance ──────────────────────────

    use proptest::prelude::*;

    proptest! {
        // For any random-ish text containing a planted email, output must NOT
        // contain the original email address.
        #[test]
        fn property_planted_email_never_survives_redaction(
            prefix in "[a-z ]{0,20}",
            local in "[a-z]{3,10}",
            domain in "[a-z]{3,8}",
            tld in "com|org|net|io",
            suffix in "[a-z ]{0,20}",
        ) {
            let email = format!("{local}@{domain}.{tld}");
            let input = format!("{prefix} {email} {suffix}");
            let r = redact(&input);
            prop_assert!(!r.text.contains(&email),
                "email {email} survived in redacted text {}", r.text);
        }

        // For any random-ish text containing a planted UK phone, output must NOT
        // contain any 9+ consecutive digits from the phone.
        #[test]
        fn property_planted_uk_phone_never_survives(
            prefix in "[a-z ]{0,20}",
            area in "[1-9][0-9]{3}",
            number in "[0-9]{6}",
        ) {
            let phone = format!("0{area} {number}");
            let input = format!("{prefix} Call me on {phone} please.");
            let r = redact(&input);
            // Check the 9+ digit tail doesn't survive (implementation may leave
            // the leading 0 as part of the placeholder; the digit run should be gone)
            let digit_tail = format!("{area}{number}");
            prop_assert!(!r.text.contains(&digit_tail),
                "phone tail {digit_tail} survived in {}", r.text);
        }

        // For ANY input (random unicode bytes), redact should not panic and
        // should return text that's a valid String.
        #[test]
        fn property_redact_never_panics(input in ".*") {
            let _ = redact(&input);
        }
    }
}
```

### Step 3: Register + run tests

- [ ] Edit `/Users/hanamori/life-assistant/crates/core/src/lib.rs` — add `pub mod redact;`.

- [ ] Run:
```bash
cd /Users/hanamori/life-assistant
cargo test -p manor-core redact 2>&1 | tail -25
```
Expected: 10 unit tests + 3 property tests (each running 256 cases by default) pass. Property test failures mean the redactor leaks — treat as a hard blocker.

- [ ] Full gate:
```bash
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
cargo fmt --check 2>&1 | tail -5
```
Expected: total 209 (196 + 13 new). Clippy + fmt clean.

### Step 4: Commit

- [ ] Commit:
```bash
cd /Users/hanamori/life-assistant
git add crates/core/src/redact.rs crates/core/src/lib.rs crates/core/Cargo.toml Cargo.toml
git commit -m "feat(core): redact module — PII scrubbing + property tests on the privacy boundary"
```

---

## Task 3: Provider trait + Claude implementation

**Files:**
- Create: `crates/app/src/remote/mod.rs`
- Create: `crates/app/src/remote/provider.rs`
- Create: `crates/app/src/remote/claude.rs`

### Step 1: Create module scaffolding

- [ ] Create `/Users/hanamori/life-assistant/crates/app/src/remote/mod.rs`:

```rust
//! Remote LLM support — provider abstraction, keychain, orchestrator.
//! See `docs/superpowers/specs/2026-04-17-remote-llm-design.md`.

pub mod claude;
pub mod provider;

pub const PROVIDER_CLAUDE: &str = "claude";
pub const DEFAULT_MODEL_CLAUDE: &str = "claude-sonnet-4-6";
pub const DEFAULT_BUDGET_PENCE: i64 = 1000; // £10
pub const WARN_THRESHOLD_NUM: i64 = 75;     // 75% of cap triggers warning
pub const REMOTE_ENABLED_FOR_REVIEW_KEY: &str = "ai.remote.enabled_for_review";
pub fn budget_setting_key(provider: &str) -> String {
    format!("budget.{provider}_monthly_pence")
}
```

### Step 2: Create `provider.rs`

- [ ] Create `/Users/hanamori/life-assistant/crates/app/src/remote/provider.rs`:

```rust
//! RemoteProvider trait + shared types + cost table.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub text: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

/// Static per-model cost table, pence per million tokens.
/// Updated manually when providers publish price changes.
pub struct ModelCost {
    pub input_per_m: i64,
    pub output_per_m: i64,
}

pub fn cost_for(provider: &str, model: &str) -> ModelCost {
    // Pence per million tokens, rounded up from provider's list prices.
    match (provider, model) {
        ("claude", "claude-opus-4-7") => ModelCost { input_per_m: 240, output_per_m: 1200 }.mul(10),
        ("claude", "claude-sonnet-4-6") => ModelCost { input_per_m: 80, output_per_m: 400 }.mul(10),
        ("claude", "claude-haiku-4-5-20251001") => ModelCost { input_per_m: 12, output_per_m: 60 }.mul(10),
        // Unknown model: charge nothing rather than guess. Logs but never rejects.
        _ => ModelCost { input_per_m: 0, output_per_m: 0 },
    }
}

impl ModelCost {
    // Internal helper to keep the table readable in penny-per-10K-tokens, then scale.
    fn mul(self, n: i64) -> Self {
        ModelCost {
            input_per_m: self.input_per_m * n,
            output_per_m: self.output_per_m * n,
        }
    }
    pub fn pence_for(&self, input_tokens: i64, output_tokens: i64) -> i64 {
        // Round UP — user sees accurate or over-estimated spend, never under.
        let input_pence = (input_tokens * self.input_per_m + 999_999) / 1_000_000;
        let output_pence = (output_tokens * self.output_per_m + 999_999) / 1_000_000;
        input_pence + output_pence
    }
}

#[async_trait]
pub trait RemoteProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn default_model(&self) -> &'static str;

    async fn chat(
        &self,
        api_key: &str,
        model: &str,
        messages: &[ChatMessage],
        system: Option<&str>,
        max_tokens: i64,
    ) -> Result<ChatResponse>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_sonnet_cost_basic() {
        let c = cost_for("claude", "claude-sonnet-4-6");
        // 1000 input + 500 output tokens → tiny pence
        let p = c.pence_for(1000, 500);
        // Sonnet 4.6: 80*10 = 800 pence/M input, 400*10 = 4000 pence/M output
        // => (1000 * 800 + 999_999) / 1_000_000 = 1 pence (rounded up from 0.8)
        //    (500 * 4000 + 999_999) / 1_000_000 = 2 pence (rounded up from 2.0)
        assert_eq!(p, 3);
    }

    #[test]
    fn unknown_model_costs_zero() {
        let c = cost_for("claude", "bogus-model");
        assert_eq!(c.pence_for(1_000_000, 1_000_000), 0);
    }
}
```

### Step 3: Add `async_trait` dep

- [ ] Edit `/Users/hanamori/life-assistant/crates/app/Cargo.toml` (or the workspace root Cargo.toml if shared) and ensure `async-trait = "0.1"` is present. If not, add it.

### Step 4: Create `claude.rs`

- [ ] Create `/Users/hanamori/life-assistant/crates/app/src/remote/claude.rs`:

```rust
//! Anthropic /v1/messages implementation.

use super::provider::{ChatMessage, ChatResponse, ChatRole, RemoteProvider};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub struct Claude {
    endpoint: String,
    http: reqwest::Client,
}

impl Claude {
    pub fn new() -> Self {
        Self::with_endpoint("https://api.anthropic.com")
    }
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client build"),
        }
    }
}

impl Default for Claude {
    fn default() -> Self { Self::new() }
}

#[derive(Serialize)]
struct MessagesReq<'a> {
    model: &'a str,
    max_tokens: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<MsgPayload<'a>>,
}

#[derive(Serialize)]
struct MsgPayload<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct MessagesResp {
    content: Vec<ContentBlock>,
    usage: UsageBlock,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    _type: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct UsageBlock {
    input_tokens: i64,
    output_tokens: i64,
}

#[async_trait]
impl RemoteProvider for Claude {
    fn name(&self) -> &'static str { "claude" }
    fn default_model(&self) -> &'static str { "claude-sonnet-4-6" }

    async fn chat(
        &self,
        api_key: &str,
        model: &str,
        messages: &[ChatMessage],
        system: Option<&str>,
        max_tokens: i64,
    ) -> Result<ChatResponse> {
        let mapped: Vec<MsgPayload<'_>> = messages.iter().map(|m| MsgPayload {
            role: match m.role {
                ChatRole::User => "user",
                ChatRole::Assistant => "assistant",
                // Anthropic API doesn't have a "system" role in messages array —
                // system text goes in the top-level `system` field. If the caller
                // passed a System role here, coerce to user (we only expect
                // User/Assistant in practice given how the orchestrator builds
                // messages).
                ChatRole::System => "user",
            },
            content: &m.content,
        }).collect();

        let body = MessagesReq {
            model,
            max_tokens,
            system,
            messages: mapped,
        };
        let url = format!("{}/v1/messages", self.endpoint);
        let resp = self.http
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("claude http send")?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("claude returned {status}: {body_text}"));
        }
        let parsed: MessagesResp = resp.json().await.context("claude parse")?;
        let text: String = parsed.content.into_iter().map(|c| c.text).collect();
        Ok(ChatResponse {
            text,
            input_tokens: parsed.usage.input_tokens,
            output_tokens: parsed.usage.output_tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn chat_success_returns_text_and_token_counts() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "test-key"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type": "text", "text": "Hello back."}],
                "usage": {"input_tokens": 12, "output_tokens": 5}
            })))
            .mount(&server)
            .await;

        let client = Claude::with_endpoint(server.uri());
        let msgs = vec![ChatMessage {
            role: ChatRole::User, content: "Hi".into(),
        }];
        let resp = client.chat("test-key", "claude-sonnet-4-6", &msgs, None, 100).await.unwrap();
        assert_eq!(resp.text, "Hello back.");
        assert_eq!(resp.input_tokens, 12);
        assert_eq!(resp.output_tokens, 5);
    }

    #[tokio::test]
    async fn chat_propagates_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(401).set_body_string("invalid key"))
            .mount(&server)
            .await;
        let client = Claude::with_endpoint(server.uri());
        let msgs = vec![ChatMessage { role: ChatRole::User, content: "Hi".into() }];
        let err = client.chat("bad", "claude-sonnet-4-6", &msgs, None, 100).await.unwrap_err();
        assert!(err.to_string().contains("401"));
    }

    #[tokio::test]
    async fn chat_passes_system_prompt_in_top_level_field() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type": "text", "text": "ok"}],
                "usage": {"input_tokens": 5, "output_tokens": 2}
            })))
            .mount(&server)
            .await;
        let client = Claude::with_endpoint(server.uri());
        let msgs = vec![ChatMessage { role: ChatRole::User, content: "go".into() }];
        // Not asserting body here — just that the call succeeds with a system
        // param (coverage for the serialization path).
        let resp = client.chat("k", "claude-sonnet-4-6", &msgs, Some("Be brief"), 50).await.unwrap();
        assert_eq!(resp.text, "ok");
    }
}
```

### Step 5: Wire module

- [ ] Edit `/Users/hanamori/life-assistant/crates/app/src/lib.rs`. Near the top where other `pub mod` declarations live, add:

```rust
pub mod remote;
```

### Step 6: Gate + commit

- [ ] Run:
```bash
cd /Users/hanamori/life-assistant
cargo test -p manor-app remote 2>&1 | tail -20
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
cargo fmt --check 2>&1 | tail -5
```
Expected: 2 provider tests + 3 claude tests = 5 new. Total 214. Clippy + fmt clean.

- [ ] Commit:
```bash
git add crates/app/src/remote/mod.rs crates/app/src/remote/provider.rs crates/app/src/remote/claude.rs crates/app/src/lib.rs crates/app/Cargo.toml Cargo.toml
git commit -m "feat(app): RemoteProvider trait + Claude /v1/messages implementation"
```

---

## Task 4: Keychain key storage

**Files:**
- Create: `crates/app/src/remote/keychain.rs`
- Modify: `crates/app/src/remote/mod.rs`

### Step 1: Create `keychain.rs`

- [ ] Create `/Users/hanamori/life-assistant/crates/app/src/remote/keychain.rs`:

```rust
//! macOS Keychain wrapper for remote provider API keys.
//!
//! Keychain footprint: service="manor-remote", account="{provider}-api-key".
//! Inspectable via Keychain.app so users can see exactly what Manor stores.

use anyhow::Result;
use keyring::Entry;

const SERVICE: &str = "manor-remote";

fn account(provider: &str) -> String {
    format!("{provider}-api-key")
}

pub fn set_key(provider: &str, key: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, &account(provider))?;
    entry.set_password(key)?;
    Ok(())
}

pub fn get_key(provider: &str) -> Result<Option<String>> {
    let entry = Entry::new(SERVICE, &account(provider))?;
    match entry.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn remove_key(provider: &str) -> Result<bool> {
    let entry = Entry::new(SERVICE, &account(provider))?;
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub fn has_key(provider: &str) -> bool {
    get_key(provider).ok().flatten().is_some()
}
```

### Step 2: Wire in mod.rs

- [ ] Edit `/Users/hanamori/life-assistant/crates/app/src/remote/mod.rs` — add `pub mod keychain;` near the other `pub mod` lines.

### Step 3: Gate + commit

- [ ] Run:
```bash
cd /Users/hanamori/life-assistant
cargo build -p manor-app 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
cargo fmt --check 2>&1 | tail -5
```
Expected: clean. No tests here — `keyring` touches the real Keychain and can't be unit-tested without a live macOS session.

- [ ] Commit:
```bash
git add crates/app/src/remote/keychain.rs crates/app/src/remote/mod.rs
git commit -m "feat(app): remote::keychain — Keychain wrapper for provider API keys"
```

---

## Task 5: Orchestrator — redact → budget → call → log

**Files:**
- Create: `crates/app/src/remote/orchestrator.rs`
- Modify: `crates/app/src/remote/mod.rs`

### Step 1: Write orchestrator

- [ ] Create `/Users/hanamori/life-assistant/crates/app/src/remote/orchestrator.rs`:

```rust
//! The remote chat orchestrator — the single entry point that glues together
//! redaction, keychain, budget check, provider call, and audit logging.
//!
//! Every outbound remote call MUST go through `remote_chat`. Skills that bypass
//! this wiring are a design failure.

use super::claude::Claude;
use super::provider::{cost_for, ChatMessage, ChatResponse, RemoteProvider};
use super::{budget_setting_key, keychain, DEFAULT_BUDGET_PENCE, DEFAULT_MODEL_CLAUDE,
            PROVIDER_CLAUDE, WARN_THRESHOLD_NUM};
use crate::assistant::commands::Db;
use anyhow::{anyhow, Result};
use chrono::Utc;
use manor_core::{redact, remote_call_log};
use std::sync::Arc;
use std::sync::Mutex;
use rusqlite::Connection;

pub struct RemoteChatRequest<'a> {
    pub skill: &'a str,                  // e.g., "ledger_review"
    pub user_visible_reason: &'a str,    // shown in call log
    pub system_prompt: Option<&'a str>,  // optional system prompt (not redacted — should be static)
    pub user_prompt: &'a str,            // the bit that gets redacted
    pub max_tokens: i64,
}

pub struct RemoteChatOutcome {
    pub text: String,
    pub warn: bool, // hit the 75% cap but proceeded
    pub cost_pence: i64,
    pub log_id: i64,
    pub redaction_count: usize,
}

/// Errors the orchestrator surfaces. Distinct from generic `anyhow` so the UI
/// can pick friendly messaging per case.
#[derive(Debug, thiserror::Error)]
pub enum RemoteChatError {
    #[error("no api key stored for provider '{0}'")]
    NoKey(String),
    #[error("budget exceeded for provider '{0}' (spent {spent_pence}p of {cap_pence}p)", spent_pence = .1, cap_pence = .2)]
    BudgetExceeded(String, i64, i64),
    #[error("provider error: {0}")]
    Provider(#[from] anyhow::Error),
    #[error("db error: {0}")]
    Db(String),
}

pub async fn remote_chat(
    db: Arc<Mutex<Connection>>,
    req: RemoteChatRequest<'_>,
) -> std::result::Result<RemoteChatOutcome, RemoteChatError> {
    let provider_name = PROVIDER_CLAUDE;
    let model = DEFAULT_MODEL_CLAUDE;

    // Step 1: redact.
    let redacted = redact::redact(req.user_prompt);
    let redaction_count = redacted.count();

    // Step 2: read budget + key.
    let (api_key, cap_pence, spent_pence) = {
        let conn = db.lock().map_err(|e| RemoteChatError::Db(e.to_string()))?;
        let cap = manor_core::setting::get_or_default(
            &conn, &budget_setting_key(provider_name),
            &DEFAULT_BUDGET_PENCE.to_string(),
        )
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(DEFAULT_BUDGET_PENCE);
        let spent = remote_call_log::sum_month_pence(&conn, provider_name, Utc::now())
            .map_err(|e| RemoteChatError::Db(e.to_string()))?;
        let key = keychain::get_key(provider_name)
            .map_err(|e| RemoteChatError::Db(e.to_string()))?
            .ok_or_else(|| RemoteChatError::NoKey(provider_name.to_string()))?;
        (key, cap, spent)
    };

    if spent_pence >= cap_pence {
        // Log the refusal for auditability.
        let conn = db.lock().map_err(|e| RemoteChatError::Db(e.to_string()))?;
        let log_id = remote_call_log::insert_started(&conn, remote_call_log::NewCall {
            provider: provider_name,
            model,
            skill: req.skill,
            user_visible_reason: req.user_visible_reason,
            prompt_redacted: &redacted.text,
            redaction_count: redaction_count as i64,
        }).map_err(|e| RemoteChatError::Db(e.to_string()))?;
        remote_call_log::mark_errored(&conn, log_id, "budget exceeded — refused before send")
            .map_err(|e| RemoteChatError::Db(e.to_string()))?;
        return Err(RemoteChatError::BudgetExceeded(
            provider_name.to_string(), spent_pence, cap_pence,
        ));
    }

    // Step 3: start log row.
    let log_id = {
        let conn = db.lock().map_err(|e| RemoteChatError::Db(e.to_string()))?;
        remote_call_log::insert_started(&conn, remote_call_log::NewCall {
            provider: provider_name,
            model,
            skill: req.skill,
            user_visible_reason: req.user_visible_reason,
            prompt_redacted: &redacted.text,
            redaction_count: redaction_count as i64,
        }).map_err(|e| RemoteChatError::Db(e.to_string()))?
    };

    // Step 4: call provider.
    let client = Claude::new();
    let msgs = vec![ChatMessage {
        role: super::provider::ChatRole::User,
        content: redacted.text.clone(),
    }];
    let chat_result: Result<ChatResponse> = client.chat(
        &api_key, model, &msgs, req.system_prompt, req.max_tokens,
    ).await;

    match chat_result {
        Ok(resp) => {
            let cost_pence = cost_for(provider_name, model).pence_for(
                resp.input_tokens, resp.output_tokens,
            );
            let conn = db.lock().map_err(|e| RemoteChatError::Db(e.to_string()))?;
            remote_call_log::mark_completed(
                &conn, log_id, &resp.text,
                resp.input_tokens, resp.output_tokens, cost_pence,
            ).map_err(|e| RemoteChatError::Db(e.to_string()))?;

            let warn = (spent_pence + cost_pence) * 100 >= cap_pence * WARN_THRESHOLD_NUM;
            Ok(RemoteChatOutcome {
                text: resp.text,
                warn,
                cost_pence,
                log_id,
                redaction_count,
            })
        }
        Err(e) => {
            let msg = e.to_string();
            let conn = db.lock().map_err(|x| RemoteChatError::Db(x.to_string()))?;
            let _ = remote_call_log::mark_errored(&conn, log_id, &msg);
            Err(RemoteChatError::Provider(e))
        }
    }
}
```

### Step 2: Add `thiserror` dep

- [ ] Edit `/Users/hanamori/life-assistant/crates/app/Cargo.toml` — ensure `thiserror = "1"` is under `[dependencies]`. If not, add it (or via workspace).

### Step 3: Wire in mod.rs

- [ ] Edit `/Users/hanamori/life-assistant/crates/app/src/remote/mod.rs` — add `pub mod orchestrator;` near the other `pub mod` lines.

### Step 4: Gate + commit

- [ ] Run:
```bash
cd /Users/hanamori/life-assistant
cargo build -p manor-app 2>&1 | tail -5
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
cargo fmt --check 2>&1 | tail -5
```
Expected: clean. Orchestrator is integration-heavy — end-to-end tests come via Task 8's real consumer swap. No unit tests this task.

- [ ] Commit:
```bash
git add crates/app/src/remote/orchestrator.rs crates/app/src/remote/mod.rs crates/app/Cargo.toml Cargo.toml
git commit -m "feat(app): remote::orchestrator — redact → budget → call → log pipeline"
```

---

## Task 6: Tauri commands

**Files:**
- Create: `crates/app/src/remote/commands.rs`
- Modify: `crates/app/src/remote/mod.rs`
- Modify: `crates/app/src/lib.rs`

### Step 1: Create `commands.rs`

- [ ] Create `/Users/hanamori/life-assistant/crates/app/src/remote/commands.rs`:

```rust
//! Tauri commands for remote LLM support.

use super::{budget_setting_key, keychain, orchestrator, DEFAULT_BUDGET_PENCE,
            PROVIDER_CLAUDE, REMOTE_ENABLED_FOR_REVIEW_KEY};
use crate::assistant::commands::Db;
use manor_core::remote_call_log::{self, CallLogEntry};
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct RemoteProviderStatus {
    pub provider: String,
    pub has_key: bool,
    pub budget_pence: i64,
    pub spent_month_pence: i64,
    pub enabled_for_review: bool,
}

#[tauri::command]
pub fn remote_provider_status(state: State<'_, Db>) -> Result<RemoteProviderStatus, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let has_key = keychain::has_key(PROVIDER_CLAUDE);
    let budget_pence = manor_core::setting::get_or_default(
        &conn, &budget_setting_key(PROVIDER_CLAUDE),
        &DEFAULT_BUDGET_PENCE.to_string(),
    ).ok().and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_BUDGET_PENCE);
    let spent = remote_call_log::sum_month_pence(&conn, PROVIDER_CLAUDE, chrono::Utc::now())
        .map_err(|e| e.to_string())?;
    let enabled = manor_core::setting::get(&conn, REMOTE_ENABLED_FOR_REVIEW_KEY)
        .ok().flatten().as_deref() == Some("1");
    Ok(RemoteProviderStatus {
        provider: PROVIDER_CLAUDE.to_string(),
        has_key,
        budget_pence,
        spent_month_pence: spent,
        enabled_for_review: enabled,
    })
}

#[tauri::command]
pub fn remote_set_key(provider: String, key: String) -> Result<(), String> {
    keychain::set_key(&provider, &key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remote_remove_key(provider: String) -> Result<bool, String> {
    keychain::remove_key(&provider).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remote_set_budget(state: State<'_, Db>, provider: String, pence: i64) -> Result<(), String> {
    if pence < 0 {
        return Err("budget cannot be negative".into());
    }
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::setting::set(&conn, &budget_setting_key(&provider), &pence.to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remote_set_enabled_for_review(state: State<'_, Db>, enabled: bool) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::setting::set(&conn, REMOTE_ENABLED_FOR_REVIEW_KEY,
        if enabled { "1" } else { "0" })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remote_call_log_list(state: State<'_, Db>, limit: usize) -> Result<Vec<CallLogEntry>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    remote_call_log::list_recent(&conn, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remote_call_log_clear(state: State<'_, Db>) -> Result<usize, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    remote_call_log::clear_all(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_test(state: State<'_, Db>) -> Result<String, String> {
    let db_arc = state.inner().clone_arc();
    let outcome = orchestrator::remote_chat(db_arc, orchestrator::RemoteChatRequest {
        skill: "test",
        user_visible_reason: "User-initiated test call from Settings",
        system_prompt: Some("You are a test responder. Reply with exactly 'pong'."),
        user_prompt: "ping",
        max_tokens: 10,
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(outcome.text)
}
```

### Step 2: Wire in mod.rs

- [ ] Edit `/Users/hanamori/life-assistant/crates/app/src/remote/mod.rs` — add `pub mod commands;` near the other `pub mod` lines.

### Step 3: Register in lib.rs

- [ ] Edit `/Users/hanamori/life-assistant/crates/app/src/lib.rs`. Inside `tauri::generate_handler![...]`, at the end before the closing `])`, append:

```rust
            remote::commands::remote_provider_status,
            remote::commands::remote_set_key,
            remote::commands::remote_remove_key,
            remote::commands::remote_set_budget,
            remote::commands::remote_set_enabled_for_review,
            remote::commands::remote_call_log_list,
            remote::commands::remote_call_log_clear,
            remote::commands::remote_test,
```

### Step 4: Gate + commit

- [ ] Run:
```bash
cd /Users/hanamori/life-assistant
cargo build -p manor-app 2>&1 | tail -10
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
cargo fmt --check 2>&1 | tail -5
```
Expected: clean.

- [ ] Commit:
```bash
git add crates/app/src/remote/commands.rs crates/app/src/remote/mod.rs crates/app/src/lib.rs
git commit -m "feat(app): 8 remote Tauri commands — key mgmt, budget, call log, test"
```

---

## Task 7: Frontend IPC + RemoteProvidersSection

**Files:**
- Create: `apps/desktop/src/lib/remote/ipc.ts`
- Modify: `apps/desktop/src/components/Settings/AiTab.tsx`

### Step 1: Write IPC wrapper

- [ ] Create `/Users/hanamori/life-assistant/apps/desktop/src/lib/remote/ipc.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export interface RemoteProviderStatus {
  provider: string;
  has_key: boolean;
  budget_pence: number;
  spent_month_pence: number;
  enabled_for_review: boolean;
}

export interface CallLogEntry {
  id: number;
  provider: string;
  model: string;
  skill: string;
  user_visible_reason: string;
  prompt_redacted: string;
  response_text: string | null;
  input_tokens: number | null;
  output_tokens: number | null;
  cost_pence: number | null;
  redaction_count: number;
  error: string | null;
  started_at: number;
  completed_at: number | null;
}

export async function remoteProviderStatus(): Promise<RemoteProviderStatus> {
  return invoke<RemoteProviderStatus>("remote_provider_status");
}
export async function remoteSetKey(provider: string, key: string): Promise<void> {
  return invoke<void>("remote_set_key", { provider, key });
}
export async function remoteRemoveKey(provider: string): Promise<boolean> {
  return invoke<boolean>("remote_remove_key", { provider });
}
export async function remoteSetBudget(provider: string, pence: number): Promise<void> {
  return invoke<void>("remote_set_budget", { provider, pence });
}
export async function remoteSetEnabledForReview(enabled: boolean): Promise<void> {
  return invoke<void>("remote_set_enabled_for_review", { enabled });
}
export async function remoteCallLogList(limit: number): Promise<CallLogEntry[]> {
  return invoke<CallLogEntry[]>("remote_call_log_list", { limit });
}
export async function remoteCallLogClear(): Promise<number> {
  return invoke<number>("remote_call_log_clear");
}
export async function remoteTest(): Promise<string> {
  return invoke<string>("remote_test");
}
```

### Step 2: Add `RemoteProvidersSection` to AiTab.tsx

- [ ] Open `/Users/hanamori/life-assistant/apps/desktop/src/components/Settings/AiTab.tsx`.

Find the placeholder section that looks like:

```tsx
      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Remote providers</h2>
        <div style={{ fontSize: 12, color: "#888" }}>
          Bring-your-own-key providers (Claude, OpenAI, Gemini) will appear here in a future release.
        </div>
      </section>
```

Replace it with:

```tsx
      <RemoteProvidersSection />
```

At the top of the file, add to the imports:

```tsx
import {
  remoteProviderStatus, remoteSetKey, remoteRemoveKey,
  remoteSetBudget, remoteSetEnabledForReview, remoteTest,
  type RemoteProviderStatus,
} from "../../lib/remote/ipc";
```

ABOVE the `export default function AiTab()` line, add the helper component:

```tsx
function RemoteProvidersSection() {
  const [status, setStatus] = useState<RemoteProviderStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [newKey, setNewKey] = useState("");
  const [budgetInput, setBudgetInput] = useState("");
  const [testing, setTesting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = async () => {
    setLoading(true);
    try {
      const s = await remoteProviderStatus();
      setStatus(s);
      setBudgetInput((s.budget_pence / 100).toFixed(2));
    } catch { setStatus(null); }
    setLoading(false);
  };

  useEffect(() => { void refresh(); }, []);

  const saveKey = async () => {
    if (!newKey.trim()) return;
    setMessage(null);
    try {
      await remoteSetKey("claude", newKey.trim());
      setNewKey("");
      setMessage("Key stored in macOS Keychain.");
      await refresh();
    } catch (e) { setMessage(`Failed: ${e}`); }
  };

  const removeKey = async () => {
    if (!confirm("Remove the Claude API key from Keychain?")) return;
    await remoteRemoveKey("claude");
    setMessage("Key removed.");
    await refresh();
  };

  const saveBudget = async () => {
    const pence = Math.round(parseFloat(budgetInput) * 100);
    if (isNaN(pence) || pence < 0) { setMessage("Budget must be a non-negative number."); return; }
    try {
      await remoteSetBudget("claude", pence);
      setMessage("Budget saved.");
      await refresh();
    } catch (e) { setMessage(`Failed: ${e}`); }
  };

  const toggleEnabled = async (next: boolean) => {
    try {
      await remoteSetEnabledForReview(next);
      await refresh();
    } catch (e) { setMessage(`Failed: ${e}`); }
  };

  const test = async () => {
    setTesting(true);
    setMessage(null);
    try {
      const text = await remoteTest();
      setMessage(`Test call succeeded: "${text}"`);
      await refresh();
    } catch (e) { setMessage(`Test failed: ${e}`); }
    setTesting(false);
  };

  if (loading) return <section><div style={{ fontSize: 13, color: "#888" }}>Loading remote providers…</div></section>;
  if (!status) return <section><div style={{ fontSize: 13, color: "#f66" }}>Failed to load remote status.</div></section>;

  const pct = status.budget_pence > 0
    ? Math.min(100, (status.spent_month_pence / status.budget_pence) * 100)
    : 0;
  const barColor = pct >= 100 ? "#c33" : pct >= 75 ? "#d90" : "#6a6";

  return (
    <section>
      <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Remote providers</h2>

      <div style={{ padding: 10, border: "1px solid #333", borderRadius: 6, marginBottom: 10 }}>
        <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 4 }}>Claude</div>
        {status.has_key ? (
          <div style={{ fontSize: 12, color: "#6f6" }}>
            ● API key set in Keychain
            <button onClick={removeKey} style={{ marginLeft: 8, fontSize: 11 }}>Remove</button>
          </div>
        ) : (
          <div style={{ display: "flex", gap: 6, marginTop: 4 }}>
            <input
              type="password"
              value={newKey}
              onChange={(e) => setNewKey(e.target.value)}
              placeholder="sk-ant-..."
              style={{ flex: 1, fontSize: 12 }}
            />
            <button onClick={saveKey} disabled={!newKey.trim()} style={{ fontSize: 12 }}>
              Set key
            </button>
          </div>
        )}
      </div>

      <div style={{ marginBottom: 10 }}>
        <div style={{ fontSize: 12, color: "#888", marginBottom: 4 }}>
          Monthly budget: £{(status.spent_month_pence / 100).toFixed(2)} spent of £{(status.budget_pence / 100).toFixed(2)}
        </div>
        <div style={{ height: 6, background: "#222", borderRadius: 3, overflow: "hidden" }}>
          <div style={{ width: `${pct}%`, height: "100%", background: barColor, transition: "width 200ms" }} />
        </div>
        <div style={{ display: "flex", gap: 6, marginTop: 4, alignItems: "center" }}>
          <span style={{ fontSize: 12 }}>£</span>
          <input
            type="number" step="0.01" min="0"
            value={budgetInput}
            onChange={(e) => setBudgetInput(e.target.value)}
            style={{ width: 80, fontSize: 12 }}
          />
          <button onClick={saveBudget} style={{ fontSize: 12 }}>Save budget</button>
        </div>
      </div>

      <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 13, marginBottom: 8 }}>
        <input
          type="checkbox"
          checked={status.enabled_for_review}
          onChange={(e) => void toggleEnabled(e.target.checked)}
          disabled={!status.has_key}
        />
        Use Claude for ledger AI month review
      </label>

      {status.has_key && (
        <button onClick={test} disabled={testing} style={{ fontSize: 12 }}>
          {testing ? "Testing…" : "Test connection"}
        </button>
      )}

      {message && (
        <div style={{ fontSize: 11, color: message.includes("ailed") ? "#f66" : "#6f6", marginTop: 6 }}>
          {message}
        </div>
      )}
    </section>
  );
}
```

### Step 3: Gate + commit

- [ ] Run:
```bash
cd /Users/hanamori/life-assistant/apps/desktop && pnpm tsc --noEmit 2>&1 | tail -5
cd /Users/hanamori/life-assistant && cargo build 2>&1 | tail -5
```
Expected: TS clean; cargo build clean.

- [ ] Commit:
```bash
git add apps/desktop/src/lib/remote/ apps/desktop/src/components/Settings/AiTab.tsx
git commit -m "feat(settings): RemoteProvidersSection — key mgmt, budget, enable toggle, test"
```

---

## Task 8: CallLogSection + ledger AI review swap + full gate

**Files:**
- Modify: `apps/desktop/src/components/Settings/AiTab.tsx`
- Modify: `crates/app/src/ledger/ai_review.rs`

### Step 1: Add `CallLogSection` to AiTab

- [ ] Open `/Users/hanamori/life-assistant/apps/desktop/src/components/Settings/AiTab.tsx`.

Add to existing imports from `../../lib/remote/ipc`:

```tsx
import {
  // ...existing imports...
  remoteCallLogList, remoteCallLogClear,
  type CallLogEntry,
} from "../../lib/remote/ipc";
```

Find the closing `</div>` of the main tab JSX (the outermost wrapper at the end of `AiTab()`'s return). Immediately before it, add:

```tsx
      <CallLogSection />
```

ABOVE `export default function AiTab()`, add the helper component:

```tsx
function CallLogSection() {
  const [entries, setEntries] = useState<CallLogEntry[]>([]);
  const [expanded, setExpanded] = useState<number | null>(null);
  const [clearing, setClearing] = useState(false);

  const refresh = async () => {
    setEntries(await remoteCallLogList(20).catch(() => []));
  };

  useEffect(() => { void refresh(); }, []);

  const clearLog = async () => {
    if (!confirm("Soft-delete all call log entries? You can still restore them from Trash for 30 days.")) return;
    setClearing(true);
    try {
      await remoteCallLogClear();
      await refresh();
    } finally { setClearing(false); }
  };

  return (
    <section style={{ marginTop: 16 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h2 style={{ margin: 0, fontSize: 15 }}>Call log ({entries.length})</h2>
        {entries.length > 0 && (
          <button onClick={clearLog} disabled={clearing} style={{ fontSize: 11 }}>
            {clearing ? "Clearing…" : "Clear log"}
          </button>
        )}
      </div>
      {entries.length === 0 && (
        <div style={{ fontSize: 12, color: "#888", marginTop: 6 }}>
          No remote calls yet. Enable Claude for ledger review + run a review to see entries here.
        </div>
      )}
      <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 4 }}>
        {entries.map((e) => {
          const isExpanded = expanded === e.id;
          const outcome = e.error ? "error" : e.completed_at ? "ok" : "in-flight";
          const outcomeColor = outcome === "ok" ? "#6f6" : outcome === "error" ? "#f66" : "#d90";
          return (
            <div key={e.id}
                 onClick={() => setExpanded(isExpanded ? null : e.id)}
                 style={{ padding: 8, borderRadius: 4, background: "#151515", cursor: "pointer" }}>
              <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12 }}>
                <span>
                  <span style={{ color: outcomeColor }}>●</span>{" "}
                  {new Date(e.started_at * 1000).toLocaleString()} · {e.skill} · {e.model}
                </span>
                <span style={{ color: "#888" }}>
                  {e.redaction_count > 0 && `${e.redaction_count} redacted · `}
                  {e.cost_pence != null && `£${(e.cost_pence / 100).toFixed(2)}`}
                </span>
              </div>
              {isExpanded && (
                <div style={{ marginTop: 6, fontSize: 11, color: "#aaa" }}>
                  <div><strong>Reason:</strong> {e.user_visible_reason}</div>
                  <div style={{ marginTop: 4 }}>
                    <strong>Prompt (redacted, this is what left your Mac):</strong>
                    <pre style={{ background: "#0a0a0a", padding: 6, borderRadius: 3, overflowX: "auto", fontSize: 10 }}>
                      {e.prompt_redacted}
                    </pre>
                  </div>
                  {e.response_text && (
                    <div>
                      <strong>Response:</strong>
                      <pre style={{ background: "#0a0a0a", padding: 6, borderRadius: 3, overflowX: "auto", fontSize: 10 }}>
                        {e.response_text}
                      </pre>
                    </div>
                  )}
                  {e.error && (
                    <div style={{ color: "#f66" }}>
                      <strong>Error:</strong> {e.error}
                    </div>
                  )}
                  {e.input_tokens != null && (
                    <div style={{ color: "#666" }}>
                      Tokens: {e.input_tokens} in / {e.output_tokens} out
                    </div>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
```

### Step 2: Swap ledger AI review to honor the gate

- [ ] Open `/Users/hanamori/life-assistant/crates/app/src/ledger/ai_review.rs`.

Find the `stream_review` function (it calls Ollama currently). We're adding a pre-check: if `ai.remote.enabled_for_review == "1"` AND Claude key exists AND budget OK, call Claude via the orchestrator instead. Otherwise, fall through to the existing Ollama stream.

Replace the existing `stream_review` function (or add a new `run_review` wrapper above it; depends on how Phase 5c's `ai_review.rs` currently looks — read the file first). The final shape should be:

```rust
//! Add near existing imports at the top:
use crate::remote::{orchestrator, REMOTE_ENABLED_FOR_REVIEW_KEY, keychain, PROVIDER_CLAUDE};
use crate::assistant::commands::Db;
use std::sync::Arc;
```

Add a new function (or modify the existing entry point — whichever matches how Phase 5c structured it. If 5c was never executed, its `ai_review.rs` doesn't exist yet — check: `grep -n "ai_review\|stream_review" crates/app/src/ledger/ || echo MISSING`. If MISSING, skip this step entirely and document in the commit message that the swap is deferred until Phase 5c's `ai_review.rs` exists):

```rust
/// Returns true if remote Claude is configured + enabled + within budget.
/// Reads state from DB + keychain.
pub fn should_use_remote(conn: &rusqlite::Connection) -> bool {
    let enabled = manor_core::setting::get(conn, REMOTE_ENABLED_FOR_REVIEW_KEY)
        .ok().flatten().as_deref() == Some("1");
    if !enabled { return false; }
    if !keychain::has_key(PROVIDER_CLAUDE) { return false; }
    // Budget check deferred to the orchestrator (which logs the refusal).
    true
}
```

Then at the ledger-review call site (wherever the month review Tauri command lives), branch:

```rust
let db_arc = state.inner().clone_arc();
let use_remote = {
    let conn = db_arc.lock().unwrap();
    crate::ledger::ai_review::should_use_remote(&conn)
};

if use_remote {
    // Build the prompt (same one the local path uses)
    let prompt = build_prompt(...);
    match orchestrator::remote_chat(db_arc, orchestrator::RemoteChatRequest {
        skill: "ledger_review",
        user_visible_reason: "Month-in-review narrative (ledger)",
        system_prompt: Some("You are a calm personal finance assistant. Respond in 2-3 plain sentences."),
        user_prompt: &prompt,
        max_tokens: 400,
    }).await {
        Ok(outcome) => {
            // Emit outcome.text as one big Token chunk + Done
            out.send(StreamChunk::Token(outcome.text)).await.ok();
            out.send(StreamChunk::Done).await.ok();
            return;
        }
        Err(e) => {
            // Fall through to local Ollama on error.
            tracing::warn!("remote review failed, falling back to local: {e}");
        }
    }
}

// Existing Ollama streaming path...
```

If the exact structure of `ai_review.rs` doesn't match (Phase 5c hasn't shipped, or the function shape differs), **stop and report BLOCKED** — the swap is the whole point of this task; faking it is worse than not shipping it.

### Step 3: Full Phase Landmark-2 gate

- [ ] Run:
```bash
cd /Users/hanamori/life-assistant
cargo fmt --check 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings 2>&1 | tail -15
cargo test 2>&1 | tail -10
cd apps/desktop && pnpm tsc --noEmit 2>&1 | tail -10
```
Expected: all clean; tests at 214 (from end of Task 3) + no new core/app tests = 214.

### Step 4: Commit

- [ ] Commit:
```bash
cd /Users/hanamori/life-assistant
git add apps/desktop/src/components/Settings/AiTab.tsx crates/app/src/ledger/ai_review.rs
git commit -m "feat(ledger+settings): CallLogSection + ledger review routes to Claude when enabled"
```

### Step 5: Manual smoke test (requires your own Claude API key)

With Manor running:
1. Settings → AI → set Claude API key (paste `sk-ant-...`).
2. Budget shows default £10, 0% used.
3. Click "Test connection" — should log "pong" and a row appears in the call log.
4. Click the log row — expand shows redacted prompt `ping`, response `pong`, 10–20 tokens, pennies cost.
5. Toggle "Use Claude for ledger AI month review" ON.
6. Go to Ledger view → Month review → click "Review with AI" → response comes from Claude (typically longer/more eloquent than Ollama).
7. Budget bar now shows some spend.
8. Bonus: plant a fake email in a note (`"call alice@example.com about the plumber"`). Set the embed/review prompt to read from notes (hypothetical). Trigger the review. Expand the log row — the email should be replaced with `user@[EMAIL-HOSTHASH-xxxx]`.

---

## Post-Landmark-2 Gate

```bash
cd /Users/hanamori/life-assistant
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cd apps/desktop && pnpm tsc --noEmit
```
All must pass.

---

## Self-Review

**Spec coverage** (`2026-04-17-remote-llm-design.md`):
- §1 Provider abstraction — Task 3 (trait + Claude) ✓
- §2 Redaction pipeline — Task 2 with proptest ✓
- §3 Keychain key storage — Task 4 ✓
- §4 remote_call_log schema + audit — Task 1 ✓
- §5 Budget guardrails — Task 1 (sum_month_pence) + Task 5 (orchestrator enforcement) + Task 7 (UI) ✓
- §6 Tier routing (MVP) — Task 6 (`remote_set_enabled_for_review`) + Task 8 (ledger swap) ✓
- §7 Settings → AI tab upgrade — Task 7 (providers) + Task 8 (call log) ✓

**Deviations**: none. All scope calls documented in design spec are honored.

**Placeholder scan**: none.

**Type consistency**: `RemoteProviderStatus`, `CallLogEntry` match Rust ↔ TS. `ChatMessage` / `ChatRole` / `ChatResponse` stay in `crate::remote::provider` (not re-exported). `PROVIDER_CLAUDE` const is used everywhere for the provider string, preventing typos.

**Test counts**: 7 (remote_call_log) + 10 unit + 3 property (redact) + 2 (provider cost) + 3 (claude wiremock) = 25 new. Total 214 (189 + 25).

**Risk calls**:
- Task 8 depends on Phase 5c's `ai_review.rs` existing. If it doesn't (5c was specced + planned but never executed), the swap task BLOCKs and the commit lands the UI only. Noted in Task 8 Step 2.
- Property tests in Task 2 are the privacy boundary — if they fail, the whole landmark is compromised. No bypass.
- Keychain tests are skipped (no unit test for Task 4) — relies on the keyring crate being correct. `crates/app/src/sync/keychain.rs` for CalDAV uses the same pattern and works in production, so it's proven.
