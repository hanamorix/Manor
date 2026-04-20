# L4e PDF Manual Extraction — Design Spec

- **Date**: 2026-04-20
- **Landmark**: v0.5 Bones → L4e (fifth and final sub-landmark)
- **Status**: Approved design. Implementation plan next.
- **Authors**: Hana (product), Nell (architect)
- **Roadmap**: `specs/2026-04-18-v0.5-bones-roadmap.md`
- **Depends on**: L4a (asset registry, shipped `5645b7c`), L4b (maintenance schedules, shipped `a22b362`), existing Ollama plumbing (`crates/app/src/assistant/ollama.rs` with the `chat_collect` helper added in L4d), existing Remote LLM orchestrator (`crates/app/src/remote/`), L3a LLM extract pattern (`crate::recipe::import::LlmClient` + `extract_json_array_block`).

## 1. Purpose

Turn a PDF appliance, vehicle, or fixture manual into a batch of `maintenance_schedule` rows via one LLM call. Hana attaches the PDF to an asset (using L4a's existing attachment flow), clicks "Extract maintenance schedules" on the attachment row, reviews a list of proposals (approve / edit-via-drawer / reject), and the approved ones land in the database as real schedules.

L4e closes the v0.5 Bones cycle: after L4a–L4d, Hana has assets, schedules, events, spend tracking, right-to-repair lookup, and now — she never has to hand-transcribe a maintenance calendar from a paper manual again.

## 2. Scope decisions (locked during brainstorm)

| Area | Decision |
|---|---|
| **Storage** | Reuse the existing `proposal` table with `kind='add_maintenance_schedule'`. No schema migration. The `diff` JSON column carries the proposal-specific payload. |
| **LLM tier** | Ollama-first. Opt-in "Re-extract with Claude" button replaces pending proposals for the same source attachment. Same two-command pattern as L4d. |
| **Approval UI** | Inline `PendingProposalsBlock` on AssetDetail, mounted between MaintenanceSection (L4b) and HistoryBlock (L4c). Per-row Approve (✓) · Edit (pencil → opens L4b's `ScheduleDrawer` pre-filled) · Reject (×). |
| **Edit path** | Reuse L4b's `ScheduleDrawer` for editing. Drawer gains one new prop — `proposalId: number \| null`. When set, Save writes the schedule AND marks the proposal `applied` in a single backend command. |
| **Re-extraction** | Auto-replaces pending proposals from the same source attachment. Any `proposal` row with `skill='pdf_extract'`, `kind='add_maintenance_schedule'`, `status='pending'`, and `json_extract(diff, '$.source_attachment_uuid') = <uuid>` is marked `rejected` before the new set is inserted. Applied + previously-rejected proposals survive. |
| **Pipeline** | `pdf-extract` crate → cap at 32 KB (Ollama) or 200 KB (Claude) → one LLM call → JSON array parse → retry once on bad JSON (L3a pattern) → insert `proposal` rows. Single LLM call per extract. |
| **Entry point** | "Extract maintenance schedules" button on the attachment row in AssetDetail's Documents section. Visible only when `mime_type === 'application/pdf'`. "Re-extract with Claude" appears alongside when pending proposals exist for that attachment. |
| **Failure modes** | Image-only PDFs (extracted text < 500 chars) → `ExtractError::ImageOnly`. PDFs > 10 MB → `ExtractError::TooLarge(10)`. LLM returns zero schedules → friendly empty-state toast (`"No maintenance schedules found in this manual."`), no proposals inserted. |
| **Edit limits** | Proposals are immutable post-apply. Approved schedules are edited via L4b's normal flow. Rejected proposals are historical markers. |

## 3. Proposal shape (no schema change)

The existing `proposal` table (from V1) already fits. We use it as-is:

```sql
-- V1__initial.sql, unchanged:
CREATE TABLE proposal (
  id                  INTEGER PRIMARY KEY,
  kind                TEXT    NOT NULL,
  rationale           TEXT    NOT NULL,
  diff                TEXT    NOT NULL,
  status              TEXT    NOT NULL DEFAULT 'pending',
  proposed_at         INTEGER NOT NULL,
  applied_at          INTEGER NULL,
  skill               TEXT    NOT NULL,
  remote_call_log_id  INTEGER NULL
);
```

For each L4e proposal:

| Column | Value |
|---|---|
| `kind` | `"add_maintenance_schedule"` |
| `rationale` | LLM-authored one-line context (e.g., `"Section 7.2 recommends annual service every 12 months."`) |
| `diff` | JSON string, shape below |
| `skill` | `"pdf_extract"` |
| `status` | `"pending"` on insert |
| `remote_call_log_id` | Populated from orchestrator outcome when tier = Claude; NULL for Ollama |

### 3.1 `diff` JSON shape

```json
{
  "asset_id": "uuid-of-asset",
  "task": "Annual service",
  "interval_months": 12,
  "notes": "",
  "source_attachment_uuid": "uuid-of-attachment",
  "tier": "ollama"
}
```

Rust struct (in `crates/core/src/assistant/proposal.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMaintenanceScheduleArgs {
    pub asset_id: String,
    pub task: String,
    pub interval_months: i32,
    pub notes: String,
    pub source_attachment_uuid: String,
    pub tier: String,  // "ollama" | "claude" — audit only
}
```

No migration. No new tables. Approval path extends the existing `proposal::apply` function with a new kind-handler that inserts a `maintenance_schedule` row via L4b's DAL.

## 4. Architecture

### 4.1 New Rust files

- `crates/core/src/pdf_extract/mod.rs` — module root + types (`ExtractedSchedule`, `ExtractError`).
- `crates/core/src/pdf_extract/text.rs` — `pdf-extract` wrapper + size/image-only guards + tier-based text capping.
- `crates/core/src/pdf_extract/llm.rs` — prompt builder + JSON-array parse (mirrors `recipe::import::extract_via_llm`).
- `crates/app/src/pdf_extract/mod.rs` — module root.
- `crates/app/src/pdf_extract/ollama_client.rs` — `LlmClient` impl over `OllamaClient::chat_collect`.
- `crates/app/src/pdf_extract/claude_client.rs` — `LlmClient` impl over `remote::orchestrator::remote_chat` with skill `"pdf_extract"`; captures `remote_call_log_id`.
- `crates/app/src/pdf_extract/pipeline.rs` — `extract_and_propose` orchestrator.
- `crates/app/src/pdf_extract/commands.rs` — 6 Tauri commands.

### 4.2 New frontend files

- `apps/desktop/src/lib/pdf_extract/ipc.ts` — 6 IPC wrappers.
- `apps/desktop/src/lib/pdf_extract/state.ts` — `usePdfExtractStore` Zustand store.
- `apps/desktop/src/components/Bones/PendingProposalsBlock.tsx` — per-asset pending-proposals list.

### 4.3 Modified Rust files

- `crates/core/src/lib.rs` — `pub mod pdf_extract;`.
- `crates/core/src/assistant/proposal.rs` — add `AddMaintenanceScheduleArgs` struct + `apply_add_maintenance_schedule_with_override` function + extend `apply(id)` dispatch with `"add_maintenance_schedule"` branch.
- `crates/core/src/recipe/import.rs` — promote `extract_json_array_block` to `pub fn extract_json_array_block_public` (mirrors the existing `extract_json_block_public`) so `pdf_extract::llm` can reuse it without duplication.
- `crates/core/Cargo.toml` — add `pdf-extract = "0.7"` (or latest stable at implementation time).
- `crates/app/src/lib.rs` — `pub mod pdf_extract;` + register 6 new Tauri commands.
- `crates/app/src/assistant/ollama.rs` — no change required (L4d already added `chat_collect`).

### 4.4 Modified frontend files

- `apps/desktop/src/components/Bones/AssetDetail.tsx` — mount `<PendingProposalsBlock assetId={id} />` between `<MaintenanceSection />` and `<HistoryBlock />`. Add Extract / Re-extract buttons to attachment rows where `mime_type === 'application/pdf'`.
- `apps/desktop/src/components/Bones/DueSoon/ScheduleDrawer.tsx` — add optional `proposalId?: number` prop. When set: Save calls `approveWithOverride(proposalId, draft)` instead of the normal create path; Delete button hidden; Save button labelled "Approve & add".

## 5. Core types

### 5.1 `crates/core/src/pdf_extract/mod.rs`

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

### 5.2 Text extraction (`crates/core/src/pdf_extract/text.rs`)

```rust
use super::ExtractError;
use std::path::Path;

pub const MAX_PDF_BYTES: u64 = 10 * 1024 * 1024;  // 10 MB
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
```

The `pdf_extract` crate's 0.7 API (`extract_text_from_mem(&[u8]) -> Result<String, OutputError>`) is assumed. If the exact version at implementation time differs, adjust — this is a thin one-call wrapper.

### 5.3 LLM extraction (`crates/core/src/pdf_extract/llm.rs`)

```rust
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
```

Post-parse validation silently drops invalid schedules (interval ≤ 0, interval > 240, empty task). The user sees a filtered proposal list — if the LLM hallucinated bogus schedules they don't reach the UI.

## 6. Proposal apply extension (`crates/core/src/assistant/proposal.rs`)

The existing `apply(conn, id)` function (around line 134) currently handles only `kind='add_task'`. Extend the dispatch:

```rust
pub fn apply(conn: &Connection, id: i64) -> Result<()> {
    let (kind, status, diff_json): (String, String, String) = conn.query_row(
        "SELECT kind, status, diff FROM proposal WHERE id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).optional()?.ok_or_else(|| anyhow!("proposal {id} not found"))?;
    if status != "pending" {
        bail!("proposal {id} is not pending (status={status})");
    }
    match kind.as_str() {
        "add_task" => {
            let args: AddTaskArgs = serde_json::from_str(&diff_json)?;
            task::insert(conn, &args.title, args.due_date.as_deref())?;
        }
        "add_maintenance_schedule" => {
            let args: AddMaintenanceScheduleArgs = serde_json::from_str(&diff_json)?;
            let draft = crate::maintenance::MaintenanceScheduleDraft {
                asset_id: args.asset_id,
                task: args.task,
                interval_months: args.interval_months,
                last_done_date: None,
                notes: args.notes,
            };
            crate::maintenance::dal::insert_schedule(conn, &draft)?;
        }
        other => bail!("proposal {id} has unsupported kind: {other}"),
    }
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE proposal SET status = 'applied', applied_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}
```

Plus a new function for edit-then-approve flow:

```rust
/// Apply an add_maintenance_schedule proposal using caller-supplied edited fields
/// instead of the diff's original values. Used by ScheduleDrawer in proposal-edit mode.
/// Returns the inserted schedule's id.
pub fn apply_add_maintenance_schedule_with_override(
    conn: &Connection,
    proposal_id: i64,
    edited: &crate::maintenance::MaintenanceScheduleDraft,
) -> Result<String> {
    let (kind, status): (String, String) = conn.query_row(
        "SELECT kind, status FROM proposal WHERE id = ?1",
        params![proposal_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional()?.ok_or_else(|| anyhow!("proposal {proposal_id} not found"))?;

    if kind != "add_maintenance_schedule" {
        bail!("proposal {proposal_id} is not an add_maintenance_schedule");
    }
    if status != "pending" {
        bail!("proposal {proposal_id} is not pending (status={status})");
    }

    let id = crate::maintenance::dal::insert_schedule(conn, edited)?;
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE proposal SET status = 'applied', applied_at = ?1 WHERE id = ?2",
        params![now, proposal_id],
    )?;
    Ok(id)
}
```

## 7. App-layer LlmClient implementations

### 7.1 `crates/app/src/pdf_extract/ollama_client.rs`

```rust
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

### 7.2 `crates/app/src/pdf_extract/claude_client.rs`

```rust
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

The `remote_call_log_id_sink` lets the pipeline capture the orchestrator's audit-log row id so it can be stored on each proposal after insert.

## 8. Pipeline (`crates/app/src/pdf_extract/pipeline.rs`)

```rust
use manor_core::pdf_extract::{
    text::{cap_for_tier, extract_text_from_pdf},
    ExtractError, ExtractedSchedule,
};
use manor_core::assistant::proposal::{self, AddMaintenanceScheduleArgs, NewProposal};
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
) -> anyhow::Result<ExtractOutcome>;

/// Test seam: caller provides the already-extracted text + LlmClient so tests
/// don't require real PDFs or a live LLM.
pub(crate) async fn run_pipeline_with_text_and_client(
    db: Arc<Mutex<rusqlite::Connection>>,
    asset_id: String,
    attachment_uuid: String,
    capped_text: String,
    tier: TierRequest,
    client: &dyn LlmClient,
    remote_call_log_id: Option<i64>,
) -> anyhow::Result<ExtractOutcome>;
```

### 8.1 `extract_and_propose` flow

1. **Resolve attachment → asset + file path** (sync lock): SELECT from `attachment` WHERE `uuid = ?1 AND deleted_at IS NULL`. Error `"Attachment not found"`. Extract `entity_id` as the asset_id. Error `"Attachment not linked to an asset"` if `entity_type != 'asset'`.
2. **Resolve asset name** (sync lock, same scope): SELECT `name` FROM `asset` WHERE `id = ?1` for Claude tier's `user_visible_reason`. Error `"Asset not found"` if missing.
3. **Build PDF path**: `attachments_dir.join(format!("{}.pdf", attachment_uuid))`. Match L4a's storage convention during implementation — if the file extension differs, read `attachment.original_name` or inspect on-disk.
4. **Extract text** via `text::extract_text_from_pdf(&path)`. ExtractError variants bubble up.
5. **Cap text** for tier via `text::cap_for_tier(&text, matches!(tier, TierRequest::Claude))`.
6. **Build LlmClient** — `OllamaExtractClient` or `ClaudeExtractClient { db.clone(), asset_name, log_id_sink }`.
7. **Extract schedules** via `llm::extract_schedules_via_llm(&capped_text, &client)`.
8. **Capture Claude log id** (Claude tier only) from the sink.
9. Call `run_pipeline_with_text_and_client` to handle the rest (see 8.2). This boundary is the test seam.

### 8.2 `run_pipeline_with_text_and_client` (test-seam entry point)

1. **Replace pending proposals** (sync lock):
   ```sql
   UPDATE proposal SET status = 'rejected'
   WHERE skill = 'pdf_extract'
     AND kind = 'add_maintenance_schedule'
     AND status = 'pending'
     AND json_extract(diff, '$.source_attachment_uuid') = ?1
   ```
   Capture `conn.changes()` as `replaced_pending_count`.
2. **Insert new proposals** (same sync lock scope): for each `ExtractedSchedule`, build `AddMaintenanceScheduleArgs`, `serde_json::to_string` → `NewProposal { kind: "add_maintenance_schedule", rationale: extracted.rationale, diff_json, skill: "pdf_extract" }` → `proposal::insert`. If Claude tier: UPDATE `proposal SET remote_call_log_id = ?1 WHERE id = ?2` post-insert.
3. **Return** `ExtractOutcome { proposals_inserted: N, replaced_pending_count }`.

### 8.3 Lock discipline

All `Mutex<Connection>` acquisitions happen inside synchronous scopes — never held across `.await`. Pattern matches L4d's pipeline.

## 9. Tauri commands (`crates/app/src/pdf_extract/commands.rs`)

```rust
#[tauri::command] pub async fn pdf_extract_ollama(attachment_uuid: String, app: AppHandle, state: State<'_, Db>) -> Result<ExtractOutcome, String>;
#[tauri::command] pub async fn pdf_extract_claude(attachment_uuid: String, app: AppHandle, state: State<'_, Db>) -> Result<ExtractOutcome, String>;
#[tauri::command] pub fn pdf_extract_pending_proposals_for_asset(asset_id: String, state: State<'_, Db>) -> Result<Vec<Proposal>, String>;
#[tauri::command] pub fn pdf_extract_pending_exists_for_attachment(attachment_uuid: String, state: State<'_, Db>) -> Result<bool, String>;
#[tauri::command] pub fn pdf_extract_approve_as_is(proposal_id: i64, state: State<'_, Db>) -> Result<(), String>;
#[tauri::command] pub fn pdf_extract_reject(proposal_id: i64, state: State<'_, Db>) -> Result<(), String>;
#[tauri::command] pub fn pdf_extract_approve_with_override(proposal_id: i64, draft: MaintenanceScheduleDraft, state: State<'_, Db>) -> Result<String, String>;
```

**Query for `pending_proposals_for_asset`:**
```sql
SELECT id, kind, rationale, diff, status, proposed_at, applied_at, skill
FROM proposal
WHERE skill = 'pdf_extract'
  AND kind = 'add_maintenance_schedule'
  AND status = 'pending'
  AND json_extract(diff, '$.asset_id') = ?1
ORDER BY proposed_at ASC
```

**Query for `pending_exists_for_attachment`:**
```sql
SELECT COUNT(*) FROM proposal
WHERE skill = 'pdf_extract'
  AND kind = 'add_maintenance_schedule'
  AND status = 'pending'
  AND json_extract(diff, '$.source_attachment_uuid') = ?1
```

Attachments directory resolution from `AppHandle`:
```rust
fn attachments_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("attachments"))
}
```

All 6 commands registered in `crates/app/src/lib.rs`'s `invoke_handler!`.

## 10. Frontend — IPC, store, UI

### 10.1 `apps/desktop/src/lib/pdf_extract/ipc.ts`

```ts
import { invoke } from "@tauri-apps/api/core";
import type { Proposal } from "../today/ipc";
import type { MaintenanceScheduleDraft } from "../maintenance/ipc";

export interface ExtractOutcome {
  proposals_inserted: number;
  replaced_pending_count: number;
}

export async function extractOllama(attachmentUuid: string): Promise<ExtractOutcome> { /* invoke */ }
export async function extractClaude(attachmentUuid: string): Promise<ExtractOutcome> { /* invoke */ }
export async function listPendingForAsset(assetId: string): Promise<Proposal[]> { /* invoke */ }
export async function pendingExistsForAttachment(attachmentUuid: string): Promise<boolean> { /* invoke */ }
export async function approveAsIs(proposalId: number): Promise<void> { /* invoke */ }
export async function reject(proposalId: number): Promise<void> { /* invoke */ }
export async function approveWithOverride(proposalId: number, draft: MaintenanceScheduleDraft): Promise<string> { /* invoke */ }
```

### 10.2 `apps/desktop/src/lib/pdf_extract/state.ts`

```ts
type ExtractStatus =
  | { kind: "idle" }
  | { kind: "extracting"; tier: "ollama" | "claude" }
  | { kind: "error"; message: string };

interface PdfExtractStore {
  proposalsByAsset: Record<string, Proposal[]>;
  pendingByAttachment: Record<string, boolean>;
  lastExtractMessage: string | null;  // success toast text
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
```

Each mutation invalidates `proposalsByAsset[assetId]` + all `pendingByAttachment[*]` entries (we don't know which attachment a proposal originated from without parsing diff; invalidating all is fine for a per-asset view). After successful extract: `lastExtractMessage` holds e.g. `"3 proposals extracted. 2 previous proposals replaced."` for the success toast.

After any mutation the L4b maintenance store's `loadForAsset(assetId)` is also triggered (so the MaintenanceSection picks up newly-created schedules).

### 10.3 `PendingProposalsBlock.tsx`

Mounted between `<MaintenanceSection />` and `<HistoryBlock />` on AssetDetail. Renders only when `proposalsByAsset[assetId]` is non-empty.

```
┌─ Proposed schedules ──────────────────────────────────────────────┐
│  Annual service · every 12 months             [✓] [✎] [×]         │
│  "Section 7.2 recommends annual service."                          │
│                                                                    │
│  Replace water filter · every 6 months        [✓] [✎] [×]         │
│  "Filter cartridges should be replaced twice yearly."              │
└────────────────────────────────────────────────────────────────────┘
```

Per row:
- Task label + interval summary (e.g. `"every 12 months"`).
- Rationale (italic, truncated to 2 lines).
- ✓ Approve button → `store.approveAsIs(proposal.id, assetId)`. Parses `diff.asset_id` to pass `assetId`.
- ✎ Edit button → opens L4b's `ScheduleDrawer` with pre-filled fields (from `diff`) + `proposalId` prop.
- × Reject button → `store.reject(proposal.id, assetId)`.

### 10.4 `ScheduleDrawer.tsx` — proposal-edit mode

Add a new optional prop:

```tsx
interface ScheduleDrawerProps {
  // existing props...
  proposalId?: number;  // NEW — when set, Save writes the schedule AND applies the proposal
}
```

When `proposalId` is a number:
- Save button label: `"Approve & add"` (instead of `"Save"`).
- Save action: `await pdfExtractIpc.approveWithOverride(proposalId, draft)` instead of `useMaintenanceStore().create(draft)`.
- Delete button hidden (can't delete a proposal from the drawer; reject is the equivalent).
- On success: close drawer + invalidate both `usePdfExtractStore.proposalsByAsset[assetId]` and `useMaintenanceStore.schedulesByAsset[assetId]`.

### 10.5 Attachment row — Extract buttons

Locate the attachment row component in AssetDetail.tsx (or wherever Documents section lives). When `attachment.mime_type === 'application/pdf'`:

- Render `Extract maintenance schedules` button → `store.extractOllama(attachment.uuid, assetId)`. Disabled while `extractStatus.kind === 'extracting'`.
- When `pendingByAttachment[attachment.uuid] === true`, also render `Re-extract with Claude` button → `store.extractClaude(attachment.uuid, assetId)`. Same disabled logic.

After any extract call, surface `lastExtractMessage` as a brief toast at the top of AssetDetail (or rely on a shared toast system if one exists in the codebase).

## 11. Error handling

Every extraction error surfaces verbatim from Rust's `ExtractError`, the pipeline's pre-flight checks, or the remote orchestrator. Frontend store transitions `extractStatus` to `{ kind: "error", message }` and surfaces it as a red band above AssetDetail's content.

| Trigger | Message |
|---|---|
| PDF file > 10 MB | `"PDF too large to extract (over 10 MB)"` |
| PDF text < 500 chars | `"PDF appears to be an image scan — text extraction isn't possible"` |
| `std::fs::read` fails | `"couldn't read PDF file: ..."` |
| `pdf_extract` crate errors | `"couldn't parse PDF: ..."` |
| Attachment uuid not found in DB | `"Attachment not found"` |
| Attachment not linked to an asset | `"Attachment not linked to an asset"` |
| Asset referenced by attachment missing | `"Asset not found"` |
| LLM returns unparseable JSON twice | `"failed to parse LLM JSON after retry: ..."` |
| Ollama unreachable | `"Local model isn't running (Ollama endpoint unreachable)"` (bubbled from `chat_collect`) |
| Claude no key / budget exceeded / network | Bubbled verbatim from orchestrator |
| LLM returns valid `[]` | Success path; UI shows toast `"No maintenance schedules found in this manual."`; no proposals inserted |

## 12. Edge cases (pinned)

| Case | Behaviour |
|---|---|
| Re-extract when no prior pending proposals exist | `replaced_pending_count = 0`. New proposals inserted as normal. |
| Same PDF attached to two assets, extract run on both | Each extraction runs independently, creating proposals against each attachment's uuid. Re-extract on one doesn't touch the other. |
| Proposal's asset_id points to a soft-deleted asset | L4e doesn't cascade — pending proposals remain orphaned. `apply` fails (FK blocks schedule insert), surfaced as `"Asset not found or trashed"`. User manually rejects. See §14 out-of-scope. |
| LLM returns a schedule with task=empty string | Dropped by post-parse filter. Doesn't reach the proposal table. |
| LLM returns interval_months=0 or 300 | Dropped by post-parse filter. |
| LLM returns 50 schedules (hallucinating) | All 50 inserted as proposals if they pass validation. User can mass-reject by clicking × on each. (Bulk-reject is v0.5.1.) |
| Two extractions run concurrently on same attachment | The pipeline's replace-then-insert is two separate SQL statements; a race could double-replace. Single-user app — not a real concern. |
| Attachment file missing on disk (DB row exists, file deleted) | `extract_text_from_pdf` returns `ExtractError::ReadFailed(...)`. |
| pdf-extract returns some text but not much (e.g. 400 chars) | Below MIN_TEXT_CHARS threshold → `ImageOnly` error. |
| User edits proposal diff then the original LLM rationale becomes stale | The rationale stays in `proposal.rationale` as historical context. Not surfaced post-apply. |
| User clicks Re-extract with Claude but no API key stored | Orchestrator returns `NoKey` error, bubbles verbatim to UI. |

## 13. Testing strategy

### 13.1 Core unit tests (`crates/core/src/pdf_extract/text.rs`)

- `extract_rejects_oversize_file` — write a 12 MB junk file, assert `ExtractError::TooLarge(10)`.
- `extract_rejects_missing_file` — non-existent path → `ExtractError::ReadFailed(...)`.
- `cap_for_tier_ollama_caps_at_32kb` — 100 KB input → ≤32 KB output.
- `cap_for_tier_claude_caps_at_200kb` — 300 KB input → ≤200 KB output.
- `cap_for_tier_respects_char_boundary` — UTF-8 multibyte at boundary → no panic.

### 13.2 Core PDF fixture tests (`crates/core/tests/pdf_extract_fixtures.rs`)

Requires two checked-in fixtures under `crates/core/tests/fixtures/`:
- `text_manual.pdf` — 1-page PDF with ≥800 chars of real text (generate once via `printpdf` crate OR commit binary fixture).
- `image_only_manual.pdf` — a scanned-style PDF with <500 chars of extractable text.

Tests:
- `extract_succeeds_on_text_pdf` — returns non-empty string ≥500 chars.
- `extract_rejects_image_only_pdf` — returns `ExtractError::ImageOnly`.

### 13.3 Core unit tests (`crates/core/src/pdf_extract/llm.rs`)

Use a local `StubLlmClient` fixture (queue of canned responses):
- `extract_schedules_parses_valid_array`
- `extract_schedules_retries_on_bad_json_then_succeeds`
- `extract_schedules_returns_empty_on_empty_array`
- `extract_schedules_filters_invalid_intervals` (interval=0 / =300 filtered out)
- `extract_schedules_filters_empty_tasks`
- `extract_schedules_errors_on_repeated_parse_failure`

### 13.4 Core tests (`crates/core/src/assistant/proposal.rs`)

- `apply_add_maintenance_schedule_inserts_schedule_and_marks_applied`
- `apply_add_maintenance_schedule_fails_on_missing_asset`
- `apply_add_maintenance_schedule_fails_if_already_applied`
- `apply_with_override_uses_edited_fields`
- `apply_with_override_rejects_wrong_kind`
- `apply_with_override_rejects_non_pending`

### 13.5 App-layer pipeline tests (`crates/app/src/pdf_extract/pipeline.rs`)

Test via the `run_pipeline_with_text_and_client` seam with a `StubLlmClient` + fresh DB (same `fresh_with_asset` helper as L4d/L4c):
- `pipeline_replaces_pending_from_same_attachment` — 2 pending for uuid-A → run extract → 2 rejected + new ones pending.
- `pipeline_does_not_touch_pending_from_other_attachments` — pending for uuid-B survives extract for uuid-A.
- `pipeline_does_not_touch_applied_or_rejected_for_same_attachment` — applied + rejected for uuid-A survive re-extract.
- `pipeline_inserts_new_proposals_with_correct_diff_shape` — 2 schedules → 2 proposal rows with correct `kind`, `skill`, and `json_extract(diff, '$.task')` matches.
- `pipeline_returns_zero_inserted_on_empty_extraction` — stub returns `[]` → `proposals_inserted: 0`.
- `pipeline_captures_remote_call_log_id_for_claude_tier` — when `remote_call_log_id` is Some, all inserted proposals have it set; None otherwise.

### 13.6 App-layer integration tests (`crates/app/src/pdf_extract/commands.rs`)

- `approve_as_is_creates_schedule_and_marks_applied`
- `reject_marks_proposal_rejected`
- `approve_with_override_uses_edited_draft`
- `list_pending_for_asset_filters_correctly` (2 pending A, 1 pending B, 1 applied A → query A returns 2 pending)
- `pending_exists_for_attachment_reflects_state`

### 13.7 Frontend (RTL)

- `PendingProposalsBlock` renders nothing when store has empty array for asset.
- `PendingProposalsBlock` renders one row per pending proposal with task + interval + rationale.
- ✓ click calls `store.approveAsIs(id, assetId)`.
- × click calls `store.reject(id, assetId)`.
- ✎ click opens `ScheduleDrawer` with `proposalId` prop set + pre-filled fields from `diff`.
- `ScheduleDrawer` with `proposalId` set: Save button labelled `"Approve & add"`; clicking calls `approveWithOverride`; Delete button hidden.
- Attachment row Extract button renders only when `mime_type === 'application/pdf'`.
- `Re-extract with Claude` button renders only when `pendingByAttachment[uuid]` is true.

## 14. Definition of done

- `pdf-extract` 0.7+ in `crates/core/Cargo.toml`, builds on macOS ARM + Linux x86_64.
- `crates/core/src/pdf_extract/*` ships with text extraction + LLM prompt/parse + fixture-based tests.
- `recipe::import::extract_json_array_block` promoted to `pub fn extract_json_array_block_public` (mirrors `extract_json_block_public`).
- `proposal::apply` and `proposal::apply_add_maintenance_schedule_with_override` handle the `add_maintenance_schedule` kind.
- Pipeline `extract_and_propose` replaces pending proposals from the same source attachment, inserts new ones, captures `remote_call_log_id` on Claude path.
- 6 Tauri commands registered via `invoke_handler!`.
- Frontend IPC + Zustand store + `PendingProposalsBlock` + `ScheduleDrawer` extension + attachment-row Extract/Re-extract buttons on AssetDetail.
- `cargo test --workspace` green. `cargo clippy --workspace -- -D warnings` clean. `pnpm tsc --noEmit` clean. `pnpm test` green. `pnpm build` succeeds.
- Manual QA scenario: attach a real boiler or washing-machine manual → Extract with Ollama → 1+ proposals within ~30 s → edit one, approve one, reject one → verify schedules appear in MaintenanceSection → Re-extract with Claude → pending replaced.

## 15. Out of scope for L4e (pinned)

- **Proposal cascade on asset lifecycle.** Pending proposals for trashed assets remain orphaned; apply fails with FK error. Fix is a follow-up that extends `soft_delete_asset` / `restore_asset` / `permanent_delete_asset` to handle proposals too.
- **Keyword-based chunking for long manuals.** 32 KB cap misses page 87 of a 120-page car manual. v0.5.1 could add "search for maintenance keywords + feed surrounding 2 KB per hit."
- **OCR for image-only PDFs.** Out of scope — error with a clear message and expect Hana to `ocrmypdf` before re-uploading.
- **Editing an already-applied proposal.** Once applied, the resulting `maintenance_schedule` is edited via L4b's normal flow. Proposals are immutable post-apply.
- **Bulk actions** (approve all / reject all). v0.5.1 at earliest.
- **Proposal expiry** / auto-cleanup of stale pending proposals. Pending proposals live forever until user-resolved.
- **Dedupe against existing schedules.** If the LLM proposes a schedule that already exists, it still appears. User rejects manually. Smart dedupe is v0.5.1.
- **Multi-asset manuals.** One extraction = one asset. To apply the same PDF to two assets, Hana re-attaches to each.
- **Warranty / recall / parts-list extraction.** Prompt is narrowly maintenance-scoped. Related features are separate v0.5.1+ work.
- **Tool-call / function-calling LLM path.** Approach 3 from the brainstorm — deferred. Straight JSON-array parse is the MVP.
- **Cross-asset proposals inbox.** Today view's `ProposalBanner` is task-scoped; extending it to include schedule proposals is a v0.5.1 consolidation.
- **Schema changes to the proposal table.** `remote_call_log_id` handled via UPDATE post-insert rather than adding a `source_attachment_uuid` column — reuse pattern preserved.

---

*End of L4e design spec. Next: implementation plan.*
