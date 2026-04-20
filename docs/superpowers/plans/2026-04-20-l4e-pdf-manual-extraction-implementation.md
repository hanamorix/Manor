# L4e PDF Manual Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the fifth and final v0.5 Bones slice — PDF manual extraction. Hana attaches a PDF to an asset, clicks "Extract maintenance schedules," reviews a list of LLM-proposed schedules (approve/edit/reject), and approved ones land as real `maintenance_schedule` rows.

**Architecture:** Two-crate split mirrors L4a/L4b/L4c/L4d. Core holds the `pdf-extract`-backed text extractor + LLM prompt/JSON-parse + the new proposal-kind handlers. App layer holds the `LlmClient` impls (Ollama via `chat_collect`, Claude via `remote::orchestrator::remote_chat`) + the pipeline orchestrator + Tauri commands. Reuses the existing `proposal` table — no schema migration. Frontend ships a Zustand store, a `PendingProposalsBlock` component on AssetDetail, Extract/Re-extract buttons on PDF attachment rows, and a proposal-edit mode on L4b's existing `ScheduleDrawer`.

**Tech Stack:** Rust (`pdf-extract` crate — new dep, `rusqlite`, `reqwest`, `async-trait`, `thiserror`, `serde_json`), React + TypeScript + Zustand, reuses `react-markdown` from L4d.

**Spec:** `docs/superpowers/specs/2026-04-20-l4e-pdf-manual-extraction-design.md`

---

## File structure

### New Rust files
- `crates/core/src/pdf_extract/mod.rs` — module root + types (`ExtractedSchedule`, `ExtractError`).
- `crates/core/src/pdf_extract/text.rs` — `pdf-extract` wrapper + size/image-only guards + `cap_for_tier`.
- `crates/core/src/pdf_extract/llm.rs` — prompt builder + JSON-array parse.
- `crates/app/src/pdf_extract/mod.rs` — module root.
- `crates/app/src/pdf_extract/ollama_client.rs` — `LlmClient` impl.
- `crates/app/src/pdf_extract/claude_client.rs` — `LlmClient` impl with `remote_call_log_id` capture.
- `crates/app/src/pdf_extract/pipeline.rs` — `extract_and_propose` + test-seam entry point.
- `crates/app/src/pdf_extract/commands.rs` — 6 Tauri commands.

### New frontend files
- `apps/desktop/src/lib/pdf_extract/ipc.ts` — 6 IPC wrappers.
- `apps/desktop/src/lib/pdf_extract/state.ts` — Zustand store.
- `apps/desktop/src/components/Bones/PendingProposalsBlock.tsx` — per-asset proposals list.
- `apps/desktop/src/components/Bones/__tests__/PendingProposalsBlock.test.tsx` — RTL tests.

### Modified Rust files
- `crates/core/Cargo.toml` — add `pdf-extract = "0.7"`.
- `crates/core/src/lib.rs` — `pub mod pdf_extract;`.
- `crates/core/src/assistant/proposal.rs` — add `AddMaintenanceScheduleArgs` struct + two new public functions (`approve_add_maintenance_schedule`, `approve_add_maintenance_schedule_with_override`). Leave existing `approve_add_task` unchanged.
- `crates/app/src/lib.rs` — `pub mod pdf_extract;` + register 6 Tauri commands.

### Modified frontend files
- `apps/desktop/src/components/Bones/AssetDetail.tsx` — mount `<PendingProposalsBlock assetId={id} />` between `<MaintenanceSection />` and `<HistoryBlock />`. Add Extract / Re-extract buttons to attachment rows where `mime_type === 'application/pdf'`.
- `apps/desktop/src/components/Bones/DueSoon/ScheduleDrawer.tsx` — add optional `proposalId?: number` prop; Save routes through `approveWithOverride` + relabels to "Approve & add"; Delete hidden in proposal-edit mode.

### Notes
- `manor_core::recipe::import::extract_json_array_block_public` is ALREADY public (lines 257+ of `recipe/import.rs`) — no promotion step required. Plan originally assumed a promotion; confirmed unnecessary during task-1 prep.
- `manor_core::attachment::file_path(root, uuid)` returns `root.join(uuid)` — attachments are stored on disk WITHOUT an extension. The pipeline uses this helper; do NOT hard-code `format!("{}.pdf", uuid)`.

---

## Phase A — Core types, text extraction, LLM parse, proposal apply

### Task 1: Scaffold + pdf-extract dep + types + text module

**Files:**
- Modify: `crates/core/Cargo.toml`
- Create: `crates/core/src/pdf_extract/mod.rs`
- Create: `crates/core/src/pdf_extract/text.rs`
- Modify: `crates/core/src/lib.rs` — register `pub mod pdf_extract;`.

- [ ] **Step 1: Add `pdf-extract` to core's Cargo.toml**

Open `crates/core/Cargo.toml`. Under `[dependencies]`, add:

```toml
pdf-extract = "0.7"
```

Run `cargo check --package manor-core` to verify it resolves + compiles. If the 0.7 API has changed at implementation time, adjust the version to the latest stable and verify the call site in `text.rs` still uses `extract_text_from_mem(&[u8]) -> Result<String, pdf_extract::OutputError>`.

- [ ] **Step 2: Create `crates/core/src/pdf_extract/mod.rs`**

```rust
//! PDF manual extraction (L4e).

pub mod llm;
pub mod text;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("PDF too large to extract (over {0} MB)")]
    TooLarge(u64),
    #[error("PDF appears to be an image scan — text extraction isn't possible")]
    ImageOnly,
    #[error("couldn't read PDF file: {0}")]
    ReadFailed(String),
    #[error("couldn't parse PDF: {0}")]
    ParseFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedSchedule {
    pub task: String,
    pub interval_months: i32,
    pub notes: String,
    pub rationale: String,
}
```

Note: `pub mod llm;` is declared but the file won't exist yet. Create a placeholder `llm.rs` so `cargo check` passes:

```bash
# Minimal placeholder (replaced in Task 3):
cat > crates/core/src/pdf_extract/llm.rs <<'EOF'
//! Repair-note LLM extraction (L4e). Full implementation in Task 3.
EOF
```

- [ ] **Step 3: Create `crates/core/src/pdf_extract/text.rs`**

```rust
//! PDF text extraction + tier-based capping (L4e).

use super::ExtractError;
use std::path::Path;

pub const MAX_PDF_BYTES: u64 = 10 * 1024 * 1024; // 10 MB
pub const MIN_TEXT_CHARS: usize = 500;
pub const OLLAMA_CAP_BYTES: usize = 32 * 1024;
pub const CLAUDE_CAP_BYTES: usize = 200 * 1024;

pub fn extract_text_from_pdf(path: &Path) -> Result<String, ExtractError> {
    let meta = std::fs::metadata(path)
        .map_err(|e| ExtractError::ReadFailed(e.to_string()))?;
    if meta.len() > MAX_PDF_BYTES {
        return Err(ExtractError::TooLarge(MAX_PDF_BYTES / (1024 * 1024)));
    }
    let bytes = std::fs::read(path)
        .map_err(|e| ExtractError::ReadFailed(e.to_string()))?;
    let text = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| ExtractError::ParseFailed(e.to_string()))?;
    let trimmed = text.trim();
    if trimmed.chars().count() < MIN_TEXT_CHARS {
        return Err(ExtractError::ImageOnly);
    }
    Ok(trimmed.to_string())
}

pub fn cap_for_tier(text: &str, tier_is_claude: bool) -> String {
    let cap = if tier_is_claude { CLAUDE_CAP_BYTES } else { OLLAMA_CAP_BYTES };
    if text.len() <= cap {
        return text.to_string();
    }
    let mut cut = cap;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text[..cut].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_rejects_oversize_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.pdf");
        // Write a file larger than MAX_PDF_BYTES. Content doesn't need to be valid PDF —
        // the size check fires before pdf-extract parses.
        let bytes = vec![0u8; (MAX_PDF_BYTES + 1) as usize];
        std::fs::write(&path, bytes).unwrap();
        let err = extract_text_from_pdf(&path).unwrap_err();
        matches!(err, ExtractError::TooLarge(10));
    }

    #[test]
    fn extract_rejects_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.pdf");
        let err = extract_text_from_pdf(&path).unwrap_err();
        matches!(err, ExtractError::ReadFailed(_));
    }

    #[test]
    fn cap_for_tier_ollama_caps_at_32kb() {
        let big = "x".repeat(100 * 1024);
        let out = cap_for_tier(&big, false);
        assert!(out.len() <= OLLAMA_CAP_BYTES);
    }

    #[test]
    fn cap_for_tier_claude_caps_at_200kb() {
        let big = "x".repeat(300 * 1024);
        let out = cap_for_tier(&big, true);
        assert!(out.len() <= CLAUDE_CAP_BYTES);
    }

    #[test]
    fn cap_for_tier_returns_whole_when_under_cap() {
        let small = "small text";
        let out = cap_for_tier(small, false);
        assert_eq!(out, small);
    }

    #[test]
    fn cap_for_tier_respects_char_boundary() {
        // Multibyte UTF-8 char right at the byte boundary. Build a string just over 32KB
        // that has a multibyte code point near the cut point.
        let prefix = "a".repeat(OLLAMA_CAP_BYTES - 2);
        let input = format!("{}€€€", prefix); // € is 3 bytes in UTF-8
        let out = cap_for_tier(&input, false);
        // Must not panic. Must be valid UTF-8 (implicit — String guarantees it).
        assert!(out.len() <= OLLAMA_CAP_BYTES);
    }
}
```

- [ ] **Step 4: Register the module in `crates/core/src/lib.rs`**

Find the existing `pub mod maintenance;` line. Add nearby (alphabetically after `note` or wherever the module-declaration block fits):

```rust
pub mod pdf_extract;
```

- [ ] **Step 5: Run tests**

```
cargo test --package manor-core pdf_extract::text
```
Expected: 6 PASS (`extract_rejects_oversize_file`, `extract_rejects_missing_file`, `cap_for_tier_ollama_caps_at_32kb`, `cap_for_tier_claude_caps_at_200kb`, `cap_for_tier_returns_whole_when_under_cap`, `cap_for_tier_respects_char_boundary`).

Note: happy-path `extract_text_from_pdf` on a real text PDF + `ImageOnly` detection on a real image-only PDF are covered by MANUAL QA (see Task 12's scenario). `pdf-extract`'s own tests cover text extraction fidelity — our code's job is only error-wrapping + guards, and the 6 unit tests above cover those.

Also run:
```
cargo test --package manor-core
cargo clippy --package manor-core -- -D warnings
```
Expected: full crate green. Clippy clean.

- [ ] **Step 6: Commit**

```bash
git add crates/core/Cargo.toml \
        crates/core/src/pdf_extract/mod.rs \
        crates/core/src/pdf_extract/text.rs \
        crates/core/src/pdf_extract/llm.rs \
        crates/core/src/lib.rs
git commit -m "feat(pdf_extract): scaffold module + text extraction + tier caps (L4e)"
```

---

### Task 2: LLM prompt + JSON array parse

**Files:**
- Modify: `crates/core/src/pdf_extract/llm.rs` (replace placeholder).

- [ ] **Step 1: Replace `crates/core/src/pdf_extract/llm.rs` with full implementation**

```rust
//! LLM-based schedule extraction (L4e).

use super::ExtractedSchedule;
use crate::recipe::import::{extract_json_array_block_public, LlmClient};
use anyhow::Result;

const PROMPT_PREFIX: &str = "\
You extract structured maintenance-schedule data from an appliance, vehicle, or \
fixture owner's manual. Output a JSON array with zero or more schedule objects:

[
  {
    \"task\": str,
    \"interval_months\": int (1..240),
    \"notes\": str,
    \"rationale\": str
  }
]

Requirements:
- task: a short imperative label like \"Annual service\" or \"Replace water filter\".
- interval_months: integer. Convert \"every 6 months\" to 6, \"yearly\" to 12, \
\"every 2 years\" to 24, etc. Ignore conditional intervals (e.g. \"when light blinks\").
- notes: short extra context from the manual, or empty string.
- rationale: one sentence citing where in the manual this came from \
(e.g. \"Section 7.2 recommends annual service.\").
- If no maintenance schedules are listed, output [].
- Output ONLY the JSON array. No prose before or after.

Manual text:
";

#[derive(serde::Deserialize)]
struct LlmSchedule {
    task: String,
    interval_months: i32,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    rationale: String,
}

pub async fn extract_schedules_via_llm(
    manual_text: &str,
    client: &dyn LlmClient,
) -> Result<Vec<ExtractedSchedule>> {
    let prompt = format!("{}{}", PROMPT_PREFIX, manual_text);
    let first = client.complete(&prompt).await?;

    let parsed: Result<Vec<LlmSchedule>, _> = extract_json_array_block_public(&first);
    let items = match parsed {
        Ok(v) => v,
        Err(_) => {
            let retry = format!(
                "{}\n\n(Previous response was not valid JSON. Output ONLY the JSON array.)",
                prompt
            );
            let second = client.complete(&retry).await?;
            extract_json_array_block_public::<Vec<LlmSchedule>>(&second)
                .map_err(|e| anyhow::anyhow!("failed to parse LLM JSON after retry: {}", e))?
        }
    };

    Ok(items
        .into_iter()
        .filter(|s| {
            s.interval_months >= 1
                && s.interval_months <= 240
                && !s.task.trim().is_empty()
        })
        .map(|s| ExtractedSchedule {
            task: s.task,
            interval_months: s.interval_months,
            notes: s.notes,
            rationale: s.rationale,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Queue-based stub: each `complete()` call dequeues the next response.
    struct StubLlmClient {
        responses: Mutex<Vec<Result<String>>>,
    }

    impl StubLlmClient {
        fn with(responses: Vec<Result<String>>) -> Self {
            Self { responses: Mutex::new(responses) }
        }
    }

    #[async_trait]
    impl LlmClient for StubLlmClient {
        async fn complete(&self, _prompt: &str) -> Result<String> {
            let mut q = self.responses.lock().unwrap();
            if q.is_empty() {
                return Err(anyhow::anyhow!("stub exhausted"));
            }
            match q.remove(0) {
                Ok(s) => Ok(s),
                Err(e) => Err(e),
            }
        }
    }

    #[tokio::test]
    async fn extract_schedules_parses_valid_array() {
        let client = StubLlmClient::with(vec![Ok(r#"[
            {"task":"Annual service","interval_months":12,"notes":"","rationale":"Section 7.2."}
        ]"#.to_string())]);
        let out = extract_schedules_via_llm("manual text", &client).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].task, "Annual service");
        assert_eq!(out[0].interval_months, 12);
        assert_eq!(out[0].rationale, "Section 7.2.");
    }

    #[tokio::test]
    async fn extract_schedules_retries_on_bad_json_then_succeeds() {
        let client = StubLlmClient::with(vec![
            Ok("Here's your JSON: [broken".to_string()),
            Ok(r#"[{"task":"Retry","interval_months":6,"notes":"","rationale":""}]"#.to_string()),
        ]);
        let out = extract_schedules_via_llm("manual", &client).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].task, "Retry");
    }

    #[tokio::test]
    async fn extract_schedules_returns_empty_on_empty_array() {
        let client = StubLlmClient::with(vec![Ok("[]".to_string())]);
        let out = extract_schedules_via_llm("manual", &client).await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn extract_schedules_filters_invalid_intervals() {
        let client = StubLlmClient::with(vec![Ok(r#"[
            {"task":"Zero interval","interval_months":0,"notes":"","rationale":""},
            {"task":"Oversized","interval_months":300,"notes":"","rationale":""},
            {"task":"Valid","interval_months":12,"notes":"","rationale":""}
        ]"#.to_string())]);
        let out = extract_schedules_via_llm("manual", &client).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].task, "Valid");
    }

    #[tokio::test]
    async fn extract_schedules_filters_empty_tasks() {
        let client = StubLlmClient::with(vec![Ok(r#"[
            {"task":"","interval_months":12,"notes":"","rationale":""},
            {"task":"  ","interval_months":6,"notes":"","rationale":""},
            {"task":"Good","interval_months":12,"notes":"","rationale":""}
        ]"#.to_string())]);
        let out = extract_schedules_via_llm("manual", &client).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].task, "Good");
    }

    #[tokio::test]
    async fn extract_schedules_errors_on_repeated_parse_failure() {
        let client = StubLlmClient::with(vec![
            Ok("not json".to_string()),
            Ok("still not json".to_string()),
        ]);
        let err = extract_schedules_via_llm("manual", &client).await.unwrap_err();
        assert!(err.to_string().contains("failed to parse LLM JSON after retry"));
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test --package manor-core pdf_extract::llm
```
Expected: 6 PASS.

Also:
```
cargo test --package manor-core
cargo clippy --package manor-core -- -D warnings
```
Full suite green, clippy clean.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/pdf_extract/llm.rs
git commit -m "feat(pdf_extract): LLM prompt + JSON array parse (L4e)"
```

---

### Task 3: Proposal apply extensions for add_maintenance_schedule

**Files:**
- Modify: `crates/core/src/assistant/proposal.rs`.

### Context

The existing `crates/core/src/assistant/proposal.rs` has:
- `pub fn insert(conn, new: NewProposal) -> Result<i64>` (line ~80) — generic insert, unchanged.
- `pub fn list(conn, status: Option<&str>) -> Result<Vec<Proposal>>` — generic list.
- `pub fn approve_add_task(conn: &mut Connection, id, today_iso) -> Result<Vec<task::Task>>` (line ~134) — kind-specific, unchanged.
- `pub fn reject(conn, id) -> Result<()>` (line ~176).

This task adds two NEW functions (does NOT modify `approve_add_task` — proposals are handled per-kind, not via a generic `apply` dispatch):

1. `approve_add_maintenance_schedule(conn, id) -> Result<String>` — applies verbatim, returns new schedule id.
2. `approve_add_maintenance_schedule_with_override(conn, id, edited) -> Result<String>` — applies with caller-supplied edited draft.

Plus a new public struct `AddMaintenanceScheduleArgs`.

- [ ] **Step 1: Add the struct + two new functions to `proposal.rs`**

Append near the existing `AddTaskArgs` (around line 55-75 area):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMaintenanceScheduleArgs {
    pub asset_id: String,
    pub task: String,
    pub interval_months: i32,
    pub notes: String,
    pub source_attachment_uuid: String,
    pub tier: String,
}
```

Append after the existing `reject` function:

```rust
/// Apply a pending `add_maintenance_schedule` proposal verbatim.
/// Inserts the schedule + marks the proposal `applied`. Returns the inserted schedule's id.
pub fn approve_add_maintenance_schedule(conn: &mut Connection, id: i64) -> Result<String> {
    let tx = conn.transaction()?;

    let row: Option<(String, String, String)> = tx
        .query_row(
            "SELECT kind, status, diff FROM proposal WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (kind, status, diff) = match row {
        Some(r) => r,
        None => bail!("proposal {id} not found"),
    };
    if status != "pending" {
        bail!("proposal {id} is not pending (status={status})");
    }
    if kind != "add_maintenance_schedule" {
        bail!("proposal {id} has unsupported kind: {kind}");
    }

    let args: AddMaintenanceScheduleArgs = serde_json::from_str(&diff)?;
    let draft = crate::maintenance::MaintenanceScheduleDraft {
        asset_id: args.asset_id,
        task: args.task,
        interval_months: args.interval_months,
        last_done_date: None,
        notes: args.notes,
    };
    let schedule_id = crate::maintenance::dal::insert_schedule(&tx, &draft)?;

    tx.execute(
        "UPDATE proposal SET status = 'applied', applied_at = ?1 WHERE id = ?2",
        params![Utc::now().timestamp(), id],
    )?;

    tx.commit()?;
    Ok(schedule_id)
}

/// Apply a pending `add_maintenance_schedule` proposal using caller-supplied
/// `edited` fields (overrides the diff's values). Used by ScheduleDrawer in
/// proposal-edit mode. Returns the inserted schedule's id.
pub fn approve_add_maintenance_schedule_with_override(
    conn: &mut Connection,
    id: i64,
    edited: &crate::maintenance::MaintenanceScheduleDraft,
) -> Result<String> {
    let tx = conn.transaction()?;

    let row: Option<(String, String)> = tx
        .query_row(
            "SELECT kind, status FROM proposal WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (kind, status) = match row {
        Some(r) => r,
        None => bail!("proposal {id} not found"),
    };
    if kind != "add_maintenance_schedule" {
        bail!("proposal {id} is not an add_maintenance_schedule");
    }
    if status != "pending" {
        bail!("proposal {id} is not pending (status={status})");
    }

    let schedule_id = crate::maintenance::dal::insert_schedule(&tx, edited)?;

    tx.execute(
        "UPDATE proposal SET status = 'applied', applied_at = ?1 WHERE id = ?2",
        params![Utc::now().timestamp(), id],
    )?;

    tx.commit()?;
    Ok(schedule_id)
}
```

- [ ] **Step 2: Add tests**

Append inside the existing `#[cfg(test)] mod tests { ... }` block:

```rust
fn insert_test_asset(conn: &Connection) -> String {
    use crate::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    asset_dal::insert_asset(conn, &AssetDraft {
        name: "Boiler".into(),
        category: AssetCategory::Appliance,
        make: None, model: None, serial_number: None, purchase_date: None,
        notes: String::new(),
        hero_attachment_uuid: None,
    }).unwrap()
}

fn insert_pending_schedule_proposal(
    conn: &Connection,
    asset_id: &str,
    task: &str,
    interval_months: i32,
    source_attachment_uuid: &str,
) -> i64 {
    let args = AddMaintenanceScheduleArgs {
        asset_id: asset_id.into(),
        task: task.into(),
        interval_months,
        notes: String::new(),
        source_attachment_uuid: source_attachment_uuid.into(),
        tier: "ollama".into(),
    };
    let diff = serde_json::to_string(&args).unwrap();
    insert(conn, NewProposal {
        kind: "add_maintenance_schedule",
        rationale: "test",
        diff_json: &diff,
        skill: "pdf_extract",
    }).unwrap()
}

#[test]
fn approve_add_maintenance_schedule_inserts_and_marks_applied() {
    use crate::assistant::db;
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::init(&dir.path().join("t.db")).unwrap();
    let asset_id = insert_test_asset(&conn);
    let pid = insert_pending_schedule_proposal(
        &conn, &asset_id, "Annual service", 12, "att-uuid-1",
    );

    let schedule_id = approve_add_maintenance_schedule(&mut conn, pid).unwrap();
    assert!(!schedule_id.is_empty());

    // schedule exists
    let s = crate::maintenance::dal::get_schedule(&conn, &schedule_id).unwrap().unwrap();
    assert_eq!(s.task, "Annual service");
    assert_eq!(s.interval_months, 12);

    // proposal applied
    let (status, applied_at): (String, Option<i64>) = conn.query_row(
        "SELECT status, applied_at FROM proposal WHERE id = ?1",
        [pid],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();
    assert_eq!(status, "applied");
    assert!(applied_at.is_some());
}

#[test]
fn approve_add_maintenance_schedule_fails_on_non_pending() {
    use crate::assistant::db;
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::init(&dir.path().join("t.db")).unwrap();
    let asset_id = insert_test_asset(&conn);
    let pid = insert_pending_schedule_proposal(
        &conn, &asset_id, "Service", 12, "att-uuid",
    );
    // Apply once (succeeds)
    approve_add_maintenance_schedule(&mut conn, pid).unwrap();
    // Apply again should error with "not pending"
    let err = approve_add_maintenance_schedule(&mut conn, pid).unwrap_err().to_string();
    assert!(err.contains("not pending"), "got: {}", err);
}

#[test]
fn approve_add_maintenance_schedule_fails_on_wrong_kind() {
    use crate::assistant::db;
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::init(&dir.path().join("t.db")).unwrap();
    // Insert an add_task proposal and try to apply it as a schedule — should error
    let pid = insert(&conn, NewProposal {
        kind: "add_task",
        rationale: "r",
        diff_json: r#"{"title":"t","due_date":null}"#,
        skill: "test",
    }).unwrap();
    let err = approve_add_maintenance_schedule(&mut conn, pid).unwrap_err().to_string();
    assert!(err.contains("unsupported kind"), "got: {}", err);
}

#[test]
fn approve_with_override_uses_edited_fields() {
    use crate::assistant::db;
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::init(&dir.path().join("t.db")).unwrap();
    let asset_id = insert_test_asset(&conn);
    let pid = insert_pending_schedule_proposal(
        &conn, &asset_id, "Original", 12, "att-uuid",
    );

    let edited = crate::maintenance::MaintenanceScheduleDraft {
        asset_id: asset_id.clone(),
        task: "Edited task".into(),
        interval_months: 24,
        last_done_date: None,
        notes: "edited notes".into(),
    };
    let sched_id = approve_add_maintenance_schedule_with_override(
        &mut conn, pid, &edited,
    ).unwrap();

    let s = crate::maintenance::dal::get_schedule(&conn, &sched_id).unwrap().unwrap();
    assert_eq!(s.task, "Edited task");
    assert_eq!(s.interval_months, 24);
    assert_eq!(s.notes, "edited notes");

    let status: String = conn.query_row(
        "SELECT status FROM proposal WHERE id = ?1",
        [pid],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(status, "applied");
}

#[test]
fn approve_with_override_rejects_wrong_kind() {
    use crate::assistant::db;
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::init(&dir.path().join("t.db")).unwrap();
    let pid = insert(&conn, NewProposal {
        kind: "add_task",
        rationale: "r",
        diff_json: r#"{"title":"t","due_date":null}"#,
        skill: "test",
    }).unwrap();
    let edited = crate::maintenance::MaintenanceScheduleDraft {
        asset_id: "x".into(),
        task: "x".into(),
        interval_months: 12,
        last_done_date: None,
        notes: "".into(),
    };
    let err = approve_add_maintenance_schedule_with_override(&mut conn, pid, &edited)
        .unwrap_err().to_string();
    assert!(err.contains("not an add_maintenance_schedule"), "got: {}", err);
}

#[test]
fn approve_with_override_rejects_non_pending() {
    use crate::assistant::db;
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::init(&dir.path().join("t.db")).unwrap();
    let asset_id = insert_test_asset(&conn);
    let pid = insert_pending_schedule_proposal(
        &conn, &asset_id, "Service", 12, "att-uuid",
    );
    // Reject the proposal
    reject(&conn, pid).unwrap();
    let edited = crate::maintenance::MaintenanceScheduleDraft {
        asset_id: asset_id.clone(),
        task: "x".into(),
        interval_months: 12,
        last_done_date: None,
        notes: "".into(),
    };
    let err = approve_add_maintenance_schedule_with_override(&mut conn, pid, &edited)
        .unwrap_err().to_string();
    assert!(err.contains("not pending"), "got: {}", err);
}
```

- [ ] **Step 3: Run tests**

```
cargo test --package manor-core assistant::proposal
```
Expected: prior tests + 6 new ones all pass.

```
cargo test --package manor-core
cargo clippy --package manor-core -- -D warnings
```
Full crate green, clippy clean.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/assistant/proposal.rs
git commit -m "feat(proposal): add_maintenance_schedule apply + with-override (L4e)"
```

---

## Phase B — App-layer LlmClient impls, pipeline, commands

### Task 4: App-layer module scaffold + LlmClient impls

**Files:**
- Create: `crates/app/src/pdf_extract/mod.rs`
- Create: `crates/app/src/pdf_extract/ollama_client.rs`
- Create: `crates/app/src/pdf_extract/claude_client.rs`
- Modify: `crates/app/src/lib.rs` — register `pub mod pdf_extract;`.

- [ ] **Step 1: Create `crates/app/src/pdf_extract/mod.rs`**

```rust
//! PDF manual extraction (L4e).

pub mod claude_client;
pub mod ollama_client;
```

- [ ] **Step 2: Create `crates/app/src/pdf_extract/ollama_client.rs`**

```rust
//! Ollama LlmClient adapter for PDF extraction (L4e).

use async_trait::async_trait;
use manor_core::recipe::import::LlmClient;

pub struct OllamaExtractClient;

#[async_trait]
impl LlmClient for OllamaExtractClient {
    async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        use crate::assistant::ollama::{
            ChatMessage, ChatRole, OllamaClient, DEFAULT_ENDPOINT, DEFAULT_MODEL,
        };
        let client = OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL);
        client
            .chat_collect(&[ChatMessage {
                role: ChatRole::User,
                content: prompt.to_string(),
            }])
            .await
    }
}
```

- [ ] **Step 3: Create `crates/app/src/pdf_extract/claude_client.rs`**

```rust
//! Claude LlmClient adapter for PDF extraction (L4e).
//!
//! Routes through the shared remote::orchestrator::remote_chat (skill=pdf_extract)
//! so redaction, budget, and audit logging apply. Captures the orchestrator's
//! RemoteCallLog row id so the pipeline can stamp it on inserted proposals.

use async_trait::async_trait;
use manor_core::recipe::import::LlmClient;
use std::sync::{Arc, Mutex};

pub struct ClaudeExtractClient {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub asset_name: String,
    pub remote_call_log_id_sink: Arc<Mutex<Option<i64>>>,
}

#[async_trait]
impl LlmClient for ClaudeExtractClient {
    async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        let reason = format!("Extract schedules from {} manual", self.asset_name);
        let req = crate::remote::orchestrator::RemoteChatRequest {
            skill: "pdf_extract",
            user_visible_reason: &reason,
            system_prompt: None,
            user_prompt: prompt,
            max_tokens: 2048,
        };
        let outcome = crate::remote::orchestrator::remote_chat(self.db.clone(), req)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        *self.remote_call_log_id_sink.lock().unwrap() = Some(outcome.log_id);
        Ok(outcome.text)
    }
}
```

- [ ] **Step 4: Register in `crates/app/src/lib.rs`**

Find `pub mod repair;` (or similar module declaration from L4d) and add:

```rust
pub mod pdf_extract;
```

- [ ] **Step 5: Compile + clippy**

```
cargo build --package manor-app
cargo clippy --workspace -- -D warnings
cargo test --workspace
```
Expected: full workspace green. No tests added in this task.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/pdf_extract/ crates/app/src/lib.rs
git commit -m "feat(pdf_extract): app-layer module + Ollama/Claude LlmClient adapters (L4e)"
```

---

### Task 5: Pipeline orchestrator

**Files:**
- Create: `crates/app/src/pdf_extract/pipeline.rs`
- Modify: `crates/app/src/pdf_extract/mod.rs` — register `pub mod pipeline;`.

- [ ] **Step 1: Create `crates/app/src/pdf_extract/pipeline.rs`**

```rust
//! PDF extraction pipeline orchestrator (L4e).

use super::claude_client::ClaudeExtractClient;
use super::ollama_client::OllamaExtractClient;
use anyhow::{anyhow, Result};
use manor_core::assistant::proposal::{self, AddMaintenanceScheduleArgs, NewProposal};
use manor_core::pdf_extract::{
    llm::extract_schedules_via_llm,
    text::{cap_for_tier, extract_text_from_pdf},
};
use manor_core::recipe::import::LlmClient;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierRequest {
    Ollama,
    Claude,
}

impl TierRequest {
    fn is_claude(self) -> bool {
        matches!(self, TierRequest::Claude)
    }
    fn as_str(self) -> &'static str {
        match self {
            TierRequest::Ollama => "ollama",
            TierRequest::Claude => "claude",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractOutcome {
    pub proposals_inserted: i64,
    pub replaced_pending_count: i64,
}

pub async fn extract_and_propose(
    db: Arc<Mutex<rusqlite::Connection>>,
    attachments_dir: PathBuf,
    attachment_uuid: String,
    tier: TierRequest,
) -> Result<ExtractOutcome> {
    // 1. Resolve attachment → (asset_id, asset_name).
    let (asset_id, asset_name) = {
        let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
        let row: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT entity_type, entity_id FROM attachment
                 WHERE uuid = ?1 AND deleted_at IS NULL",
                [&attachment_uuid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        let (entity_type, entity_id) = row.ok_or_else(|| anyhow!("Attachment not found"))?;
        if entity_type.as_deref() != Some("asset") {
            return Err(anyhow!("Attachment not linked to an asset"));
        }
        let asset_id = entity_id.ok_or_else(|| anyhow!("Attachment not linked to an asset"))?;
        let asset_name: Option<String> = conn
            .query_row(
                "SELECT name FROM asset WHERE id = ?1 AND deleted_at IS NULL",
                [&asset_id],
                |r| r.get(0),
            )
            .ok();
        let asset_name = asset_name.ok_or_else(|| anyhow!("Asset not found"))?;
        (asset_id, asset_name)
    };

    // 2. Build PDF path via manor_core::attachment::file_path (stored without extension).
    let path = manor_core::attachment::file_path(&attachments_dir, &attachment_uuid);

    // 3. Extract text. ExtractError bubbles up with its Display impl.
    let text = extract_text_from_pdf(&path).map_err(|e| anyhow!(e.to_string()))?;

    // 4. Cap for tier.
    let capped = cap_for_tier(&text, tier.is_claude());

    // 5. Build LlmClient + invoke.
    let (schedules, remote_call_log_id) = match tier {
        TierRequest::Ollama => {
            let client = OllamaExtractClient;
            let schedules = extract_schedules_via_llm(&capped, &client).await?;
            (schedules, None)
        }
        TierRequest::Claude => {
            let log_sink = Arc::new(Mutex::new(None));
            let client = ClaudeExtractClient {
                db: db.clone(),
                asset_name: asset_name.clone(),
                remote_call_log_id_sink: log_sink.clone(),
            };
            let schedules = extract_schedules_via_llm(&capped, &client).await?;
            let captured = *log_sink.lock().unwrap();
            (schedules, captured)
        }
    };

    // 6. Replace pending + insert new (test-seam entry point).
    run_pipeline_persist(
        db,
        asset_id,
        attachment_uuid,
        schedules,
        tier,
        remote_call_log_id,
    )
}

pub(crate) fn run_pipeline_persist(
    db: Arc<Mutex<rusqlite::Connection>>,
    asset_id: String,
    attachment_uuid: String,
    schedules: Vec<manor_core::pdf_extract::ExtractedSchedule>,
    tier: TierRequest,
    remote_call_log_id: Option<i64>,
) -> Result<ExtractOutcome> {
    let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;

    // Replace pending proposals from the same source attachment.
    let replaced = conn.execute(
        "UPDATE proposal SET status = 'rejected'
         WHERE skill = 'pdf_extract'
           AND kind = 'add_maintenance_schedule'
           AND status = 'pending'
           AND json_extract(diff, '$.source_attachment_uuid') = ?1",
        [&attachment_uuid],
    )? as i64;

    // Insert each schedule as a proposal.
    let mut inserted = 0i64;
    for sched in schedules {
        let args = AddMaintenanceScheduleArgs {
            asset_id: asset_id.clone(),
            task: sched.task,
            interval_months: sched.interval_months,
            notes: sched.notes,
            source_attachment_uuid: attachment_uuid.clone(),
            tier: tier.as_str().to_string(),
        };
        let diff_json = serde_json::to_string(&args)?;
        let pid = proposal::insert(&conn, NewProposal {
            kind: "add_maintenance_schedule",
            rationale: &sched.rationale,
            diff_json: &diff_json,
            skill: "pdf_extract",
        })?;
        if let Some(log_id) = remote_call_log_id {
            conn.execute(
                "UPDATE proposal SET remote_call_log_id = ?1 WHERE id = ?2",
                rusqlite::params![log_id, pid],
            )?;
        }
        inserted += 1;
    }

    Ok(ExtractOutcome {
        proposals_inserted: inserted,
        replaced_pending_count: replaced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use manor_core::assistant::db;
    use manor_core::pdf_extract::ExtractedSchedule;

    fn fresh_db() -> (tempfile::TempDir, Arc<Mutex<rusqlite::Connection>>, String) {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let asset = AssetDraft {
            name: "Boiler".into(),
            category: AssetCategory::Appliance,
            make: None, model: None, serial_number: None, purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        };
        let asset_id = asset_dal::insert_asset(&conn, &asset).unwrap();
        (dir, Arc::new(Mutex::new(conn)), asset_id)
    }

    fn insert_pending_schedule_proposal(
        db: &Arc<Mutex<rusqlite::Connection>>,
        asset_id: &str,
        source_attachment_uuid: &str,
        task: &str,
    ) -> i64 {
        let conn = db.lock().unwrap();
        let args = AddMaintenanceScheduleArgs {
            asset_id: asset_id.into(),
            task: task.into(),
            interval_months: 12,
            notes: String::new(),
            source_attachment_uuid: source_attachment_uuid.into(),
            tier: "ollama".into(),
        };
        let diff_json = serde_json::to_string(&args).unwrap();
        proposal::insert(&conn, NewProposal {
            kind: "add_maintenance_schedule",
            rationale: "old",
            diff_json: &diff_json,
            skill: "pdf_extract",
        }).unwrap()
    }

    fn sample_extract(n: usize) -> Vec<ExtractedSchedule> {
        (0..n).map(|i| ExtractedSchedule {
            task: format!("Task {}", i),
            interval_months: 12,
            notes: "".into(),
            rationale: format!("Rationale {}", i),
        }).collect()
    }

    #[test]
    fn pipeline_persist_replaces_pending_from_same_attachment() {
        let (_d, db, asset_id) = fresh_db();
        let _p1 = insert_pending_schedule_proposal(&db, &asset_id, "uuid-A", "old1");
        let _p2 = insert_pending_schedule_proposal(&db, &asset_id, "uuid-A", "old2");

        let out = run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-A".into(),
            sample_extract(1),
            TierRequest::Ollama,
            None,
        ).unwrap();
        assert_eq!(out.replaced_pending_count, 2);
        assert_eq!(out.proposals_inserted, 1);

        // Old ones now rejected; new one pending.
        let conn = db.lock().unwrap();
        let rejected_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM proposal WHERE status = 'rejected'
             AND kind = 'add_maintenance_schedule' AND skill = 'pdf_extract'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(rejected_count, 2);
        let pending_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM proposal WHERE status = 'pending'
             AND kind = 'add_maintenance_schedule' AND skill = 'pdf_extract'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(pending_count, 1);
    }

    #[test]
    fn pipeline_persist_does_not_touch_other_attachments() {
        let (_d, db, asset_id) = fresh_db();
        let _p_b = insert_pending_schedule_proposal(&db, &asset_id, "uuid-B", "B-pending");

        run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-A".into(),
            sample_extract(1),
            TierRequest::Ollama,
            None,
        ).unwrap();

        let conn = db.lock().unwrap();
        let b_still_pending: i64 = conn.query_row(
            "SELECT COUNT(*) FROM proposal WHERE status = 'pending'
             AND json_extract(diff, '$.source_attachment_uuid') = 'uuid-B'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(b_still_pending, 1);
    }

    #[test]
    fn pipeline_persist_does_not_touch_applied_or_rejected_same_attachment() {
        let (_d, db, asset_id) = fresh_db();
        let applied_pid = insert_pending_schedule_proposal(&db, &asset_id, "uuid-A", "applied-one");
        let rejected_pid = insert_pending_schedule_proposal(&db, &asset_id, "uuid-A", "rejected-one");
        // Flip one to applied, one to rejected (via raw SQL to avoid exercising apply path here).
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "UPDATE proposal SET status = 'applied', applied_at = 1 WHERE id = ?1",
                rusqlite::params![applied_pid],
            ).unwrap();
            conn.execute(
                "UPDATE proposal SET status = 'rejected' WHERE id = ?1",
                rusqlite::params![rejected_pid],
            ).unwrap();
        }

        let out = run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-A".into(),
            sample_extract(1),
            TierRequest::Ollama,
            None,
        ).unwrap();
        assert_eq!(out.replaced_pending_count, 0);  // nothing pending to replace

        let conn = db.lock().unwrap();
        let applied_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM proposal WHERE status = 'applied'
             AND id = ?1",
            rusqlite::params![applied_pid],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(applied_count, 1);
        let rejected_row_status: String = conn.query_row(
            "SELECT status FROM proposal WHERE id = ?1",
            rusqlite::params![rejected_pid],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(rejected_row_status, "rejected");
    }

    #[test]
    fn pipeline_persist_inserts_correct_diff_shape() {
        let (_d, db, asset_id) = fresh_db();
        run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-X".into(),
            sample_extract(2),
            TierRequest::Ollama,
            None,
        ).unwrap();

        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT kind, skill, json_extract(diff, '$.task'),
                    json_extract(diff, '$.source_attachment_uuid'),
                    json_extract(diff, '$.tier')
             FROM proposal WHERE status = 'pending' ORDER BY id ASC"
        ).unwrap();
        let rows: Vec<(String, String, String, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(rows.len(), 2);
        for row in &rows {
            assert_eq!(row.0, "add_maintenance_schedule");
            assert_eq!(row.1, "pdf_extract");
            assert_eq!(row.3, "uuid-X");
            assert_eq!(row.4, "ollama");
        }
        assert_eq!(rows[0].2, "Task 0");
        assert_eq!(rows[1].2, "Task 1");
    }

    #[test]
    fn pipeline_persist_zero_inserted_on_empty_extraction() {
        let (_d, db, asset_id) = fresh_db();
        let out = run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-Z".into(),
            vec![],
            TierRequest::Ollama,
            None,
        ).unwrap();
        assert_eq!(out.proposals_inserted, 0);
    }

    #[test]
    fn pipeline_persist_captures_remote_call_log_id_for_claude() {
        let (_d, db, asset_id) = fresh_db();
        // Insert a canned remote_call_log row first — the FK on proposal.remote_call_log_id
        // doesn't require this in the schema (column is nullable; no FK declared), but we
        // still want a realistic value.
        let log_id = {
            let conn = db.lock().unwrap();
            conn.execute(
                "INSERT INTO remote_call_log
                 (provider, skill, user_visible_reason, model, input_tokens, output_tokens,
                  cost_pence, redaction_count, redaction_entities_json,
                  requested_at, completed_at, status)
                 VALUES ('claude', 'pdf_extract', 'test', 'claude-opus-4-7',
                  100, 100, 5, 0, '[]', 1, 2, 'ok')",
                [],
            ).unwrap();
            conn.last_insert_rowid()
        };

        run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-claude".into(),
            sample_extract(1),
            TierRequest::Claude,
            Some(log_id),
        ).unwrap();

        let conn = db.lock().unwrap();
        let stored_log_id: Option<i64> = conn.query_row(
            "SELECT remote_call_log_id FROM proposal WHERE kind = 'add_maintenance_schedule'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(stored_log_id, Some(log_id));
    }
}
```

If the `remote_call_log` INSERT in the last test fails because the columns don't match Manor's actual schema, adjust by running `grep -n "CREATE TABLE remote_call_log" crates/core/migrations/*.sql` and matching column names. The goal is just to insert any valid row so we have an `id` to reference.

- [ ] **Step 2: Update `crates/app/src/pdf_extract/mod.rs`**

```rust
//! PDF manual extraction (L4e).

pub mod claude_client;
pub mod ollama_client;
pub mod pipeline;
```

- [ ] **Step 3: Run tests**

```
cargo test --package manor-app pdf_extract::pipeline
cargo test --workspace
cargo clippy --workspace -- -D warnings
```
Expected: 6 pipeline tests pass. Workspace green. Clippy clean.

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/pdf_extract/pipeline.rs crates/app/src/pdf_extract/mod.rs
git commit -m "feat(pdf_extract): pipeline orchestrator + persist path (L4e)"
```

---

### Task 6: Tauri commands + register in lib.rs

**Files:**
- Create: `crates/app/src/pdf_extract/commands.rs`
- Modify: `crates/app/src/pdf_extract/mod.rs` — register `pub mod commands;`.
- Modify: `crates/app/src/lib.rs` — register 6 Tauri commands in `invoke_handler!`.

- [ ] **Step 1: Create `crates/app/src/pdf_extract/commands.rs`**

```rust
//! Tauri commands for PDF extraction (L4e).

use super::pipeline::{extract_and_propose, ExtractOutcome, TierRequest};
use crate::assistant::commands::Db;
use manor_core::assistant::proposal::{self, Proposal};
use manor_core::maintenance::MaintenanceScheduleDraft;
use tauri::{AppHandle, Manager, State};

fn attachments_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("attachments"))
}

#[tauri::command]
pub async fn pdf_extract_ollama(
    attachment_uuid: String,
    app: AppHandle,
    state: State<'_, Db>,
) -> Result<ExtractOutcome, String> {
    let db = state.0.clone();
    let dir = attachments_dir(&app)?;
    extract_and_propose(db, dir, attachment_uuid, TierRequest::Ollama)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pdf_extract_claude(
    attachment_uuid: String,
    app: AppHandle,
    state: State<'_, Db>,
) -> Result<ExtractOutcome, String> {
    let db = state.0.clone();
    let dir = attachments_dir(&app)?;
    extract_and_propose(db, dir, attachment_uuid, TierRequest::Claude)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pdf_extract_pending_proposals_for_asset(
    asset_id: String,
    state: State<'_, Db>,
) -> Result<Vec<Proposal>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, rationale, diff, status, proposed_at, applied_at, skill
             FROM proposal
             WHERE skill = 'pdf_extract'
               AND kind = 'add_maintenance_schedule'
               AND status = 'pending'
               AND json_extract(diff, '$.asset_id') = ?1
             ORDER BY proposed_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([&asset_id], |r| {
            Ok(Proposal {
                id: r.get(0)?,
                kind: r.get(1)?,
                rationale: r.get(2)?,
                diff: r.get(3)?,
                status: r.get(4)?,
                proposed_at: r.get(5)?,
                applied_at: r.get(6)?,
                skill: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[tauri::command]
pub fn pdf_extract_pending_exists_for_attachment(
    attachment_uuid: String,
    state: State<'_, Db>,
) -> Result<bool, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM proposal
             WHERE skill = 'pdf_extract'
               AND kind = 'add_maintenance_schedule'
               AND status = 'pending'
               AND json_extract(diff, '$.source_attachment_uuid') = ?1",
            [&attachment_uuid],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count > 0)
}

#[tauri::command]
pub fn pdf_extract_approve_as_is(
    proposal_id: i64,
    state: State<'_, Db>,
) -> Result<String, String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    proposal::approve_add_maintenance_schedule(&mut conn, proposal_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pdf_extract_reject(
    proposal_id: i64,
    state: State<'_, Db>,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    proposal::reject(&conn, proposal_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pdf_extract_approve_with_override(
    proposal_id: i64,
    draft: MaintenanceScheduleDraft,
    state: State<'_, Db>,
) -> Result<String, String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    proposal::approve_add_maintenance_schedule_with_override(&mut conn, proposal_id, &draft)
        .map_err(|e| e.to_string())
}
```

Also add inline integration tests at the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use manor_core::assistant::{db, proposal::{AddMaintenanceScheduleArgs, NewProposal}};
    use rusqlite::Connection;

    fn fresh_conn_with_asset() -> (tempfile::TempDir, Connection, String) {
        let dir = tempfile::tempdir().unwrap();
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

    fn insert_pending(
        conn: &Connection,
        asset_id: &str,
        task: &str,
        source: &str,
    ) -> i64 {
        let args = AddMaintenanceScheduleArgs {
            asset_id: asset_id.into(),
            task: task.into(),
            interval_months: 12,
            notes: String::new(),
            source_attachment_uuid: source.into(),
            tier: "ollama".into(),
        };
        let diff = serde_json::to_string(&args).unwrap();
        proposal::insert(conn, NewProposal {
            kind: "add_maintenance_schedule",
            rationale: "r",
            diff_json: &diff,
            skill: "pdf_extract",
        }).unwrap()
    }

    #[test]
    fn pending_proposals_filters_by_asset() {
        let (_d, conn, asset_a) = fresh_conn_with_asset();
        // Second asset
        let asset_b = asset_dal::insert_asset(&conn, &AssetDraft {
            name: "Other".into(),
            category: AssetCategory::Appliance,
            make: None, model: None, serial_number: None, purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        }).unwrap();

        let _p1 = insert_pending(&conn, &asset_a, "A1", "uuid-A");
        let _p2 = insert_pending(&conn, &asset_a, "A2", "uuid-A");
        let _p3 = insert_pending(&conn, &asset_b, "B1", "uuid-B");

        // Mark one as applied — shouldn't count in pending query.
        let applied_pid = insert_pending(&conn, &asset_a, "A3", "uuid-A");
        conn.execute(
            "UPDATE proposal SET status = 'applied', applied_at = 1 WHERE id = ?1",
            rusqlite::params![applied_pid],
        ).unwrap();

        // Run the raw query that the command executes.
        let mut stmt = conn.prepare(
            "SELECT id FROM proposal
             WHERE skill = 'pdf_extract' AND kind = 'add_maintenance_schedule'
               AND status = 'pending'
               AND json_extract(diff, '$.asset_id') = ?1"
        ).unwrap();
        let a_ids: Vec<i64> = stmt.query_map([&asset_a], |r| r.get(0)).unwrap()
            .map(|r| r.unwrap()).collect();
        assert_eq!(a_ids.len(), 2);

        let mut stmt = conn.prepare(
            "SELECT id FROM proposal
             WHERE skill = 'pdf_extract' AND kind = 'add_maintenance_schedule'
               AND status = 'pending'
               AND json_extract(diff, '$.asset_id') = ?1"
        ).unwrap();
        let b_ids: Vec<i64> = stmt.query_map([&asset_b], |r| r.get(0)).unwrap()
            .map(|r| r.unwrap()).collect();
        assert_eq!(b_ids.len(), 1);
    }

    #[test]
    fn pending_exists_for_attachment_reflects_state() {
        let (_d, conn, asset_id) = fresh_conn_with_asset();
        let count_before: i64 = conn.query_row(
            "SELECT COUNT(*) FROM proposal
             WHERE skill = 'pdf_extract' AND kind = 'add_maintenance_schedule'
               AND status = 'pending'
               AND json_extract(diff, '$.source_attachment_uuid') = 'uuid-X'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count_before, 0);

        insert_pending(&conn, &asset_id, "T", "uuid-X");
        let count_after: i64 = conn.query_row(
            "SELECT COUNT(*) FROM proposal
             WHERE skill = 'pdf_extract' AND kind = 'add_maintenance_schedule'
               AND status = 'pending'
               AND json_extract(diff, '$.source_attachment_uuid') = 'uuid-X'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count_after, 1);
    }
}
```

- [ ] **Step 2: Update `crates/app/src/pdf_extract/mod.rs`**

```rust
//! PDF manual extraction (L4e).

pub mod claude_client;
pub mod commands;
pub mod ollama_client;
pub mod pipeline;
```

- [ ] **Step 3: Register the 6 commands in `crates/app/src/lib.rs` invoke_handler!**

Find `.invoke_handler(tauri::generate_handler![...])`. Append BEFORE the closing `])`:

```rust
crate::pdf_extract::commands::pdf_extract_ollama,
crate::pdf_extract::commands::pdf_extract_claude,
crate::pdf_extract::commands::pdf_extract_pending_proposals_for_asset,
crate::pdf_extract::commands::pdf_extract_pending_exists_for_attachment,
crate::pdf_extract::commands::pdf_extract_approve_as_is,
crate::pdf_extract::commands::pdf_extract_reject,
crate::pdf_extract::commands::pdf_extract_approve_with_override,
```

Wait — that's 7 entries. Count: ollama, claude, pending_proposals_for_asset, pending_exists_for_attachment, approve_as_is, reject, approve_with_override = 7 commands total. The spec said "6 commands" — it was undercounting. Register all 7.

- [ ] **Step 4: Run tests**

```
cargo test --package manor-app pdf_extract
cargo test --workspace
cargo clippy --workspace -- -D warnings
```
Expected: 6 pipeline + 2 command tests pass. Workspace green. Clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/pdf_extract/ crates/app/src/lib.rs
git commit -m "feat(pdf_extract): Tauri commands + invoke_handler registration (L4e)"
```

---

## Phase C — Frontend

### Task 7: Frontend IPC + Zustand store

**Files:**
- Create: `apps/desktop/src/lib/pdf_extract/ipc.ts`
- Create: `apps/desktop/src/lib/pdf_extract/state.ts`

- [ ] **Step 1: Create `apps/desktop/src/lib/pdf_extract/ipc.ts`**

```ts
import { invoke } from "@tauri-apps/api/core";
import type { Proposal } from "../today/ipc";
import type { MaintenanceScheduleDraft } from "../maintenance/ipc";

export interface ExtractOutcome {
  proposals_inserted: number;
  replaced_pending_count: number;
}

export async function extractOllama(attachmentUuid: string): Promise<ExtractOutcome> {
  return await invoke<ExtractOutcome>("pdf_extract_ollama", { attachmentUuid });
}

export async function extractClaude(attachmentUuid: string): Promise<ExtractOutcome> {
  return await invoke<ExtractOutcome>("pdf_extract_claude", { attachmentUuid });
}

export async function listPendingForAsset(assetId: string): Promise<Proposal[]> {
  return await invoke<Proposal[]>(
    "pdf_extract_pending_proposals_for_asset",
    { assetId },
  );
}

export async function pendingExistsForAttachment(attachmentUuid: string): Promise<boolean> {
  return await invoke<boolean>(
    "pdf_extract_pending_exists_for_attachment",
    { attachmentUuid },
  );
}

export async function approveAsIs(proposalId: number): Promise<string> {
  return await invoke<string>("pdf_extract_approve_as_is", { proposalId });
}

export async function reject(proposalId: number): Promise<void> {
  await invoke<void>("pdf_extract_reject", { proposalId });
}

export async function approveWithOverride(
  proposalId: number,
  draft: MaintenanceScheduleDraft,
): Promise<string> {
  return await invoke<string>(
    "pdf_extract_approve_with_override",
    { proposalId, draft },
  );
}
```

- [ ] **Step 2: Create `apps/desktop/src/lib/pdf_extract/state.ts`**

```ts
import { create } from "zustand";
import * as ipc from "./ipc";
import type { Proposal } from "../today/ipc";
import type { MaintenanceScheduleDraft } from "../maintenance/ipc";

type ExtractStatus =
  | { kind: "idle" }
  | { kind: "extracting"; tier: "ollama" | "claude" }
  | { kind: "error"; message: string };

interface PdfExtractStore {
  proposalsByAsset: Record<string, Proposal[]>;
  pendingByAttachment: Record<string, boolean>;
  lastExtractMessage: string | null;
  extractStatus: ExtractStatus;

  loadForAsset(assetId: string): Promise<void>;
  loadPendingFlag(attachmentUuid: string): Promise<void>;
  extractOllama(attachmentUuid: string, assetId: string): Promise<void>;
  extractClaude(attachmentUuid: string, assetId: string): Promise<void>;
  approveAsIs(proposalId: number, assetId: string): Promise<void>;
  reject(proposalId: number, assetId: string): Promise<void>;
  approveWithOverride(
    proposalId: number,
    assetId: string,
    draft: MaintenanceScheduleDraft,
  ): Promise<void>;
  clearLastMessage(): void;
}

export const usePdfExtractStore = create<PdfExtractStore>((set, get) => ({
  proposalsByAsset: {},
  pendingByAttachment: {},
  lastExtractMessage: null,
  extractStatus: { kind: "idle" },

  async loadForAsset(assetId) {
    try {
      const rows = await ipc.listPendingForAsset(assetId);
      set((s) => ({ proposalsByAsset: { ...s.proposalsByAsset, [assetId]: rows } }));
    } catch (e: unknown) {
      console.error("pdf_extract: loadForAsset failed", e);
    }
  },

  async loadPendingFlag(attachmentUuid) {
    try {
      const exists = await ipc.pendingExistsForAttachment(attachmentUuid);
      set((s) => ({
        pendingByAttachment: { ...s.pendingByAttachment, [attachmentUuid]: exists },
      }));
    } catch (e: unknown) {
      console.error("pdf_extract: loadPendingFlag failed", e);
    }
  },

  async extractOllama(attachmentUuid, assetId) {
    set({ extractStatus: { kind: "extracting", tier: "ollama" } });
    try {
      const outcome = await ipc.extractOllama(attachmentUuid);
      await get().loadForAsset(assetId);
      await get().loadPendingFlag(attachmentUuid);
      set({
        extractStatus: { kind: "idle" },
        lastExtractMessage: describeOutcome(outcome),
      });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ extractStatus: { kind: "error", message } });
      throw e;
    }
  },

  async extractClaude(attachmentUuid, assetId) {
    set({ extractStatus: { kind: "extracting", tier: "claude" } });
    try {
      const outcome = await ipc.extractClaude(attachmentUuid);
      await get().loadForAsset(assetId);
      await get().loadPendingFlag(attachmentUuid);
      set({
        extractStatus: { kind: "idle" },
        lastExtractMessage: describeOutcome(outcome),
      });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ extractStatus: { kind: "error", message } });
      throw e;
    }
  },

  async approveAsIs(proposalId, assetId) {
    await ipc.approveAsIs(proposalId);
    await get().loadForAsset(assetId);
    // Invalidate all attachment flags — we don't know which one this proposal belongs to
    // without re-parsing diff; easiest to re-query any we care about on next render.
    set({ pendingByAttachment: {} });
  },

  async reject(proposalId, assetId) {
    await ipc.reject(proposalId);
    await get().loadForAsset(assetId);
    set({ pendingByAttachment: {} });
  },

  async approveWithOverride(proposalId, assetId, draft) {
    await ipc.approveWithOverride(proposalId, draft);
    await get().loadForAsset(assetId);
    set({ pendingByAttachment: {} });
  },

  clearLastMessage() {
    set({ lastExtractMessage: null });
  },
}));

function describeOutcome(outcome: ipc.ExtractOutcome): string {
  if (outcome.proposals_inserted === 0) {
    return "No maintenance schedules found in this manual.";
  }
  const base = `${outcome.proposals_inserted} proposal${outcome.proposals_inserted === 1 ? "" : "s"} extracted`;
  if (outcome.replaced_pending_count > 0) {
    return `${base}. ${outcome.replaced_pending_count} previous proposal${outcome.replaced_pending_count === 1 ? "" : "s"} replaced.`;
  }
  return `${base}.`;
}
```

- [ ] **Step 3: Type-check + existing tests**

```
cd apps/desktop
pnpm tsc --noEmit
pnpm test
```
Expected: TS clean. Existing tests still pass (count unchanged — no new tests in this task).

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/lib/pdf_extract/
git commit -m "feat(pdf_extract): frontend IPC + Zustand store (L4e)"
```

---

### Task 8: PendingProposalsBlock + mount on AssetDetail

**Files:**
- Create: `apps/desktop/src/components/Bones/PendingProposalsBlock.tsx`
- Modify: `apps/desktop/src/components/Bones/AssetDetail.tsx` — mount the block between MaintenanceSection and HistoryBlock.

- [ ] **Step 1: Create `PendingProposalsBlock.tsx`**

```tsx
import { useEffect, useState } from "react";
import { Check, Pencil, X } from "lucide-react";
import { usePdfExtractStore } from "../../lib/pdf_extract/state";
import { useMaintenanceStore } from "../../lib/maintenance/state";
import type { Proposal } from "../../lib/today/ipc";
import { ScheduleDrawer } from "./DueSoon/ScheduleDrawer";

interface Props {
  assetId: string;
}

interface ParsedDiff {
  asset_id: string;
  task: string;
  interval_months: number;
  notes: string;
  source_attachment_uuid: string;
  tier: string;
}

function parseDiff(proposal: Proposal): ParsedDiff | null {
  try {
    return JSON.parse(proposal.diff) as ParsedDiff;
  } catch {
    return null;
  }
}

export function PendingProposalsBlock({ assetId }: Props) {
  const { proposalsByAsset, loadForAsset, approveAsIs, reject } =
    usePdfExtractStore();
  const { loadForAsset: loadSchedules } = useMaintenanceStore();

  const [editProposal, setEditProposal] = useState<Proposal | null>(null);

  useEffect(() => {
    if (!proposalsByAsset[assetId]) void loadForAsset(assetId);
  }, [assetId, proposalsByAsset, loadForAsset]);

  const rows = proposalsByAsset[assetId] ?? [];

  if (rows.length === 0) return null;

  const onApprove = async (id: number) => {
    try {
      await approveAsIs(id, assetId);
      await loadSchedules(assetId);
    } catch (e: unknown) {
      console.error("approve failed", e);
    }
  };

  const onReject = async (id: number) => {
    try {
      await reject(id, assetId);
    } catch (e: unknown) {
      console.error("reject failed", e);
    }
  };

  return (
    <section style={{ marginTop: 24 }}>
      <h3 style={{ margin: "0 0 12px 0" }}>Proposed schedules</h3>
      <div
        style={{
          border: "1px solid var(--border, #e5e5e5)",
          borderRadius: 6,
          padding: 12,
          background: "var(--surface-subtle, #fafafa)",
        }}
      >
        {rows.map((p) => {
          const diff = parseDiff(p);
          if (!diff) return null;
          return (
            <div
              key={p.id}
              style={{
                display: "flex",
                alignItems: "flex-start",
                gap: 8,
                padding: "8px 0",
                borderBottom: "1px solid var(--border, #eee)",
              }}
            >
              <div style={{ flex: 1 }}>
                <div style={{ fontWeight: 500 }}>
                  {diff.task} · every {diff.interval_months} month
                  {diff.interval_months === 1 ? "" : "s"}
                </div>
                {p.rationale.trim() && (
                  <div
                    style={{
                      fontSize: 12,
                      color: "var(--ink-soft, #888)",
                      fontStyle: "italic",
                      marginTop: 2,
                      // two-line truncation
                      display: "-webkit-box",
                      WebkitLineClamp: 2,
                      WebkitBoxOrient: "vertical",
                      overflow: "hidden",
                    }}
                  >
                    &ldquo;{p.rationale}&rdquo;
                  </div>
                )}
              </div>
              <button
                type="button"
                onClick={() => void onApprove(p.id)}
                aria-label="Approve proposal"
                title="Approve as-is"
                style={buttonStyle}
              >
                <Check size={16} />
              </button>
              <button
                type="button"
                onClick={() => setEditProposal(p)}
                aria-label="Edit proposal"
                title="Edit then approve"
                style={buttonStyle}
              >
                <Pencil size={14} />
              </button>
              <button
                type="button"
                onClick={() => void onReject(p.id)}
                aria-label="Reject proposal"
                title="Reject"
                style={buttonStyle}
              >
                <X size={14} />
              </button>
            </div>
          );
        })}
      </div>

      {editProposal && (() => {
        const diff = parseDiff(editProposal);
        if (!diff) return null;
        // Create a synthetic MaintenanceSchedule-shaped object for the drawer's
        // `schedule` prop. The drawer's proposal-edit mode uses `proposalId` +
        // initial fields.
        const now = Math.floor(Date.now() / 1000);
        const syntheticSchedule = {
          id: "",  // not used in proposal-edit mode
          asset_id: diff.asset_id,
          task: diff.task,
          interval_months: diff.interval_months,
          last_done_date: null,
          next_due_date: "",
          notes: diff.notes,
          created_at: now,
          updated_at: now,
          deleted_at: null,
        };
        return (
          <ScheduleDrawer
            schedule={syntheticSchedule}
            initialAssetId={diff.asset_id}
            lockAsset={true}
            proposalId={editProposal.id}
            onClose={() => setEditProposal(null)}
            onSaved={() => {
              setEditProposal(null);
              void loadForAsset(assetId);
              void loadSchedules(assetId);
            }}
          />
        );
      })()}
    </section>
  );
}

const buttonStyle: React.CSSProperties = {
  background: "transparent",
  border: "1px solid var(--border, #ddd)",
  borderRadius: 4,
  padding: 6,
  cursor: "pointer",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};
```

- [ ] **Step 2: Mount on `AssetDetail.tsx`**

Open `apps/desktop/src/components/Bones/AssetDetail.tsx`. Find the `<MaintenanceSection assetId={id} />` mount (around line 124 based on L4b) and the `<HistoryBlock assetId={id} />` mount (added in L4c). Between them, insert:

```tsx
<PendingProposalsBlock assetId={id} />
```

Add the import at the top of the file:

```tsx
import { PendingProposalsBlock } from "./PendingProposalsBlock";
```

- [ ] **Step 3: Run tsc + tests**

```
cd apps/desktop
pnpm tsc --noEmit
pnpm test
```
Expected: clean. Existing tests still pass.

Note: `pnpm tsc --noEmit` will emit an error about `ScheduleDrawer` not accepting the `proposalId` prop. That's expected — Task 9 adds the prop to the drawer. DO NOT commit this task until Task 9 is done. Instead, proceed directly to Task 9 and commit them together.

**Alternative:** if you want clean commits per task, comment out the `<ScheduleDrawer ... proposalId=... />` block temporarily with `TODO: proposalId prop added in Task 9`, land this task, then un-comment in Task 9. Pick whichever is easier — the review can accept either flow.

Preferred path: batch tasks 8 + 9 into one commit (both touch the drawer interface).

- [ ] **Step 4: Proceed to Task 9 before committing**

Skip the commit step for Task 8. Proceed to Task 9, which extends `ScheduleDrawer` with the `proposalId` prop. After Task 9, commit both tasks together.

---

### Task 9: ScheduleDrawer proposal-edit mode

**Files:**
- Modify: `apps/desktop/src/components/Bones/DueSoon/ScheduleDrawer.tsx` — add optional `proposalId?: number` prop.

- [ ] **Step 1: Read the current ScheduleDrawer**

```
sed -n '1,60p' apps/desktop/src/components/Bones/DueSoon/ScheduleDrawer.tsx
```

Locate: the Props interface, the `EMPTY_DRAFT` or equivalent, the Save button handler, the Delete button render, and the component's return JSX.

- [ ] **Step 2: Extend Props with `proposalId?: number`**

Add `proposalId?: number` to the `Props` interface:

```tsx
interface Props {
  schedule?: MaintenanceSchedule;
  initialAssetId?: string;
  lockAsset?: boolean;
  proposalId?: number;           // NEW — when set, Save applies the proposal
  onClose: () => void;
  onSaved: () => void;
  onDeleted?: () => void;
}
```

- [ ] **Step 3: Update the Save handler**

In the component body, find where the Save button's `onClick` handler lives. Modify it to branch on `proposalId`:

```tsx
import { approveWithOverride } from "../../../lib/pdf_extract/ipc";

// inside the component:
const handleSave = async () => {
  // existing validation...
  const draft: MaintenanceScheduleDraft = {
    asset_id: form.asset_id,
    task: form.task,
    interval_months: form.interval_months,
    last_done_date: form.last_done_date,
    notes: form.notes,
  };

  try {
    if (proposalId !== undefined && proposalId !== null) {
      // Proposal-edit mode: apply via pdf_extract.
      await approveWithOverride(proposalId, draft);
      onSaved();
    } else if (schedule) {
      // Existing edit mode
      await update(schedule.id, draft);
      onSaved();
    } else {
      // Existing create mode
      await create(draft);
      onSaved();
    }
  } catch (e: unknown) {
    // existing error handling...
  }
};
```

Adjust the exact lines to match the current drawer body — the structure above shows the decision tree; preserve the existing variable names (`create`, `update`, `form`, etc.) from the file.

- [ ] **Step 4: Update the Save button label**

```tsx
const saveLabel =
  proposalId !== undefined && proposalId !== null
    ? "Approve & add"
    : schedule
      ? "Save changes"
      : "Save";

// in JSX:
<button type="submit" disabled={saving}>{saveLabel}</button>
```

- [ ] **Step 5: Hide the Delete button in proposal-edit mode**

Where the Delete button is rendered (inside a conditional that checks `schedule` and `onDelete`), add the `!proposalId` guard:

```tsx
{schedule && onDelete && proposalId === undefined && (
  <button type="button" onClick={handleDelete} ...>
    Delete
  </button>
)}
```

- [ ] **Step 6: Type-check + run all tests**

```
cd apps/desktop
pnpm tsc --noEmit
pnpm test
```
Expected: clean. Existing tests still pass. Task 8's AssetDetail edit should now compile.

- [ ] **Step 7: Commit Tasks 8 + 9 together**

```bash
git add apps/desktop/src/components/Bones/PendingProposalsBlock.tsx \
        apps/desktop/src/components/Bones/AssetDetail.tsx \
        apps/desktop/src/components/Bones/DueSoon/ScheduleDrawer.tsx
git commit -m "feat(pdf_extract): PendingProposalsBlock + drawer proposal-edit mode (L4e)"
```

---

### Task 10: Attachment-row Extract / Re-extract buttons

**Files:**
- Modify: the attachment row component in `apps/desktop/src/components/Bones/AssetDetail.tsx` or wherever Documents section lives.

- [ ] **Step 1: Locate the attachment row rendering**

Run:
```
grep -n "application/pdf\|mime_type\|attachment" apps/desktop/src/components/Bones/AssetDetail.tsx
```

Find the JSX that renders each attachment (e.g. inside a Documents section, iterating over attachments with `name, mime_type, uuid` fields). If attachment rendering is inside its own subcomponent, modify that file instead.

- [ ] **Step 2: Add Extract + Re-extract buttons**

Import the store + an `useEffect` to load the pending flag for each PDF attachment:

```tsx
import { usePdfExtractStore } from "../../lib/pdf_extract/state";
import { useEffect } from "react";
```

Inside the attachment row component (or inline JSX), when `attachment.mime_type === 'application/pdf'`:

```tsx
const { pendingByAttachment, loadPendingFlag, extractOllama, extractClaude, extractStatus } =
  usePdfExtractStore();

useEffect(() => {
  if (attachment.mime_type === "application/pdf") {
    void loadPendingFlag(attachment.uuid);
  }
}, [attachment.uuid, attachment.mime_type, loadPendingFlag]);

// ... in JSX, within the attachment row, when mime_type is 'application/pdf':
{attachment.mime_type === "application/pdf" && (
  <div style={{ display: "flex", gap: 8, marginTop: 4 }}>
    <button
      type="button"
      onClick={() => void extractOllama(attachment.uuid, assetId)}
      disabled={extractStatus.kind === "extracting"}
    >
      {extractStatus.kind === "extracting" && extractStatus.tier === "ollama"
        ? "Extracting…"
        : "Extract maintenance schedules"}
    </button>
    {pendingByAttachment[attachment.uuid] && (
      <button
        type="button"
        onClick={() => void extractClaude(attachment.uuid, assetId)}
        disabled={extractStatus.kind === "extracting"}
      >
        {extractStatus.kind === "extracting" && extractStatus.tier === "claude"
          ? "Extracting with Claude…"
          : "Re-extract with Claude"}
      </button>
    )}
  </div>
)}
```

Exact indentation + surrounding JSX depends on the existing attachment-row structure. Preserve the existing layout.

- [ ] **Step 3: Surface `lastExtractMessage` + error somewhere visible**

Find where AssetDetail renders its top-level error/status. If there's a top band, append a small conditional:

```tsx
const { lastExtractMessage, extractStatus, clearLastMessage } = usePdfExtractStore();

{lastExtractMessage && (
  <div style={infoBandStyle} onClick={() => clearLastMessage()}>
    {lastExtractMessage}
  </div>
)}
{extractStatus.kind === "error" && (
  <div style={errorBandStyle}>
    Error: {extractStatus.message}
  </div>
)}
```

Use whatever `infoBandStyle` / `errorBandStyle` conventions exist elsewhere in the codebase (L4d's TroubleshootBlock has an error band pattern — see `apps/desktop/src/components/Bones/TroubleshootBlock.tsx`).

- [ ] **Step 4: Type-check + tests**

```
cd apps/desktop
pnpm tsc --noEmit
pnpm test
```
Expected: clean. 51 frontend tests still pass.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/Bones/AssetDetail.tsx
# plus any sub-file you modified
git commit -m "feat(pdf_extract): Extract + Re-extract buttons on PDF attachment rows (L4e)"
```

---

### Task 11: RTL tests for PendingProposalsBlock

**Files:**
- Create: `apps/desktop/src/components/Bones/__tests__/PendingProposalsBlock.test.tsx`

- [ ] **Step 1: Create the test file**

```tsx
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { PendingProposalsBlock } from "../PendingProposalsBlock";
import { usePdfExtractStore } from "../../../lib/pdf_extract/state";
import { useMaintenanceStore } from "../../../lib/maintenance/state";

vi.mock("../../../lib/pdf_extract/state", () => ({
  usePdfExtractStore: vi.fn(),
}));

vi.mock("../../../lib/maintenance/state", () => ({
  useMaintenanceStore: vi.fn(),
}));

// ScheduleDrawer depends on many things; stub it.
vi.mock("../DueSoon/ScheduleDrawer", () => ({
  ScheduleDrawer: ({ proposalId, onSaved }: { proposalId?: number; onSaved: () => void }) => (
    <div data-testid="schedule-drawer">
      <span>proposalId={proposalId}</span>
      <button onClick={onSaved}>Drawer Save</button>
    </div>
  ),
}));

const makeProposal = (id: number, task: string, interval: number) => ({
  id,
  kind: "add_maintenance_schedule",
  rationale: `rationale-${id}`,
  diff: JSON.stringify({
    asset_id: "a1",
    task,
    interval_months: interval,
    notes: "",
    source_attachment_uuid: "att-uuid",
    tier: "ollama",
  }),
  status: "pending",
  proposed_at: 1,
  applied_at: null,
  skill: "pdf_extract",
});

describe("PendingProposalsBlock", () => {
  const loadForAsset = vi.fn();
  const approveAsIs = vi.fn().mockResolvedValue(undefined);
  const reject = vi.fn().mockResolvedValue(undefined);
  const loadSchedules = vi.fn();

  function mockStores(proposals: any[]) {
    (usePdfExtractStore as unknown as ReturnType<typeof vi.fn>).mockImplementation(() => ({
      proposalsByAsset: { a1: proposals },
      loadForAsset,
      approveAsIs,
      reject,
    }));
    (useMaintenanceStore as unknown as ReturnType<typeof vi.fn>).mockImplementation(() => ({
      loadForAsset: loadSchedules,
    }));
  }

  beforeEach(() => {
    approveAsIs.mockClear();
    reject.mockClear();
    loadForAsset.mockClear();
    loadSchedules.mockClear();
  });

  afterEach(() => cleanup());

  it("renders nothing when no proposals", () => {
    mockStores([]);
    const { container } = render(<PendingProposalsBlock assetId="a1" />);
    expect(container.textContent).toBe("");
  });

  it("renders one row per proposal with task + interval + rationale", () => {
    mockStores([
      makeProposal(1, "Annual service", 12),
      makeProposal(2, "Filter change", 6),
    ]);
    render(<PendingProposalsBlock assetId="a1" />);
    expect(screen.getByText(/Annual service · every 12 months/)).toBeInTheDocument();
    expect(screen.getByText(/Filter change · every 6 months/)).toBeInTheDocument();
    expect(screen.getByText(/rationale-1/)).toBeInTheDocument();
    expect(screen.getByText(/rationale-2/)).toBeInTheDocument();
  });

  it("approve click calls approveAsIs(id, assetId) + loadSchedules", async () => {
    mockStores([makeProposal(42, "Task", 12)]);
    render(<PendingProposalsBlock assetId="a1" />);
    fireEvent.click(screen.getByLabelText("Approve proposal"));
    await new Promise((r) => setTimeout(r, 0));
    expect(approveAsIs).toHaveBeenCalledWith(42, "a1");
    expect(loadSchedules).toHaveBeenCalledWith("a1");
  });

  it("reject click calls reject(id, assetId)", async () => {
    mockStores([makeProposal(7, "Task", 12)]);
    render(<PendingProposalsBlock assetId="a1" />);
    fireEvent.click(screen.getByLabelText("Reject proposal"));
    await new Promise((r) => setTimeout(r, 0));
    expect(reject).toHaveBeenCalledWith(7, "a1");
  });

  it("edit click opens drawer with proposalId set", () => {
    mockStores([makeProposal(99, "Task", 12)]);
    render(<PendingProposalsBlock assetId="a1" />);
    fireEvent.click(screen.getByLabelText("Edit proposal"));
    expect(screen.getByTestId("schedule-drawer")).toBeInTheDocument();
    expect(screen.getByText(/proposalId=99/)).toBeInTheDocument();
  });

  it("handles invalid diff JSON gracefully (skips row)", () => {
    const badProposal = {
      id: 1,
      kind: "add_maintenance_schedule",
      rationale: "r",
      diff: "not valid json",
      status: "pending",
      proposed_at: 1,
      applied_at: null,
      skill: "pdf_extract",
    };
    mockStores([badProposal]);
    const { container } = render(<PendingProposalsBlock assetId="a1" />);
    // Row is filtered out silently; the section header still renders but no row inside.
    expect(container.querySelector("[aria-label='Approve proposal']")).toBeNull();
  });
});
```

- [ ] **Step 2: Run tests**

```
cd apps/desktop
pnpm test PendingProposalsBlock
```
Expected: 6 PASS.

Run full suite:
```
pnpm test
```
Expected: previous count + 6 new = ~57 tests. All green.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/components/Bones/__tests__/PendingProposalsBlock.test.tsx
git commit -m "test(pdf_extract): RTL tests for PendingProposalsBlock (L4e)"
```

---

## Phase D — Integration + ship

### Task 12: Full green battery + manual QA handoff

**Files:** no new files. Runs the green-check battery and produces the merge handoff.

- [ ] **Step 1: Run the full battery**

```
cargo test --workspace
cargo clippy --workspace -- -D warnings
cd apps/desktop && pnpm tsc --noEmit && pnpm test && pnpm build
cd ..
```

Record each count. All must pass.

- [ ] **Step 2: Fmt discipline check**

```
cargo fmt --package manor-core --check
cargo fmt --package manor-app --check
```

If your L4e-touched files are flagged, run `cargo fmt` and verify only those files get modified. Restore any pre-existing drift on unrelated files via `git checkout --` before stopping.

- [ ] **Step 3: No integration commit required**

If the battery is clean, no additional commit is needed — the branch is already at its final implementation state.

- [ ] **Step 4: Report ready-to-merge**

Collect and report:
1. Full branch history: `git log --oneline main..HEAD` — expect ~9-10 commits spanning Tasks 1-11.
2. Full Rust test count from `cargo test --workspace`.
3. Frontend test count from `pnpm test`.
4. `pnpm build` bundle size if shown.
5. Clippy/fmt status.
6. Manual QA script (below).

### Manual QA script (for Hana)

1. `cd apps/desktop && pnpm tauri dev`.
2. Open any asset in Bones. Navigate to the Documents section.
3. Upload a real PDF manual (boiler or washing machine manual). Verify it appears as an attachment row with `Extract maintenance schedules` button.
4. Click Extract. Wait up to ~30 s.
5. Verify the `Proposed schedules` block appears above History, with 1+ rows.
6. Click ✓ on one proposal → row disappears; new schedule appears in the Maintenance section above.
7. Click ✎ on another → ScheduleDrawer opens; button says "Approve & add"; Delete button is hidden. Tweak the task name. Click Approve & add → proposal disappears; edited schedule appears in MaintenanceSection.
8. Click × on a third → proposal disappears; no schedule added.
9. Refresh or re-open asset. Verify remaining pending proposals persist (survive page-state changes).
10. Click `Re-extract with Claude` (visible because pending still exists). Any remaining pending proposals should be replaced with a fresh Claude-tier set. Tier chip on new proposals would say `claude` if surfaced (we don't currently surface it in the UI — it's in the diff).
11. Upload an image-only PDF → error toast "PDF appears to be an image scan".
12. Upload a 12 MB+ PDF → error toast "PDF too large to extract (over 10 MB)".

### Do NOT merge

The merge (`git checkout main && git merge --no-ff feature/l4e-pdf-extract -m "..."`) + worktree cleanup is user-driven. Report the branch state and wait for authorization.

---

## Definition of done recap

- `pdf-extract` 0.7+ dep added to `crates/core/Cargo.toml`; builds on macOS ARM + Linux x86_64.
- `crates/core/src/pdf_extract/*` ships: text extraction + tier caps + LLM prompt/parse + unit tests.
- `proposal.rs` gains `AddMaintenanceScheduleArgs` + `approve_add_maintenance_schedule` + `approve_add_maintenance_schedule_with_override` + 6 new tests.
- Pipeline `extract_and_propose` resolves attachment → asset → PDF path → text → cap → LLM → replace pending → insert proposals, capturing `remote_call_log_id` on Claude path.
- 7 Tauri commands registered via `invoke_handler!`.
- Frontend IPC + Zustand store + `PendingProposalsBlock` + `ScheduleDrawer` extension + attachment-row Extract/Re-extract buttons.
- 6 RTL tests for `PendingProposalsBlock`.
- `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `pnpm tsc --noEmit`, `pnpm test`, `pnpm build` all green.
- Manual QA walkthrough (steps 1-12) ready for Hana.

---

*End of L4e implementation plan.*
