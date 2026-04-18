# L3b Meal Plan + Staples — Design Spec

- **Date**: 2026-04-18
- **Landmark**: v0.4 Hearth → L3b (second sub-landmark)
- **Status**: Approved design. Implementation plan next.
- **Authors**: Hana (product), Nell (architect)
- **Roadmap**: `specs/2026-04-18-v0.4-hearth-roadmap.md`
- **Depends on**: L3a Recipe Library (shipped 2026-04-18, commit `bace41e`).

## 1. Purpose

Ship Manor's meal planning: a weekly grid that maps recipes to days, a "Tonight" reflection on the Today view, block-style presence in TimeBlocks, and a "staples" exclusion list that L3c's shopping list will use to skip pantry regulars. Ships alone — useful as "I know what we're eating this week" without yet requiring a shopping-list generator.

## 2. Scope decisions (locked during brainstorm)

| Area | Decision |
|---|---|
| **Grid layout** | Single Mon–Sun strip. One slot per day (implicit dinner). |
| **Week navigation** | Prev/Next arrows + jump-to-today + date picker popover. |
| **Assignment UX** | Tap empty slot → right-side picker drawer → tap recipe. No drag-drop. |
| **Staples model** | Flat `name` field by default + optional `aliases` (JSON array) for power users. |
| **Recipe deletion → meal plan** | Soft-ghost. Entry keeps the recipe_id; UI renders a "Recipe deleted" ghost state with restore/unplan actions. No FK cascade. |
| **TimeBlocks integration** | Meal renders as a block at `hearth.dinner_time` (household setting, default 19:00), fixed 45 min duration. Read-only from TimeBlocks. |
| **Today band** | New "Tonight" strip below Weather. Click-through to recipe detail. Dismissible via setting. |
| **Hearth sub-nav** | Horizontal tabs at top of Hearth: Recipes · This Week · Staples. Default view: This Week. Last-selected persists. |
| **Calendar integration** | Internal only. No CalDAV write (deferred to v0.4.1 earliest per roadmap). |

## 3. Architecture

Two-crate split, same pattern as L3a.

### 3.1 `crates/core/src/meal_plan/`

- `mod.rs` — module root + types:
  - `MealPlanEntry { id, entry_date (ISO string), recipe_id (Option<String>), created_at, updated_at }`.
  - `MealPlanEntryDraft { entry_date, recipe_id: Option<String> }`.
  - `StapleItem { id, name, aliases: Vec<String>, created_at, updated_at, deleted_at: Option<i64> }`.
  - `StapleDraft { name, aliases: Vec<String> }`.
- `dal.rs` — CRUD:
  - Meal plan: `get_week(conn, start_date) -> Vec<MealPlanEntry>` (always 7 entries, one per day, recipe_id may be None); `set_entry(conn, date, recipe_id)` (upsert on `entry_date`); `clear_entry(conn, date)`; `get_entry(conn, date) -> Option<MealPlanEntry>` for Today band + TimeBlocks lookups.
  - Staples: `list_staples(conn) -> Vec<StapleItem>` (excludes trashed by default); `insert_staple(conn, draft)`; `update_staple(conn, id, draft)`; `soft_delete_staple(conn, id)`; `restore_staple(conn, id)`.
- `matcher.rs` — pure function `staple_matches(ingredient_name: &str, staples: &[StapleItem]) -> bool`. Normalises both sides (lowercase, trim, strip trailing `s`/`es` for crude singularisation), substring-or-word match against each staple's `name` plus each of its aliases. L3c consumes this.

### 3.2 `crates/app/src/meal_plan/`

- `mod.rs` — module root.
- `commands.rs` — Tauri IPC:
  - `meal_plan_week_get(start_date: String) -> Vec<MealPlanEntryWithRecipe>` — returns 7 entries. `MealPlanEntryWithRecipe` embeds the joined recipe (nullable) so the frontend doesn't need a second round trip per cell.
  - `meal_plan_set_entry(date: String, recipe_id: String) -> ()` — upserts.
  - `meal_plan_clear_entry(date: String) -> ()`.
  - `meal_plan_today_get() -> Option<MealPlanEntryWithRecipe>` — convenience for Today band.
  - `staple_list() -> Vec<StapleItem>`.
  - `staple_create(draft: StapleDraft) -> String`.
  - `staple_update(id: String, draft: StapleDraft) -> ()`.
  - `staple_delete(id: String) -> ()`.
  - `staple_restore(id: String) -> ()`.

### 3.3 Shared view-model type

`MealPlanEntryWithRecipe { entry_date, recipe: Option<Recipe> }` — one struct the frontend consumes for both the grid and the Today band. `recipe` is `None` when no meal planned, and `Some(Recipe)` even if the recipe is soft-deleted (ghost state is detected by the frontend via `recipe.deleted_at != null`). Note: `get_recipe` in L3a now filters soft-deletes. For meal plan lookups we need a `get_recipe_including_trashed(conn, id)` helper added to `recipe::dal` to surface ghost recipes. This is an additive change.

### 3.4 Reused infrastructure

- `setting` key-value table for:
  - `hearth.dinner_time` (default `"19:00"`, format `HH:MM`).
  - `hearth.show_tonight_band` (default `"true"`).
  - `hearth.last_subview` (default `"this_week"`, values `"this_week" | "recipes" | "staples"`).
- `trash` sweeper: register `staple_item` so soft-deletes purge after 30 days. (Meal plan entries don't need trash integration — they're ephemeral state; clearing a slot hard-deletes the row.)

## 4. Schema — migration V16

```sql
-- V16__meal_plan.sql
-- L3b Meal Plan: meal_plan_entry + staple_item.

CREATE TABLE meal_plan_entry (
    id          TEXT PRIMARY KEY,
    entry_date  TEXT NOT NULL UNIQUE,       -- ISO YYYY-MM-DD; one entry per day
    recipe_id   TEXT REFERENCES recipe(id), -- nullable; NO cascade (soft-ghost on recipe delete)
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX idx_meal_plan_date ON meal_plan_entry(entry_date);

CREATE TABLE staple_item (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    aliases    TEXT,                        -- JSON array of strings, nullable
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);
CREATE INDEX idx_staple_deleted ON staple_item(deleted_at);
CREATE INDEX idx_staple_name    ON staple_item(name COLLATE NOCASE);
```

Trash sweeper registration: add `staple_item` to the `REGISTRY` in `crates/core/src/trash.rs`.

Timestamp unit: **seconds** (matching the recipe DAL fix shipped in L3a commit `7326ea8`, and the sweeper's existing seconds-based cutoff).

## 5. Hearth sub-navigation

Hearth's content area gains a horizontal tab row at the top:

```
Recipes  ·  This Week  ·  Staples
─────────
(active tab content)
```

- Tabs: 14px medium weight, active underlined in `--ink-strong`, inactive `--ink-soft`.
- State: `useHearthViewStore` Zustand hook — `{ subview, setSubview(v) }`. Hydrate from `setting(hearth.last_subview)` on mount; `setSubview` writes back via the existing setting IPC.
- Default: `this_week` on first ever open (fresh install with no stored value).

`HearthTab.tsx` becomes a thin router:

```tsx
export function HearthTab() {
  const { subview } = useHearthViewStore();
  return (
    <div>
      <HearthSubNav />
      {subview === "recipes"   && <RecipesView />}
      {subview === "this_week" && <ThisWeekView />}
      {subview === "staples"   && <StaplesView />}
    </div>
  );
}
```

`RecipesView` is the existing list view extracted from L3a's HearthTab body — no behavioural change. Detail/edit/import drawer flows remain unchanged.

## 6. This Week view

### 6.1 Layout

Single Mon–Sun strip, 7 equal columns.

- **Header row**: `← prev | week range label (e.g. "Apr 20 – 26, 2026") | next → | [📅 date picker] | [ Today ]` button.
- **Day columns**: day name (`Mon`), day-of-month (`20`). Today's column gets a subtle accent background (reuse an existing Flat-Notion hover/active token — whichever the codebase already uses for "selected" states) for orientation.
- **Slot card**: one per column, below the day header.

Responsive: below 900px viewport, transition to vertical stacked list (one row per day). Preserves readability on iPad and future iOS arc.

### 6.2 Slot card states

**Empty** — dashed 1px border `--hairline`, centered `+` icon, subtle "Plan a meal" label. Click → opens recipe picker drawer scoped to that date.

**Filled** — recipe hero image (4:3 thumbnail, `ImageOff` placeholder if no hero), title (1-line clamp), meta strip (`{cook+prep}m · {servings}p`). Click anywhere on card → navigate to recipe detail view. On hover, a `⋯` overflow button reveals in the top-right: **Swap** (reopens picker) and **Remove** (clears the slot, no confirm — quick un-plan is the common case).

**Ghost** (recipe soft-deleted, `recipe.deleted_at != null`) — `⊘` icon instead of thumbnail, title "Recipe deleted," subtle "Tap to restore or unplan" prompt. Click → drawer with two buttons: **Restore recipe** (calls `recipe_restore(id)`) and **Unplan this day** (calls `meal_plan_clear_entry(date)`).

### 6.3 Recipe picker drawer

Same right-side drawer pattern as L3a's `RecipeEditDrawer` / `RecipeImportDrawer` (480px, slide-in, backdrop dim).

- Header: `Plan {Thursday, Apr 23}` + `✕` close.
- Search input (title substring, 200ms debounced — mirror the HearthTab list search).
- Scrollable list of recipe mini-cards: thumbnail (48x48) + title + meta strip.
- Tap a card → calls `meal_plan_set_entry(date, recipe_id)` → drawer closes → grid refreshes.
- No tag filter for MVP (library search is enough for now).

### 6.4 Week navigation

- Prev/Next buttons: step 7 days.
- Today button: snap to current week (Mon–Sun containing today in local time).
- Date picker: popover calendar (reuse existing Manor calendar component if one exists; otherwise a minimal month grid with month nav). Selecting a date loads the Mon–Sun week containing it.

Active `start_date` lives in a component-local `useState` (not in a store) — navigation is transient and doesn't need global persistence.

### 6.5 Data flow

On mount / on week change:
```ts
const entries = await mealPlanIpc.weekGet(startDate);  // 7 MealPlanEntryWithRecipe
```

Backend builds the week by iterating 7 dates and LEFT JOIN `meal_plan_entry` → `recipe`, preserving empty dates (as entries with `recipe: null`). Ghost detection happens purely via the recipe's `deleted_at` field visible to the frontend.

## 7. Today band ("Tonight")

### 7.1 Placement

Inside the Today view, as a new band directly below the Weather strip, above the task bands. 56px tall.

### 7.2 States

```
🍽  Tonight — Miso-glazed aubergine  (30m · serves 4)         [ View recipe → ]
🍽  No dinner planned                                         [ Plan one → ]
🍽  Recipe deleted — restore or replace?                      [ ⊘ ]
```

- **Planned + live**: thumbnail (40x40) on left, title + meta, tap opens recipe detail.
- **Unplanned**: "Plan one →" button opens the L3b picker drawer scoped to today's date.
- **Ghost**: tap opens the same restore/unplan drawer as the grid's ghost state.

### 7.3 Dismissibility

Controlled by `setting(hearth.show_tonight_band)`, default `"true"`. Settings → Hearth section gets a toggle "Show tonight's meal on Today." When off, the band doesn't render.

### 7.4 Implementation

New component `apps/desktop/src/components/Today/TonightBand.tsx`. Fetches via `mealPlanIpc.todayGet()`. Rendered from the Today view's top section — insert as a sibling to WeatherStrip.

## 8. TimeBlocks integration

### 8.1 Block rendering

TimeBlocks renders tonight's meal (if planned) as a read-only block:

- Start: `hearth.dinner_time` (parsed as HH:MM, combined with today's date in local TZ).
- Duration: 45 minutes (hard-coded — not a setting).
- Title: `🍽 {recipe.title}` (ghost: `⊘ Recipe deleted`).
- Style: same chrome as other blocks, no special color — the 🍽 glyph is enough differentiation.
- Click → recipe detail view.

### 8.2 Implementation

Modify whichever TimeBlocks component composes the list of blocks for the day. Fetch the meal plan entry alongside existing blocks; map to the same `TimeBlock` shape with a new `origin: "meal"` discriminator so the click handler can route to the recipe detail. Not editable from TimeBlocks (no drag-to-move, no resize) — the source of truth is the meal plan grid.

If `hearth.dinner_time` is absent or malformed, skip rendering (log once). Default `"19:00"` is seeded during first Hearth access.

## 9. Staples sub-view

### 9.1 Layout

```
Staples
Items your shopping list skips by default.
                                          [ + Add staple ]

┌ Olive oil ──────────────────────────── [ ⋯ ] [ × ] ┐
│ also: EVOO, extra virgin olive oil                  │
├ Salt ──────────────────────────────────────── [ × ] ┤
├ Garlic ─────────────────────────────────── [ ⋯ ] [ × ]┤
│ also: garlic cloves                                 │
└─────────────────────────────────────────────────────┘
```

### 9.2 Behaviour

- List sorted alphabetically by `name` (case-insensitive).
- `+ Add staple` opens an inline input row at the top. Type name, press Enter → `staple_create({ name, aliases: [] })` → row appears.
- `⋯` overflow (only on rows with aliases, or to add aliases) → "Edit" opens a popover with name + chip-input for aliases. Save → `staple_update(id, draft)`.
- `×` → `staple_delete(id)` (soft-delete to Trash). No confirm — recoverable.

### 9.3 Alias chip-input

- Existing alias chips shown inline, each with an inline `×`.
- Text input at the end; Enter adds the current value as a chip.
- Empty aliases array is the common case; the chip-input just renders the input with no chips.

### 9.4 Empty state

"No staples yet. Add 'salt', 'olive oil', or anything else you always have so your shopping list won't repeat them."

## 10. Settings integration

Settings gains a **Hearth** section (new) with three controls:

- Toggle: **Show tonight's meal on Today** (`hearth.show_tonight_band`).
- Time picker: **Dinner time** (`hearth.dinner_time`, HH:MM, default 19:00).
- Link: **Manage staples** → jumps to Hearth → Staples sub-view.

Implementation: extend existing `Settings` component layout; reuse existing toggle + time picker primitives.

## 11. Error handling

| Error | User sees | Recovery |
|---|---|---|
| Picker drawer save fails (network/DB) | Inline error at top of drawer; drawer stays open | User retries Save |
| Week fetch fails | Grid shows "Couldn't load this week." + Retry button | User retries |
| Dinner-time setting malformed | TimeBlock not rendered; Today band still works (it doesn't read the time); log to console | User fixes in Settings |
| Recipe ghost + restore fails | Ghost drawer shows inline error | User tries Unplan instead |
| Staple name duplicates an existing active staple | Ignore — case-insensitive `name` allows duplicates intentionally (user may split "oil" vs "olive oil") | — |

## 12. Testing strategy

### 12.1 Unit (`crates/core`)

- `meal_plan::dal::get_week` — golden test with a mix of present + absent entries across 7 dates.
- `meal_plan::dal::set_entry` — insert path, then upsert path (UNIQUE on entry_date).
- `meal_plan::dal::clear_entry` — hard-delete.
- `staples::dal` — CRUD incl. soft-delete round-trip; aliases round-trip as JSON.
- `matcher::staple_matches` — table test covering:
  - exact name match ("olive oil" matches "olive oil"),
  - alias match ("EVOO" matches via alias),
  - plural recipe ingredient ("garlic cloves" matches staple "garlic clove"),
  - substring partial (staple "olive oil", ingredient "extra virgin olive oil" → match),
  - no match case,
  - empty alias array.
- Soft-ghost behaviour: insert recipe, add to meal plan, soft-delete recipe, `get_week` returns entry with recipe whose `deleted_at` is set (requires `get_recipe_including_trashed` helper).

### 12.2 Integration (`crates/app`)

- `meal_plan_week_get` returns 7 entries, empty dates have `recipe: None`.
- `meal_plan_set_entry` → `meal_plan_week_get` reflects the update.
- Setting a recipe on the same date twice upserts (no duplicate row error).
- Today helper returns None when no entry for today.
- Staple CRUD end-to-end.

### 12.3 Frontend

- `useHearthViewStore` persistence round-trip via mocked setting IPC.
- ThisWeek view renders 7 cells given mocked `weekGet` response.
- Slot-card click → picker drawer opens → card selection → `setEntry` called with correct args.
- Ghost state renders when recipe has `deleted_at != null`.
- TonightBand shows correct state across the 4 permutations (planned-live, planned-ghost, unplanned, dismissed-via-setting).

## 13. Out of scope for L3b

- Shopping list generation — **L3c**.
- Meal ideas reshuffle — **L3d**.
- Per-entry custom time (drag to move in TimeBlocks).
- Per-entry labels (e.g. "Saturday brunch" — rejected earlier in brainstorm for simplicity).
- Breakfast / lunch slots (one dinner per day only for MVP).
- Household-member assignees (who's cooking / eating).
- Meal plan templates or "repeat last week" convenience.
- CalDAV write-back (v0.4.1 earliest).
- Bulk week-at-a-time operations (clear week, swap recipes between days, etc.).

## 14. Definition of done

- Migration V16 runs cleanly on fresh install + existing dev DBs.
- Hearth sub-nav renders three tabs; last-selected persists.
- This Week view: 7-slot grid, week navigation, date picker, Today button, picker drawer, all three slot states (empty / filled / ghost) functional.
- Staples view: list, add inline, edit aliases, soft-delete, restore via trash UI.
- Today view: "Tonight" band renders for planned/unplanned/ghost/dismissed states.
- TimeBlocks: meal block renders at `hearth.dinner_time` for 45 min when planned.
- Settings: Hearth section with three controls.
- `cargo test --workspace` green. Clippy clean. TypeScript clean. Production build green.
- Manual QA pass: Sunday-evening plan-the-week golden path works end-to-end.

---

*End of L3b design spec. Next: implementation plan.*
