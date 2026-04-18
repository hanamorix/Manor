# L3c Shopping List Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Manor's shopping list: a fourth Hearth sub-tab that regenerates from the week shown in This Week, subtracts staples via L3b's matcher, accepts manual items that survive regenerations, and sinks ticked rows to the bottom.

**Architecture:** New `shopping_list` core module with DAL + pure regenerator. Thin Tauri layer. React UI with Zustand store. Reuses `staple_matches` + `meal_plan::dal::get_week` + `recipe::dal::get_recipe_including_trashed` from L3a/L3b.

**Tech Stack:** Rust (rusqlite, refinery, chrono), React + TypeScript + Zustand, Lucide icons.

**Spec:** `docs/superpowers/specs/2026-04-18-l3c-shopping-list-design.md`

---

## File structure

### New Rust files
- `crates/core/migrations/V17__shopping_list.sql`
- `crates/core/src/shopping_list/mod.rs` — types + module root.
- `crates/core/src/shopping_list/dal.rs` — item CRUD + list + toggle.
- `crates/core/src/shopping_list/generator.rs` — `regenerate_from_week`.
- `crates/app/src/shopping_list/mod.rs` — module root.
- `crates/app/src/shopping_list/commands.rs` — Tauri IPC.

### New frontend files
- `apps/desktop/src/lib/shopping_list/ipc.ts`
- `apps/desktop/src/lib/shopping_list/state.ts`
- `apps/desktop/src/components/Hearth/Shopping/ShoppingView.tsx`
- `apps/desktop/src/components/Hearth/Shopping/ShoppingItemRow.tsx`

### Modified files
- `crates/core/src/lib.rs` — `pub mod shopping_list;`.
- `crates/app/src/lib.rs` — register new Tauri commands + `pub mod shopping_list;`.
- `apps/desktop/src/lib/hearth/view-state.ts` — extend `HearthSubview` union with `"shopping"`.
- `apps/desktop/src/components/Hearth/HearthSubNav.tsx` — add Shopping tab.
- `apps/desktop/src/components/Hearth/HearthTab.tsx` — route `"shopping"` to `<ShoppingView />`.

---

## Task 1: Migration V17

**Files:** Create `crates/core/migrations/V17__shopping_list.sql`.

- [ ] **Step 1: Write migration SQL**

```sql
-- V17__shopping_list.sql
-- L3c Shopping List: single always-on shopping list items.

CREATE TABLE shopping_list_item (
    id              TEXT PRIMARY KEY,
    ingredient_name TEXT NOT NULL,
    quantity_text   TEXT,
    note            TEXT,
    recipe_id       TEXT REFERENCES recipe(id),
    recipe_title    TEXT,
    source          TEXT NOT NULL,
    position        INTEGER NOT NULL,
    ticked          INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX idx_shopping_item_order  ON shopping_list_item(ticked, position);
CREATE INDEX idx_shopping_item_source ON shopping_list_item(source);
```

- [ ] **Step 2: Verify refinery picks it up**

Run: `cargo test -p manor-core --lib -- migrations`
Expected: green.

- [ ] **Step 3: Commit**

```bash
git add crates/core/migrations/V17__shopping_list.sql
git commit -m "feat(shopping_list): migration V17 — shopping_list_item"
```

---

## Task 2: Core types + DAL

**Files:**
- Create: `crates/core/src/shopping_list/mod.rs`
- Create: `crates/core/src/shopping_list/dal.rs`
- Stub: `crates/core/src/shopping_list/generator.rs` (filled in Task 3)
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: `mod.rs` with types**

```rust
//! Shopping list — types + CRUD + regenerator. Pure data layer.

pub mod dal;
pub mod generator;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemSource {
    Generated,
    Manual,
}

impl ItemSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemSource::Generated => "generated",
            ItemSource::Manual => "manual",
        }
    }
    pub fn from_db(s: Option<&str>) -> Self {
        match s {
            Some("generated") => Self::Generated,
            _ => Self::Manual,
        }
    }
}

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeneratedReport {
    pub items_added: usize,
    pub items_skipped_staple: usize,
    pub ghost_recipes_skipped: usize,
}
```

- [ ] **Step 2: Stub `generator.rs`**

```rust
//! Shopping list regenerator — filled in Task 3.
```

- [ ] **Step 3: Add `pub mod shopping_list;` to `crates/core/src/lib.rs`**

Insert alphabetically in the existing module list.

- [ ] **Step 4: Write failing DAL tests**

Create `crates/core/src/shopping_list/dal.rs`:

```rust
//! Shopping list DAL: list/insert/toggle/delete/wipe.

use super::{ItemSource, ShoppingListItem};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_secs() -> i64 { chrono::Utc::now().timestamp() }

pub fn list_items(conn: &Connection) -> Result<Vec<ShoppingListItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, ingredient_name, quantity_text, note, recipe_id, recipe_title,
                source, position, ticked, created_at, updated_at
         FROM shopping_list_item
         ORDER BY ticked ASC, position ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        let source_s: String = r.get(6)?;
        let ticked_i: i64 = r.get(8)?;
        Ok(ShoppingListItem {
            id: r.get(0)?,
            ingredient_name: r.get(1)?,
            quantity_text: r.get(2)?,
            note: r.get(3)?,
            recipe_id: r.get(4)?,
            recipe_title: r.get(5)?,
            source: ItemSource::from_db(Some(source_s.as_str())),
            position: r.get(7)?,
            ticked: ticked_i != 0,
            created_at: r.get(9)?,
            updated_at: r.get(10)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn insert_manual(conn: &Connection, ingredient_name: &str) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    let pos = next_position(conn)?;
    conn.execute(
        "INSERT INTO shopping_list_item
           (id, ingredient_name, quantity_text, note, recipe_id, recipe_title,
            source, position, ticked, created_at, updated_at)
         VALUES (?1, ?2, NULL, NULL, NULL, NULL, 'manual', ?3, 0, ?4, ?4)",
        params![id, ingredient_name, pos, now],
    )?;
    Ok(id)
}

pub fn toggle_tick(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    conn.execute(
        "UPDATE shopping_list_item SET ticked = 1 - ticked, updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn delete_item(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM shopping_list_item WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn wipe_generated(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM shopping_list_item WHERE source = 'generated'", [])?;
    Ok(())
}

pub fn counts(conn: &Connection) -> Result<(usize, usize)> {
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM shopping_list_item", [], |r| r.get(0),
    )?;
    let ticked: i64 = conn.query_row(
        "SELECT COUNT(*) FROM shopping_list_item WHERE ticked = 1", [], |r| r.get(0),
    )?;
    Ok((total as usize, ticked as usize))
}

pub(crate) fn next_position(conn: &Connection) -> Result<i64> {
    let max: Option<i64> = conn.query_row(
        "SELECT MAX(position) FROM shopping_list_item", [], |r| r.get(0),
    ).optional()?.flatten();
    Ok(max.map(|m| m + 1).unwrap_or(0))
}

/// Insert a generated row. Called by the generator only.
pub(crate) fn insert_generated(
    conn: &Connection,
    ingredient_name: &str,
    quantity_text: Option<&str>,
    note: Option<&str>,
    recipe_id: &str,
    recipe_title: &str,
    position: i64,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    conn.execute(
        "INSERT INTO shopping_list_item
           (id, ingredient_name, quantity_text, note, recipe_id, recipe_title,
            source, position, ticked, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'generated', ?7, 0, ?8, ?8)",
        params![id, ingredient_name, quantity_text, note, recipe_id, recipe_title, position, now],
    )?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn insert_manual_roundtrips_with_expected_defaults() {
        let (_d, conn) = fresh();
        let id = insert_manual(&conn, "bin bags").unwrap();
        let list = list_items(&conn).unwrap();
        assert_eq!(list.len(), 1);
        let item = &list[0];
        assert_eq!(item.id, id);
        assert_eq!(item.ingredient_name, "bin bags");
        assert!(item.quantity_text.is_none());
        assert!(item.recipe_id.is_none());
        assert_eq!(item.source, ItemSource::Manual);
        assert!(!item.ticked);
    }

    #[test]
    fn manual_positions_land_after_all_existing_rows() {
        let (_d, conn) = fresh();
        insert_generated(&conn, "salt", None, None, "r1", "Recipe 1", 0).unwrap();
        insert_generated(&conn, "garlic", None, None, "r1", "Recipe 1", 1).unwrap();
        let manual_id = insert_manual(&conn, "bin bags").unwrap();
        let list = list_items(&conn).unwrap();
        // Expect order: salt (pos 0), garlic (pos 1), bin bags (pos 2).
        assert_eq!(list[2].id, manual_id);
        assert_eq!(list[2].position, 2);
    }

    #[test]
    fn list_orders_ticked_to_bottom_preserving_position() {
        let (_d, conn) = fresh();
        let a = insert_manual(&conn, "A").unwrap();
        let b = insert_manual(&conn, "B").unwrap();
        let c = insert_manual(&conn, "C").unwrap();
        toggle_tick(&conn, &b).unwrap();
        let list = list_items(&conn).unwrap();
        let names: Vec<_> = list.iter().map(|i| i.ingredient_name.as_str()).collect();
        assert_eq!(names, vec!["A", "C", "B"]);
        let _ = (a, c);
    }

    #[test]
    fn toggle_tick_flips() {
        let (_d, conn) = fresh();
        let id = insert_manual(&conn, "milk").unwrap();
        toggle_tick(&conn, &id).unwrap();
        assert!(list_items(&conn).unwrap()[0].ticked);
        toggle_tick(&conn, &id).unwrap();
        assert!(!list_items(&conn).unwrap()[0].ticked);
    }

    #[test]
    fn delete_item_removes_row() {
        let (_d, conn) = fresh();
        let id = insert_manual(&conn, "A").unwrap();
        delete_item(&conn, &id).unwrap();
        assert!(list_items(&conn).unwrap().is_empty());
    }

    #[test]
    fn wipe_generated_leaves_manual_untouched() {
        let (_d, conn) = fresh();
        insert_generated(&conn, "salt", None, None, "r1", "R1", 0).unwrap();
        let manual = insert_manual(&conn, "bin bags").unwrap();
        wipe_generated(&conn).unwrap();
        let list = list_items(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, manual);
    }

    #[test]
    fn counts_total_and_ticked() {
        let (_d, conn) = fresh();
        let a = insert_manual(&conn, "A").unwrap();
        let _b = insert_manual(&conn, "B").unwrap();
        let _c = insert_manual(&conn, "C").unwrap();
        toggle_tick(&conn, &a).unwrap();
        let (total, ticked) = counts(&conn).unwrap();
        assert_eq!(total, 3);
        assert_eq!(ticked, 1);
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p manor-core --lib shopping_list::dal`
Expected: 7 PASS.

Run: `cargo test --workspace --lib`
Expected: +7 (baseline 301 → 308).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/shopping_list/ crates/core/src/lib.rs
git commit -m "feat(shopping_list): types + DAL (list/insert/toggle/delete/wipe/counts)"
```

---

## Task 3: Regenerator

**Files:** Modify `crates/core/src/shopping_list/generator.rs`.

- [ ] **Step 1: Write failing tests**

Overwrite the stub with tests + implementation:

```rust
//! Shopping list regenerator — pure function.

use super::dal;
use super::GeneratedReport;
use anyhow::Result;
use rusqlite::Connection;

/// Regenerate the shopping list from the planned week starting at `week_start` (ISO YYYY-MM-DD).
/// Wipes rows where source='generated'; leaves manual rows. Appends newly-generated items
/// after existing manual rows in position space.
pub fn regenerate_from_week(conn: &Connection, week_start: &str) -> Result<GeneratedReport> {
    let staples = crate::meal_plan::staples::list_staples(conn)?;
    let entries = crate::meal_plan::dal::get_week(conn, week_start)?;

    let tx = conn.unchecked_transaction()?;

    dal::wipe_generated(&tx)?;
    let mut next_pos: i64 = dal::next_position(&tx)?;

    let mut report = GeneratedReport::default();

    for entry in entries {
        let Some(recipe_id) = entry.recipe_id else { continue };
        let Some(recipe) = crate::recipe::dal::get_recipe_including_trashed(&tx, &recipe_id)? else {
            continue;
        };
        if recipe.deleted_at.is_some() {
            report.ghost_recipes_skipped += 1;
            continue;
        }
        for ing in &recipe.ingredients {
            let matched = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::meal_plan::matcher::staple_matches(&ing.ingredient_name, &staples)
            })).unwrap_or(false);
            if matched {
                report.items_skipped_staple += 1;
                continue;
            }
            dal::insert_generated(
                &tx,
                &ing.ingredient_name,
                ing.quantity_text.as_deref(),
                ing.note.as_deref(),
                &recipe.id,
                &recipe.title,
                next_pos,
            )?;
            next_pos += 1;
            report.items_added += 1;
        }
    }

    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use crate::recipe::{IngredientLine, ImportMethod, RecipeDraft};
    use crate::meal_plan::{StapleDraft};
    use tempfile::tempdir;

    fn fresh() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn insert_recipe_with(conn: &Connection, title: &str, ingredients: Vec<IngredientLine>) -> String {
        let draft = RecipeDraft {
            title: title.into(),
            servings: None, prep_time_mins: None, cook_time_mins: None,
            instructions: "".into(),
            source_url: None, source_host: None,
            import_method: ImportMethod::Manual,
            hero_attachment_uuid: None,
            ingredients,
        };
        crate::recipe::dal::insert_recipe(conn, &draft).unwrap()
    }

    fn line(name: &str) -> IngredientLine {
        IngredientLine { quantity_text: None, ingredient_name: name.into(), note: None }
    }

    #[test]
    fn happy_path_generates_minus_staples() {
        let (_d, conn) = fresh();
        let rid = insert_recipe_with(&conn, "Miso", vec![line("aubergine"), line("miso paste"), line("salt")]);
        crate::meal_plan::staples::insert_staple(&conn, &StapleDraft {
            name: "salt".into(), aliases: vec![],
        }).unwrap();
        crate::meal_plan::dal::set_entry(&conn, "2026-04-22", &rid).unwrap();

        let report = regenerate_from_week(&conn, "2026-04-20").unwrap();
        assert_eq!(report.items_added, 2);
        assert_eq!(report.items_skipped_staple, 1);
        assert_eq!(report.ghost_recipes_skipped, 0);

        let items = dal::list_items(&conn).unwrap();
        assert_eq!(items.len(), 2);
        let names: Vec<_> = items.iter().map(|i| i.ingredient_name.as_str()).collect();
        assert_eq!(names, vec!["aubergine", "miso paste"]);
    }

    #[test]
    fn ghost_recipe_is_skipped() {
        let (_d, conn) = fresh();
        let rid = insert_recipe_with(&conn, "Gone", vec![line("x")]);
        crate::meal_plan::dal::set_entry(&conn, "2026-04-22", &rid).unwrap();
        crate::recipe::dal::soft_delete_recipe(&conn, &rid).unwrap();

        let report = regenerate_from_week(&conn, "2026-04-20").unwrap();
        assert_eq!(report.ghost_recipes_skipped, 1);
        assert_eq!(report.items_added, 0);
        assert!(dal::list_items(&conn).unwrap().is_empty());
    }

    #[test]
    fn no_meals_wipes_existing_generated_and_keeps_manual() {
        let (_d, conn) = fresh();
        // Pre-seed: a generated leftover + a manual row.
        dal::insert_generated(&conn, "stale", None, None, "xxx", "xxx", 0).unwrap();
        let manual_id = dal::insert_manual(&conn, "bin bags").unwrap();

        let report = regenerate_from_week(&conn, "2026-04-20").unwrap();
        assert_eq!(report.items_added, 0);

        let list = dal::list_items(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, manual_id);
    }

    #[test]
    fn duplicate_ingredients_across_recipes_keep_both() {
        let (_d, conn) = fresh();
        let r1 = insert_recipe_with(&conn, "Miso", vec![line("onion")]);
        let r2 = insert_recipe_with(&conn, "Dal", vec![line("onion")]);
        crate::meal_plan::dal::set_entry(&conn, "2026-04-22", &r1).unwrap();
        crate::meal_plan::dal::set_entry(&conn, "2026-04-23", &r2).unwrap();

        let report = regenerate_from_week(&conn, "2026-04-20").unwrap();
        assert_eq!(report.items_added, 2);
        let items = dal::list_items(&conn).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].recipe_title.as_deref(), Some("Miso"));
        assert_eq!(items[1].recipe_title.as_deref(), Some("Dal"));
    }

    #[test]
    fn idempotent_when_called_twice() {
        let (_d, conn) = fresh();
        let rid = insert_recipe_with(&conn, "Miso", vec![line("onion"), line("aubergine")]);
        crate::meal_plan::dal::set_entry(&conn, "2026-04-22", &rid).unwrap();

        regenerate_from_week(&conn, "2026-04-20").unwrap();
        let first_count = dal::list_items(&conn).unwrap().len();
        regenerate_from_week(&conn, "2026-04-20").unwrap();
        let second_count = dal::list_items(&conn).unwrap().len();

        assert_eq!(first_count, 2);
        assert_eq!(second_count, 2);
    }

    #[test]
    fn manual_items_survive_and_stay_at_top_positions() {
        let (_d, conn) = fresh();
        let manual = dal::insert_manual(&conn, "bin bags").unwrap();
        let rid = insert_recipe_with(&conn, "Miso", vec![line("onion")]);
        crate::meal_plan::dal::set_entry(&conn, "2026-04-22", &rid).unwrap();

        regenerate_from_week(&conn, "2026-04-20").unwrap();
        let items = dal::list_items(&conn).unwrap();
        assert_eq!(items.len(), 2);
        // Manual row (position 0) stays first; generated item lands at position 1.
        assert_eq!(items[0].id, manual);
        assert_eq!(items[0].source, super::super::ItemSource::Manual);
        assert_eq!(items[1].ingredient_name, "onion");
        assert_eq!(items[1].source, super::super::ItemSource::Generated);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p manor-core --lib shopping_list::generator`
Expected: 6 PASS.

Run: `cargo test --workspace --lib`
Expected: +6 additional passes.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/shopping_list/generator.rs
git commit -m "feat(shopping_list): regenerator with staple skip + ghost skip + manual preservation"
```

---

## Task 4: Tauri commands

**Files:**
- Create: `crates/app/src/shopping_list/mod.rs`
- Create: `crates/app/src/shopping_list/commands.rs`
- Modify: `crates/app/src/lib.rs`

- [ ] **Step 1: Module root**

`crates/app/src/shopping_list/mod.rs`:

```rust
//! Shopping list — Tauri command layer.

pub mod commands;
```

- [ ] **Step 2: Commands**

Inspect `crates/app/src/meal_plan/commands.rs` first for the canonical `Db` state pattern (it's `use crate::assistant::commands::Db;` + `state.0.lock().map_err(|e| e.to_string())?`).

`crates/app/src/shopping_list/commands.rs`:

```rust
use crate::assistant::commands::Db;
use manor_core::shopping_list::{dal, generator, GeneratedReport, ShoppingListItem};
use tauri::State;

#[tauri::command]
pub fn shopping_list_list(state: State<'_, Db>) -> Result<Vec<ShoppingListItem>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::list_items(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shopping_list_add_manual(ingredient_name: String, state: State<'_, Db>) -> Result<String, String> {
    let name = ingredient_name.trim();
    if name.is_empty() {
        return Err("Ingredient name cannot be empty".into());
    }
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::insert_manual(&conn, name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shopping_list_toggle(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::toggle_tick(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shopping_list_delete(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::delete_item(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shopping_list_regenerate(week_start: String, state: State<'_, Db>) -> Result<GeneratedReport, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    generator::regenerate_from_week(&conn, &week_start).map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register in `crates/app/src/lib.rs`**

Add `pub mod shopping_list;` near other module declarations. Append to the `invoke_handler`:

```rust
            shopping_list::commands::shopping_list_list,
            shopping_list::commands::shopping_list_add_manual,
            shopping_list::commands::shopping_list_toggle,
            shopping_list::commands::shopping_list_delete,
            shopping_list::commands::shopping_list_regenerate,
```

- [ ] **Step 4: Build + clippy**

```bash
cargo build -p manor-app
cargo clippy --workspace -- -D warnings
```
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/shopping_list/ crates/app/src/lib.rs
git commit -m "feat(shopping_list): Tauri commands for list/add/toggle/delete/regenerate"
```

---

## Task 5: Frontend IPC + Zustand store

**Files:**
- Create: `apps/desktop/src/lib/shopping_list/ipc.ts`
- Create: `apps/desktop/src/lib/shopping_list/state.ts`

- [ ] **Step 1: IPC**

```ts
import { invoke } from "@tauri-apps/api/core";

export type ItemSource = "generated" | "manual";

export interface ShoppingListItem {
  id: string;
  ingredient_name: string;
  quantity_text: string | null;
  note: string | null;
  recipe_id: string | null;
  recipe_title: string | null;
  source: ItemSource;
  position: number;
  ticked: boolean;
  created_at: number;
  updated_at: number;
}

export interface GeneratedReport {
  items_added: number;
  items_skipped_staple: number;
  ghost_recipes_skipped: number;
}

export async function list(): Promise<ShoppingListItem[]> {
  return await invoke<ShoppingListItem[]>("shopping_list_list");
}

export async function addManual(ingredientName: string): Promise<string> {
  return await invoke<string>("shopping_list_add_manual", { ingredientName });
}

export async function toggle(id: string): Promise<void> {
  await invoke("shopping_list_toggle", { id });
}

export async function deleteItem(id: string): Promise<void> {
  await invoke("shopping_list_delete", { id });
}

export async function regenerate(weekStart: string): Promise<GeneratedReport> {
  return await invoke<GeneratedReport>("shopping_list_regenerate", { weekStart });
}
```

- [ ] **Step 2: Store**

```ts
import { create } from "zustand";
import * as ipc from "./ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface ShoppingListStore {
  items: ipc.ShoppingListItem[];
  loadStatus: LoadStatus;
  lastReport: ipc.GeneratedReport | null;

  load(): Promise<void>;
  toggle(id: string): Promise<void>;
  addManual(name: string): Promise<void>;
  deleteItem(id: string): Promise<void>;
  regenerate(weekStart: string): Promise<ipc.GeneratedReport>;
}

export const useShoppingListStore = create<ShoppingListStore>((set, get) => ({
  items: [],
  loadStatus: { kind: "idle" },
  lastReport: null,

  async load() {
    set({ loadStatus: { kind: "loading" } });
    try {
      const items = await ipc.list();
      set({ items, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  async toggle(id) {
    // Optimistic: flip locally, revert on error.
    const before = get().items;
    set({
      items: before.map((i) => i.id === id ? { ...i, ticked: !i.ticked } : i),
    });
    try {
      await ipc.toggle(id);
      await get().load();
    } catch (e: unknown) {
      set({ items: before });
      throw e;
    }
  },

  async addManual(name) {
    await ipc.addManual(name);
    await get().load();
  },

  async deleteItem(id) {
    await ipc.deleteItem(id);
    await get().load();
  },

  async regenerate(weekStart) {
    const report = await ipc.regenerate(weekStart);
    set({ lastReport: report });
    await get().load();
    return report;
  },
}));
```

- [ ] **Step 3: Typecheck**

```bash
cd apps/desktop && pnpm tsc --noEmit
```
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/lib/shopping_list/
git commit -m "feat(shopping_list): frontend IPC + Zustand store"
```

---

## Task 6: Extend Hearth sub-nav with Shopping tab

**Files:**
- Modify: `apps/desktop/src/lib/hearth/view-state.ts`
- Modify: `apps/desktop/src/components/Hearth/HearthSubNav.tsx`
- Modify: `apps/desktop/src/components/Hearth/HearthTab.tsx`
- Create stub: `apps/desktop/src/components/Hearth/Shopping/ShoppingView.tsx`

- [ ] **Step 1: Extend the HearthSubview union**

In `apps/desktop/src/lib/hearth/view-state.ts` change:

```ts
export type HearthSubview = "recipes" | "this_week" | "staples";
```

to:

```ts
export type HearthSubview = "recipes" | "this_week" | "shopping" | "staples";
```

Also extend the `hydrate()` validation list to accept `"shopping"`:

```ts
if (v === "recipes" || v === "this_week" || v === "shopping" || v === "staples") {
  set({ subview: v, hydrated: true });
}
```

- [ ] **Step 2: Add Shopping to the TABS array**

Modify `apps/desktop/src/components/Hearth/HearthSubNav.tsx`:

```tsx
const TABS: { key: HearthSubview; label: string }[] = [
  { key: "recipes", label: "Recipes" },
  { key: "this_week", label: "This Week" },
  { key: "shopping", label: "Shopping" },
  { key: "staples", label: "Staples" },
];
```

- [ ] **Step 3: Route `"shopping"` in HearthTab**

Modify `apps/desktop/src/components/Hearth/HearthTab.tsx`. Add import + new branch:

```tsx
import { ShoppingView } from "./Shopping/ShoppingView";
// ...
{subview === "shopping" && <ShoppingView />}
```

- [ ] **Step 4: Stub ShoppingView**

Create `apps/desktop/src/components/Hearth/Shopping/ShoppingView.tsx`:

```tsx
export function ShoppingView() {
  return <p style={{ color: "var(--ink-soft, #999)" }}>Shopping view — coming in Task 7.</p>;
}
```

- [ ] **Step 5: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/lib/hearth/ apps/desktop/src/components/Hearth/
git commit -m "feat(shopping_list): add Shopping to Hearth sub-nav + stub view"
```

---

## Task 7: Shopping list view (full UI)

**Files:**
- Create: `apps/desktop/src/components/Hearth/Shopping/ShoppingItemRow.tsx`
- Overwrite: `apps/desktop/src/components/Hearth/Shopping/ShoppingView.tsx`

- [ ] **Step 1: Row component**

Create `apps/desktop/src/components/Hearth/Shopping/ShoppingItemRow.tsx`:

```tsx
import { X } from "lucide-react";
import type { ShoppingListItem } from "../../../lib/shopping_list/ipc";

interface Props {
  item: ShoppingListItem;
  onToggle: () => void;
  onDelete?: () => void;  // undefined for generated items (no manual delete)
}

export function ShoppingItemRow({ item, onToggle, onDelete }: Props) {
  const label = [
    item.quantity_text ?? "",
    item.ingredient_name,
    item.note ? `, ${item.note}` : "",
  ].join(" ").trim().replace(/ +,/g, ",");

  const meta = item.source === "manual"
    ? "manual"
    : (item.recipe_title ?? "recipe");

  return (
    <div
      onClick={onToggle}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "10px 12px",
        borderBottom: "1px solid var(--hairline, #e5e5e5)",
        cursor: "pointer",
        opacity: item.ticked ? 0.5 : 1,
      }}
    >
      <input
        type="checkbox"
        checked={item.ticked}
        onChange={onToggle}
        onClick={(e) => e.stopPropagation()}
        aria-label={`Tick ${item.ingredient_name}`}
      />
      <div style={{
        flex: 1,
        fontSize: 14,
        textDecoration: item.ticked ? "line-through" : "none",
        whiteSpace: "nowrap",
        overflow: "hidden",
        textOverflow: "ellipsis",
      }}>
        {label}
      </div>
      <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
        · {meta}
      </div>
      {onDelete && (
        <button
          type="button"
          aria-label="Remove item"
          onClick={(e) => { e.stopPropagation(); onDelete(); }}
          style={{ background: "transparent", border: "none", cursor: "pointer", padding: 4 }}
        >
          <X size={14} strokeWidth={1.8} />
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Full view**

Overwrite `apps/desktop/src/components/Hearth/Shopping/ShoppingView.tsx`:

```tsx
import { useEffect, useState } from "react";
import { Plus, RefreshCw } from "lucide-react";
import { useShoppingListStore } from "../../../lib/shopping_list/state";
import { useMealPlanStore } from "../../../lib/meal_plan/meal-plan-state";
import { useHearthViewStore } from "../../../lib/hearth/view-state";
import { ShoppingItemRow } from "./ShoppingItemRow";

function formatWeekRange(weekStart: string): string {
  const start = new Date(weekStart + "T00:00:00");
  const end = new Date(start);
  end.setDate(start.getDate() + 6);
  const fmt = (d: Date, opts: Intl.DateTimeFormatOptions) =>
    d.toLocaleDateString(undefined, opts);
  const sameMonth = start.getMonth() === end.getMonth();
  return sameMonth
    ? `${fmt(start, { month: "short", day: "numeric" })}–${fmt(end, { day: "numeric" })}`
    : `${fmt(start, { month: "short", day: "numeric" })} – ${fmt(end, { month: "short", day: "numeric" })}`;
}

export function ShoppingView() {
  const { items, loadStatus, load, toggle, addManual, deleteItem, regenerate } = useShoppingListStore();
  const { weekStart } = useMealPlanStore();
  const { setSubview } = useHearthViewStore();
  const [adding, setAdding] = useState(false);
  const [newName, setNewName] = useState("");
  const [toast, setToast] = useState<string | null>(null);

  useEffect(() => { void load(); }, [load]);

  const total = items.length;
  const ticked = items.filter((i) => i.ticked).length;
  const generatedCount = items.filter((i) => i.source === "generated").length;
  const manualCount = items.filter((i) => i.source === "manual").length;

  const submitNew = async () => {
    const v = newName.trim();
    if (!v) { setAdding(false); return; }
    try { await addManual(v); } catch (e: unknown) {
      setToast(e instanceof Error ? e.message : String(e));
    }
    setNewName("");
    setAdding(false);
  };

  const doRegenerate = async () => {
    const range = formatWeekRange(weekStart);
    const msg = `Replace ${generatedCount} generated items with a fresh list from ${range}? Your ${manualCount} manual items will be kept.`;
    if (!window.confirm(msg)) return;
    try {
      const report = await regenerate(weekStart);
      if (report.ghost_recipes_skipped > 0) {
        setToast(`Skipped ${report.ghost_recipes_skipped} deleted recipe(s) from your plan.`);
      }
    } catch (e: unknown) {
      setToast(`Couldn't regenerate: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  // Empty states
  const allEmpty = total === 0;
  const noMealsPlanned = false;  // We can't cheaply know this without a round-trip; offer the CTA either way.

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
        <div>
          <div style={{ fontSize: 18, fontWeight: 600 }}>Shopping list</div>
          <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
            {total} item{total === 1 ? "" : "s"} · {ticked} ticked
          </div>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button type="button" onClick={() => setAdding(true)}
            style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <Plus size={14} strokeWidth={1.8} /> Add item
          </button>
          <button type="button" onClick={doRegenerate}
            style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <RefreshCw size={14} strokeWidth={1.8} /> Regenerate
          </button>
        </div>
      </div>

      {loadStatus.kind === "loading" && (
        <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>
      )}
      {loadStatus.kind === "error" && (
        <p style={{ color: "var(--ink-danger, #b00020)" }}>
          {loadStatus.message} — <button type="button" onClick={() => void load()}>Retry</button>
        </p>
      )}

      {adding && (
        <div style={{ padding: "10px 12px", borderBottom: "1px solid var(--hairline, #e5e5e5)" }}>
          <input
            autoFocus
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onBlur={() => void submitNew()}
            onKeyDown={(e) => {
              if (e.key === "Enter") void submitNew();
              if (e.key === "Escape") { setNewName(""); setAdding(false); }
            }}
            placeholder="e.g. bin bags"
            style={{ width: "100%", fontSize: 14, padding: 4 }}
          />
        </div>
      )}

      {loadStatus.kind === "idle" && allEmpty && !adding && (
        <div style={{ padding: 48, textAlign: "center", color: "var(--ink-soft, #999)" }}>
          <div style={{ marginBottom: 16 }}>Your shopping list is empty.</div>
          <div style={{ display: "inline-flex", gap: 8 }}>
            <button type="button" onClick={doRegenerate}
              style={{ display: "flex", alignItems: "center", gap: 4 }}>
              <RefreshCw size={14} strokeWidth={1.8} /> Generate from this week
            </button>
            <button type="button" onClick={() => setSubview("this_week")}>
              Plan meals →
            </button>
          </div>
        </div>
      )}

      {items.map((item) => (
        <ShoppingItemRow
          key={item.id}
          item={item}
          onToggle={() => { void toggle(item.id); }}
          onDelete={item.source === "manual" ? () => { void deleteItem(item.id); } : undefined}
        />
      ))}

      {toast && (
        <div style={{
          position: "fixed", bottom: 24, left: "50%", transform: "translateX(-50%)",
          background: "var(--paper, #fff)", border: "1px solid var(--hairline, #e5e5e5)",
          padding: "8px 16px", borderRadius: 6, fontSize: 13,
          boxShadow: "0 2px 8px rgba(0,0,0,0.1)",
        }}>
          {toast}
          <button type="button" onClick={() => setToast(null)}
            style={{ marginLeft: 12, background: "transparent", border: "none", cursor: "pointer" }}>
            ✕
          </button>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Hearth/Shopping/
git commit -m "feat(shopping_list): Shopping view — list, add inline, toggle, regenerate, empty-state"
```

---

## Task 8: Manual QA pass

- [ ] **Step 1: Full test suite**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l3c-shopping-list
cargo test --workspace
```
Expected: 301 prior + 7 DAL + 6 generator = 314 lib tests + 3 integration.

- [ ] **Step 2: Clippy + typecheck + build**

```bash
cargo clippy --workspace -- -D warnings
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```
Expected: clean.

- [ ] **Step 3: Dev-server golden path**

```bash
pnpm tauri dev
```

Walk:
- Hearth → Shopping tab: empty-state renders with "Generate from this week" + "Plan meals →".
- Click Plan meals → navigates to This Week subview.
- Plan 2 recipes for the current week. Go to Shopping. Click Regenerate, confirm. Items appear with recipe provenance suffixes.
- Add a staple matching one of the ingredients (e.g. "salt" if a recipe has salt). Click Regenerate. That ingredient is skipped.
- Add a manual item ("bin bags"). Verify it appears at the bottom of the un-ticked section.
- Click Regenerate again. Verify manual item is preserved.
- Tick "bin bags" → sinks to bottom with strikethrough + opacity.
- Untick "bin bags" → rises back.
- Remove "bin bags" via `×` → row disappears.
- Soft-delete a recipe that's on the plan (Recipes tab → a recipe → Delete). Back to Shopping. Click Regenerate → toast "Skipped 1 deleted recipe(s) from your plan."

- [ ] **Step 4: If all green → Finish**

Invoke `superpowers:finishing-a-development-branch`.

---

## Self-review

**Spec coverage:**
- §3 architecture → Tasks 2–4. ✓
- §4 migration V17 → Task 1. ✓
- §5 types → Task 2. ✓
- §6 regenerator algorithm → Task 3. ✓
- §7 DAL API → Task 2. ✓
- §8 Tauri commands → Task 4. ✓
- §9 UI (sub-nav + view + row + empty states + add-inline + regenerate flow) → Tasks 5–7. ✓
- §10 error handling → inline in Task 7's view via toast + retry + optimistic revert. ✓
- §11 testing strategy — core unit tests in Tasks 2 and 3 (13 tests total). Frontend component tests deferred to Task 8 manual QA. ✓

**Placeholder scan:** None.

**Type consistency:** `ShoppingListItem`, `GeneratedReport`, `ItemSource` used consistently across Rust + TS. `HearthSubview` extension additive. Store method names (`load`, `toggle`, `addManual`, `deleteItem`, `regenerate`) match across ipc.ts/state.ts/ShoppingView.tsx.

---

*End of plan. Next: `superpowers:subagent-driven-development`.*
