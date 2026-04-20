# L4d Right-to-Repair Lookup — Design Spec

- **Date**: 2026-04-20
- **Landmark**: v0.5 Bones → L4d (fourth sub-landmark)
- **Status**: Approved design. Implementation plan next.
- **Authors**: Hana (product), Nell (architect)
- **Roadmap**: `specs/2026-04-18-v0.5-bones-roadmap.md`
- **Depends on**: L4a Asset Registry (shipped `5645b7c`), existing Ollama plumbing (`crates/app/src/assistant/ollama.rs`), Remote LLM orchestrator (`crates/app/src/remote/`), L3a fetch patterns (`crates/app/src/recipe/importer.rs`).

## 1. Purpose

Give every asset a "Something's wrong" button. Hana types the symptom ("won't drain", "making grinding noise"). Manor queries the web, synthesises the top hits through a local LLM, and saves the answer as a `repair_note` so history is searchable later. Claude is available as an opt-in escalation when the local model can't produce a usable answer.

## 2. Scope decisions (locked during brainstorm)

| Area | Decision |
|---|---|
| **Sources** | DuckDuckGo HTML (top-3 web) + YouTube search scrape (top-2 video titles/URLs as sidecar). No iFixit or Reddit site-specific scrapers. |
| **LLM tier** | Ollama (qwen2.5:7b-instruct) runs first. If output < 50 chars or the call errors, UI transitions to "Try with Claude" button state (no auto-fallback). Otherwise shows the Ollama answer with a secondary "Try with Claude" button. |
| **Claude escalation** | Manual click only. Routes through existing `remote::orchestrator::remote_chat(skill: "right_to_repair", ...)`. Existing redaction + budget + call log apply. |
| **Storage** | New `repair_note` table with TEXT UUID keys matching L4a/L4b/L4c convention. Not an extension of the `note` table (type incompatibility between `note.entity_id: i64` and `asset.id: TEXT`). |
| **Query shape** | Asset-context auto-augmented: `"{make} {model} {symptom}"`, collapsed whitespace. Falls back to raw `symptom` if both make+model blank. The **raw** symptom is what persists in `repair_note.symptom`. |
| **History persistence** | Auto-save every successful synthesis. Failed runs (Ollama error or empty + Claude not invoked) do not persist. Trash/restore/permanent-delete via existing trash registry. |
| **Pipeline shape** | Fetch top-3 URLs → DIY readability trim (strip `script/style/nav/header/footer/aside`; extract `main/article/body`; cap ~2KB/page) → one LLM call synthesises + cites. Single LLM call per search. |
| **Tolerance** | Pipeline continues as long as ≥1 of the 3 fetches succeeds. All 3 failing → `empty_or_failed: true`, no synth, no persist. |
| **Rate limiting** | Polite scraper: User-Agent `Manor/0.4 (+https://manor.app)`, 10s timeout, 2 MB body cap. No retries. No robots.txt enforcement. |

## 3. Architecture

Two-crate split mirrors L4a/L4b/L4c.

### 3.1 New Rust files

- `crates/core/migrations/V21__repair_note.sql`
- `crates/core/src/repair/mod.rs` — module root + types (`RepairNote`, `RepairNoteDraft`, `RepairSource`, `LlmTier`).
- `crates/core/src/repair/dal.rs` — CRUD + list_for_asset + trash helpers.
- `crates/app/src/repair/mod.rs` — module root.
- `crates/app/src/repair/search.rs` — `duckduckgo_top_n` + `youtube_top_n`.
- `crates/app/src/repair/fetch.rs` — page fetch + DIY readability trim.
- `crates/app/src/repair/synth.rs` — Ollama + Claude synthesis paths, shared prompt builder.
- `crates/app/src/repair/pipeline.rs` — orchestrator (search → fetch → synth → persist).
- `crates/app/src/repair/commands.rs` — Tauri IPC.

### 3.2 New frontend files

- `apps/desktop/src/lib/repair/ipc.ts`
- `apps/desktop/src/lib/repair/state.ts` — `useRepairStore`.
- `apps/desktop/src/components/Bones/TroubleshootBlock.tsx` — composite on AssetDetail.
- `apps/desktop/src/components/Bones/TroubleshootResultCard.tsx` — transient just-searched card.
- `apps/desktop/src/components/Bones/RepairNoteCard.tsx` — one persisted history entry (collapsed + expanded).

### 3.3 Modified Rust files

- `crates/core/src/lib.rs` — `pub mod repair;`.
- `crates/core/src/trash.rs` — append `("repair_note", "symptom")` to REGISTRY.
- `crates/core/src/asset/dal.rs` — extend `soft_delete_asset`, `restore_asset`, `permanent_delete_asset` cascades (4th row).
- `crates/app/src/lib.rs` — `pub mod repair;` + register 5 Tauri commands.
- `crates/app/src/safety/trash_commands.rs` — add `"repair_note"` arms.

### 3.4 Modified frontend files

- `apps/desktop/src/components/Bones/AssetDetail.tsx` — mount `<TroubleshootBlock assetId={id} />` between `<HistoryBlock />` and `<h2>Documents</h2>`.

## 4. Schema — migration V21

```sql
-- V21__repair_note.sql
-- L4d: right-to-repair search history.

CREATE TABLE repair_note (
    id              TEXT PRIMARY KEY,
    asset_id        TEXT NOT NULL REFERENCES asset(id),
    symptom         TEXT NOT NULL,
    body_md         TEXT NOT NULL,
    sources         TEXT NOT NULL,            -- JSON array: [{"url":"...","title":"..."}]
    video_sources   TEXT,                     -- nullable JSON array; same shape
    tier            TEXT NOT NULL
                    CHECK (tier IN ('ollama','claude')),
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER
);

CREATE INDEX idx_repair_asset    ON repair_note(asset_id);
CREATE INDEX idx_repair_created  ON repair_note(created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_repair_deleted  ON repair_note(deleted_at);
```

Trash registry entry: `("repair_note", "symptom")` appended in `crates/core/src/trash.rs` after the `("maintenance_event", "title")` line from L4c.

## 5. Core types

```rust
// crates/core/src/repair/mod.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmTier {
    Ollama,
    Claude,
}

impl LlmTier {
    pub fn as_str(self) -> &'static str {
        match self {
            LlmTier::Ollama => "ollama",
            LlmTier::Claude => "claude",
        }
    }

    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "ollama" => Ok(LlmTier::Ollama),
            "claude" => Ok(LlmTier::Claude),
            other => Err(anyhow::anyhow!("unknown LlmTier: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairSource {
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairNoteDraft {
    pub asset_id: String,
    pub symptom: String,
    pub body_md: String,
    pub sources: Vec<RepairSource>,
    pub video_sources: Option<Vec<RepairSource>>,
    pub tier: LlmTier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairNote {
    pub id: String,
    pub asset_id: String,
    pub symptom: String,
    pub body_md: String,
    pub sources: Vec<RepairSource>,
    pub video_sources: Option<Vec<RepairSource>>,
    pub tier: LlmTier,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}
```

`sources` and `video_sources` serialise to JSON TEXT on disk via `serde_json::to_string`; `row_to_repair_note` parses with `serde_json::from_str`.

## 6. DAL API

`crates/core/src/repair/dal.rs`:

```rust
pub fn insert_repair_note(conn: &Connection, draft: &RepairNoteDraft) -> Result<String>;
pub fn get_repair_note(conn: &Connection, id: &str) -> Result<Option<RepairNote>>;
pub fn list_for_asset(conn: &Connection, asset_id: &str) -> Result<Vec<RepairNote>>;
pub fn soft_delete_repair_note(conn: &Connection, id: &str) -> Result<()>;
pub fn restore_repair_note(conn: &Connection, id: &str) -> Result<()>;
pub fn permanent_delete_repair_note(conn: &Connection, id: &str) -> Result<()>;
```

**Behaviour notes:**
- `list_for_asset` orders `created_at DESC`, filters `deleted_at IS NULL`.
- No `update_*` — repair notes are immutable once saved.
- JSON column serialisation in insert; deserialisation in `row_to_repair_note`.

### 6.1 Asset cascade extensions (`crates/core/src/asset/dal.rs`)

Each of the three existing functions (`soft_delete_asset`, `restore_asset`, `permanent_delete_asset` — all extended for L4c events) gains a 4th row matching the event pattern:

```rust
pub fn soft_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    conn.execute(
        "UPDATE maintenance_event SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    // L4d: cascade to repair_note.
    conn.execute(
        "UPDATE repair_note SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    conn.execute(
        "UPDATE asset SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    Ok(())
}

pub fn restore_asset(conn: &Connection, id: &str) -> Result<()> {
    // Read asset's deleted_at; restore sibling rows sharing that exact timestamp.
    // (Existing L4c cascade pattern — extend to repair_note.)
    /* ... */
    conn.execute(
        "UPDATE repair_note SET deleted_at = NULL WHERE asset_id = ?1 AND deleted_at = ?2",
        params![id, ts],
    )?;
    /* ... */
}

pub fn permanent_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    // Attachments soft-delete (existing).
    // Events hard-delete (L4c).
    // L4d: repair_notes hard-delete BEFORE schedule cascade (FK ordering).
    conn.execute(
        "DELETE FROM repair_note WHERE asset_id = ?1",
        params![id],
    )?;
    // Schedules hard-delete (L4b).
    // Asset hard-delete (L4b).
}
```

FK-order for permanent delete: attachment soft-delete → event hard-delete → **repair_note hard-delete (new)** → schedule hard-delete → asset hard-delete.

## 7. Search module

`crates/app/src/repair/search.rs`:

```rust
pub async fn duckduckgo_top_n(client: &reqwest::Client, query: &str, n: usize) -> Result<Vec<RepairSource>>;
pub async fn youtube_top_n(client: &reqwest::Client, query: &str, n: usize) -> Result<Vec<RepairSource>>;
```

**DuckDuckGo:** GET `https://html.duckduckgo.com/html/?q=<urlencoded_query>`. Parse via `scraper::Html`. Select `.result__title a` anchors. For each, extract `href` + inner text. Dedupe by `href`. Take first N. Empty results or parse mismatch → `Ok(vec![])`. HTTP failure → `Err`.

**YouTube:** GET `https://www.youtube.com/results?search_query=<urlencoded_query>`. YouTube embeds video metadata in a `ytInitialData` JSON blob inside a `<script>` tag. Parse via regex `var ytInitialData = (\{...\});` (the blob is large but regex with non-greedy `.*?` + the closing brace pattern works — or use the `scraper` crate to extract the script text and then parse the JSON). Walk the JSON to `contents.twoColumnSearchResultsRenderer.primaryContents.sectionListRenderer.contents[0].itemSectionRenderer.contents[i].videoRenderer`, extract `videoId` + `title.runs[0].text`. Build `{url: "https://www.youtube.com/watch?v=<videoId>", title}`. Take first N. If parsing fails at any step → `Ok(vec![])` (YouTube is a nice-to-have sidecar; graceful degradation).

Both use the shared `reqwest::Client` created in `pipeline.rs` (10 s timeout, Manor UA).

## 8. Fetch + readability

`crates/app/src/repair/fetch.rs`:

```rust
pub const MAX_BODY_BYTES: u64 = 2 * 1024 * 1024;
pub const FETCH_TIMEOUT_SECS: u64 = 10;
pub const TRIMMED_TEXT_CAP_BYTES: usize = 2 * 1024;

pub async fn fetch_and_trim(client: &reqwest::Client, url: &str) -> Result<String>;
```

Flow:
1. `client.get(url).send()`. Timeout comes from the client builder.
2. Check `Content-Type` header contains `text/html`; otherwise `Err(NotHtml(ctype))`.
3. Check `Content-Length` if present; if > `MAX_BODY_BYTES`, `Err(TooLarge)`.
4. `resp.text()`.
5. `scraper::Html::parse_document(&body)`.
6. Remove subtrees for selectors: `script, style, nav, header, footer, aside, noscript, form, iframe, svg`.
7. Extract text from first match of `main, article, [role="main"]`. Fall back to `body` if none match.
8. Collapse whitespace: multiple `\s+` → single space. Trim leading/trailing.
9. Truncate to `TRIMMED_TEXT_CAP_BYTES` bytes on a byte boundary (ASCII-safe since we collapse whitespace; for non-ASCII, use `char_indices` to find a safe cut-point near the limit).
10. Return the trimmed string.

Error enum:
```rust
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("couldn't reach {0}")]
    FetchFailed(String),
    #[error("{0} isn't html (content-type: {1})")]
    NotHtml(String, String),
    #[error("{0} is too large")]
    TooLarge(String),
    #[error("parse failed for {0}")]
    ParseFailed(String),
}
```

The pipeline treats each URL's fetch as independent — partial failures are tolerated.

## 9. Synthesis

`crates/app/src/repair/synth.rs`:

```rust
pub struct SynthInput<'a> {
    pub asset_name: &'a str,
    pub asset_make: Option<&'a str>,
    pub asset_model: Option<&'a str>,
    pub asset_category: &'a str,
    pub symptom: &'a str,
    pub augmented_query: &'a str,
    pub pages: &'a [PageExcerpt],
}

pub struct PageExcerpt {
    pub url: String,
    pub title: String,
    pub trimmed_text: String,
}

pub fn build_user_prompt(input: &SynthInput<'_>) -> String;

pub async fn synth_via_ollama(input: &SynthInput<'_>) -> Result<String>;

pub async fn synth_via_claude(
    db: std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
    input: &SynthInput<'_>,
) -> Result<String>;
```

### 9.1 Prompt (shared between tiers)

System prompt:
```
You are a concise home-repair troubleshooter. You help a homeowner diagnose
appliance, vehicle, and fixture problems using search-result excerpts.
```

User prompt template (output of `build_user_prompt`):
```
You are helping a homeowner troubleshoot an appliance or fixture problem.

## About the item
- Name: {asset_name}
- Make: {make_or_unknown}
- Model: {model_or_unknown}
- Category: {asset_category}

## Reported symptom
{symptom}

## Search results (trimmed excerpts from the top {N} pages)
[Source 1 — {url1}]
{trimmed_text_1}

[Source 2 — {url2}]
{trimmed_text_2}

[Source 3 — {url3}]
{trimmed_text_3}

## Your task
Synthesise a concise troubleshooting summary (150–300 words).

Requirements:
- Start with the most likely cause in plain language.
- List 2–4 specific things the user can check or try, in order.
- Flag any "call a professional" cases (gas, high voltage, sealed systems).
- At the end, list the source URLs as a Markdown bulleted list under "## Sources".
- Do NOT invent model-specific steps that aren't in the excerpts.
- If the excerpts are thin or off-topic, say so and suggest a more specific search.
```

`{N}` reflects actual page count (may be 1 or 2 if fetches partially failed).

### 9.2 Ollama path

Uses `crate::assistant::ollama::chat` (non-streaming variant; add if only streaming exists). Model: `qwen2.5:7b-instruct` (the module's DEFAULT_MODEL). `max_tokens`: 800. Returns the response text verbatim. Timeout handled by Ollama's endpoint / the HTTP client.

### 9.3 Claude path

Wraps the existing orchestrator:

```rust
let req = crate::remote::orchestrator::RemoteChatRequest {
    skill: "right_to_repair",
    user_visible_reason: &format!("Troubleshooting {}", input.asset_name),
    system_prompt: Some("You are a concise home-repair troubleshooter..."),
    user_prompt: &build_user_prompt(input),
    max_tokens: 1024,
};
let outcome = crate::remote::orchestrator::remote_chat(db, req).await?;
Ok(outcome.text)
```

Redaction (PII scrubbing), keychain lookup, budget check, and call-log entry are all automatic — the orchestrator owns that. Budget-exceeded / no-key / network errors bubble up as distinct `RemoteChatError` variants which the pipeline surfaces to the UI.

## 10. Pipeline

`crates/app/src/repair/pipeline.rs`:

```rust
pub enum TierRequest {
    Ollama,
    Claude,
}

pub struct PipelineOutcome {
    pub note: Option<manor_core::repair::RepairNote>,
    pub sources: Vec<manor_core::repair::RepairSource>,
    pub video_sources: Vec<manor_core::repair::RepairSource>,
    pub empty_or_failed: bool,
}

pub async fn run_repair_search(
    db: std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
    asset_id: String,
    symptom: String,
    tier: TierRequest,
) -> anyhow::Result<PipelineOutcome>;

pub fn build_augmented_query(
    make: Option<&str>,
    model: Option<&str>,
    symptom: &str,
) -> String;
```

### 10.1 `build_augmented_query` (pure helper, unit-testable)

- If both `make` and `model` are `Some(non_empty)`: return `"{make} {model} {symptom}"` with collapsed whitespace.
- If only one is `Some`: return `"{that_field} {symptom}"`.
- If both are `None` or empty: return `symptom` verbatim.

### 10.2 `run_repair_search` flow

1. **Load asset.** `asset::dal::get_asset(&conn, &asset_id)` inside a scoped lock. Error `"Asset not found"` if `None`. Drop the lock before any `.await`.
2. **Build augmented query.** `build_augmented_query(asset.make.as_deref(), asset.model.as_deref(), &symptom)`.
3. **Build shared HTTP client.** `reqwest::Client::builder().timeout(10s).user_agent("Manor/0.4 (+https://manor.app)").build()`.
4. **Search.** `tokio::join!(duckduckgo_top_n(&client, &q, 3), youtube_top_n(&client, &q, 2))`. DDG result is the critical path; YouTube failure → empty vec.
5. **Fetch.** Concurrent fetch of the 3 DDG URLs with `futures::future::join_all`. Collect successes into `Vec<PageExcerpt>`. If all 3 fail, return `PipelineOutcome { note: None, sources: ddg_results, video_sources: youtube_results, empty_or_failed: true }`.
6. **Synthesise.**
   - `TierRequest::Ollama` → `synth_via_ollama(&input).await`. If `Err` OR `text.len() < 50`, return `PipelineOutcome { note: None, sources, video_sources, empty_or_failed: true }`.
   - `TierRequest::Claude` → `synth_via_claude(db.clone(), &input).await`. Errors propagate (distinct messages for no-key / budget / network).
7. **Persist.** Build `RepairNoteDraft { asset_id, symptom (raw), body_md: synth_text, sources: ddg_results, video_sources: Some(youtube_results).filter(|v| !v.is_empty()), tier }`. Call `repair::dal::insert_repair_note` inside a scoped lock. Load the fresh row via `get_repair_note`.
8. **Return** `PipelineOutcome { note: Some(persisted), sources, video_sources: yt, empty_or_failed: false }`.

### 10.3 Locking discipline

The `Arc<Mutex<Connection>>` is held only inside synchronous scopes — never across `.await`. Pattern:

```rust
let asset = {
    let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
    asset::dal::get_asset(&conn, &asset_id)?
        .ok_or_else(|| anyhow!("Asset not found"))?
};
// ... async work (fetch, synth) ...
let persisted = {
    let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
    let id = repair::dal::insert_repair_note(&conn, &draft)?;
    repair::dal::get_repair_note(&conn, &id)?.expect("inserted row must exist")
};
```

## 11. Tauri commands

`crates/app/src/repair/commands.rs`:

```rust
#[tauri::command]
pub async fn repair_search_ollama(
    asset_id: String,
    symptom: String,
    state: State<'_, Db>,
) -> Result<PipelineOutcome, String>;

#[tauri::command]
pub async fn repair_search_claude(
    asset_id: String,
    symptom: String,
    state: State<'_, Db>,
) -> Result<PipelineOutcome, String>;

#[tauri::command]
pub fn repair_note_list_for_asset(
    asset_id: String,
    state: State<'_, Db>,
) -> Result<Vec<RepairNote>, String>;

#[tauri::command]
pub fn repair_note_get(id: String, state: State<'_, Db>) -> Result<Option<RepairNote>, String>;

#[tauri::command]
pub fn repair_note_delete(id: String, state: State<'_, Db>) -> Result<(), String>;
```

Sync commands use `state.0.lock()` directly (matching L4c `event_commands.rs`). Async commands pass `state.0.clone()` (an `Arc` clone) into `run_repair_search`. If the app's `Db` type is currently `pub struct Db(pub Mutex<Connection>)` without an `Arc`, wrap it at construction or refactor — the refactor is a small one-line change in `lib.rs` state setup and is part of this task.

All 5 commands registered in `crates/app/src/lib.rs` `tauri::generate_handler!`.

Trash commands (`crates/app/src/safety/trash_commands.rs`) gain `"repair_note"` arms in both `trash_restore` and `trash_permanent_delete` match blocks.

## 12. Frontend IPC (`apps/desktop/src/lib/repair/ipc.ts`)

```ts
import { invoke } from "@tauri-apps/api/core";

export type LlmTier = "ollama" | "claude";

export interface RepairSource { url: string; title: string; }

export interface RepairNote {
  id: string;
  asset_id: string;
  symptom: string;
  body_md: string;
  sources: RepairSource[];
  video_sources: RepairSource[] | null;
  tier: LlmTier;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface PipelineOutcome {
  note: RepairNote | null;
  sources: RepairSource[];
  video_sources: RepairSource[];
  empty_or_failed: boolean;
}

export async function searchOllama(assetId: string, symptom: string): Promise<PipelineOutcome> { /* invoke */ }
export async function searchClaude(assetId: string, symptom: string): Promise<PipelineOutcome> { /* invoke */ }
export async function listForAsset(assetId: string): Promise<RepairNote[]> { /* invoke */ }
export async function get(id: string): Promise<RepairNote | null> { /* invoke */ }
export async function deleteNote(id: string): Promise<void> { /* invoke */ }
```

## 13. Zustand store (`apps/desktop/src/lib/repair/state.ts`)

```ts
type SearchStatus =
  | { kind: "idle" }
  | { kind: "searching"; tier: LlmTier }
  | { kind: "error"; message: string };

interface RepairStore {
  notesByAsset: Record<string, RepairNote[]>;
  lastOutcomeByAsset: Record<string, PipelineOutcome | null>;
  searchStatus: SearchStatus;

  loadForAsset(assetId: string): Promise<void>;
  invalidateAsset(assetId: string): void;
  searchOllama(assetId: string, symptom: string): Promise<PipelineOutcome>;
  searchClaude(assetId: string, symptom: string): Promise<PipelineOutcome>;
  deleteNote(id: string, assetId: string): Promise<void>;
  clearLastOutcome(assetId: string): void;
}
```

After a successful search, store invalidates `notesByAsset[assetId]` so the history list refreshes on the next `loadForAsset`. The transient `lastOutcomeByAsset` holds the just-searched result so the result card can render independently of the persisted history.

## 14. UI — `TroubleshootBlock`

Rendered between `HistoryBlock` (L4c) and `<h2>Documents</h2>` on AssetDetail.

Structure:
1. **Header** — `<h3>Troubleshoot</h3>` + italic subtitle "Search the web and summarise — uses your local model first."
2. **Search form** — text input (placeholder `"What's wrong? e.g., won't drain, making grinding noise"`, maxLength 200 characters, whitespace-trimmed on submit), `Search` button. Enter-key submits.
3. **Result card** (`TroubleshootResultCard`) — conditional on `lastOutcomeByAsset[assetId]`.
4. **History list** — `notesByAsset[assetId]` rendered as collapsed `RepairNoteCard` rows, excluding the note that's in the active result card.

Loading/error states:
- `searchStatus.kind === "searching"` → Search button disabled, muted label `"Asking qwen2.5…"` (Ollama) or `"Asking Claude…"`.
- `searchStatus.kind === "error"` → red error band above the input.

## 15. UI — `TroubleshootResultCard`

Three visual modes keyed on `PipelineOutcome`:

### Mode A — Success (Ollama or Claude)

`outcome.note !== null && outcome.empty_or_failed === false`.

Layout:
- Symptom header (the raw query).
- Rendered markdown `body_md` (react-markdown or existing Manor renderer; see §16).
- Tier chip (`local` / `claude`) + relative timestamp.
- Source list — each source a clickable link (title + host), opens via `@tauri-apps/plugin-shell` `open()`.
- Video sidecar (if `video_sources` non-empty) — up to 2 "Watch on YouTube" links with the external-link icon.
- Footer actions:
  - `Try with Claude` button — rendered only if `outcome.note.tier === "ollama"`. Clicking calls `store.searchClaude(assetId, outcome.note.symptom)`.
  - `Close` — clears `lastOutcomeByAsset[assetId]` via `store.clearLastOutcome(assetId)`. Note remains in history.

### Mode B — Empty/failed Ollama

`outcome.empty_or_failed === true` AND last tier attempt was Ollama.

Layout:
- Banner: "The local model didn't return a usable answer for this one."
- Source list — the 3 DDG URLs (fetched successfully, just not synthesised).
- Prominent `Try with Claude` button (routes to `store.searchClaude(assetId, lastSymptom)`).
- `Dismiss` button.
- No markdown body; no persisted `repair_note`.

### Mode C — Claude hard-failure

A prior Claude attempt threw — surfaced via `searchStatus.kind === "error"` and kept in a local state for the card.

Layout:
- Error message verbatim from orchestrator (e.g. `"Remote Claude not configured — no API key stored"`, `"Remote budget exceeded this month (500p of 500p)"`, `"Claude service unreachable"`).
- Retry button for network errors; no retry for budget-exceeded.
- `Dismiss`.

### Symptom passthrough on "Try with Claude"

The button uses the **raw symptom** from the current outcome (if Mode A) or the last submitted form value (if Mode B). Not the augmented query — that's a backend concern.

## 16. UI — `RepairNoteCard` (history row)

Collapsed (default):
- Calendar icon + relative date ("2 days ago", "3 weeks ago").
- Truncated symptom (~60 chars).
- Tier chip.
- Trash button.

Expanded (on row click):
- Full markdown `body_md`.
- Source list + optional video sidecar.
- Delete button (soft-delete → trash).

Click the row again to collapse.

## 17. Markdown rendering

Check `apps/desktop/src/components/` for an existing markdown component during implementation (recipes/notes may already have one). If none:
- `pnpm add react-markdown remark-gfm`
- Create a small `<RepairMarkdown body={string}>` wrapper with GFM support.
- All `<a>` elements get an `onClick` that calls `@tauri-apps/plugin-shell`'s `open()` to launch the external browser, preventing in-webview navigation.

Verify `@tauri-apps/plugin-shell` is present in `apps/desktop/package.json`. L3a's recipe-source link handling likely already uses it; reuse that pattern.

## 18. Error handling

**Backend-side messages the UI surfaces verbatim:**

| Trigger | Message |
|---|---|
| `asset_id` not found / trashed | `"Asset not found"` |
| All 3 DDG fetches fail | `"Couldn't reach the search sources — check your internet connection"` |
| DDG HTTP failure (500, timeout) | `"Search failed — try again in a moment"` |
| Ollama unreachable | `"Local model isn't running (Ollama endpoint unreachable)"` |
| Ollama returned empty/short | *no error* — UI enters Mode B automatically |
| Claude: no API key | `"Remote Claude not configured — no API key stored"` |
| Claude: budget exceeded | `"Remote budget exceeded this month ({spent}p of {cap}p)"` |
| Claude: network failure | `"Claude service unreachable"` |

Each Tauri command maps the Rust-side error enum into one of these strings. The orchestrator already produces distinct `RemoteChatError` variants for the last three — the command layer maps them verbatim.

**Frontend behaviour on error:**
- `searchStatus` transitions to `{ kind: "error", message }`.
- `lastOutcomeByAsset` is NOT cleared — if a Mode A card is visible, it stays. The error renders above the search input as a small red band.
- Re-running the search (Ollama or Claude) clears the error and returns `searchStatus` to `"searching"` then `"idle"`.

## 19. Edge cases (pinned)

| Case | Behaviour |
|---|---|
| Asset has no make + no model + no category | Pipeline uses raw `symptom` as query. Prompt renders make/model as "unknown". |
| Symptom is empty string | Pipeline returns `Err("symptom required")` — UI blocks form submit when trimmed input is empty. |
| Symptom contains URL or code | Sent verbatim to DDG. LLM prompt includes it verbatim. Redaction (in `remote_chat`) strips emails/phone/addresses on the Claude path. |
| Asset soft-deleted with repair_notes → restored | Notes come back with the same `deleted_at` timestamp match (L4c cascade pattern). |
| Asset permanent-deleted with notes | Notes hard-deleted BEFORE schedule cascade (FK-safe ordering). |
| Ollama installed but model not pulled | `synth_via_ollama` returns error ("model not found"). UI enters Mode B ("try Claude"). |
| Network offline | DDG fetch fails immediately. Pipeline returns `empty_or_failed: true` with zero sources. UI renders a generic "couldn't reach sources" message. |
| LLM produces output without the "## Sources" section | Persist anyway. The body is still useful; source URLs are in the separate `sources` column regardless. |
| LLM produces output that exceeds 1000 chars | Persist verbatim. No truncation on the backend. UI may visually clip the expanded card but the full text is retrievable. |
| Duplicate search (same symptom twice) | Each run creates a new `repair_note`. No dedupe — running the same query later may produce different results as web pages change. |

## 20. Testing strategy

### 20.1 Core DAL tests (`crates/core/src/repair/dal.rs`)

- Migration V21 creates `repair_note` + 3 indexes (mirror L4c V20 migration-tests module pattern in `maintenance/mod.rs`).
- `insert_repair_note` round-trip — all fields persist; `sources` JSON round-trips through `serde_json::{to,from}_string`; `video_sources = None` persists as SQL NULL and round-trips back.
- `list_for_asset` orders by `created_at DESC`, excludes soft-deleted.
- Soft-delete / restore / permanent-delete round-trips.
- Trash registry test generalises to cover `repair_note`.

### 20.2 Asset cascade tests (`crates/core/src/asset/dal.rs`)

Extending the L4c cascade tests:
- `soft_delete_asset_cascades_repair_notes` — soft-delete asset with 1+ notes → notes disappear from `list_for_asset` but exist with `deleted_at` set.
- `restore_asset_restores_repair_notes_from_same_cascade` — timestamp-scoped restore works; notes trashed at an earlier timestamp stay trashed.
- `permanent_delete_asset_hard_deletes_repair_notes` — notes hard-deleted; assert `COUNT(*) FROM repair_note WHERE asset_id = ?1` is 0 post-purge.

### 20.3 Fetch/readability tests (`crates/app/src/repair/fetch.rs`)

Use `wiremock` (or `httpmock` if already in the workspace — check `Cargo.toml`; add whichever the existing integration tests already use):
- `fetch_and_trim_strips_nav_script_style` — HTML with nav/script/style nodes → trimmed text excludes their content.
- `fetch_and_trim_prefers_main_over_body` — HTML with both `<main>` and other content → only main's text.
- `fetch_and_trim_caps_at_2kb` — large `<main>` → truncated to ~2KB.
- `fetch_and_trim_rejects_non_html_content_type` — `Content-Type: application/json` → `Err(NotHtml)`.
- `fetch_and_trim_rejects_oversized_body` — `Content-Length > 2MB` → `Err(TooLarge)`.

### 20.4 Search module tests (`crates/app/src/repair/search.rs`)

- `duckduckgo_parses_result_titles_and_hrefs` — canned DDG HTML fixture with 5 results → top 3 parsed correctly (title + href).
- `duckduckgo_returns_empty_on_no_results` — DDG "No Results" page → `Ok(vec![])`.
- `youtube_parses_ytInitialData` — canned YouTube HTML with embedded JSON → top N `{videoId → url, title}`.
- `youtube_returns_empty_on_unparseable_html` — HTML without `ytInitialData` → `Ok(vec![])`.

### 20.5 Synth tests (`crates/app/src/repair/synth.rs`)

Pure function tests only (live LLM calls covered by manual QA):
- `build_user_prompt_includes_all_fields` — all asset fields + symptom + all page excerpts appear verbatim.
- `build_user_prompt_handles_missing_make_model` — `None` values render as "unknown".
- `build_user_prompt_handles_partial_pages` — 1 page / 2 pages / 3 pages all format correctly.

### 20.6 Pipeline tests (`crates/app/src/repair/pipeline.rs`)

- `build_augmented_query` — cases: make + model + symptom; make only; model only; neither. All collapse whitespace correctly.
- `pipeline_persists_on_successful_synth` — wiremock for DDG/YouTube/fetch; stub synth via trait seam returns `Ok("a body long enough".into())` → `repair_note` row exists post-call.
- `pipeline_no_persist_on_empty_synth` — stub returns `""` → no row, `empty_or_failed: true`.
- `pipeline_no_persist_on_synth_error` — stub returns `Err(...)` → no row, `empty_or_failed: true`.
- `pipeline_tolerates_2_of_3_fetch_failures` — wiremock responds 200 for one URL, 500 for two → synth called with one excerpt; persist succeeds.
- `pipeline_bails_when_all_fetches_fail` — all wiremock 500s → `empty_or_failed: true`, synth never called.

**Trait seam for stubbing synth:** in `synth.rs`, define

```rust
#[async_trait::async_trait]
pub trait SynthBackend: Send + Sync {
    async fn synth(&self, input: &SynthInput<'_>) -> anyhow::Result<String>;
}
```

with `OllamaSynth` and `ClaudeSynth(Arc<Mutex<Connection>>)` impls. Pipeline takes `&dyn SynthBackend`. Tests pass a `StubSynth { response: Result<String, String> }`. No `mockall` needed.

### 20.7 Integration tests (`crates/app/src/repair/commands_tests.rs`)

- `repair_note_list_for_asset_round_trip` — insert via DAL directly, list via command, assert one row.
- `repair_note_delete_soft_deletes` — insert + delete command + list returns empty; `get_repair_note` returns Some with `deleted_at`.

### 20.8 Frontend (RTL)

- `TroubleshootBlock` renders input + empty history state.
- `TroubleshootBlock` disables Search button while `searchStatus` is searching.
- `TroubleshootBlock` blocks submit when symptom is empty/whitespace-only.
- `TroubleshootBlock` enforces 200-char maxLength.
- `TroubleshootResultCard` Mode A — renders markdown body + source list + `Try with Claude` iff tier === ollama.
- `TroubleshootResultCard` Mode B — "try Claude" prompt with sources visible.
- `TroubleshootResultCard` Mode C — surfaces orchestrator error text.
- `RepairNoteCard` collapsed → expanded round-trip via click.
- Mocks `useRepairStore` (pattern from L4c Task 12).

## 21. Definition of done

- Migration V21 runs cleanly on fresh + existing dev DBs.
- `repair_note` CRUD Tauri commands round-trip.
- End-to-end pipeline works on a real asset in dev: DDG + YouTube scrape, top-3 fetch with DIY readability, Ollama synth, persist.
- Ollama empty/error path transitions to "Try with Claude" button state; clicking routes through `remote::orchestrator::remote_chat` with skill `"right_to_repair"`.
- Markdown renders on history + result cards; external links open via Tauri shell plugin.
- History list renders persisted notes collapsed; click expands; delete moves to trash.
- Asset cascade: soft-delete hides notes, restore brings them back (timestamp-scoped), permanent-delete removes them (FK-order-safe).
- Trash sweep includes `repair_note`; restore + permanent-delete match arms extended.
- `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `pnpm tsc --noEmit`, `pnpm test`, `pnpm build` all green.
- Manual QA: search a real appliance ("Bosch dishwasher won't drain"); verify Ollama answer; click "Try with Claude"; verify escalation works; delete one note; restore from Trash.

## 22. Out of scope for L4d (pinned)

- iFixit / RepairClinic / Reddit site-specific scrapers. DuckDuckGo + YouTube only.
- YouTube transcripts — sidecar is titles + URLs only.
- YouTube Data API — HTML scraping path only.
- Robots.txt enforcement — single-user app, polite-scraper posture only.
- Pagination of history list (revisit if >20 notes per asset proves real).
- Editing a saved `repair_note` — notes are immutable; delete + re-search instead.
- Re-running a prior search from a history row — only the transient result card offers the tier toggle.
- Storing fetched page bodies for offline reference — only the synth + source URLs persist.
- Suggesting related symptoms — LLM synth is the only assist.
- Feeding right-to-repair results into maintenance-schedule proposals (that's L4e territory for PDF-driven proposals).
- Caching DDG/YouTube responses or individual page fetches — every search refetches.
- Remote-provider switching beyond Claude — orchestrator is Claude-only.
- Live unit tests against real DDG/YouTube — fixtures only; manual QA covers the wire.

---

*End of L4d design spec. Next: implementation plan.*
