# L4b Maintenance Schedules — Design Spec

- **Date**: 2026-04-19
- **Landmark**: v0.5 Bones → L4b (second sub-landmark)
- **Status**: Approved design. Implementation plan next.
- **Authors**: Hana (product), Nell (architect)
- **Roadmap**: `specs/2026-04-18-v0.5-bones-roadmap.md`
- **Depends on**: L4a Asset Registry (shipped at `5645b7c`).

## 1. Purpose

Answer the question "what maintenance is due in my house?" Manor gains a per-asset schedule list (with inline add/edit/mark-done) plus a cross-asset "Due soon" overview that groups schedules into Overdue / Due this week / Upcoming (30 days). Overdue items surface on the Today view (summary band) and on Rhythm (virtual render, no table duplication). Mark-done bumps the schedule's dates — no event log yet; L4c adds that layer.

L4b is the MVP daily-worry answer: "Is my boiler service due? How many things am I behind on?"

## 2. Scope decisions (locked during brainstorm)

| Area | Decision |
|---|---|
| **Interval kinds** | Time-only for MVP. `interval_months: i32`. Usage-based (km, hours) deferred to v0.5.1. |
| **Mark done mechanics** | Bump columns on the schedule: `last_done_date = today`, `next_due_date = today + interval_months`. No event log yet. L4c will add `maintenance_event` and can backfill from schedules. |
| **Schedule creation entry points** | Both asset-detail inline AND Due-soon `+ New schedule` drawer. Same drawer, different prefill. |
| **Bones sub-nav** | Introduced in L4b. Tabs: `Assets · Due soon`. Persisted via `setting(bones.last_subview)`. Default `"assets"`. |
| **Due soon grouping** | 3 bands: Overdue / Due this week (next 7d) / Upcoming (next 30d). Empty bands hide. Far-future schedules (>30d) don't appear. |
| **Today surface** | Single "N maintenance items overdue" summary band. Hides at count=0. Dismissible via `setting(bones.show_maintenance_band)` default `true`. |
| **Rhythm surface** | Virtual render — Rhythm view pulls overdue + due-today maintenance items via a new Tauri command and renders inline with a `Wrench` icon. No duplicate rows in `chore` table. |
| **`next_due_date` storage** | Denormalised. Recomputed on insert/update/mark_done. Cheap band queries; one SQL column lookup. |
| **Never-done schedules** | `last_done_date = NULL`; `next_due_date = created_at_date + interval_months` (policy A). |
| **Asset ↔ schedule link** | `maintenance_schedule.asset_id NOT NULL REFERENCES asset(id)` — every schedule hangs off exactly one asset. No FK cascade; L4a's `permanent_delete_asset` extended to cascade-soft-delete schedules. |

## 3. Architecture

Two-crate split, same pattern as L3a/L3b/L3c/L3d/L4a.

### 3.1 New Rust files

- `crates/core/src/maintenance/mod.rs` — types (`MaintenanceSchedule`, `MaintenanceScheduleDraft`, `DueBand`).
- `crates/core/src/maintenance/dal.rs` — CRUD + `list_for_asset` + `list_due_before` + `list_due_today_and_overdue` + `mark_done`.
- `crates/core/src/maintenance/due.rs` — pure `compute_next_due` + `classify` functions.
- `crates/app/src/maintenance/mod.rs` — module root.
- `crates/app/src/maintenance/commands.rs` — Tauri IPC.

### 3.2 New frontend files

- `apps/desktop/src/lib/maintenance/ipc.ts` — IPC wrappers + types.
- `apps/desktop/src/lib/maintenance/state.ts` — Zustand store.
- `apps/desktop/src/lib/bones/view-state.ts` — sub-nav state store (mirror of `lib/hearth/view-state.ts`).
- `apps/desktop/src/components/Bones/BonesSubNav.tsx` — two-tab header.
- `apps/desktop/src/components/Bones/AssetsView.tsx` — extracted from current `BonesTab.tsx` body.
- `apps/desktop/src/components/Bones/DueSoon/DueSoonView.tsx` — banded list.
- `apps/desktop/src/components/Bones/DueSoon/ScheduleRow.tsx` — row component used by both DueSoon + AssetDetail's Maintenance section.
- `apps/desktop/src/components/Bones/DueSoon/ScheduleDrawer.tsx` — create/edit drawer.
- `apps/desktop/src/components/Bones/MaintenanceSection.tsx` — "Maintenance" block inside AssetDetail.
- `apps/desktop/src/components/Today/MaintenanceOverdueBand.tsx` — Today-view summary band.

### 3.3 Modified files

- `crates/core/src/lib.rs` — `pub mod maintenance;`.
- `crates/core/src/trash.rs` — append `("maintenance_schedule", "task")` to `REGISTRY`.
- `crates/core/src/asset/dal.rs` — extend `permanent_delete_asset` to also soft-delete linked `maintenance_schedule` rows.
- `crates/app/src/lib.rs` — register new Tauri commands, `pub mod maintenance;`.
- `crates/app/src/safety/trash_commands.rs` — add `"maintenance_schedule"` arms to `trash_restore` + `trash_permanent_delete` match blocks (following the same pattern used for asset + recipe + staple_item).
- `apps/desktop/src/components/Bones/BonesTab.tsx` — becomes a thin sub-view router.
- `apps/desktop/src/components/Bones/AssetDetail.tsx` — new Maintenance section inserted between Notes and Documents.
- `apps/desktop/src/components/Today/Today.tsx` — mount `<MaintenanceOverdueBand />` between `<TonightBand />` and the first existing card.
- `apps/desktop/src/components/TimeBlocks/` OR `apps/desktop/src/components/Chores/` (whichever is the Rhythm view) — extend data source to pull maintenance items.

## 4. Schema — migration V19

```sql
-- V19__maintenance_schedule.sql
-- L4b: maintenance_schedule table per asset. Time-only intervals for MVP.

CREATE TABLE maintenance_schedule (
    id              TEXT PRIMARY KEY,
    asset_id        TEXT NOT NULL REFERENCES asset(id),
    task            TEXT NOT NULL,
    interval_months INTEGER NOT NULL CHECK (interval_months >= 1),
    last_done_date  TEXT,
    next_due_date   TEXT NOT NULL,
    notes           TEXT NOT NULL DEFAULT '',
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER
);

CREATE INDEX idx_maint_asset    ON maintenance_schedule(asset_id);
CREATE INDEX idx_maint_deleted  ON maintenance_schedule(deleted_at);
CREATE INDEX idx_maint_next_due ON maintenance_schedule(next_due_date) WHERE deleted_at IS NULL;
```

Timestamps: seconds since epoch (matching the rest of Manor).

Trash sweep: append `("maintenance_schedule", "task")` to `REGISTRY` in `crates/core/src/trash.rs`.

## 5. Types (core)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceScheduleDraft {
    pub asset_id: String,
    pub task: String,
    pub interval_months: i32,
    pub last_done_date: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceSchedule {
    pub id: String,
    pub asset_id: String,
    pub task: String,
    pub interval_months: i32,
    pub last_done_date: Option<String>,
    pub next_due_date: String,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DueBand {
    Overdue,
    DueThisWeek,
    Upcoming,
    Far,
}
```

Commands returning Rhythm + DueSoon rows join against `asset` for display. A view-model type:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleWithAsset {
    pub schedule: MaintenanceSchedule,
    pub asset_name: String,
    pub asset_category: String,   // asset_category.as_str() value
}
```

## 6. Due-date computation

`crates/core/src/maintenance/due.rs`:

```rust
use super::DueBand;
use anyhow::Result;
use chrono::{Months, NaiveDate};

/// Compute next_due_date given last_done (or None → use fallback_start).
/// Fallback is the asset's schedule creation date, per policy A (§2).
pub fn compute_next_due(
    last_done_date: Option<&str>,
    interval_months: i32,
    fallback_start: &str,
) -> Result<String> {
    let anchor = last_done_date.unwrap_or(fallback_start);
    let parsed = NaiveDate::parse_from_str(anchor, "%Y-%m-%d")?;
    let next = parsed.checked_add_months(Months::new(interval_months as u32))
        .ok_or_else(|| anyhow::anyhow!("date overflow adding {} months", interval_months))?;
    Ok(next.format("%Y-%m-%d").to_string())
}

pub fn classify(next_due_date: &str, today: &str) -> Result<DueBand> {
    let due = NaiveDate::parse_from_str(next_due_date, "%Y-%m-%d")?;
    let today = NaiveDate::parse_from_str(today, "%Y-%m-%d")?;
    let days = (due - today).num_days();
    Ok(match days {
        n if n <= 0 => DueBand::Overdue,
        n if n <= 7 => DueBand::DueThisWeek,
        n if n <= 30 => DueBand::Upcoming,
        _ => DueBand::Far,
    })
}
```

Tests cover the 8 cases listed in §2 of the prior brainstorm (month-end edge, never-done fallback, overdue-today, etc).

## 7. DAL API

`crates/core/src/maintenance/dal.rs`:

```rust
pub fn insert_schedule(conn: &Connection, draft: &MaintenanceScheduleDraft) -> Result<String>;
pub fn get_schedule(conn: &Connection, id: &str) -> Result<Option<MaintenanceSchedule>>;
pub fn list_for_asset(conn: &Connection, asset_id: &str) -> Result<Vec<MaintenanceSchedule>>;
pub fn list_due_before(conn: &Connection, cutoff_date: &str) -> Result<Vec<MaintenanceSchedule>>;
pub fn list_due_today_and_overdue(conn: &Connection, today: &str) -> Result<Vec<MaintenanceSchedule>>;
pub fn update_schedule(conn: &Connection, id: &str, draft: &MaintenanceScheduleDraft) -> Result<()>;
pub fn mark_done(conn: &Connection, id: &str, today: &str) -> Result<()>;
pub fn soft_delete_schedule(conn: &Connection, id: &str) -> Result<()>;
pub fn restore_schedule(conn: &Connection, id: &str) -> Result<()>;
pub fn permanent_delete_schedule(conn: &Connection, id: &str) -> Result<()>;  // called by trash_permanent_delete
```

**Algorithm notes:**

- `insert_schedule`: compute `next_due_date` using `compute_next_due(draft.last_done_date.as_deref(), draft.interval_months, today)` where `today` is passed in or computed from `chrono::Local::now().date_naive()`.
- `update_schedule`: same recomputation of `next_due_date`.
- `mark_done`: compute `next_due_date = compute_next_due(Some(today), interval_months, today)`; update both columns.
- `list_for_asset`: `WHERE asset_id = ?1 AND deleted_at IS NULL ORDER BY next_due_date ASC`.
- `list_due_before(cutoff)`: `WHERE next_due_date <= ?1 AND deleted_at IS NULL ORDER BY next_due_date ASC`. Used by Due soon view (cutoff = today + 30 days).
- `list_due_today_and_overdue(today)`: `WHERE next_due_date <= ?1 AND deleted_at IS NULL ORDER BY next_due_date ASC`. Used by Rhythm.

### 7.1 Extension to `asset::dal::permanent_delete_asset`

Modify the existing function (shipped in L4a post-review fix) to also soft-delete linked schedules:

```rust
pub fn permanent_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    // Soft-delete linked attachments (L4a).
    conn.execute(
        "UPDATE attachment SET deleted_at = ?1 WHERE entity_type = 'asset' AND entity_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    // Soft-delete linked maintenance schedules (L4b addition).
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    // Hard-delete the asset (only if trashed).
    conn.execute(
        "DELETE FROM asset WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
}
```

## 8. Tauri commands

`crates/app/src/maintenance/commands.rs`:

```rust
#[tauri::command] pub fn maintenance_schedule_list_for_asset(asset_id: String, state: State<'_, Db>) -> Result<Vec<MaintenanceSchedule>, String>;
#[tauri::command] pub fn maintenance_schedule_get(id: String, state: State<'_, Db>) -> Result<Option<MaintenanceSchedule>, String>;
#[tauri::command] pub fn maintenance_schedule_create(draft: MaintenanceScheduleDraft, state: State<'_, Db>) -> Result<String, String>;
#[tauri::command] pub fn maintenance_schedule_update(id: String, draft: MaintenanceScheduleDraft, state: State<'_, Db>) -> Result<(), String>;
#[tauri::command] pub fn maintenance_schedule_mark_done(id: String, state: State<'_, Db>) -> Result<(), String>;
#[tauri::command] pub fn maintenance_schedule_delete(id: String, state: State<'_, Db>) -> Result<(), String>;    // soft-delete
#[tauri::command] pub fn maintenance_schedule_restore(id: String, state: State<'_, Db>) -> Result<(), String>;
#[tauri::command] pub fn maintenance_due_soon(state: State<'_, Db>) -> Result<Vec<ScheduleWithAsset>, String>;   // today + 30 days
#[tauri::command] pub fn maintenance_due_today_and_overdue(state: State<'_, Db>) -> Result<Vec<ScheduleWithAsset>, String>;  // for Rhythm
#[tauri::command] pub fn maintenance_overdue_count(state: State<'_, Db>) -> Result<i64, String>;  // for Today band
```

`ScheduleWithAsset` joins `asset.name` and `asset.category` server-side so frontend doesn't need a second fetch per row.

## 9. UI

### 9.1 Bones sub-nav

`BonesTab.tsx` refactors into a thin router (mirror of Hearth's L3b refactor):

```tsx
export function BonesTab() {
  const { subview, hydrate, hydrated } = useBonesViewStore();
  useEffect(() => { void hydrate(); }, [hydrate]);
  if (!hydrated) return null;
  return (
    <div style={{ padding: 32, maxWidth: 1200, margin: "0 auto" }}>
      <BonesSubNav />
      {subview === "assets"   && <AssetsView />}
      {subview === "due_soon" && <DueSoonView />}
    </div>
  );
}
```

`AssetsView` = the existing `BonesTab.tsx` body extracted verbatim, no behavioural change.

`BonesSubNav.tsx`: two tabs `Assets` and `Due soon`, underline-on-active styling, mirrors `HearthSubNav.tsx` chrome.

`useBonesViewStore`: mirror of `useHearthViewStore`. `{ subview, hydrated, hydrate(), setSubview(v), pendingAssetDetailId, openAssetDetail(id), clearPendingDetail() }`. The `openAssetDetail` plumbing lets DueSoonView's row tap navigate to an asset detail by flipping `subview` + setting the pending id which `AssetsView` picks up.

### 9.2 Due soon view

3 banded sections: Overdue / Due this week / Upcoming. Each band renders a `<h2>` + count badge, then list of `ScheduleRow` cards. Sections with 0 items don't render.

```tsx
interface DueSoonViewProps {}

export function DueSoonView() {
  // Fetch via useMaintenanceStore().loadDueSoon() on mount.
  // Render 3 sections filtered by classify(schedule.next_due_date, today).
  // + New schedule button at the bottom opens ScheduleDrawer with no prefilled assetId.
}
```

Empty state (all 3 bands empty): `"Nothing due in the next 30 days. Everything in order."` with a `+ New schedule` button.

### 9.3 `ScheduleRow` component

Shared between DueSoon + MaintenanceSection. Props let the caller opt-out of the "asset name" line (redundant on the asset detail view):

```tsx
interface ScheduleRowProps {
  schedule: MaintenanceSchedule;
  assetName?: string;         // undefined → skip rendering the asset name line
  assetCategory?: string;
  onMarkDone: () => void;
  onEdit: () => void;
  onDelete?: () => void;     // undefined on the DueSoon view; present on asset detail
}
```

Row layout: `Wrench` icon + task + (asset name line — only if prop set) + relative-date string + action buttons.

Relative-date helper: `formatRelativeDue(next_due_date: string, today: string) -> string` — returns "14 days overdue" / "due today" / "due in 4 days" / "due in 3 weeks" (pluralisation handled).

Overdue rows get a subtle red left-border or red date text (pick one per Flat-Notion design; decide during implementation).

### 9.4 `ScheduleDrawer` (create + edit)

Right-side drawer. Fields per §3.4 of the brainstorm:

- Asset: `<select>` populated from `asset_list()`. Required. If `initialDraft.asset_id` is set + `lockAsset=true` prop, the select is disabled (create-from-asset-detail path).
- Task: text input. Required.
- Interval months: number input, min 1.
- Last done date: `<input type="date">`. Optional.
- Notes: markdown textarea.

Save → `maintenance_schedule_create` or `..._update` → drawer closes → current view reloads via store.

### 9.5 `MaintenanceSection` (inside AssetDetail)

Inserted between Notes and Documents. Renders a list of `ScheduleRow`s (with `assetName` omitted). `+ Add schedule` button at the bottom opens the drawer with `assetId` prefilled and locked.

Per-row overflow menu: 3-dot → Edit (open drawer) + Delete (soft-delete, recoverable from Trash).

### 9.6 Today's `MaintenanceOverdueBand`

Fetches `maintenance_overdue_count()` on mount. If count > 0 AND `setting(bones.show_maintenance_band) != "false"`:

```tsx
<div style={bandStyle}>
  <Wrench size={18} strokeWidth={1.6} color="var(--ink-soft, #999)" />
  <span style={{ flex: 1 }}>{count} maintenance item{count === 1 ? "" : "s"} overdue</span>
  <button onClick={() => {
    setView("bones");
    useBonesViewStore.getState().setSubview("due_soon");
  }}>View →</button>
</div>
```

Otherwise renders null.

### 9.7 Rhythm integration

Inspect the existing Rhythm view (likely `apps/desktop/src/components/Chores/*` or `apps/desktop/src/components/TimeBlocks/*` — check during implementation). Extend its data source:

```ts
const maintenanceItems = await maintenanceIpc.dueTodayAndOverdue();
```

Merge into the existing chores list with a `kind: "maintenance"` discriminator on each row. Render with `Wrench` icon prefix + a "maintenance" label chip.

Checking off a maintenance row → `maintenance_schedule_mark_done(id)` → reload.

## 10. Zustand stores

### `apps/desktop/src/lib/maintenance/state.ts`

```ts
type LoadStatus = { kind: "idle" } | { kind: "loading" } | { kind: "error"; message: string };

interface MaintenanceStore {
  schedulesByAsset: Record<string, MaintenanceSchedule[]>;
  dueSoon: ScheduleWithAsset[];
  overdueCount: number;

  loadStatus: LoadStatus;

  loadDueSoon(): Promise<void>;
  loadOverdueCount(): Promise<void>;
  loadForAsset(assetId: string): Promise<void>;
  create(draft: MaintenanceScheduleDraft): Promise<string>;
  update(id: string, draft: MaintenanceScheduleDraft): Promise<void>;
  markDone(id: string): Promise<void>;
  deleteSchedule(id: string): Promise<void>;
}
```

### `apps/desktop/src/lib/bones/view-state.ts`

Same shape as `useHearthViewStore`. Mirror it.

## 11. Error handling

Per §4 of brainstorm. Inline error states in drawers; toasts for background failures; Retry buttons where appropriate.

## 12. Testing strategy

### 12.1 Core unit tests

- `due.rs`: 8 cases covering compute_next_due (never-done fallback, month-end edge, multi-month) + classify (overdue, due today, due this week, upcoming, far).
- `dal.rs`:
  - insert + get + list round-trip; `next_due_date` correctly populated for never-done (uses fallback_start).
  - update recomputes next_due when interval or last_done changes.
  - mark_done sets both `last_done_date = today` and `next_due_date = today + interval`.
  - list_for_asset excludes trashed; ordered by next_due ASC.
  - list_due_before returns schedules in window, sorted.
  - list_due_today_and_overdue returns only items where next_due <= today.
  - soft-delete + restore round-trip.
  - permanent_delete_schedule hard-deletes only trashed rows.
- `asset::dal::permanent_delete_asset` extension: verify it cascade-soft-deletes linked schedules (add test alongside the existing attachment-cascade test).
- `trash::REGISTRY` includes `("maintenance_schedule", "task")`; existing trash sweep test generalised.

### 12.2 Integration tests (`crates/app`)

- `maintenance_schedule_create` → `list_for_asset` round-trip.
- `maintenance_schedule_mark_done` → both dates updated, subsequent `list_due_today_and_overdue` excludes it.
- `maintenance_due_soon` returns asset_name + category joined correctly.
- `maintenance_overdue_count` returns accurate count.

### 12.3 Frontend

RTL tests:
- `DueSoonView` renders 3 bands with correct counts (mocked store).
- Empty state renders.
- Schedule drawer submits valid inputs, rejects invalid (interval < 1).
- `MaintenanceOverdueBand` hides at count=0, renders at >0.
- `BonesSubNav` tab-switch round-trips via `setting(bones.last_subview)`.

## 13. Out of scope for L4b (pinned)

- `maintenance_event` table + history log — L4c.
- Cost tracking + Ledger linkage — L4c.
- Usage-based intervals (km, hours) — v0.5.1.
- Auto-schedule generation from PDF manuals — L4e.
- Auto-proposing schedules from Ledger transactions ("paid British Gas £180 — log boiler service?") — deferred.
- Notifications / macOS banners for overdue items.
- Skip / snooze beyond manually editing `last_done_date`.
- Bulk mark-done.
- Per-completion assignee tracking.
- Complex recurrence patterns ("2nd Tuesday of each month"). Only simple month-based intervals.
- Schedules that span multiple assets ("HVAC system" as a parent of several filters).

## 14. Definition of done

- Migration V19 runs cleanly on fresh + existing dev DBs.
- Bones sub-nav (`Assets · Due soon`) renders; last_subview persists across sessions.
- Due soon view: 3 bands with urgency colouring, empty-state messaging, `+ New schedule` drawer.
- Asset detail: Maintenance section with inline list + `+ Add schedule`.
- `ScheduleDrawer`: creates with prefilled asset (from asset detail) or asset-picker (from Due soon).
- Mark done updates both columns, row leaves current band.
- Today `MaintenanceOverdueBand` renders when count > 0, dismissible via setting.
- Rhythm view shows overdue + due-today maintenance items inline with Wrench icon; mark-done works from Rhythm.
- Soft-delete + restore via Trash round-trip. Permanent-delete-asset cascades schedules.
- Trash-commands asset-match-block extended to `"maintenance_schedule"`.
- `cargo test --workspace` green. Clippy clean. TypeScript clean. Production build green.
- Manual QA: add a boiler + 3 schedules with different intervals, mark some done, verify they move between bands + appear/disappear on Today + Rhythm.

---

*End of L4b design spec. Next: implementation plan.*
