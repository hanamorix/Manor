# Manor v0.2 Rhythm — Design Spec

- **Status**: Draft — pending user review
- **Date**: 2026-04-16
- **Authors**: Hana (product owner / editor), Nell (lead architect / engineer)
- **Phase**: 4 (v0.2 Rhythm)
- **Depends on**: Phase 3 complete (CalDAV read + today context injection on main)

---

## 1. Scope

v0.2 Rhythm adds two household management capabilities to Manor — **chores** and **time blocks** — along with the **navigation shell** needed to host their management views. These three things are built together because:

- The sidebar navigation is shared infrastructure for both features.
- Chores and time blocks are small enough to implement in parallel once the nav shell is in place.
- Both features share the Today view surface (new cards) and a similar management-view pattern.

**In scope:**
- Icon sidebar navigation shell
- `chore`, `chore_completion`, `rotation` tables + backend
- `time_block` table + backend
- ChoresCard and TimeBlocksCard on the Today view
- Chores management view (full CRUD)
- Time Blocks management view (full CRUD)
- Moderate AI: pattern detection nudge + chore fairness nudge (both local, no LLM)

**Out of scope:**
- Full theme/design system overhaul (deferred — flagged for a dedicated design pass)
- Money, Meals, Home features (v0.3+)
- Remote LLM involvement in chore/block suggestions

---

## 2. Navigation Shell

### 2.1 Layout

A narrow **icon sidebar** (58px wide) replaces the current single-view layout. It sits on the left edge of the app window and is always visible.

```
┌──────────────────────────────────┐
│ ● ● ●  Manor                     │  ← titlebar
├────┬─────────────────────────────┤
│ 🌸 │                             │  ← avatar
│    │                             │
│ 🏠 │   <active view content>     │  ← Today (active)
│ 🧹 │                             │  ← Chores
│ ⏱  │                             │  ← Time Blocks
│    │                             │
│ ⚙️  │                             │  ← Settings (bottom)
└────┴─────────────────────────────┘
```

- **Active icon**: white pill background, iMessage blue icon tint, soft shadow
- **Inactive icons**: no background, 35% opacity
- **Avatar** (top of sidebar): amber gradient circle, 30px — matches existing avatar design
- **Settings icon** (bottom of sidebar): pushed to the bottom via `flex: 1` spacer
- Sidebar background: `--paper-muted` (#f1f1ee), right border: `--hairline`

### 2.2 Routing

Client-side only — no URL routing needed. A `view` atom (Jotai) holds the active view name: `today | chores | timeblocks`. Clicking a nav icon sets the atom; the main content area renders the matching view component.

### 2.3 Existing Settings modal

The existing ⚙️ gear icon in HeaderCard (calendar settings) is **not removed** — it remains for calendar account management. The sidebar ⚙️ opens a broader Settings view (placeholder for now; calendar settings migrate there in a future cleanup pass).

---

## 3. Data Model

All tables follow the universal row shape: `id TEXT PRIMARY KEY` (UUIDv7), `created_at INTEGER`, `updated_at INTEGER`, `device_id TEXT`, `deleted_at INTEGER` (soft-delete). Standard indices on `updated_at` and a partial index `WHERE deleted_at IS NULL`.

### 3.1 `chore`

```sql
CREATE TABLE chore (
    id          TEXT PRIMARY KEY,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    device_id   TEXT NOT NULL,
    deleted_at  INTEGER,

    title       TEXT NOT NULL,
    emoji       TEXT NOT NULL DEFAULT '🧹',
    rrule       TEXT NOT NULL,           -- RFC 5545 RRULE string, e.g. "FREQ=WEEKLY"
    next_due    INTEGER NOT NULL,        -- unix ms of next occurrence
    rotation    TEXT NOT NULL DEFAULT 'none',  -- 'round_robin' | 'least_completed' | 'fixed' | 'none'
    active      INTEGER NOT NULL DEFAULT 1     -- 0 = paused
);
```

### 3.2 `chore_completion`

```sql
CREATE TABLE chore_completion (
    id              TEXT PRIMARY KEY,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    device_id       TEXT NOT NULL,
    deleted_at      INTEGER,

    chore_id        TEXT NOT NULL REFERENCES chore(id),
    completed_at    INTEGER NOT NULL,   -- unix ms
    completed_by    TEXT               -- nullable person.id
);

CREATE INDEX idx_chore_completion_chore ON chore_completion(chore_id);
CREATE INDEX idx_chore_completion_person ON chore_completion(completed_by) WHERE completed_by IS NOT NULL;
```

### 3.3 `rotation`

```sql
CREATE TABLE rotation (
    id          TEXT PRIMARY KEY,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    device_id   TEXT NOT NULL,
    deleted_at  INTEGER,

    chore_id    TEXT NOT NULL REFERENCES chore(id),
    person_id   TEXT NOT NULL REFERENCES person(id),
    position    INTEGER NOT NULL,       -- 0-indexed order in the rotation
    current     INTEGER NOT NULL DEFAULT 0  -- 1 = this person is up next
);

CREATE INDEX idx_rotation_chore ON rotation(chore_id);
```

### 3.4 `time_block`

```sql
CREATE TABLE time_block (
    id          TEXT PRIMARY KEY,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    device_id   TEXT NOT NULL,
    deleted_at  INTEGER,

    title       TEXT NOT NULL,
    kind        TEXT NOT NULL,          -- 'focus' | 'errands' | 'admin' | 'dnd'
    date        INTEGER NOT NULL,       -- unix ms, midnight UTC of the day
    start_time  TEXT NOT NULL,          -- "HH:MM" 24h
    end_time    TEXT NOT NULL,          -- "HH:MM" 24h
    rrule       TEXT,                   -- NULL for one-off; set when promoted to recurring
    is_pattern  INTEGER NOT NULL DEFAULT 0,  -- 1 = user confirmed this as a recurring pattern
    pattern_nudge_dismissed_at INTEGER       -- unix ms; NULL = nudge eligible; set by dismiss_pattern_nudge
);

CREATE INDEX idx_time_block_date ON time_block(date);
```

### 3.5 Migration

New migration file: `crates/core/migrations/V4__rhythm.sql` containing all four tables above.

---

## 4. Backend (Tauri Commands)

All commands live in `crates/app/src/` under a new `rhythm` module. Thin glue only — business logic in `crates/core/src/assistant/`.

### 4.1 Chore commands

| Command | Signature | Notes |
|---|---|---|
| `list_chores_due_today` | `() → Vec<ChoreRow>` | WHERE next_due ≤ today midnight AND active=1 AND deleted_at IS NULL |
| `list_all_chores` | `() → Vec<ChoreRow>` | All active chores, sorted by next_due |
| `create_chore` | `(CreateChoreArgs) → ChoreRow` | Computes first next_due from rrule |
| `update_chore` | `(UpdateChoreArgs) → ChoreRow` | Recomputes next_due if rrule changed |
| `delete_chore` | `(id: String) → ()` | Soft-delete |
| `complete_chore` | `(CompleteChoreArgs) → ()` | Inserts chore_completion; advances next_due using original next_due as base (not today) so the schedule stays predictable; advances rotation current if applicable |
| `list_chore_completions` | `(chore_id: String, limit: u32) → Vec<CompletionRow>` | Last N completions for history tab |

### 4.2 Time block commands

| Command | Signature | Notes |
|---|---|---|
| `list_blocks_for_date` | `(date_ms: i64) → Vec<TimeBlockRow>` | Today's blocks |
| `list_blocks_for_week` | `(week_start_ms: i64) → Vec<TimeBlockRow>` | Management view |
| `list_recurring_blocks` | `() → Vec<TimeBlockRow>` | WHERE is_pattern=1 AND deleted_at IS NULL |
| `create_time_block` | `(CreateBlockArgs) → TimeBlockRow` | |
| `update_time_block` | `(UpdateBlockArgs) → TimeBlockRow` | |
| `delete_time_block` | `(id: String) → ()` | Soft-delete |
| `promote_to_pattern` | `(id: String, rrule: String) → TimeBlockRow` | Sets is_pattern=1, rrule; also backfills rrule on prior identical blocks |
| `dismiss_pattern_nudge` | `(id: String) → ()` | Sets `pattern_nudge_dismissed_at` on the time_block row; pattern detection suppressed for 14 days from that timestamp |

### 4.3 AI / nudge queries

These are pure SQL queries — no LLM involved.

**Pattern detection** (`check_time_block_pattern`): after any `create_time_block`, query for blocks with matching `kind` and `start_time`/`end_time` window (±15 min) on the same weekday in the last 6 weeks. If count ≥ 3 and is_pattern=0 and nudge not recently dismissed, return a `PatternSuggestion` to the frontend.

**Chore fairness** (`check_chore_fairness`): on Chores view load, for each active chore with rotation ≠ 'none', compute days-since-last-completion per assigned person. If any person's value is > 2× the median, return a `FairnessNudge { chore_title, person_name, days_ago }`.

---

## 5. Frontend

### 5.1 Today view changes

Existing card order extended:

```
HeaderCard       (existing)
EventsCard       (existing)
TimeBlocksCard   ← new
ChoresCard       ← new
TasksCard        (existing)
```

**TimeBlocksCard**
- Header: "Time Blocks" + "+" button (opens quick-add drawer)
- Body: slim colored pills per block — `focus`=blue (`#007aff`), `errands`=amber (`#FFC15C`), `admin`=purple (`#9b59b6`), `dnd`=red (`#ff3b30`). Each pill shows title + time range.
- Pattern nudge: soft banner at card bottom when `PatternSuggestion` is active. Two actions: "Make recurring" (calls `promote_to_pattern`) and "Not now" (calls `dismiss_pattern_nudge`).
- Empty state: "No blocks today — time is yours."

**ChoresCard**
- Header: "Chores" + link to Chores view
- Body: one row per due-today chore. Row = emoji + title + assignee avatar (if rotation set). Tap row → complete (confetti micro-animation via CSS keyframes, then row fades out). Long-press or right-click → skip (advances next_due without recording completion).
- Empty state: "All clear today 🧹"

### 5.2 Chores view (`/chores`)

Three sections rendered as a scrollable single column:

1. **Due soon** — chores due in next 7 days. Sorted by next_due. Each row: emoji, title, assignee chip, due-date badge. Tap to complete early.
2. **All chores** — full list, sorted by title. Same row format. Tap opens edit drawer.
3. **+ Add chore** CTA button at the bottom.

**Chore drawer** (create + edit):
- Fields: emoji picker, title input, recurrence picker (plain-English: "Daily" / "Every week" / "Every 2 weeks" / "Monthly" / "Custom…"), rotation strategy selector, assignee multi-select (from household members).
- Second tab in drawer: **History** — last 20 completions with person avatar + timestamp.
- Delete action: soft-delete with 4s undo toast (same pattern as tasks).

**Fairness banner**: if `FairnessNudge` present, quiet amber banner at top of view: *"Rosa hasn't done Bins in 3 weeks."* Dismissable.

### 5.3 Time Blocks view (`/timeblocks`)

Two sections:

1. **This week** — blocks grouped by day (Mon–Sun). Each row: kind pill, title, time range. Tap to edit; trash icon to delete.
2. **Recurring** — promoted patterns only. Each row: kind pill, title, rrule in plain English ("Every weekday 9–11am"). Tap to edit rrule or remove pattern (asks: remove just the pattern, or delete all future instances?).

**+ Add block** CTA: opens quick-add drawer (title, kind, date, start time, end time). No recurring toggle — Nell promotes via nudge.

---

## 6. State management

Two new Zustand slices (matching the settings/today pattern already in the codebase):

- `useChoresStore` — `choresDueToday`, `allChores`, `fairnessNudge`, actions: `completeChore`, `skipChore`, `createChore`, `updateChore`, `deleteChore`
- `useTimeBlocksStore` — `todayBlocks`, `weekBlocks`, `recurringBlocks`, `patternSuggestion`, actions: `createBlock`, `updateBlock`, `deleteBlock`, `promoteToPattern`, `dismissPatternNudge`

Both hydrate on app start alongside the existing event/task hydration in `src/main.tsx`.

---

## 7. Testing

### 7.1 Rust (target: ~40 new tests)

- `chore` DAL: CRUD, soft-delete, next_due computation from rrule
- `chore_completion`: insert, list with limit, cascade on chore soft-delete
- `rotation`: round_robin advance, least_completed logic, fixed assignment
- `time_block` DAL: CRUD, date range queries, is_pattern flag
- Pattern detection query: 0/1/2/3 matches, dismissed-nudge suppression
- Fairness query: even distribution (no nudge), one outlier (nudge), single-person (no nudge)

### 7.2 Frontend (target: ~20 new tests, Vitest)

- `useChoresStore`: completeChore optimistic update, skipChore, fairnessNudge present/absent
- `useTimeBlocksStore`: createBlock, promoteToPattern, dismissPatternNudge
- ChoresCard: renders due chores, empty state, tap-to-complete flow
- TimeBlocksCard: renders pills, pattern nudge banner, dismiss action

---

## 8. Open questions (non-blocking)

| # | Question |
|---|---|
| OQ-1 | Sidebar icon set — currently emoji placeholders. Real SVG icons deferred to full design pass. |
| OQ-2 | `person` table seeding — creating household members needs a UI. For v0.2 we seed via the Settings view (placeholder); a proper People management screen is v0.3+. |
| OQ-3 | `rrule` plain-English picker — "Custom…" option needs a simple day-of-week + interval UI. Scope for implementation phase. |
| OQ-4 | Notification for due chores — out of scope for v0.2. macOS `UNUserNotificationCenter` considered for v0.3. |

---

## 9. Implementation phases

This spec is implemented in two sub-phases:

**Phase 4a — Nav shell + foundations**
- V4 migration (all four tables)
- Icon sidebar component + Jotai view atom
- Chores view scaffold (empty, routed)
- Time Blocks view scaffold (empty, routed)

**Phase 4b — Chores and Time Blocks (parallel)**
- Backend + frontend for chores (parallel subagent)
- Backend + frontend for time blocks (parallel subagent)
- Today card integration
- AI nudge queries wired up
- Tests
