# L4c Maintenance Events + Ledger Link — Design Spec

- **Date**: 2026-04-20
- **Landmark**: v0.5 Bones → L4c (third sub-landmark)
- **Status**: Approved design. Implementation plan next.
- **Authors**: Hana (product), Nell (architect)
- **Roadmap**: `specs/2026-04-18-v0.5-bones-roadmap.md`
- **Depends on**: L4a Asset Registry (shipped at `5645b7c`) + L4b Maintenance Schedules (shipped at `a22b362`).

## 1. Purpose

Close the money loop on Bones. Every mark-done becomes an event row. Events carry an optional cost and an optional link to a Ledger transaction. Users gain per-asset history, per-asset totals, a cross-asset Bones **Spend** tab, and a Ledger-transaction picker surfaced on the Log completion drawer.

L4c answers: "What's happened to this boiler? How much did we spend on the house this year?"

## 2. Scope decisions (locked during brainstorm)

| Area | Decision |
|---|---|
| **Scope** | All five roadmap bullets in a single slice — schema, mark-done captures event, per-asset history, cost rollups (per asset + per category), Ledger transaction suggest-link on event creation. |
| **Mark-done UX** | Two paths. Silent one-click `Mark done` (on Rhythm, Today, ScheduleRow) inserts a minimal event (cost NULL, notes '', transaction_id NULL). A separate `Log completion…` action opens a drawer for rich capture. |
| **One-off events** | Allowed. `schedule_id` is nullable. Entry point: new **History** block on AssetDetail with a `+ Log work` button. Schedule rows still get their own `Log completion…` action for schedule-linked events. |
| **Ledger match suggestions** | When cost is entered, top 3 transactions in `[completed_date − 7d, completed_date + 2d]` ordered by `ABS(amount_pence + cost_pence)`. When cost is blank, top 5 in the same date window ordered by date DESC. "None of these — search all…" fallback filters by description/merchant LIKE. |
| **Rollup placement** | AssetDetail gains a spend strip (12m + lifetime). Bones gets a third sub-nav tab: `Assets · Due soon · Spend` showing category totals + per-asset sortable list. |
| **Backfill** | Migration V20 synthesises one event per non-trashed `maintenance_schedule` row with `last_done_date NOT NULL`. Synthesised rows have `cost_pence = NULL`, `source = 'backfill'`, and render with a muted "backfill" pill until enriched. |
| **Mutability** | Events are editable (cost, notes, completed_date, transaction_id) but not user-deletable. Soft-delete only happens via asset-trash cascade. |
| **Cascade** | Schedule trashed → events stay (`schedule_id` orphans tolerated). Asset soft-deleted → events soft-deleted with it (same timestamp). Asset restored → events restored. Asset permanent-deleted → events hard-deleted. |
| **Linkage shape** | Inline `cost_pence` + inline `transaction_id` on the event row. One event ↔ 0-or-1 transaction, enforced by a partial unique index. No junction table (YAGNI). |
| **Rollup windows** | 12-month (`completed_date ≥ today − 365d`) + lifetime. No 30-day window. |
| **Currency** | GBP only. Multi-currency deferred to v1.x. |

## 3. Architecture

Two-crate split mirrors L3a/L3b/L3c/L3d/L4a/L4b.

### 3.1 New Rust files

- `crates/core/migrations/V20__maintenance_event.sql`
- `crates/core/src/maintenance/event.rs` — types (`MaintenanceEvent`, `MaintenanceEventDraft`, `EventSource`, `EventWithContext`, `AssetSpendTotal`, `CategorySpendTotal`).
- `crates/core/src/maintenance/event_dal.rs` — event CRUD + rollup queries + transaction suggest/search.
- `crates/app/src/maintenance/event_commands.rs` — Tauri IPC.

### 3.2 New frontend files

- `apps/desktop/src/lib/maintenance/event-ipc.ts`
- `apps/desktop/src/lib/maintenance/event-state.ts` — `useMaintenanceEventsStore`.
- `apps/desktop/src/lib/maintenance/spend-state.ts` — `useSpendStore`.
- `apps/desktop/src/components/Bones/HistoryBlock.tsx`
- `apps/desktop/src/components/Bones/LogCompletionDrawer.tsx` — shared drawer for three entry modes.
- `apps/desktop/src/components/Bones/EventRow.tsx`
- `apps/desktop/src/components/Bones/TransactionSuggest.tsx`
- `apps/desktop/src/components/Bones/AssetSpendStrip.tsx`
- `apps/desktop/src/components/Bones/Spend/SpendView.tsx`
- `apps/desktop/src/components/Bones/Spend/SpendAssetRow.tsx`
- `apps/desktop/src/components/Bones/Spend/SpendCategoryStrip.tsx`

### 3.3 Modified Rust files

- `crates/core/src/maintenance/mod.rs` — `pub mod event; pub mod event_dal;`.
- `crates/core/src/maintenance/dal.rs` — `mark_done` grows an optional `event_draft: Option<&MaintenanceEventDraft>` parameter and is now responsible for inserting an event row as well as bumping `last_done_date` + `next_due_date`.
- `crates/core/src/asset/dal.rs` — `soft_delete_asset`, `restore_asset`, and `permanent_delete_asset` cascade to `maintenance_event`.
- `crates/core/src/trash.rs` — append `("maintenance_event", "title")` to `REGISTRY`.
- `crates/app/src/maintenance/mod.rs` — `pub mod event_commands;`.
- `crates/app/src/maintenance/commands.rs` — `maintenance_schedule_mark_done` continues to exist; internally calls the extended `mark_done(.., None)`.
- `crates/app/src/lib.rs` — register new Tauri commands.
- `crates/app/src/safety/trash_commands.rs` — add `"maintenance_event"` arms to `trash_restore` + `trash_permanent_delete` match blocks.

### 3.4 Modified frontend files

- `apps/desktop/src/lib/bones/view-state.ts` — widen `subview` union to `"assets" | "due_soon" | "spend"`.
- `apps/desktop/src/components/Bones/BonesSubNav.tsx` — third tab (`Spend`).
- `apps/desktop/src/components/Bones/BonesTab.tsx` — render `<SpendView />` on `subview === "spend"`.
- `apps/desktop/src/components/Bones/AssetDetail.tsx` — mount `<AssetSpendStrip />` near the top + `<HistoryBlock />` after the Maintenance section.
- `apps/desktop/src/components/Bones/DueSoon/ScheduleRow.tsx` — existing silent mark-done unchanged; overflow menu gains `Log completion…` that opens the drawer in schedule-completion mode.
- `apps/desktop/src/components/Bones/MaintenanceSection.tsx` — same overflow-menu addition on schedule rows.
- `apps/desktop/src/components/Today/MaintenanceOverdueBand.tsx` — no code change required; the silent path continues to work because the Rust side now transparently writes an event when `mark_done` is called.
- Rhythm view (`apps/desktop/src/components/Chores/*` or equivalent — locate during implementation) — no change; same rationale.

## 4. Schema — migration V20

```sql
-- V20__maintenance_event.sql
-- L4c: maintenance event log + Ledger transaction linkage.

CREATE TABLE maintenance_event (
    id              TEXT PRIMARY KEY,
    asset_id        TEXT NOT NULL REFERENCES asset(id),
    schedule_id     TEXT REFERENCES maintenance_schedule(id),   -- nullable: one-off work
    title           TEXT NOT NULL DEFAULT '',
    completed_date  TEXT NOT NULL,                              -- YYYY-MM-DD
    cost_pence      INTEGER,                                    -- nullable
    currency        TEXT NOT NULL DEFAULT 'GBP',
    notes           TEXT NOT NULL DEFAULT '',
    transaction_id  INTEGER REFERENCES ledger_transaction(id),  -- nullable
    source          TEXT NOT NULL DEFAULT 'manual'
                    CHECK (source IN ('manual','backfill')),
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER
);

CREATE INDEX idx_evt_asset     ON maintenance_event(asset_id);
CREATE INDEX idx_evt_schedule  ON maintenance_event(schedule_id) WHERE schedule_id IS NOT NULL;
CREATE INDEX idx_evt_completed ON maintenance_event(completed_date) WHERE deleted_at IS NULL;
CREATE INDEX idx_evt_deleted   ON maintenance_event(deleted_at);

-- One transaction can be linked to at most one non-trashed event.
CREATE UNIQUE INDEX idx_evt_tx_unique
    ON maintenance_event(transaction_id)
    WHERE transaction_id IS NOT NULL AND deleted_at IS NULL;

-- Backfill: one synthetic event per schedule with a recorded completion.
-- Idempotent — the NOT EXISTS guard protects against re-runs.
INSERT INTO maintenance_event (
    id, asset_id, schedule_id, title, completed_date,
    cost_pence, currency, notes, transaction_id, source,
    created_at, updated_at, deleted_at
)
SELECT
    lower(hex(randomblob(16))),
    ms.asset_id,
    ms.id,
    ms.task,
    ms.last_done_date,
    NULL, 'GBP', '', NULL, 'backfill',
    unixepoch(), unixepoch(), NULL
FROM maintenance_schedule ms
WHERE ms.last_done_date IS NOT NULL
  AND ms.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1 FROM maintenance_event me
      WHERE me.schedule_id = ms.id AND me.source = 'backfill'
  );
```

Trash registry append (in `crates/core/src/trash.rs`): `("maintenance_event", "title")`.

## 5. Core types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource { Manual, Backfill }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceEventDraft {
    pub asset_id: String,
    pub schedule_id: Option<String>,     // None = one-off
    pub title: String,
    pub completed_date: String,          // YYYY-MM-DD
    pub cost_pence: Option<i64>,
    pub currency: String,                // default "GBP"
    pub notes: String,
    pub transaction_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceEvent {
    pub id: String,
    pub asset_id: String,
    pub schedule_id: Option<String>,
    pub title: String,
    pub completed_date: String,
    pub cost_pence: Option<i64>,
    pub currency: String,
    pub notes: String,
    pub transaction_id: Option<i64>,
    pub source: EventSource,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventWithContext {
    pub event: MaintenanceEvent,
    pub schedule_task: Option<String>,           // populated from schedule regardless of deleted_at;
                                                 // None only if schedule_id is None or row was hard-deleted
    pub schedule_deleted: bool,                  // true if the schedule_id row has deleted_at IS NOT NULL
    pub transaction_description: Option<String>, // None if transaction_id is None or tx is soft-deleted
    pub transaction_amount_pence: Option<i64>,
    pub transaction_date: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSpendTotal {
    pub asset_id: String,
    pub asset_name: String,
    pub asset_category: String,
    pub total_last_12m_pence: i64,
    pub total_lifetime_pence: i64,
    pub event_count_last_12m: i64,
    pub event_count_lifetime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySpendTotal {
    pub category: String,  // 'appliance' | 'vehicle' | 'fixture' | 'other'
    pub total_last_12m_pence: i64,
    pub total_lifetime_pence: i64,
}
```

## 6. DAL API

### 6.1 `event_dal.rs`

```rust
pub fn insert_event(conn: &Connection, draft: &MaintenanceEventDraft) -> Result<String>;
pub fn get_event(conn: &Connection, id: &str) -> Result<Option<MaintenanceEvent>>;
pub fn update_event(conn: &Connection, id: &str, draft: &MaintenanceEventDraft) -> Result<()>;

// No user-facing delete_event. Soft-delete only happens via asset cascade.

pub fn list_for_asset(conn: &Connection, asset_id: &str) -> Result<Vec<EventWithContext>>;

pub fn asset_spend_totals(conn: &Connection, today: &str) -> Result<Vec<AssetSpendTotal>>;
pub fn asset_spend_for_asset(conn: &Connection, asset_id: &str, today: &str) -> Result<AssetSpendTotal>;
pub fn category_spend_totals(conn: &Connection, today: &str) -> Result<Vec<CategorySpendTotal>>;

pub fn suggest_transactions(
    conn: &Connection,
    completed_date: &str,
    cost_pence: Option<i64>,
    exclude_event_id: Option<&str>,
    limit: usize,
) -> Result<Vec<Transaction>>;

pub fn search_transactions(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<Transaction>>;
```

**Validation rules inside `insert_event` / `update_event`:**
- `cost_pence >= 0` or None. Reject negative.
- `completed_date` parses as `YYYY-MM-DD`. Reject otherwise.
- If `schedule_id` is Some, the referenced schedule's `asset_id` must equal `draft.asset_id`. Reject mismatch.
- Constraint violation on `idx_evt_tx_unique` returns `anyhow!("Transaction already linked to another event")`.

### 6.2 `mark_done` (extended in `dal.rs`)

```rust
pub fn mark_done(
    conn: &Connection,
    schedule_id: &str,
    today: &str,
    event_draft: Option<&MaintenanceEventDraft>,
) -> Result<String>  // returns the inserted event id
```

- Loads the schedule row. Returns `anyhow!("Schedule not found")` if missing or soft-deleted.
- Bumps `last_done_date = today`, `next_due_date = compute_next_due(Some(today), interval_months, today)`.
- Inserts a `maintenance_event`. If `event_draft` is None, builds a minimal draft internally:
  - `asset_id = schedule.asset_id`
  - `schedule_id = Some(schedule.id)`
  - `title = schedule.task`
  - `completed_date = today`
  - `cost_pence = None`
  - `currency = "GBP"`
  - `notes = ""`
  - `transaction_id = None`
  - `source = Manual` (implicit via default).
- If `event_draft` is Some, uses it as-is (caller has already set `asset_id` and `schedule_id`).
- Returns the event id.

### 6.3 Rollup SQL

**`asset_spend_totals`** returns one row per non-trashed asset, zeros included:

```sql
WITH cutoff AS (SELECT date(?1, '-365 days') AS d365)
SELECT
    a.id          AS asset_id,
    a.name        AS asset_name,
    a.category    AS asset_category,
    COALESCE(SUM(CASE WHEN e.completed_date >= (SELECT d365 FROM cutoff)
                       AND e.cost_pence IS NOT NULL
                       AND e.deleted_at IS NULL
                  THEN e.cost_pence END), 0) AS total_last_12m_pence,
    COALESCE(SUM(CASE WHEN e.cost_pence IS NOT NULL
                       AND e.deleted_at IS NULL
                  THEN e.cost_pence END), 0) AS total_lifetime_pence,
    COALESCE(SUM(CASE WHEN e.completed_date >= (SELECT d365 FROM cutoff)
                       AND e.deleted_at IS NULL
                  THEN 1 END), 0) AS event_count_last_12m,
    COALESCE(SUM(CASE WHEN e.deleted_at IS NULL
                  THEN 1 END), 0) AS event_count_lifetime
FROM asset a
LEFT JOIN maintenance_event e ON e.asset_id = a.id
WHERE a.deleted_at IS NULL
GROUP BY a.id
ORDER BY total_last_12m_pence DESC, a.name COLLATE NOCASE ASC;
```

Counts include events with NULL cost (backfill + silent mark-done). Cost sums exclude them.

**`category_spend_totals`** groups through asset so trashed-asset events don't leak:

```sql
WITH cutoff AS (SELECT date(?1, '-365 days') AS d365)
SELECT
    a.category AS category,
    COALESCE(SUM(CASE WHEN e.completed_date >= (SELECT d365 FROM cutoff)
                       AND e.cost_pence IS NOT NULL
                       AND e.deleted_at IS NULL
                  THEN e.cost_pence END), 0) AS total_last_12m_pence,
    COALESCE(SUM(CASE WHEN e.cost_pence IS NOT NULL
                       AND e.deleted_at IS NULL
                  THEN e.cost_pence END), 0) AS total_lifetime_pence
FROM asset a
LEFT JOIN maintenance_event e ON e.asset_id = a.id
WHERE a.deleted_at IS NULL
GROUP BY a.category;
```

Returns ≤4 rows. Frontend pads zeros for categories the SQL didn't emit.

**`asset_spend_for_asset`** — same query with `WHERE a.id = ?`, returns one row; Err on not-found.

### 6.4 Ledger candidate query

```sql
-- When cost_pence is Some.
SELECT lt.*
FROM ledger_transaction lt
LEFT JOIN maintenance_event me
    ON me.transaction_id = lt.id AND me.deleted_at IS NULL
WHERE lt.deleted_at IS NULL
  AND (me.id IS NULL OR me.id = ?exclude_event_id)
  AND date(lt.date, 'unixepoch') BETWEEN date(?completed_date, '-7 days')
                                     AND date(?completed_date, '+2 days')
ORDER BY ABS(lt.amount_pence + ?cost_pence) ASC
LIMIT 3;
```

When `cost_pence` is None: skip the `ABS` ordering, use `ORDER BY lt.date DESC LIMIT 5`. `exclude_event_id` is Some only when editing an existing event (lets that event's current transaction stay visible in the list).

Spends are stored as negative pence in `ledger_transaction.amount_pence`; cost is positive; `ABS(amount_pence + cost_pence) ≈ 0` for a matching outflow.

### 6.5 Asset cascade (modifications to `crates/core/src/asset/dal.rs`)

```rust
pub fn soft_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    // existing schedule cascade from L4b
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    // new: event cascade — same timestamp lets restore reverse it
    conn.execute(
        "UPDATE maintenance_event SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    conn.execute(
        "UPDATE asset SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    Ok(())
}

pub fn restore_asset(conn: &Connection, id: &str) -> Result<()> {
    // Find the cascade timestamp from the asset row, restore siblings sharing it.
    let deleted_at: Option<i64> = conn.query_row(
        "SELECT deleted_at FROM asset WHERE id = ?1", params![id], |r| r.get(0),
    )?;
    let Some(ts) = deleted_at else { return Ok(()); };
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = NULL WHERE asset_id = ?1 AND deleted_at = ?2",
        params![id, ts],
    )?;
    conn.execute(
        "UPDATE maintenance_event SET deleted_at = NULL WHERE asset_id = ?1 AND deleted_at = ?2",
        params![id, ts],
    )?;
    conn.execute(
        "UPDATE asset SET deleted_at = NULL WHERE id = ?1", params![id],
    )?;
    Ok(())
}

pub fn permanent_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    conn.execute(
        "UPDATE attachment SET deleted_at = ?1 WHERE entity_type = 'asset' AND entity_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    // New: hard-delete events before schedules so FK references don't choke.
    conn.execute(
        "DELETE FROM maintenance_event WHERE asset_id = ?1",
        params![id],
    )?;
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    conn.execute(
        "DELETE FROM asset WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
}
```

Note: schedule soft-delete does **not** cascade to events — events survive an orphaned schedule by design.

## 7. Tauri commands

`crates/app/src/maintenance/event_commands.rs`:

```rust
#[tauri::command] pub fn maintenance_event_list_for_asset(asset_id: String, state: State<'_, Db>) -> Result<Vec<EventWithContext>, String>;
#[tauri::command] pub fn maintenance_event_get(id: String, state: State<'_, Db>) -> Result<Option<MaintenanceEvent>, String>;
#[tauri::command] pub fn maintenance_event_create_oneoff(draft: MaintenanceEventDraft, state: State<'_, Db>) -> Result<String, String>;
#[tauri::command] pub fn maintenance_event_log_completion(schedule_id: String, draft: MaintenanceEventDraft, state: State<'_, Db>) -> Result<String, String>;
#[tauri::command] pub fn maintenance_event_update(id: String, draft: MaintenanceEventDraft, state: State<'_, Db>) -> Result<(), String>;

#[tauri::command] pub fn maintenance_spend_asset_totals(state: State<'_, Db>) -> Result<Vec<AssetSpendTotal>, String>;
#[tauri::command] pub fn maintenance_spend_for_asset(asset_id: String, state: State<'_, Db>) -> Result<AssetSpendTotal, String>;
#[tauri::command] pub fn maintenance_spend_category_totals(state: State<'_, Db>) -> Result<Vec<CategorySpendTotal>, String>;

#[tauri::command] pub fn maintenance_suggest_transactions(
    completed_date: String,
    cost_pence: Option<i64>,
    exclude_event_id: Option<String>,
    state: State<'_, Db>,
) -> Result<Vec<Transaction>, String>;

#[tauri::command] pub fn maintenance_search_transactions(query: String, state: State<'_, Db>) -> Result<Vec<Transaction>, String>;
```

Existing `maintenance_schedule_mark_done` stays — internally calls `dal::mark_done(.., None)` for the silent path.

`maintenance_event_log_completion` calls `dal::mark_done(.., Some(&draft))`.

## 8. UI — shared `LogCompletionDrawer`

One component, three modes:

| Mode | Entry | Prefill |
|---|---|---|
| **One-off** | AssetDetail History block `+ Log work` | `asset_id` locked; `schedule_id = null`; user types `title`. |
| **Schedule completion** | `Log completion…` in a ScheduleRow overflow menu (DueSoon + MaintenanceSection) | `asset_id` + `schedule_id` locked; `title = schedule.task` but user can override via a tiny "✎ edit" affordance. |
| **Edit existing** | Pencil icon on EventRow | All fields populated from the event; `asset_id`/`schedule_id` locked (can't re-parent). |

Fields:
- **Title** — text, required. Schedule-completion mode shows title locked by default with ✎ affordance.
- **Completed date** — `<input type="date">`, defaults to today.
- **Cost** — currency input (pence under the hood, £ with 2dp on display). Blank = NULL.
- **Link transaction** — collapsible section containing `<TransactionSuggest />`.
- **Notes** — markdown textarea, optional.

Save semantics:
- One-off → `maintenance_event_create_oneoff(draft)`.
- Schedule completion → `maintenance_event_log_completion(schedule_id, draft)`.
- Edit → `maintenance_event_update(id, draft)`.

After save: drawer closes; store refreshes (events for asset, asset totals, Bones Spend data if mounted).

## 9. UI — `TransactionSuggest`

```tsx
interface Props {
  completedDate: string;           // YYYY-MM-DD
  costPence: number | null;
  selectedTransactionId: number | null;
  excludeEventId: string | null;   // for edit mode
  onSelect(txId: number | null, tx: Transaction | null): void;
}
```

Behavior:
- On mount and on debounced (~300 ms) change to `completedDate` / `costPence`, calls `maintenance_suggest_transactions(completed_date, cost_pence, exclude_event_id)`.
- Renders up to N candidates as rows: `emoji category · merchant · £amount · MMM D`. Selected row highlights with a ✓ Linked badge.
- Bottom: "None of these — search all…" inline input; ≥2 chars triggers `maintenance_search_transactions(query)` which replaces the suggestion list.
- Selected pill renders above the list with a × to unlink.
- If the backend rejects a selection with the already-linked error, surface inline: "That transaction is already linked to another event."

## 10. UI — `HistoryBlock` on AssetDetail

Inserted between MaintenanceSection and Documents. Renders `list_for_asset(asset_id)` most-recent-first. No pagination in MVP; revisit at 100+ rows.

```
┌─ History ─────────────────────────────────── + Log work ┐
│  🗓 Apr 12 · Annual boiler service · £145 · 🔗 British Gas  ✎ │
│  🗓 Jan 03 · Filter replaced · £22                            ✎ │
│  🗓 Nov 18 · Water heater check                    (backfill) ✎ │
│  …                                                             │
└───────────────────────────────────────────────────────────────┘
```

- Backfill pill shows only when `source='backfill' AND cost_pence IS NULL`. Adding a cost drops the pill (the event is now enriched).
- 🔗 transaction pill renders when `transaction_id` is Some AND the transaction is not soft-deleted. Click → opens Ledger focused on the transaction (if Ledger has no focus API yet, render as a static tooltip showing merchant + amount — revisit in plan).
- Pencil opens `LogCompletionDrawer` in edit mode.
- Schedule-orphan events (schedule was trashed): render the title but add a muted "(schedule removed)" suffix.
- Empty state: "No completions logged yet." + `+ Log work` button inline.

## 11. UI — `AssetSpendStrip`

Tiny inline pill strip directly below the asset header:

```
12-month spend  £847   ·   Lifetime  £1,923   ·   14 completions
```

Hidden when `total_lifetime_pence = 0 AND event_count_lifetime = 0`.

## 12. UI — Bones `Spend` sub-nav tab

Third tab on `BonesSubNav`. `BonesTab.tsx` routes `subview === "spend"` to `<SpendView />`.

```
[Assets]  [Due soon]  [Spend]
─────────────────────────────
         ─────
  12-month spend across the house

  🏠 Appliance  £614      🚗 Vehicle   £240
  🔧 Fixture    £180      📦 Other     £0

  ─────────────────────────────

  Per asset (12 months)                   ↓ Sort by spend ▾

  🏠 Worcester Bosch boiler          £347
  🚗 2019 Skoda Octavia              £240
  🏠 Dishwasher (Bosch)              £180
  🏠 Washing machine                  £87
  🔧 Front door lock set               £0
  …
```

- Category strip always in fixed order (Appliance, Vehicle, Fixture, Other); zeros shown.
- Sort toggle: `12m spend DESC` (default) or `Lifetime spend DESC`.
- Row click → `useBonesViewStore.openAssetDetail(id)` → navigates via existing L4b plumbing.
- Empty state (zero events across all assets): "No maintenance spend logged yet. Mark things done to start tracking." with a link to `Due soon`.

## 13. Zustand stores

### 13.1 `useMaintenanceEventsStore` (`lib/maintenance/event-state.ts`)

```ts
interface MaintenanceEventsStore {
  eventsByAsset: Record<string, EventWithContext[]>;
  loadStatus: LoadStatus;

  loadForAsset(assetId: string): Promise<void>;
  createOneOff(draft: MaintenanceEventDraft): Promise<string>;
  logCompletion(scheduleId: string, draft: MaintenanceEventDraft): Promise<string>;
  update(id: string, draft: MaintenanceEventDraft): Promise<void>;
  suggestTransactions(
    completedDate: string,
    costPence: number | null,
    excludeEventId: string | null,
  ): Promise<Transaction[]>;
  searchTransactions(query: string): Promise<Transaction[]>;
}
```

### 13.2 `useSpendStore` (`lib/maintenance/spend-state.ts`)

```ts
interface SpendStore {
  assetTotals: AssetSpendTotal[];
  categoryTotals: CategorySpendTotal[];
  loadStatus: LoadStatus;

  loadAssetTotals(): Promise<void>;
  loadCategoryTotals(): Promise<void>;
  refresh(): Promise<void>;   // both, in parallel
}
```

### 13.3 Cache invalidation

After any mutation that creates or updates an event:
1. Drop `eventsByAsset[assetId]` so the next `loadForAsset` refetches.
2. If `useSpendStore` has loaded data, trigger `refresh()` (no-op if not mounted).

`useMaintenanceStore.markDone(id)` (L4b) keeps the same signature, but its body gains post-resolve invalidation: drop `useMaintenanceEventsStore.eventsByAsset[assetId]` + call `useSpendStore.refresh()` (no-op if not mounted). Rhythm, Today, and ScheduleRow callsites don't need to change — they still call `markDone(id)`; the invalidation happens inside the store action.

## 14. Error handling

- **Backend** wraps SQLite constraint errors into human messages via `anyhow!`. Tauri commands convert `Err` into `Err(String)`. Specific cases:
  - Transaction already linked: `"Transaction already linked to another event"`.
  - Schedule/asset mismatch: `"Schedule does not belong to asset"`.
  - Negative cost: `"Cost must be zero or positive"`.
  - Invalid date: `"Date must be in YYYY-MM-DD format"`.
  - Schedule not found for `mark_done`: `"Schedule not found"`.
- **Frontend drawer**: save failures leave the drawer open with the error message above the Save button. Form state preserved.
- **Frontend rollups**: failures show a toast + a retry link on the affected view (AssetSpendStrip or SpendView). Rest of Bones keeps working.
- **TransactionSuggest**: inline "That transaction is already linked to another event" when the backend rejects a link.

## 15. Edge cases (pinned)

| Case | Behavior |
|---|---|
| Backfill event edited to add a cost | Allowed. `source` stays `'backfill'`. "backfill" pill disappears (rendered only when `source='backfill' AND cost_pence IS NULL`). |
| `completed_date` edited to far past | Allowed. Rollups recompute on next fetch; event moves in history. |
| Linked transaction is later soft-deleted in Ledger | `EventWithContext` LEFT JOIN returns `transaction_description = None`. History renders without the link pill. No cascade; restoring the transaction brings the link back. |
| Schedule deleted while drawer is mid-save | Event inserts with the original `schedule_id`; orphan tolerated. History renders with "(schedule removed)" suffix. |
| Asset soft-deleted then restored | Events share the asset's `deleted_at` timestamp; `restore_asset` reverses the cascade for exactly the rows it trashed. Events soft-deleted at an *earlier* timestamp stay trashed. |
| Migration V20 re-run (shouldn't happen) | `NOT EXISTS` guard in the backfill INSERT prevents duplicates. |
| User tries to link a transaction already linked to a soft-deleted event | Allowed — partial unique index is scoped `WHERE deleted_at IS NULL`. |
| User attempts to link via Search-all to a soft-deleted transaction | Search excludes `deleted_at IS NOT NULL`; can't happen by construction. |
| Silent `mark_done` for a schedule where `asset_id` points to a now-trashed asset | Should not occur — L4b's asset-cascade already trashes schedules. Defence-in-depth: mark_done loads the schedule; if soft-deleted, returns `"Schedule not found"`. |

## 16. Testing strategy

### 16.1 Core unit tests (`event_dal.rs`)

- `insert_event` full-field round-trip.
- `insert_event` with NULL cost / NULL schedule / NULL transaction — all allowed.
- `insert_event` rejects when schedule's `asset_id` differs from draft's.
- `insert_event` rejects when `cost_pence < 0`.
- `insert_event` rejects duplicate `transaction_id` (partial unique index).
- `insert_event` allows same `transaction_id` after the previous event is soft-deleted.
- `update_event` mutates cost/notes/completed_date/transaction_id; preserves `source` + `asset_id` + `schedule_id`.
- `update_event` can set `transaction_id` back to None.
- `list_for_asset` orders DESC by `completed_date`, excludes `deleted_at IS NOT NULL`.
- `list_for_asset` populates `schedule_deleted = true` when schedule is soft-deleted.
- `list_for_asset` populates `transaction_description = None` when transaction is soft-deleted.
- `mark_done` silent path (None draft): inserts minimal event AND bumps dates.
- `mark_done` drawer path (Some draft): uses caller's draft verbatim AND bumps dates.
- `asset_spend_totals`: zero-event assets appear with £0 / 0 counts.
- `asset_spend_totals`: 12m window cuts at exactly 365 days.
- `asset_spend_totals`: events with NULL cost count toward `event_count_*` but not cost sums.
- `asset_spend_totals` excludes trashed assets.
- `category_spend_totals` sums correctly; soft-deleted assets absent.
- `suggest_transactions` with cost: top-3 ordered by `ABS(amount_pence + cost_pence)`.
- `suggest_transactions` without cost: top-5 ordered by date DESC.
- `suggest_transactions` excludes already-linked transactions.
- `suggest_transactions` with `exclude_event_id = Some(id)` re-includes that event's own linked transaction.
- `search_transactions` matches description and merchant; respects already-linked exclusion.

### 16.2 Cascade tests (`asset::dal`)

- `soft_delete_asset` soft-deletes the asset's events (same timestamp as schedules and asset).
- `restore_asset` restores schedules + events that share the asset's `deleted_at` timestamp.
- `restore_asset` does **not** resurrect events with earlier `deleted_at` timestamps (they were trashed separately).
- `permanent_delete_asset` hard-deletes events before schedules before the asset.

### 16.3 Trash tests (`trash.rs`)

- `REGISTRY` contains `("maintenance_event", "title")`.
- Trash sweep test generalises to cover all registered tables.

### 16.4 Migration tests

- V20 on a dev DB with L4b data: one backfill event per non-trashed schedule with `last_done_date` set, `source='backfill'`, `cost_pence=NULL`.
- V20 idempotency: re-running the backfill INSERT (hypothetically) creates no duplicates.
- V20 on a fresh DB: applies cleanly, zero events.

### 16.5 Integration tests (`crates/app`)

- `maintenance_event_create_oneoff` round-trips with `list_for_asset`.
- `maintenance_event_log_completion` updates schedule dates AND inserts event.
- `maintenance_event_update` updates cost + links transaction; `list_for_asset` reflects.
- `maintenance_suggest_transactions`: top-3 with cost; top-5 without; exclusion works.
- `maintenance_spend_asset_totals` reflects a newly-logged event with cost.
- Silent mark-done followed by `maintenance_event_list_for_asset` returns one minimal event row.
- Schedule soft-deleted → its events still appear in history with `schedule_deleted = true`.

### 16.6 Frontend (RTL)

- `LogCompletionDrawer` in all three modes: one-off requires title; schedule-completion prefills title; edit populates all fields.
- `LogCompletionDrawer` rejects negative cost inline.
- `TransactionSuggest` fetches on mount; debounced refetch on cost change.
- `TransactionSuggest` shows selected pill with × to unlink; "Search all" filter works.
- `HistoryBlock` renders events most-recent-first; pencil opens edit drawer; empty state shows.
- `HistoryBlock` shows backfill pill only when `source='backfill' AND cost_pence IS NULL`.
- `AssetSpendStrip` hides when lifetime = £0 AND lifetime count = 0.
- `SpendView`: category strip renders four fixed pills (zeros for missing categories); sortable asset list; empty state.
- `BonesSubNav` three-tab switch round-trips via `setting(bones.last_subview)`; `"spend"` is a valid persisted value.

## 17. Definition of done

- Migration V20 runs cleanly on fresh + existing dev DBs, backfilling schedules-with-completions.
- `maintenance_event` CRUD Tauri commands round-trip.
- Silent `maintenance_schedule_mark_done` also writes an event row (verified via `list_for_asset`).
- `LogCompletionDrawer` works in all three modes (one-off, schedule completion, edit).
- `TransactionSuggest` surfaces top-3/top-5 matches via the two heuristics; "None of these" search-all works; already-linked transactions excluded.
- `HistoryBlock` renders below MaintenanceSection on AssetDetail; backfill pill visible on synthesised rows.
- `AssetSpendStrip` renders at top of AssetDetail; hides at £0 lifetime + zero events.
- Bones `Spend` third sub-nav tab ships; category strip + per-asset sortable list; empty state.
- Soft-delete-asset cascades events; restore-asset restores them; permanent-delete-asset hard-deletes them.
- Partial unique index on `transaction_id` prevents duplicate links.
- Trash sweep includes `maintenance_event`; restore/permanent-delete match arms extended.
- `cargo test --workspace` green. Clippy clean. TypeScript clean. `pnpm build` green. Tauri production bundle builds.
- Manual QA: create a boiler → add a schedule with `last_done_date = 2 months ago` → run migration → see backfill event in History → log one completion with cost £145 + link a British Gas transaction → edit cost to £150 → view Spend tab → verify totals update.

## 18. Out of scope for L4c (pinned)

- Auto-proposing events from Ledger transactions ("paid British Gas £180 — log boiler service?"). v0.5.1 or later.
- Notifications when a schedule becomes due.
- Multi-transaction linkage per event (parts receipt + labour receipt). Junction-table migration if/when the need appears.
- Attachments per event (receipt PDF). Reuses L4a's attachment plumbing when added; not in this slice.
- Skip / snooze a due schedule without marking done.
- Retroactive cost estimation ("typical boiler service ≈ £120?"). No LLM in this slice.
- PDF manual extraction → auto-schedule proposals. L4e.
- Right-to-repair lookup integration on asset detail. L4d.
- Warranty tracking. Out of v0.5.
- Per-completion contractor tracking ("who did it").
- Editable `source` field — users can't promote backfill ↔ manual.
- Bulk log-completion across multiple schedules.
- Currency other than GBP. Multi-currency is a v1.x concern.
- Ledger view changes. L4c reads transactions only; no writes, no Ledger-surface additions.

---

*End of L4c design spec. Next: implementation plan.*
