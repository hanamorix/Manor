# L3c Shopping List — Design Spec

- **Date**: 2026-04-18
- **Landmark**: v0.4 Hearth → L3c (third sub-landmark)
- **Status**: Approved design. Implementation plan next.
- **Authors**: Hana (product), Nell (architect)
- **Roadmap**: `specs/2026-04-18-v0.4-hearth-roadmap.md`
- **Depends on**: L3a Recipe Library (shipped at `bace41e`) and L3b Meal Plan + Staples (shipped at `efd5fb7`).

## 1. Purpose

Ship Manor's shopping list: a single always-on list inside Hearth that generates grocery items from the planned week's recipes, subtracts household staples, and lets the user add manual items (bin bags, milk, birthday card). Ticked items collapse to the bottom as you shop. Manual items survive across regenerations; recipe-derived items are overwritten.

L3c completes the L3a→L3c chain of "plan a recipe → plan a week → shop for it."

## 2. Scope decisions (locked during brainstorm)

| Area | Decision |
|---|---|
| **Regenerate scope** | The week currently shown in This Week grid (`useMealPlanStore().weekStart`). |
| **Manual items** | Mixed into the same list, tagged by `source='manual'`. Survive regenerations. |
| **Tick behaviour** | Cross-out + collapse-to-bottom. Un-ticking rises back. Ticked rows get opacity and strikethrough. |
| **UI placement** | Fourth Hearth sub-tab: Recipes · This Week · Shopping · Staples. |
| **Sorting** | Flat, insertion-order, ticked rows sink (`ORDER BY ticked ASC, position ASC`). No alphabetical sort, no recipe grouping, no aisle categorisation. |
| **Dedup** | None — "2 onions (miso aubergine)" and "1 onion (quick dal)" coexist as two rows. |
| **Quantity maths** | None — no summation, no unit conversion. Out of scope. |
| **Trash integration** | None — items delete outright, no 30-day retention. |

## 3. Architecture

New core module mirroring L3a/L3b pattern:

- `crates/core/src/shopping_list/mod.rs` — types + module root.
- `crates/core/src/shopping_list/dal.rs` — item CRUD (insert/list/toggle_tick/delete_manual/wipe_generated).
- `crates/core/src/shopping_list/generator.rs` — pure function `regenerate_from_week(conn, week_start) -> GeneratedReport`.

Thin Tauri layer:

- `crates/app/src/shopping_list/mod.rs` — module root.
- `crates/app/src/shopping_list/commands.rs` — IPC commands.

**Reused infrastructure:**
- `manor_core::meal_plan::matcher::staple_matches` (shipped in L3b) — used by the generator to skip ingredients that match a staple.
- `manor_core::meal_plan::staples::list_staples` — loads current staples once per regenerate.
- `manor_core::meal_plan::dal::get_week` — pulls meal plan entries for the target week.
- `manor_core::recipe::dal::get_recipe_including_trashed` — detects ghost recipes so the generator can skip them.

No schema changes to existing tables.

## 4. Schema — migration V17

```sql
-- V17__shopping_list.sql
-- L3c Shopping List: single always-on list of items.

CREATE TABLE shopping_list_item (
    id              TEXT PRIMARY KEY,
    ingredient_name TEXT NOT NULL,
    quantity_text   TEXT,                        -- "2", "400ml", or NULL for manual items without qty
    note            TEXT,                        -- snapshot of recipe_ingredient.note
    recipe_id       TEXT REFERENCES recipe(id),  -- NULL for manual; no cascade (row survives recipe deletion)
    recipe_title    TEXT,                        -- denormalized snapshot for provenance display
    source          TEXT NOT NULL,               -- 'generated' | 'manual'
    position        INTEGER NOT NULL,
    ticked          INTEGER NOT NULL DEFAULT 0,  -- 0 | 1
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX idx_shopping_item_order  ON shopping_list_item(ticked, position);
CREATE INDEX idx_shopping_item_source ON shopping_list_item(source);
```

**One-list model.** No `shopping_list` parent table — there's exactly one list in the system. Regenerate wipes `source='generated'` rows and rebuilds them; `source='manual'` rows persist.

**`recipe_title` denormalisation** makes the list robust to recipe renames and deletions without forcing joins everywhere.

**Timestamp unit:** seconds (matching the rest of Manor since the L3a fix at `7326ea8`).

## 5. Types (core)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoppingListItem {
    pub id: String,
    pub ingredient_name: String,
    pub quantity_text: Option<String>,
    pub note: Option<String>,
    pub recipe_id: Option<String>,
    pub recipe_title: Option<String>,
    pub source: ItemSource,
    pub position: i64,
    pub ticked: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemSource { Generated, Manual }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedReport {
    pub items_added: usize,
    pub items_skipped_staple: usize,
    pub ghost_recipes_skipped: usize,
}
```

`ItemSource::as_str()` and `from_db(Option<&str>)` helpers mirror the pattern from `recipe::ImportMethod`.

## 6. Regenerator algorithm

Pure function `regenerate_from_week(conn: &Connection, week_start: &str) -> Result<GeneratedReport>`:

1. Load staples once: `staples::list_staples(conn)`.
2. Load week entries: `meal_plan::dal::get_week(conn, week_start)` — 7 entries.
3. Begin transaction.
4. Delete existing rows where `source='generated'`. Manual rows untouched.
5. Compute `next_pos`: `SELECT MAX(position) FROM shopping_list_item WHERE source='manual'` + 1, falling back to 0 if no manual rows.
6. For each week entry with a non-null `recipe_id`:
    - Fetch the recipe via `recipe::dal::get_recipe_including_trashed(conn, id)`.
    - If `recipe.deleted_at` is Some → increment `ghost_recipes_skipped`; skip to next entry. (Do not shop for a meal whose recipe was trashed.)
    - For each `recipe.ingredients` line (preserved order):
        - If `staple_matches(&line.ingredient_name, &staples)` → increment `items_skipped_staple`; skip.
        - Insert shopping_list_item with:
          - `ingredient_name = line.ingredient_name`
          - `quantity_text = line.quantity_text`
          - `note = line.note`
          - `recipe_id = Some(recipe.id)`
          - `recipe_title = Some(recipe.title.clone())`
          - `source = 'generated'`
          - `position = next_pos` → then `next_pos += 1`
          - `ticked = 0`, `created_at = updated_at = now_secs()`.
        - Increment `items_added`.
7. Commit.
8. Return `GeneratedReport`.

**Ordering guarantee:** generated items land after all manual items in `position` space, in meal-plan-day order (Mon first, Sun last), and within each day in recipe-ingredient order.

**Defensive wrap:** `staple_matches` is pure and shouldn't fail, but wrap `.unwrap_or(false)` on any ingredient match path to ensure a single odd alias can't abort the whole regeneration.

## 7. DAL API

`crates/core/src/shopping_list/dal.rs`:

```rust
pub fn list_items(conn: &Connection) -> Result<Vec<ShoppingListItem>>;  // ORDER BY ticked ASC, position ASC
pub fn insert_manual(conn: &Connection, ingredient_name: &str) -> Result<String>;
pub fn toggle_tick(conn: &Connection, id: &str) -> Result<()>;          // flips 0↔1, bumps updated_at
pub fn delete_item(conn: &Connection, id: &str) -> Result<()>;          // hard delete (manual rows only per UI; backend allows any)
pub fn wipe_generated(conn: &Connection) -> Result<()>;                 // used by generator; also exposed for tests
pub fn counts(conn: &Connection) -> Result<(usize, usize)>;             // (total, ticked) for the header
```

`insert_manual` computes its own position as `MAX(position) + 1` across both source types so manual items always land at the end of un-ticked rows. Source is always `'manual'`, `quantity_text`/`note`/`recipe_id`/`recipe_title` all NULL, `ticked=0`.

## 8. Tauri commands

`crates/app/src/shopping_list/commands.rs`:

```rust
#[tauri::command] pub fn shopping_list_list(state: State<'_, Db>) -> Result<Vec<ShoppingListItem>, String>;
#[tauri::command] pub fn shopping_list_add_manual(ingredient_name: String, state: State<'_, Db>) -> Result<String, String>;
#[tauri::command] pub fn shopping_list_toggle(id: String, state: State<'_, Db>) -> Result<(), String>;
#[tauri::command] pub fn shopping_list_delete(id: String, state: State<'_, Db>) -> Result<(), String>;
#[tauri::command] pub fn shopping_list_regenerate(week_start: String, state: State<'_, Db>) -> Result<GeneratedReport, String>;
```

All follow the canonical `Db(Arc<Mutex<Connection>>)` pattern established in L3a/L3b.

## 9. UI

### 9.1 Hearth sub-nav extension

`HearthSubview` union in `apps/desktop/src/lib/hearth/view-state.ts`:

```ts
export type HearthSubview = "recipes" | "this_week" | "shopping" | "staples";
```

`HearthSubNav.tsx` tab order:

```
Recipes · This Week · Shopping · Staples
```

Persisted via `setting(hearth.last_subview)` as before. Migration: old stored values of `"recipes" | "this_week" | "staples"` continue to work unchanged.

### 9.2 Shopping view layout

```
Shopping list
12 items · 3 ticked                         [ + Add item ]  [ ↻ Regenerate ]

┌──────────────────────────────────────────────────────┐
│ ☐  2 aubergines, halved         · miso aubergine     │
│ ☐  3 tbsp white miso            · miso aubergine     │
│ ☐  400ml coconut milk           · thai green curry   │
│ ☐  2 garlic cloves, crushed     · thai green curry   │
│ ☐  bin bags                     · manual        [×]  │
│ ☐  birthday card for Sam        · manual        [×]  │
├──────────────────────────────────────────────────────┤
│ ☑  1 tsp salt                   · thai curry       (dim) │
│ ☑  milk                         · manual     [×]   (dim) │
└──────────────────────────────────────────────────────┘
```

**Header:** title "Shopping list" + subtitle `{total} items · {ticked} ticked`. Right: `+ Add item` and `↻ Regenerate` buttons.

**Row anatomy:**
- Left: checkbox (click → `shopping_list_toggle`).
- Middle: `{quantity_text} {ingredient_name}{note ? ", " + note : ""}`. Strikethrough + `opacity: 0.5` when `ticked`.
- Right-meta: `· {recipe_title}` for generated; `· manual` for manual. `--ink-soft`, small font.
- Manual-only: `[×]` remove button. Hover-revealed on wide screens, always-visible narrow. Calls `shopping_list_delete` with no confirm.

**Sorting:** rows come back from backend already ordered `ticked ASC, position ASC`. Frontend renders them in that order. A thin divider appears between last un-ticked and first ticked row (purely visual CSS — a `border-top` on the first ticked row via `:has()` or a conditional classname).

### 9.3 Empty states

- **No items + no meals planned this week**: "Your shopping list is empty. Plan some meals in This Week to generate ingredients." + link button → `setSubview("this_week")`.
- **No items + meals ARE planned**: "Your shopping list is empty." + big `↻ Generate from this week` button front-and-centre. Subtitle: small text showing which week will be generated.
- **Only manual items, nothing generated**: list renders manual rows; header buttons `+ Add item` and `↻ Regenerate` both available.

### 9.4 Add-item inline

Click `+ Add item` → inline input appears at the top of the un-ticked section. Placeholder "e.g. bin bags". Enter → calls `shopping_list_add_manual(value)`, input clears (or closes — match the Staples add-inline pattern shipped in L3b). Escape or blur with empty value → closes.

### 9.5 Regenerate flow

Click `↻ Regenerate` → native confirm dialog (or reuse whatever confirm primitive is canonical — inspect L3b's usage):

> "Replace {n_generated} generated items with a fresh list from {week range}? Your {n_manual} manual items will be kept."

Confirm → calls `shopping_list_regenerate(weekStart)` with `weekStart` pulled from `useMealPlanStore().weekStart`. On success: reloads the list; if `report.ghost_recipes_skipped > 0` shows a toast: "Skipped {n} deleted recipe(s) from your plan."

If `useMealPlanStore.weekStart` isn't set (unlikely — the store seeds it to Monday-of-today on construction), fall back to computing Monday-of-today locally.

### 9.6 Zustand store

`apps/desktop/src/lib/shopping_list/state.ts`:

```ts
type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface ShoppingListStore {
  items: ShoppingListItem[];
  counts: { total: number; ticked: number };
  loadStatus: LoadStatus;
  load(): Promise<void>;
  toggle(id: string): Promise<void>;
  addManual(name: string): Promise<void>;
  deleteItem(id: string): Promise<void>;
  regenerate(weekStart: string): Promise<GeneratedReport>;
}
```

Counts derive from `items.length` and `items.filter(i => i.ticked).length` rather than round-tripping a separate count query — keeps the store simple.

## 10. Error handling

| Error | User sees | Recovery |
|---|---|---|
| Regenerate transaction fails | Toast "Couldn't regenerate shopping list." + Retry | Retry button |
| List load fails | In-place "Couldn't load" + Retry button | Retry |
| Tick toggle fails | Optimistic UI reverts, toast "Couldn't save" | User retries tap |
| Add-manual fails | Inline error under input, input stays open | Retry/cancel |
| Delete-manual fails | Row stays, inline toast "Couldn't delete" | Retry |
| `useMealPlanStore.weekStart` is missing | Fallback to computed Monday-of-today | Invisible — user regenerates normally |

## 11. Testing strategy

### 11.1 Core unit tests

`crates/core/src/shopping_list/`:

- **`generator.rs`**:
  - Happy path: 2 meals planned, 6 total ingredients, 1 staple match → 5 items, `items_skipped_staple = 1`.
  - Ghost recipe on plan → `ghost_recipes_skipped += 1`, no ingredients added for it.
  - No meals planned for week → existing generated rows wiped, 0 added.
  - Manual rows present before regenerate → untouched across regenerate, positions preserved.
  - Duplicate ingredient across two recipes → two rows, no dedup.
  - Idempotent: regenerate twice with identical inputs → same output rows (modulo new UUIDs).
- **`dal.rs`**:
  - Insert manual + generated → list ordering (ticked ASC, position ASC).
  - Toggle tick → row sinks (position unchanged but ticked=1; list query re-orders).
  - Toggle tick twice → rises back.
  - Delete manual → row gone.
  - Wipe generated → only generated rows removed.
  - `insert_manual` position lands at `MAX(position) + 1` across all source types.

### 11.2 Integration (`crates/app`)

- `shopping_list_regenerate` end-to-end: seed DB with recipe + ingredients + staple + meal plan entry, call command, assert report + list contents.
- `shopping_list_add_manual` + `toggle` + `delete` round-trip.

### 11.3 Frontend

React Testing Library:
- List renders with mocked IPC; ticked rows rendered with strikethrough class.
- Tick toggle calls IPC + optimistic UI swap.
- Empty-state branches render the right CTA per state.
- Add-item inline Enter creates manual row via IPC.

## 12. Out of scope for L3c (pinned to prevent drift)

- Quantity summation across duplicate ingredients.
- Unit normalisation (grams ↔ cups ↔ "a handful").
- Aisle-category grouping (Produce / Dairy / Pantry).
- Share/export the list (copy as markdown, send to another device).
- Multiple named lists ("Tesco" vs "Ocado").
- Recurring "always-buy" items (considered for v0.4.1).
- iOS companion app read-only access (v1.0 Companion).
- Barcode scanner, cost tracking, receipt OCR.
- Price lookups, budget links to Ledger.
- Drag-to-reorder rows.
- Bulk operations (tick all, clear all ticked).

## 13. Definition of done

- Migration V17 runs cleanly on fresh + existing dev DBs.
- Hearth sub-nav has four tabs; Shopping default renders empty-state on first visit.
- Regenerate from current week works end-to-end: staples excluded, ghost recipes skipped, report toast on ghosts.
- Manual items add inline, persist across regenerations, remove with one tap.
- Tick collapses row to bottom visually; untick rises. Strikethrough + opacity applied.
- `cargo test --workspace` green. Clippy clean. TypeScript clean. Production build green.
- Manual QA pass: Sunday-night-plan → Saturday-shop cycle walks clean.

---

*End of L3c design spec. Next: implementation plan.*
