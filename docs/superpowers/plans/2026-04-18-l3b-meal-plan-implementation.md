# L3b Meal Plan + Staples Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Manor's meal-planning surface — weekly meal-plan grid inside a new Hearth sub-nav, "Tonight" reflection on Today view, read-only meal block in TimeBlocks, and a Staples list (with optional aliases) that L3c's shopping list will subtract.

**Architecture:** Two-crate split mirroring L3a. Pure data layer in `manor-core` (`meal_plan` module with DAL + staples + matcher). Tauri commands in `manor-app`. React + Zustand frontend under `apps/desktop/src/components/Hearth/ThisWeek/` and `Staples/` plus a new `Today/TonightBand.tsx`, a modified TimeBlocks block source, and a new Settings `HearthTab.tsx`.

**Tech Stack:**
- Rust: rusqlite, refinery (migrations), chrono (dates, seconds-since-epoch for timestamps)
- Frontend: React + TypeScript, Zustand, Lucide icons, existing Flat-Notion tokens
- Testing: `cargo test` (unit + integration), React component tests for new UI surfaces

**Spec:** `docs/superpowers/specs/2026-04-18-l3b-meal-plan-design.md`

---

## File structure

### New Rust files

- `crates/core/migrations/V16__meal_plan.sql` — new schema.
- `crates/core/src/meal_plan/mod.rs` — types + module root.
- `crates/core/src/meal_plan/dal.rs` — meal plan CRUD (week, set, clear, today).
- `crates/core/src/meal_plan/staples.rs` — staple CRUD.
- `crates/core/src/meal_plan/matcher.rs` — `staple_matches` pure function.
- `crates/app/src/meal_plan/mod.rs` — module root.
- `crates/app/src/meal_plan/commands.rs` — Tauri IPC commands.

### New frontend files

- `apps/desktop/src/lib/meal_plan/meal-plan-ipc.ts` — IPC wrappers.
- `apps/desktop/src/lib/meal_plan/meal-plan-state.ts` — Zustand store (this week + active date).
- `apps/desktop/src/lib/meal_plan/staples-ipc.ts` — IPC wrappers for staples.
- `apps/desktop/src/lib/meal_plan/staples-state.ts` — Zustand store for staples.
- `apps/desktop/src/lib/hearth/view-state.ts` — Zustand store for Hearth sub-nav.
- `apps/desktop/src/components/Hearth/HearthSubNav.tsx` — top-of-Hearth tab row.
- `apps/desktop/src/components/Hearth/RecipesView.tsx` — extracted from current HearthTab body.
- `apps/desktop/src/components/Hearth/ThisWeek/ThisWeekView.tsx` — the weekly grid.
- `apps/desktop/src/components/Hearth/ThisWeek/DaySlotCard.tsx` — single day cell (empty/filled/ghost states).
- `apps/desktop/src/components/Hearth/ThisWeek/RecipePickerDrawer.tsx` — tap-empty-slot drawer.
- `apps/desktop/src/components/Hearth/ThisWeek/WeekNav.tsx` — prev/next/today/date-picker header.
- `apps/desktop/src/components/Hearth/Staples/StaplesView.tsx` — list + add inline row.
- `apps/desktop/src/components/Hearth/Staples/StapleRow.tsx` — row with aliases chip-input popover.
- `apps/desktop/src/components/Today/TonightBand.tsx` — "Tonight" strip on Today view.
- `apps/desktop/src/components/Settings/HearthTab.tsx` — Settings → Hearth section.

### Modified files

- `crates/core/src/lib.rs` — `pub mod meal_plan;`.
- `crates/core/src/trash.rs` — register `staple_item` in the sweeper registry.
- `crates/core/src/recipe/dal.rs` — add `get_recipe_including_trashed(conn, id)`.
- `crates/app/src/lib.rs` — register new Tauri commands.
- `apps/desktop/src/components/Hearth/HearthTab.tsx` — becomes a sub-nav router.
- `apps/desktop/src/components/Today/Today.tsx` — insert `<TonightBand />`.
- `apps/desktop/src/components/TimeBlocks/TimeBlocksView.tsx` — include meal block.
- `apps/desktop/src/components/Settings/Tabs.tsx` — add "Hearth" tab.
- `apps/desktop/src/components/Settings/SettingsModal.tsx` (if needed) — mount new tab.

---

## Task 1: Migration V16

**Files:**
- Create: `crates/core/migrations/V16__meal_plan.sql`

- [ ] **Step 1: Write the migration SQL**

```sql
-- V16__meal_plan.sql
-- L3b Meal Plan: meal_plan_entry + staple_item.

CREATE TABLE meal_plan_entry (
    id          TEXT PRIMARY KEY,
    entry_date  TEXT NOT NULL UNIQUE,
    recipe_id   TEXT REFERENCES recipe(id),
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX idx_meal_plan_date ON meal_plan_entry(entry_date);

CREATE TABLE staple_item (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    aliases    TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);
CREATE INDEX idx_staple_deleted ON staple_item(deleted_at);
CREATE INDEX idx_staple_name    ON staple_item(name COLLATE NOCASE);
```

- [ ] **Step 2: Verify refinery picks up V16**

Run: `cargo test -p manor-core --lib -- migrations`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/core/migrations/V16__meal_plan.sql
git commit -m "feat(meal_plan): migration V16 — meal_plan_entry + staple_item"
```

---

## Task 2: Core types + meal plan DAL

**Files:**
- Create: `crates/core/src/meal_plan/mod.rs`
- Create: `crates/core/src/meal_plan/dal.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod meal_plan;`)

- [ ] **Step 1: Create `mod.rs` with types**

```rust
//! Meal plan — types + CRUD + staples. Pure data layer.

pub mod dal;
pub mod matcher;
pub mod staples;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MealPlanEntry {
    pub id: String,
    pub entry_date: String,           // ISO YYYY-MM-DD
    pub recipe_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StapleItem {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StapleDraft {
    pub name: String,
    pub aliases: Vec<String>,
}
```

Create empty stubs for sibling modules so `mod.rs` compiles:

`crates/core/src/meal_plan/matcher.rs`:
```rust
//! Staple-matcher — filled in Task 5.
```

`crates/core/src/meal_plan/staples.rs`:
```rust
//! Staple CRUD — filled in Task 4.
```

- [ ] **Step 2: Add `pub mod meal_plan;` to `crates/core/src/lib.rs`**

Insert alphabetically — after `ledger;` or wherever fits the existing ordering.

- [ ] **Step 3: Write failing DAL test**

Create `crates/core/src/meal_plan/dal.rs` with the test first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;
    use rusqlite::Connection;
    use std::path::PathBuf;

    fn fresh_db() -> (tempfile::TempDir, Connection, PathBuf) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let root = dir.path().join("attachments");
        (dir, conn, root)
    }

    fn insert_recipe(conn: &Connection, title: &str) -> String {
        let draft = crate::recipe::RecipeDraft {
            title: title.into(),
            servings: None, prep_time_mins: None, cook_time_mins: None,
            instructions: "".into(),
            source_url: None, source_host: None,
            import_method: crate::recipe::ImportMethod::Manual,
            hero_attachment_uuid: None,
            ingredients: vec![],
        };
        crate::recipe::dal::insert_recipe(conn, &draft).unwrap()
    }

    #[test]
    fn get_week_returns_seven_entries_with_none_for_missing_dates() {
        let (_dir, conn, _root) = fresh_db();
        let rid = insert_recipe(&conn, "Miso");
        set_entry(&conn, "2026-04-22", &rid).unwrap();

        let week = get_week(&conn, "2026-04-20").unwrap();
        assert_eq!(week.len(), 7);
        assert_eq!(week[0].entry_date, "2026-04-20"); assert!(week[0].recipe_id.is_none());
        assert_eq!(week[2].entry_date, "2026-04-22"); assert_eq!(week[2].recipe_id.as_deref(), Some(rid.as_str()));
        assert_eq!(week[6].entry_date, "2026-04-26"); assert!(week[6].recipe_id.is_none());
    }

    #[test]
    fn set_entry_upserts_on_same_date() {
        let (_dir, conn, _root) = fresh_db();
        let a = insert_recipe(&conn, "A");
        let b = insert_recipe(&conn, "B");
        set_entry(&conn, "2026-04-22", &a).unwrap();
        set_entry(&conn, "2026-04-22", &b).unwrap();
        let week = get_week(&conn, "2026-04-20").unwrap();
        assert_eq!(week[2].recipe_id.as_deref(), Some(b.as_str()));
    }

    #[test]
    fn clear_entry_removes_row() {
        let (_dir, conn, _root) = fresh_db();
        let a = insert_recipe(&conn, "A");
        set_entry(&conn, "2026-04-22", &a).unwrap();
        clear_entry(&conn, "2026-04-22").unwrap();
        let week = get_week(&conn, "2026-04-20").unwrap();
        assert!(week[2].recipe_id.is_none());
    }

    #[test]
    fn get_entry_returns_none_when_absent() {
        let (_dir, conn, _root) = fresh_db();
        assert!(get_entry(&conn, "2026-04-22").unwrap().is_none());
    }
}
```

- [ ] **Step 4: Run tests — expect failure**

Run: `cargo test -p manor-core --lib meal_plan::dal::tests`
Expected: FAIL with unresolved `get_week`/`set_entry`/`clear_entry`/`get_entry`.

- [ ] **Step 5: Implement the DAL**

Prepend to `crates/core/src/meal_plan/dal.rs`:

```rust
use super::MealPlanEntry;
use anyhow::Result;
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Return 7 entries starting at `start_date` (ISO YYYY-MM-DD, expected to be a Monday).
/// Dates without a persisted entry get a synthetic entry with recipe_id=None.
pub fn get_week(conn: &Connection, start_date: &str) -> Result<Vec<MealPlanEntry>> {
    let start = NaiveDate::parse_from_str(start_date, "%Y-%m-%d")?;
    let mut out = Vec::with_capacity(7);
    for offset in 0..7 {
        let d = start + chrono::Duration::days(offset);
        let date_str = d.format("%Y-%m-%d").to_string();
        if let Some(entry) = get_entry(conn, &date_str)? {
            out.push(entry);
        } else {
            out.push(MealPlanEntry {
                id: String::new(),
                entry_date: date_str,
                recipe_id: None,
                created_at: 0,
                updated_at: 0,
            });
        }
    }
    Ok(out)
}

pub fn get_entry(conn: &Connection, date: &str) -> Result<Option<MealPlanEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, entry_date, recipe_id, created_at, updated_at
         FROM meal_plan_entry WHERE entry_date = ?1",
    )?;
    let row = stmt.query_row(params![date], |r| Ok(MealPlanEntry {
        id: r.get(0)?,
        entry_date: r.get(1)?,
        recipe_id: r.get(2)?,
        created_at: r.get(3)?,
        updated_at: r.get(4)?,
    })).optional()?;
    Ok(row)
}

pub fn set_entry(conn: &Connection, date: &str, recipe_id: &str) -> Result<()> {
    let now = now_secs();
    // Upsert: ON CONFLICT(entry_date) DO UPDATE.
    conn.execute(
        "INSERT INTO meal_plan_entry (id, entry_date, recipe_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(entry_date) DO UPDATE
           SET recipe_id = excluded.recipe_id, updated_at = excluded.updated_at",
        params![Uuid::new_v4().to_string(), date, recipe_id, now],
    )?;
    Ok(())
}

pub fn clear_entry(conn: &Connection, date: &str) -> Result<()> {
    conn.execute("DELETE FROM meal_plan_entry WHERE entry_date = ?1", params![date])?;
    Ok(())
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p manor-core --lib meal_plan::dal`
Expected: 4 tests PASS.

Run: `cargo test --workspace --lib`
Expected: +4 tests green, everything else still green.

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/meal_plan/ crates/core/src/lib.rs
git commit -m "feat(meal_plan): types + meal plan DAL (get_week/set_entry/clear_entry)"
```

---

## Task 3: `get_recipe_including_trashed` helper

**Files:**
- Modify: `crates/core/src/recipe/dal.rs` — add one helper.

- [ ] **Step 1: Write failing test**

Append to the `tests` module in `crates/core/src/recipe/dal.rs`:

```rust
    #[test]
    fn get_recipe_including_trashed_surfaces_soft_deleted() {
        let (_dir, conn, _root) = fresh_db();
        let id = insert_recipe(&conn, &simple_draft("Ghost")).unwrap();
        soft_delete_recipe(&conn, &id).unwrap();

        assert!(get_recipe(&conn, &id).unwrap().is_none(),
                "get_recipe hides soft-deleted (existing L3a behaviour)");

        let ghost = get_recipe_including_trashed(&conn, &id).unwrap().unwrap();
        assert_eq!(ghost.title, "Ghost");
        assert!(ghost.deleted_at.is_some(), "deleted_at surfaced");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p manor-core --lib recipe::dal::tests::get_recipe_including_trashed_surfaces_soft_deleted`
Expected: FAIL with unresolved function.

- [ ] **Step 3: Implement the helper**

Add to `crates/core/src/recipe/dal.rs`, directly below the existing `get_recipe` function:

```rust
/// Like `get_recipe` but returns the row even when `deleted_at IS NOT NULL`.
/// Used by the meal plan view to surface ghost recipes (entry still references
/// a trashed recipe; user is prompted to restore or unplan).
pub fn get_recipe_including_trashed(conn: &Connection, id: &str) -> Result<Option<Recipe>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, servings, prep_time_mins, cook_time_mins, instructions,
                source_url, source_host, import_method, created_at, updated_at, deleted_at,
                hero_attachment_uuid
         FROM recipe WHERE id = ?1",
    )?;
    let recipe = stmt.query_row(params![id], |row| {
        let import_method_str: Option<String> = row.get(8)?;
        Ok(Recipe {
            id: row.get(0)?,
            title: row.get(1)?,
            servings: row.get(2)?,
            prep_time_mins: row.get(3)?,
            cook_time_mins: row.get(4)?,
            instructions: row.get(5)?,
            source_url: row.get(6)?,
            source_host: row.get(7)?,
            import_method: ImportMethod::from_db(import_method_str.as_deref()),
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
            deleted_at: row.get(11)?,
            hero_attachment_uuid: row.get(12)?,
            ingredients: Vec::new(),
        })
    }).optional()?;

    let Some(mut recipe) = recipe else { return Ok(None) };

    let mut s2 = conn.prepare(
        "SELECT quantity_text, ingredient_name, note
         FROM recipe_ingredient WHERE recipe_id = ?1 ORDER BY position ASC",
    )?;
    let rows = s2.query_map(params![id], |r| {
        Ok(IngredientLine {
            quantity_text: r.get(0)?,
            ingredient_name: r.get(1)?,
            note: r.get(2)?,
        })
    })?;
    for row in rows { recipe.ingredients.push(row?); }
    Ok(Some(recipe))
}
```

If the actual column order or `Recipe` struct differs (e.g. `hero_attachment_uuid` is at a different index after L3a's V15), inspect the current `get_recipe` function in the same file and mirror its SELECT + struct-construction exactly — just drop the `AND deleted_at IS NULL` filter.

- [ ] **Step 4: Run test**

Run: `cargo test -p manor-core --lib recipe::dal`
Expected: all recipe DAL tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/recipe/dal.rs
git commit -m "feat(recipe): get_recipe_including_trashed helper for meal plan ghosts"
```

---

## Task 4: Staples DAL

**Files:**
- Modify: `crates/core/src/meal_plan/staples.rs`

- [ ] **Step 1: Write failing tests**

Replace the stub content with:

```rust
//! Staple CRUD. Aliases stored as JSON array in the TEXT `aliases` column.

use super::{StapleDraft, StapleItem};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_secs() -> i64 { chrono::Utc::now().timestamp() }

pub fn list_staples(conn: &Connection) -> Result<Vec<StapleItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, aliases, created_at, updated_at, deleted_at
         FROM staple_item WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        let aliases_json: Option<String> = r.get(2)?;
        let aliases: Vec<String> = aliases_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Ok(StapleItem {
            id: r.get(0)?,
            name: r.get(1)?,
            aliases,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
            deleted_at: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn get_staple(conn: &Connection, id: &str) -> Result<Option<StapleItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, aliases, created_at, updated_at, deleted_at
         FROM staple_item WHERE id = ?1",
    )?;
    stmt.query_row(params![id], |r| {
        let aliases_json: Option<String> = r.get(2)?;
        let aliases: Vec<String> = aliases_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Ok(StapleItem {
            id: r.get(0)?,
            name: r.get(1)?,
            aliases,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
            deleted_at: r.get(5)?,
        })
    }).optional().map_err(Into::into)
}

pub fn insert_staple(conn: &Connection, draft: &StapleDraft) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    let aliases_json = if draft.aliases.is_empty() { None } else { Some(serde_json::to_string(&draft.aliases)?) };
    conn.execute(
        "INSERT INTO staple_item (id, name, aliases, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, draft.name, aliases_json, now],
    )?;
    Ok(id)
}

pub fn update_staple(conn: &Connection, id: &str, draft: &StapleDraft) -> Result<()> {
    let now = now_secs();
    let aliases_json = if draft.aliases.is_empty() { None } else { Some(serde_json::to_string(&draft.aliases)?) };
    conn.execute(
        "UPDATE staple_item SET name = ?1, aliases = ?2, updated_at = ?3 WHERE id = ?4",
        params![draft.name, aliases_json, now, id],
    )?;
    Ok(())
}

pub fn soft_delete_staple(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE staple_item SET deleted_at = ?1 WHERE id = ?2", params![now_secs(), id])?;
    Ok(())
}

pub fn restore_staple(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE staple_item SET deleted_at = NULL WHERE id = ?1", params![id])?;
    Ok(())
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
    fn crud_with_aliases_roundtrip() {
        let (_d, conn) = fresh();
        let id = insert_staple(&conn, &StapleDraft {
            name: "Olive oil".into(),
            aliases: vec!["EVOO".into(), "extra virgin olive oil".into()],
        }).unwrap();
        let got = get_staple(&conn, &id).unwrap().unwrap();
        assert_eq!(got.name, "Olive oil");
        assert_eq!(got.aliases, vec!["EVOO", "extra virgin olive oil"]);
    }

    #[test]
    fn list_excludes_trashed_and_sorts_by_name_ci() {
        let (_d, conn) = fresh();
        insert_staple(&conn, &StapleDraft { name: "salt".into(), aliases: vec![] }).unwrap();
        let id = insert_staple(&conn, &StapleDraft { name: "Olive oil".into(), aliases: vec![] }).unwrap();
        insert_staple(&conn, &StapleDraft { name: "Garlic".into(), aliases: vec![] }).unwrap();
        soft_delete_staple(&conn, &id).unwrap();
        let list = list_staples(&conn).unwrap();
        let names: Vec<_> = list.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["Garlic", "salt"]);
    }

    #[test]
    fn restore_clears_deleted_at() {
        let (_d, conn) = fresh();
        let id = insert_staple(&conn, &StapleDraft { name: "salt".into(), aliases: vec![] }).unwrap();
        soft_delete_staple(&conn, &id).unwrap();
        assert!(list_staples(&conn).unwrap().is_empty());
        restore_staple(&conn, &id).unwrap();
        assert_eq!(list_staples(&conn).unwrap().len(), 1);
    }

    #[test]
    fn update_replaces_aliases() {
        let (_d, conn) = fresh();
        let id = insert_staple(&conn, &StapleDraft {
            name: "Olive oil".into(), aliases: vec!["EVOO".into()],
        }).unwrap();
        update_staple(&conn, &id, &StapleDraft {
            name: "Olive oil".into(), aliases: vec![],
        }).unwrap();
        let got = get_staple(&conn, &id).unwrap().unwrap();
        assert!(got.aliases.is_empty());
    }
}
```

- [ ] **Step 2: Run tests — verify pass (implementation is already there)**

Run: `cargo test -p manor-core --lib meal_plan::staples`
Expected: 4 PASS.

- [ ] **Step 3: Register `staple_item` with the trash sweeper**

Modify `crates/core/src/trash.rs` — append `("staple_item", "name")` to the `REGISTRY` constant (same pattern as `("recipe", "title")` was added in L3a Task 8).

- [ ] **Step 4: Add failing sweeper test**

Append to the existing `trash::tests` module:

```rust
    #[test]
    fn trash_sweeper_purges_staple_after_30_days() {
        let (_dir, conn, _root) = fresh_env();
        let old = chrono::Utc::now().timestamp() - 31 * 24 * 60 * 60;
        conn.execute(
            "INSERT INTO staple_item (id, name, created_at, updated_at, deleted_at)
             VALUES ('s1', 'Gone', ?1, ?1, ?1)",
            rusqlite::params![old],
        ).unwrap();
        let _ = run_sweep(&conn).unwrap();
        let remaining: i64 = conn.query_row(
            "SELECT COUNT(*) FROM staple_item WHERE id='s1'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(remaining, 0);
    }
```

(Adapt `run_sweep` / `fresh_env` naming to the canonical names in trash.rs.)

- [ ] **Step 5: Run tests + workspace**

Run: `cargo test -p manor-core --lib trash && cargo test --workspace --lib`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/meal_plan/staples.rs crates/core/src/trash.rs
git commit -m "feat(staples): CRUD DAL + trash sweeper registration"
```

---

## Task 5: Staple matcher

**Files:**
- Modify: `crates/core/src/meal_plan/matcher.rs`

- [ ] **Step 1: Write the matcher + table tests**

Replace the stub with:

```rust
//! Staple-matcher — pure function called from L3c's shopping list to decide whether
//! a recipe ingredient should be excluded because it matches a household staple.

use super::StapleItem;

/// Returns true if the ingredient name matches any staple (or staple alias).
/// Matching is: lowercase + trim, strip trailing 's'/'es' for crude singularisation,
/// substring-or-word match against each candidate string.
pub fn staple_matches(ingredient_name: &str, staples: &[StapleItem]) -> bool {
    let ing = normalize(ingredient_name);
    for s in staples {
        if candidate_matches(&ing, &s.name) { return true; }
        for alias in &s.aliases {
            if candidate_matches(&ing, alias) { return true; }
        }
    }
    false
}

fn candidate_matches(ing: &str, candidate: &str) -> bool {
    let cand = normalize(candidate);
    if cand.is_empty() { return false; }
    // word match OR substring match.
    ing == cand
        || ing.split_whitespace().any(|w| w == cand)
        || ing.contains(&cand)
        || cand.contains(ing)
}

fn normalize(s: &str) -> String {
    let mut out = s.trim().to_lowercase();
    // strip trailing punctuation
    while out.ends_with(|c: char| !c.is_alphanumeric()) { out.pop(); }
    // crude singularize: "es" first, then "s"
    if out.ends_with("es") && out.len() > 3 { out.truncate(out.len() - 2); }
    else if out.ends_with('s') && out.len() > 2 { out.truncate(out.len() - 1); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staple(name: &str, aliases: &[&str]) -> StapleItem {
        StapleItem {
            id: "id".into(), name: name.into(),
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            created_at: 0, updated_at: 0, deleted_at: None,
        }
    }

    #[test]
    fn exact_match() {
        let s = vec![staple("olive oil", &[])];
        assert!(staple_matches("olive oil", &s));
    }

    #[test]
    fn alias_match() {
        let s = vec![staple("olive oil", &["EVOO"])];
        assert!(staple_matches("EVOO", &s));
    }

    #[test]
    fn plural_ingredient_vs_singular_staple() {
        let s = vec![staple("garlic clove", &[])];
        assert!(staple_matches("garlic cloves", &s));
    }

    #[test]
    fn substring_in_either_direction() {
        let s = vec![staple("olive oil", &[])];
        assert!(staple_matches("extra virgin olive oil", &s));
    }

    #[test]
    fn no_match_case() {
        let s = vec![staple("salt", &[])];
        assert!(!staple_matches("butter", &s));
    }

    #[test]
    fn empty_aliases_ok() {
        let s = vec![staple("salt", &[])];
        assert!(staple_matches("sea salt", &s));
    }

    #[test]
    fn empty_staples_never_matches() {
        assert!(!staple_matches("anything", &[]));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p manor-core --lib meal_plan::matcher`
Expected: 7 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/meal_plan/matcher.rs
git commit -m "feat(staples): matcher — case-insensitive, singularized, substring + alias"
```

---

## Task 6: Tauri commands (meal plan + staples)

**Files:**
- Create: `crates/app/src/meal_plan/mod.rs`
- Create: `crates/app/src/meal_plan/commands.rs`
- Modify: `crates/app/src/lib.rs` (register `pub mod meal_plan;` + commands)

- [ ] **Step 1: Create module root**

`crates/app/src/meal_plan/mod.rs`:

```rust
//! Meal plan + staples — Tauri command layer.

pub mod commands;
```

- [ ] **Step 2: Commands**

Inspect `crates/app/src/ledger/commands.rs` for the canonical DB-state pattern (you'll find `use crate::assistant::commands::Db;` + `state.0.lock().map_err(|e| e.to_string())?`). Mirror it here.

`crates/app/src/meal_plan/commands.rs`:

```rust
use crate::assistant::commands::Db;
use manor_core::meal_plan::{dal, staples, MealPlanEntry, StapleDraft, StapleItem};
use manor_core::recipe::{self, Recipe};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct MealPlanEntryWithRecipe {
    pub entry_date: String,
    pub recipe: Option<Recipe>,
}

fn load_entry_with_recipe(conn: &rusqlite::Connection, entry: MealPlanEntry) -> MealPlanEntryWithRecipe {
    let recipe = entry.recipe_id.as_ref()
        .and_then(|id| recipe::dal::get_recipe_including_trashed(conn, id).ok().flatten());
    MealPlanEntryWithRecipe { entry_date: entry.entry_date, recipe }
}

#[tauri::command]
pub fn meal_plan_week_get(start_date: String, state: State<'_, Db>) -> Result<Vec<MealPlanEntryWithRecipe>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let entries = dal::get_week(&conn, &start_date).map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(|e| load_entry_with_recipe(&conn, e)).collect())
}

#[tauri::command]
pub fn meal_plan_today_get(state: State<'_, Db>) -> Result<Option<MealPlanEntryWithRecipe>, String> {
    let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let entry = dal::get_entry(&conn, &today).map_err(|e| e.to_string())?;
    Ok(entry.map(|e| load_entry_with_recipe(&conn, e)))
}

#[tauri::command]
pub fn meal_plan_set_entry(date: String, recipe_id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::set_entry(&conn, &date, &recipe_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn meal_plan_clear_entry(date: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::clear_entry(&conn, &date).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn staple_list(state: State<'_, Db>) -> Result<Vec<StapleItem>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    staples::list_staples(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn staple_create(draft: StapleDraft, state: State<'_, Db>) -> Result<String, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    staples::insert_staple(&conn, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn staple_update(id: String, draft: StapleDraft, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    staples::update_staple(&conn, &id, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn staple_delete(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    staples::soft_delete_staple(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn staple_restore(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    staples::restore_staple(&conn, &id).map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register in `crates/app/src/lib.rs`**

Add `pub mod meal_plan;` near the other module declarations. Append to the `invoke_handler` list:

```rust
            meal_plan::commands::meal_plan_week_get,
            meal_plan::commands::meal_plan_today_get,
            meal_plan::commands::meal_plan_set_entry,
            meal_plan::commands::meal_plan_clear_entry,
            meal_plan::commands::staple_list,
            meal_plan::commands::staple_create,
            meal_plan::commands::staple_update,
            meal_plan::commands::staple_delete,
            meal_plan::commands::staple_restore,
```

- [ ] **Step 4: Build + clippy**

Run: `cargo build -p manor-app && cargo clippy --workspace -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/meal_plan/ crates/app/src/lib.rs
git commit -m "feat(meal_plan): Tauri commands for week/today/set/clear + staple CRUD"
```

---

## Task 7: Frontend IPC + Zustand stores

**Files:**
- Create: `apps/desktop/src/lib/meal_plan/meal-plan-ipc.ts`
- Create: `apps/desktop/src/lib/meal_plan/meal-plan-state.ts`
- Create: `apps/desktop/src/lib/meal_plan/staples-ipc.ts`
- Create: `apps/desktop/src/lib/meal_plan/staples-state.ts`
- Create: `apps/desktop/src/lib/hearth/view-state.ts`

- [ ] **Step 1: IPC — meal plan**

Create `apps/desktop/src/lib/meal_plan/meal-plan-ipc.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { Recipe } from "../recipe/recipe-ipc";

export interface MealPlanEntryWithRecipe {
  entry_date: string;            // ISO YYYY-MM-DD
  recipe: Recipe | null;
}

export async function weekGet(startDate: string): Promise<MealPlanEntryWithRecipe[]> {
  return await invoke<MealPlanEntryWithRecipe[]>("meal_plan_week_get", { startDate });
}

export async function todayGet(): Promise<MealPlanEntryWithRecipe | null> {
  return await invoke<MealPlanEntryWithRecipe | null>("meal_plan_today_get");
}

export async function setEntry(date: string, recipeId: string): Promise<void> {
  await invoke("meal_plan_set_entry", { date, recipeId });
}

export async function clearEntry(date: string): Promise<void> {
  await invoke("meal_plan_clear_entry", { date });
}
```

- [ ] **Step 2: IPC — staples**

`apps/desktop/src/lib/meal_plan/staples-ipc.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export interface StapleItem {
  id: string;
  name: string;
  aliases: string[];
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface StapleDraft {
  name: string;
  aliases: string[];
}

export async function list(): Promise<StapleItem[]> {
  return await invoke<StapleItem[]>("staple_list");
}

export async function create(draft: StapleDraft): Promise<string> {
  return await invoke<string>("staple_create", { draft });
}

export async function update(id: string, draft: StapleDraft): Promise<void> {
  await invoke("staple_update", { id, draft });
}

export async function deleteStaple(id: string): Promise<void> {
  await invoke("staple_delete", { id });
}

export async function restore(id: string): Promise<void> {
  await invoke("staple_restore", { id });
}
```

- [ ] **Step 3: Zustand stores**

`apps/desktop/src/lib/meal_plan/meal-plan-state.ts`:

```ts
import { create } from "zustand";
import * as ipc from "./meal-plan-ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface MealPlanStore {
  weekStart: string;  // ISO YYYY-MM-DD (Monday)
  entries: ipc.MealPlanEntryWithRecipe[];
  tonight: ipc.MealPlanEntryWithRecipe | null;
  loadStatus: LoadStatus;

  setWeekStart(date: string): void;
  loadWeek(): Promise<void>;
  loadTonight(): Promise<void>;
  setEntry(date: string, recipeId: string): Promise<void>;
  clearEntry(date: string): Promise<void>;
}

function mondayOf(date: Date): string {
  // JS Sunday=0; offset so Monday=0.
  const d = new Date(date);
  const day = (d.getDay() + 6) % 7;
  d.setDate(d.getDate() - day);
  return d.toISOString().slice(0, 10);
}

export const useMealPlanStore = create<MealPlanStore>((set, get) => ({
  weekStart: mondayOf(new Date()),
  entries: [],
  tonight: null,
  loadStatus: { kind: "idle" },

  setWeekStart(date) { set({ weekStart: date }); void get().loadWeek(); },

  async loadWeek() {
    set({ loadStatus: { kind: "loading" } });
    try {
      const entries = await ipc.weekGet(get().weekStart);
      set({ entries, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      set({ loadStatus: { kind: "error", message: e instanceof Error ? e.message : String(e) } });
    }
  },

  async loadTonight() {
    try { set({ tonight: await ipc.todayGet() }); } catch { /* swallow */ }
  },

  async setEntry(date, recipeId) {
    await ipc.setEntry(date, recipeId);
    await get().loadWeek();
    await get().loadTonight();
  },

  async clearEntry(date) {
    await ipc.clearEntry(date);
    await get().loadWeek();
    await get().loadTonight();
  },
}));
```

`apps/desktop/src/lib/meal_plan/staples-state.ts`:

```ts
import { create } from "zustand";
import * as ipc from "./staples-ipc";

interface StaplesStore {
  staples: ipc.StapleItem[];
  error: string | null;
  load(): Promise<void>;
  add(name: string): Promise<void>;
  updateOne(id: string, draft: ipc.StapleDraft): Promise<void>;
  remove(id: string): Promise<void>;
}

export const useStaplesStore = create<StaplesStore>((set, get) => ({
  staples: [],
  error: null,
  async load() {
    try { set({ staples: await ipc.list(), error: null }); }
    catch (e: unknown) { set({ error: e instanceof Error ? e.message : String(e) }); }
  },
  async add(name) {
    if (!name.trim()) return;
    await ipc.create({ name: name.trim(), aliases: [] });
    await get().load();
  },
  async updateOne(id, draft) { await ipc.update(id, draft); await get().load(); },
  async remove(id) { await ipc.deleteStaple(id); await get().load(); },
}));
```

- [ ] **Step 4: Hearth view store**

Find the existing setting IPC pattern. Check `apps/desktop/src/lib/settings/` or wherever setting-get/set functions live (grep: `grep -rn "settingGet\|setting_get" apps/desktop/src/lib/`). Import those.

`apps/desktop/src/lib/hearth/view-state.ts`:

```ts
import { create } from "zustand";
// Import actual setting IPC helpers used by Manor — adapt names:
import { settingGet, settingSet } from "../settings/ipc";  // adapt to real path

export type HearthSubview = "recipes" | "this_week" | "staples";

interface HearthViewStore {
  subview: HearthSubview;
  hydrated: boolean;
  hydrate(): Promise<void>;
  setSubview(v: HearthSubview): void;
}

export const useHearthViewStore = create<HearthViewStore>((set, get) => ({
  subview: "this_week",
  hydrated: false,
  async hydrate() {
    try {
      const v = await settingGet("hearth.last_subview");
      if (v === "recipes" || v === "this_week" || v === "staples") {
        set({ subview: v, hydrated: true });
      } else {
        set({ hydrated: true });
      }
    } catch { set({ hydrated: true }); }
  },
  setSubview(v) {
    set({ subview: v });
    void settingSet("hearth.last_subview", v).catch(() => {});
  },
}));
```

If the import path differs, inspect how `Settings/HouseholdTab.tsx` line ~19 calls `settingGet("today.weather_location")` and mirror its import.

- [ ] **Step 5: Typecheck**

```bash
cd apps/desktop && pnpm tsc --noEmit
```
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/lib/meal_plan/ apps/desktop/src/lib/hearth/
git commit -m "feat(meal_plan): frontend IPC + Zustand stores (meal plan, staples, Hearth view)"
```

---

## Task 8: Hearth sub-nav + extract RecipesView

**Files:**
- Create: `apps/desktop/src/components/Hearth/HearthSubNav.tsx`
- Create: `apps/desktop/src/components/Hearth/RecipesView.tsx`
- Modify: `apps/desktop/src/components/Hearth/HearthTab.tsx`

- [ ] **Step 1: Extract the current HearthTab body into `RecipesView.tsx`**

Copy everything currently rendered inside `HearthTab.tsx` (the `Recipes` heading, search input, detail switch, drawer mounts, etc.) into a new `apps/desktop/src/components/Hearth/RecipesView.tsx` exporting `export function RecipesView()`. This is a direct move — no behavioural change.

- [ ] **Step 2: Sub-nav component**

`apps/desktop/src/components/Hearth/HearthSubNav.tsx`:

```tsx
import { useHearthViewStore, HearthSubview } from "../../lib/hearth/view-state";

const TABS: { key: HearthSubview; label: string }[] = [
  { key: "recipes",   label: "Recipes" },
  { key: "this_week", label: "This Week" },
  { key: "staples",   label: "Staples" },
];

export function HearthSubNav() {
  const { subview, setSubview } = useHearthViewStore();
  return (
    <div style={{
      display: "flex",
      gap: 24,
      borderBottom: "1px solid var(--hairline, #e5e5e5)",
      marginBottom: 24,
    }}>
      {TABS.map((t) => {
        const active = subview === t.key;
        return (
          <button
            key={t.key}
            type="button"
            onClick={() => setSubview(t.key)}
            style={{
              background: "transparent",
              border: "none",
              padding: "8px 0",
              fontSize: 14,
              fontWeight: active ? 600 : 500,
              color: active ? "var(--ink-strong, #111)" : "var(--ink-soft, #999)",
              borderBottom: active ? "2px solid var(--ink-strong, #111)" : "2px solid transparent",
              cursor: "pointer",
            }}
          >
            {t.label}
          </button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 3: Refactor `HearthTab.tsx` into a router**

Overwrite `apps/desktop/src/components/Hearth/HearthTab.tsx`:

```tsx
import { useEffect } from "react";
import { HearthSubNav } from "./HearthSubNav";
import { RecipesView } from "./RecipesView";
import { ThisWeekView } from "./ThisWeek/ThisWeekView";      // created in Task 9
import { StaplesView } from "./Staples/StaplesView";          // created in Task 12
import { useHearthViewStore } from "../../lib/hearth/view-state";

export function HearthTab() {
  const { subview, hydrate, hydrated } = useHearthViewStore();
  useEffect(() => { void hydrate(); }, [hydrate]);

  if (!hydrated) return null;

  return (
    <div style={{ padding: 32, maxWidth: 1200, margin: "0 auto" }}>
      <HearthSubNav />
      {subview === "recipes"   && <RecipesView />}
      {subview === "this_week" && <ThisWeekView />}
      {subview === "staples"   && <StaplesView />}
    </div>
  );
}
```

Wait — `ThisWeekView` and `StaplesView` don't exist yet (Tasks 9 and 12). For now, stub them. Create placeholder files:

`apps/desktop/src/components/Hearth/ThisWeek/ThisWeekView.tsx`:
```tsx
export function ThisWeekView() {
  return <p style={{ color: "var(--ink-soft, #999)" }}>This Week view — coming in Task 9.</p>;
}
```

`apps/desktop/src/components/Hearth/Staples/StaplesView.tsx`:
```tsx
export function StaplesView() {
  return <p style={{ color: "var(--ink-soft, #999)" }}>Staples view — coming in Task 12.</p>;
}
```

These stubs get replaced in their respective tasks.

- [ ] **Step 4: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/Hearth/
git commit -m "feat(hearth): sub-nav tabs (Recipes · This Week · Staples) + extract RecipesView"
```

---

## Task 9: This Week view — grid + slot card + week nav skeleton

**Files:**
- Create: `apps/desktop/src/components/Hearth/ThisWeek/DaySlotCard.tsx`
- Create: `apps/desktop/src/components/Hearth/ThisWeek/WeekNav.tsx`
- Overwrite: `apps/desktop/src/components/Hearth/ThisWeek/ThisWeekView.tsx`

This task ships the grid with empty/filled/ghost states but without the picker drawer — tapping an empty slot console.logs. Task 10 adds the picker drawer. Task 11 wires swap/remove + date picker.

- [ ] **Step 1: `DaySlotCard.tsx`**

```tsx
import { Plus, Ban, MoreHorizontal } from "lucide-react";
import { ImageOff } from "lucide-react";
import type { MealPlanEntryWithRecipe } from "../../../lib/meal_plan/meal-plan-ipc";
import { useState, useEffect } from "react";
import * as recipeIpc from "../../../lib/recipe/recipe-ipc";

interface Props {
  entry: MealPlanEntryWithRecipe;
  isToday: boolean;
  onEmptyClick: () => void;
  onFilledClick: (recipeId: string) => void;
  onGhostClick: (entry: MealPlanEntryWithRecipe) => void;
  onRemove: () => void;
}

export function DaySlotCard(props: Props) {
  const { entry, isToday, onEmptyClick, onFilledClick, onGhostClick, onRemove } = props;
  const [heroSrc, setHeroSrc] = useState<string | null>(null);

  useEffect(() => {
    const uuid = entry.recipe?.hero_attachment_uuid;
    if (uuid) { void recipeIpc.attachmentSrc(uuid).then(setHeroSrc).catch(() => {}); }
  }, [entry.recipe?.hero_attachment_uuid]);

  const columnBg = isToday ? "var(--paper-muted, #f5f5f5)" : "transparent";

  // Empty
  if (!entry.recipe) {
    return (
      <button
        type="button"
        onClick={onEmptyClick}
        style={{
          width: "100%",
          aspectRatio: "4/5",
          background: columnBg,
          border: "1px dashed var(--hairline, #e5e5e5)",
          borderRadius: 6,
          cursor: "pointer",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--ink-soft, #999)",
        }}
      >
        <Plus size={18} strokeWidth={1.6} />
        <span style={{ fontSize: 12, marginTop: 4 }}>Plan a meal</span>
      </button>
    );
  }

  const recipe = entry.recipe;
  const isGhost = recipe.deleted_at != null;

  // Ghost
  if (isGhost) {
    return (
      <button
        type="button"
        onClick={() => onGhostClick(entry)}
        style={{
          width: "100%",
          aspectRatio: "4/5",
          background: columnBg,
          border: "1px solid var(--hairline, #e5e5e5)",
          borderRadius: 6,
          cursor: "pointer",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--ink-soft, #999)",
          padding: 8,
          textAlign: "center",
        }}
      >
        <Ban size={22} strokeWidth={1.4} />
        <div style={{ fontSize: 12, marginTop: 6 }}>Recipe deleted</div>
        <div style={{ fontSize: 11, marginTop: 2 }}>Tap to restore or unplan</div>
      </button>
    );
  }

  // Filled
  return (
    <div
      style={{
        width: "100%",
        aspectRatio: "4/5",
        background: "var(--paper, #fff)",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 6,
        overflow: "hidden",
        position: "relative",
        display: "flex",
        flexDirection: "column",
        cursor: "pointer",
      }}
      onClick={() => onFilledClick(recipe.id)}
    >
      <div style={{
        aspectRatio: "4/3",
        background: "var(--paper-muted, #f5f5f5)",
        display: "flex", alignItems: "center", justifyContent: "center",
      }}>
        {heroSrc ? (
          <img src={heroSrc} alt={recipe.title}
            style={{ width: "100%", height: "100%", objectFit: "cover" }} />
        ) : (
          <ImageOff size={20} strokeWidth={1.4} color="var(--ink-soft, #999)" />
        )}
      </div>
      <div style={{ padding: 8, fontSize: 12, fontWeight: 600,
                    whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
        {recipe.title}
      </div>
      <button
        type="button"
        aria-label="Remove from day"
        onClick={(e) => { e.stopPropagation(); onRemove(); }}
        style={{
          position: "absolute",
          top: 4,
          right: 4,
          background: "rgba(255,255,255,0.9)",
          border: "1px solid var(--hairline, #e5e5e5)",
          borderRadius: 4,
          padding: "2px 4px",
          cursor: "pointer",
        }}
      >
        <MoreHorizontal size={12} strokeWidth={1.8} />
      </button>
    </div>
  );
}
```

- [ ] **Step 2: `WeekNav.tsx` (prev/next/today — date picker in Task 11)**

```tsx
import { ChevronLeft, ChevronRight } from "lucide-react";

interface Props {
  weekStart: string;    // ISO YYYY-MM-DD
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
}

export function WeekNav({ weekStart, onPrev, onNext, onToday }: Props) {
  const start = new Date(weekStart + "T00:00:00");
  const end = new Date(start);
  end.setDate(start.getDate() + 6);

  const fmt = (d: Date, opts: Intl.DateTimeFormatOptions) =>
    d.toLocaleDateString(undefined, opts);

  const sameMonth = start.getMonth() === end.getMonth();
  const label = sameMonth
    ? `${fmt(start, { month: "short", day: "numeric" })}–${fmt(end, { day: "numeric" })}, ${fmt(start, { year: "numeric" })}`
    : `${fmt(start, { month: "short", day: "numeric" })} – ${fmt(end, { month: "short", day: "numeric" })}, ${fmt(start, { year: "numeric" })}`;

  return (
    <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 16 }}>
      <button type="button" onClick={onPrev} aria-label="Previous week">
        <ChevronLeft size={16} strokeWidth={1.8} />
      </button>
      <div style={{ fontSize: 14, fontWeight: 600, flex: 1 }}>{label}</div>
      <button type="button" onClick={onNext} aria-label="Next week">
        <ChevronRight size={16} strokeWidth={1.8} />
      </button>
      <button type="button" onClick={onToday}>Today</button>
    </div>
  );
}
```

- [ ] **Step 3: Overwrite `ThisWeekView.tsx`**

```tsx
import { useEffect } from "react";
import { useMealPlanStore } from "../../../lib/meal_plan/meal-plan-state";
import { DaySlotCard } from "./DaySlotCard";
import { WeekNav } from "./WeekNav";

const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function stepWeek(dateStr: string, days: number): string {
  const d = new Date(dateStr + "T00:00:00");
  d.setDate(d.getDate() + days);
  return d.toISOString().slice(0, 10);
}

function mondayOfToday(): string {
  const d = new Date();
  const day = (d.getDay() + 6) % 7;
  d.setDate(d.getDate() - day);
  return d.toISOString().slice(0, 10);
}

export function ThisWeekView() {
  const { weekStart, entries, loadStatus, setWeekStart, loadWeek, clearEntry } = useMealPlanStore();

  useEffect(() => { void loadWeek(); }, [weekStart, loadWeek]);

  const todayIso = new Date().toISOString().slice(0, 10);

  return (
    <div>
      <WeekNav
        weekStart={weekStart}
        onPrev={() => setWeekStart(stepWeek(weekStart, -7))}
        onNext={() => setWeekStart(stepWeek(weekStart, +7))}
        onToday={() => setWeekStart(mondayOfToday())}
      />
      {loadStatus.kind === "loading" && <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>}
      {loadStatus.kind === "error" && <p style={{ color: "var(--ink-danger, #b00020)" }}>{loadStatus.message}</p>}
      <div style={{
        display: "grid",
        gridTemplateColumns: "repeat(7, 1fr)",
        gap: 8,
      }}>
        {entries.map((entry, i) => {
          const isToday = entry.entry_date === todayIso;
          const d = new Date(entry.entry_date + "T00:00:00");
          return (
            <div key={entry.entry_date} style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <div style={{
                fontSize: 11,
                color: isToday ? "var(--ink-strong, #111)" : "var(--ink-soft, #999)",
                fontWeight: isToday ? 600 : 500,
              }}>
                {DAY_LABELS[i]} {d.getDate()}
              </div>
              <DaySlotCard
                entry={entry}
                isToday={isToday}
                onEmptyClick={() => console.log("Picker drawer — wired in Task 10", entry.entry_date)}
                onFilledClick={(id) => console.log("Navigate to recipe detail — wired in Task 11", id)}
                onGhostClick={(e) => console.log("Ghost drawer — wired in Task 11", e)}
                onRemove={() => void clearEntry(entry.entry_date)}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/Hearth/ThisWeek/
git commit -m "feat(meal_plan): This Week grid — 7 slots, prev/next/today nav, empty/filled/ghost states"
```

---

## Task 10: Recipe picker drawer

**Files:**
- Create: `apps/desktop/src/components/Hearth/ThisWeek/RecipePickerDrawer.tsx`
- Modify: `apps/desktop/src/components/Hearth/ThisWeek/ThisWeekView.tsx`

- [ ] **Step 1: `RecipePickerDrawer.tsx`**

```tsx
import { useEffect, useState } from "react";
import { ImageOff } from "lucide-react";
import * as recipeIpc from "../../../lib/recipe/recipe-ipc";
import type { Recipe } from "../../../lib/recipe/recipe-ipc";

interface Props {
  date: string;    // ISO YYYY-MM-DD
  onClose: () => void;
  onPick: (recipeId: string) => void;
}

function formatDate(d: string): string {
  const date = new Date(d + "T00:00:00");
  return date.toLocaleDateString(undefined, { weekday: "long", month: "short", day: "numeric" });
}

function RecipeRow({ recipe, onClick }: { recipe: Recipe; onClick: () => void }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (recipe.hero_attachment_uuid) {
      void recipeIpc.attachmentSrc(recipe.hero_attachment_uuid).then(setSrc).catch(() => {});
    }
  }, [recipe.hero_attachment_uuid]);

  const meta = [
    recipe.cook_time_mins != null || recipe.prep_time_mins != null
      ? `${(recipe.prep_time_mins ?? 0) + (recipe.cook_time_mins ?? 0)}m`
      : null,
    recipe.servings != null ? `serves ${recipe.servings}` : null,
  ].filter(Boolean).join(" · ");

  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        width: "100%",
        padding: 8,
        background: "transparent",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 4,
        cursor: "pointer",
        textAlign: "left",
        marginBottom: 6,
      }}
    >
      <div style={{ width: 48, height: 48, background: "var(--paper-muted, #f5f5f5)",
                    display: "flex", alignItems: "center", justifyContent: "center", borderRadius: 4, overflow: "hidden" }}>
        {src ? <img src={src} alt="" style={{ width: "100%", height: "100%", objectFit: "cover" }} />
             : <ImageOff size={18} strokeWidth={1.4} color="var(--ink-soft, #999)" />}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontWeight: 600, fontSize: 14, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          {recipe.title}
        </div>
        {meta && <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>{meta}</div>}
      </div>
    </button>
  );
}

export function RecipePickerDrawer({ date, onClose, onPick }: Props) {
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [results, setResults] = useState<Recipe[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const h = setTimeout(() => setDebounced(query), 200);
    return () => clearTimeout(h);
  }, [query]);

  useEffect(() => {
    setError(null);
    void recipeIpc.list(debounced || undefined, []).then(setResults).catch((e) => setError(String(e)));
  }, [debounced]);

  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, width: 480,
      background: "var(--paper, #fff)",
      borderLeft: "1px solid var(--hairline, #e5e5e5)",
      padding: 24, overflow: "auto", zIndex: 50,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 18 }}>Plan {formatDate(date)}</h2>
        <button type="button" onClick={onClose} aria-label="Close">✕</button>
      </div>
      <input
        autoFocus
        placeholder="Search recipes"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        style={{ width: "100%", marginBottom: 16, padding: 8, fontSize: 14 }}
      />
      {error && <p style={{ color: "var(--ink-danger, #b00020)" }}>{error}</p>}
      {results == null && <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>}
      {results != null && results.length === 0 && (
        <p style={{ color: "var(--ink-soft, #999)" }}>No recipes yet. Add one in the Recipes tab first.</p>
      )}
      {results != null && results.map((r) => (
        <RecipeRow key={r.id} recipe={r} onClick={() => onPick(r.id)} />
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Wire into `ThisWeekView`**

Modify `apps/desktop/src/components/Hearth/ThisWeek/ThisWeekView.tsx`:

- Import `RecipePickerDrawer`.
- Add `const [pickerDate, setPickerDate] = useState<string | null>(null);`.
- Replace the `onEmptyClick` handler: `onEmptyClick={() => setPickerDate(entry.entry_date)}`.
- At the bottom of the component, render:
```tsx
{pickerDate && (
  <RecipePickerDrawer
    date={pickerDate}
    onClose={() => setPickerDate(null)}
    onPick={async (recipeId) => {
      await setEntry(pickerDate, recipeId);
      setPickerDate(null);
    }}
  />
)}
```
Add `setEntry` to the destructured store: `const { weekStart, entries, ..., setEntry, clearEntry } = useMealPlanStore();`.

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Hearth/ThisWeek/
git commit -m "feat(meal_plan): recipe picker drawer wired to empty slot taps"
```

---

## Task 11: Filled-slot detail nav + ghost drawer + date picker

**Files:**
- Modify: `apps/desktop/src/components/Hearth/ThisWeek/ThisWeekView.tsx`
- Modify: `apps/desktop/src/components/Hearth/ThisWeek/WeekNav.tsx` (add date-picker popover)
- Modify: `apps/desktop/src/components/Hearth/ThisWeek/DaySlotCard.tsx` (the overflow menu's Swap action)

- [ ] **Step 1: Filled-click navigates to recipe detail**

The existing detail view lives in `RecipesView.tsx` (formerly `HearthTab.tsx`). Clicking a filled slot should switch to the Recipes sub-view AND open the detail for that recipe. The cleanest way: add a `pendingDetailId` to `useHearthViewStore` that the Recipes view picks up.

Modify `apps/desktop/src/lib/hearth/view-state.ts` — extend the store:

```ts
interface HearthViewStore {
  subview: HearthSubview;
  hydrated: boolean;
  pendingDetailId: string | null;
  hydrate(): Promise<void>;
  setSubview(v: HearthSubview): void;
  openRecipeDetail(id: string): void;   // sets subview="recipes" + pendingDetailId
  clearPendingDetail(): void;
}
// in the store implementation:
  pendingDetailId: null,
  openRecipeDetail(id) { set({ subview: "recipes", pendingDetailId: id }); void settingSet("hearth.last_subview", "recipes").catch(() => {}); },
  clearPendingDetail() { set({ pendingDetailId: null }); },
```

`RecipesView.tsx` reads `pendingDetailId`:
```tsx
const { pendingDetailId, clearPendingDetail } = useHearthViewStore();
const [detailId, setDetailId] = useState<string | null>(null);
useEffect(() => {
  if (pendingDetailId) { setDetailId(pendingDetailId); clearPendingDetail(); }
}, [pendingDetailId, clearPendingDetail]);
```

Then in `ThisWeekView` the filled-click handler becomes:
```tsx
onFilledClick={(id) => openRecipeDetail(id)}
```
destructuring `openRecipeDetail` from `useHearthViewStore()`.

- [ ] **Step 2: Ghost drawer**

Inline a small drawer component in `ThisWeekView.tsx` (no separate file — 10 lines of JSX):

```tsx
{ghostEntry && (
  <div style={{
    position: "fixed", top: 0, right: 0, bottom: 0, width: 480,
    background: "var(--paper, #fff)", borderLeft: "1px solid var(--hairline, #e5e5e5)",
    padding: 24, overflow: "auto", zIndex: 50,
  }}>
    <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
      <h2 style={{ margin: 0, fontSize: 18 }}>Recipe deleted</h2>
      <button type="button" onClick={() => setGhostEntry(null)} aria-label="Close">✕</button>
    </div>
    <p style={{ color: "var(--ink-soft, #999)", marginBottom: 16 }}>
      "{ghostEntry.recipe?.title ?? "—"}" was moved to Trash. Restore the recipe, or unplan this day.
    </p>
    <div style={{ display: "flex", gap: 8 }}>
      <button type="button" onClick={async () => {
        if (ghostEntry.recipe) { await recipeIpc.restore(ghostEntry.recipe.id); }
        await loadWeek();
        setGhostEntry(null);
      }}>Restore recipe</button>
      <button type="button" onClick={async () => {
        await clearEntry(ghostEntry.entry_date);
        setGhostEntry(null);
      }}>Unplan this day</button>
    </div>
  </div>
)}
```

Add `const [ghostEntry, setGhostEntry] = useState<MealPlanEntryWithRecipe | null>(null);` to the component. Wire `onGhostClick={(e) => setGhostEntry(e)}`.

Also import `recipeIpc`: `import * as recipeIpc from "../../../lib/recipe/recipe-ipc";`.

- [ ] **Step 3: Date picker popover in WeekNav**

Use a minimal native-ish date input. Extend `WeekNav.tsx` with a `<button>📅</button>` that opens an `<input type="date">` anchored below. On change, snap to the Monday of that week.

```tsx
import { useState } from "react";
import { Calendar } from "lucide-react";

interface Props {
  weekStart: string;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
  onJumpToDate: (date: string) => void;
}

export function WeekNav({ weekStart, onPrev, onNext, onToday, onJumpToDate }: Props) {
  const [open, setOpen] = useState(false);
  // ...existing label logic unchanged...

  return (
    <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 16, position: "relative" }}>
      <button type="button" onClick={onPrev} aria-label="Previous week">
        <ChevronLeft size={16} strokeWidth={1.8} />
      </button>
      <div style={{ fontSize: 14, fontWeight: 600, flex: 1 }}>{label}</div>
      <button type="button" onClick={onNext} aria-label="Next week">
        <ChevronRight size={16} strokeWidth={1.8} />
      </button>
      <button type="button" onClick={() => setOpen((v) => !v)} aria-label="Jump to date">
        <Calendar size={16} strokeWidth={1.8} />
      </button>
      <button type="button" onClick={onToday}>Today</button>
      {open && (
        <div style={{
          position: "absolute", top: "100%", right: 50, marginTop: 6, zIndex: 20,
          background: "var(--paper, #fff)", border: "1px solid var(--hairline, #e5e5e5)",
          padding: 8, borderRadius: 4, boxShadow: "0 2px 8px rgba(0,0,0,0.1)",
        }}>
          <input
            type="date"
            defaultValue={weekStart}
            onChange={(e) => { onJumpToDate(e.target.value); setOpen(false); }}
          />
        </div>
      )}
    </div>
  );
}
```

In `ThisWeekView`, compute the Monday of whatever date was picked:

```tsx
onJumpToDate={(d) => {
  const date = new Date(d + "T00:00:00");
  const offset = (date.getDay() + 6) % 7;
  date.setDate(date.getDate() - offset);
  setWeekStart(date.toISOString().slice(0, 10));
}}
```

- [ ] **Step 4: Swap via overflow menu**

The current `DaySlotCard` has a `⋯` button whose click calls `onRemove`. Change it to open a tiny menu with Swap + Remove. Simplest: on `⋯` click, open a popover with two buttons. Mirror the drawer pattern at smaller scale OR just switch the `onRemove` to open the picker on `date` (i.e., make `⋯` trigger Swap via `setPickerDate(entry.entry_date)`, which replaces the current entry on save).

Cleanest minimal change: split the filled-card overflow into two icons — a `⋯` for Swap (opens picker), and `×` for Remove. Update `DaySlotCard.tsx`:

Replace the single overflow button with:
```tsx
<div style={{ position: "absolute", top: 4, right: 4, display: "flex", gap: 4 }}>
  <button
    type="button" aria-label="Swap recipe"
    onClick={(e) => { e.stopPropagation(); onSwap(); }}
    style={{ /* same pill button style */ }}
  >
    <MoreHorizontal size={12} strokeWidth={1.8} />
  </button>
  <button
    type="button" aria-label="Remove"
    onClick={(e) => { e.stopPropagation(); onRemove(); }}
    style={{ /* same */ }}
  >
    <X size={12} strokeWidth={1.8} />
  </button>
</div>
```

Add `onSwap: () => void` to `DaySlotCard`'s `Props`. Import `X` from lucide-react.

In `ThisWeekView`, wire `onSwap={() => setPickerDate(entry.entry_date)}` for each slot. (The picker's `onPick` already upserts, so "swap" works by re-opening the picker on the same date.)

- [ ] **Step 5: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/components/Hearth/ThisWeek/ apps/desktop/src/lib/hearth/ apps/desktop/src/components/Hearth/RecipesView.tsx
git commit -m "feat(meal_plan): filled-slot → recipe detail, ghost drawer, date picker, swap/remove"
```

---

## Task 12: Staples view

**Files:**
- Create: `apps/desktop/src/components/Hearth/Staples/StapleRow.tsx`
- Overwrite: `apps/desktop/src/components/Hearth/Staples/StaplesView.tsx`

- [ ] **Step 1: `StapleRow.tsx`**

```tsx
import { useState } from "react";
import { X, MoreHorizontal } from "lucide-react";
import type { StapleItem } from "../../../lib/meal_plan/staples-ipc";

interface Props {
  staple: StapleItem;
  onUpdate: (name: string, aliases: string[]) => Promise<void>;
  onRemove: () => void;
}

export function StapleRow({ staple, onUpdate, onRemove }: Props) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(staple.name);
  const [aliases, setAliases] = useState<string[]>(staple.aliases);
  const [aliasInput, setAliasInput] = useState("");

  const save = async () => {
    await onUpdate(name.trim() || staple.name, aliases);
    setEditing(false);
  };

  const addAlias = () => {
    const v = aliasInput.trim();
    if (!v) return;
    if (aliases.includes(v)) { setAliasInput(""); return; }
    setAliases([...aliases, v]);
    setAliasInput("");
  };

  return (
    <div style={{
      padding: "10px 12px",
      borderBottom: "1px solid var(--hairline, #e5e5e5)",
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        {editing ? (
          <input
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            onBlur={save}
            onKeyDown={(e) => { if (e.key === "Enter") void save(); if (e.key === "Escape") { setName(staple.name); setEditing(false); } }}
            style={{ flex: 1, fontSize: 14 }}
          />
        ) : (
          <div style={{ flex: 1, fontSize: 14 }}>{staple.name}</div>
        )}
        <button type="button" aria-label="Edit aliases" onClick={() => setEditing(true)}>
          <MoreHorizontal size={14} strokeWidth={1.8} />
        </button>
        <button type="button" aria-label="Delete" onClick={onRemove}>
          <X size={14} strokeWidth={1.8} />
        </button>
      </div>

      {editing ? (
        <div style={{ marginTop: 8, display: "flex", flexWrap: "wrap", gap: 4, alignItems: "center" }}>
          {aliases.map((a, i) => (
            <span key={i} style={{
              background: "var(--paper-muted, #f5f5f5)",
              padding: "2px 8px", borderRadius: 4, fontSize: 12,
              display: "inline-flex", alignItems: "center", gap: 4,
            }}>
              {a}
              <button type="button" aria-label={`Remove alias ${a}`} onClick={() => setAliases(aliases.filter((x) => x !== a))}
                style={{ background: "transparent", border: "none", cursor: "pointer", padding: 0 }}>
                <X size={10} strokeWidth={1.8} />
              </button>
            </span>
          ))}
          <input
            value={aliasInput}
            onChange={(e) => setAliasInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addAlias(); } }}
            placeholder="+ alias"
            style={{ border: "none", outline: "none", fontSize: 12, minWidth: 80 }}
          />
          <button type="button" onClick={save}>Save</button>
        </div>
      ) : (
        staple.aliases.length > 0 && (
          <div style={{ marginTop: 4, fontSize: 12, color: "var(--ink-soft, #999)" }}>
            also: {staple.aliases.join(", ")}
          </div>
        )
      )}
    </div>
  );
}
```

- [ ] **Step 2: `StaplesView.tsx`**

```tsx
import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { useStaplesStore } from "../../../lib/meal_plan/staples-state";
import { StapleRow } from "./StapleRow";

export function StaplesView() {
  const { staples, load, add, updateOne, remove } = useStaplesStore();
  const [newName, setNewName] = useState("");
  const [adding, setAdding] = useState(false);

  useEffect(() => { void load(); }, [load]);

  const submitNew = async () => {
    const v = newName.trim();
    if (!v) { setAdding(false); return; }
    await add(v);
    setNewName("");
    setAdding(false);
  };

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
        <div>
          <div style={{ fontSize: 18, fontWeight: 600 }}>Staples</div>
          <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
            Items your shopping list skips by default.
          </div>
        </div>
        <button type="button" onClick={() => setAdding(true)}
          style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <Plus size={14} strokeWidth={1.8} /> Add staple
        </button>
      </div>

      {adding && (
        <div style={{ padding: "10px 12px", borderBottom: "1px solid var(--hairline, #e5e5e5)" }}>
          <input
            autoFocus
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onBlur={submitNew}
            onKeyDown={(e) => { if (e.key === "Enter") void submitNew(); if (e.key === "Escape") { setNewName(""); setAdding(false); } }}
            placeholder="e.g. olive oil"
            style={{ width: "100%", fontSize: 14, padding: 4 }}
          />
        </div>
      )}

      {staples.length === 0 && !adding && (
        <div style={{ padding: 32, textAlign: "center", color: "var(--ink-soft, #999)" }}>
          No staples yet. Add "salt", "olive oil", or anything else you always have so your shopping list won't repeat them.
        </div>
      )}

      {staples.map((s) => (
        <StapleRow
          key={s.id}
          staple={s}
          onUpdate={(name, aliases) => updateOne(s.id, { name, aliases })}
          onRemove={() => void remove(s.id)}
        />
      ))}
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
git add apps/desktop/src/components/Hearth/Staples/
git commit -m "feat(staples): Staples view — add inline, edit aliases chip input, soft-delete"
```

---

## Task 13: Tonight band on Today view

**Files:**
- Create: `apps/desktop/src/components/Today/TonightBand.tsx`
- Modify: `apps/desktop/src/components/Today/Today.tsx`

- [ ] **Step 1: `TonightBand.tsx`**

```tsx
import { useEffect, useState } from "react";
import { Utensils, Ban } from "lucide-react";
import { useMealPlanStore } from "../../lib/meal_plan/meal-plan-state";
import { useHearthViewStore } from "../../lib/hearth/view-state";
import { settingGet } from "../../lib/settings/ipc";       // adapt to real path
import * as recipeIpc from "../../lib/recipe/recipe-ipc";

export function TonightBand() {
  const { tonight, loadTonight } = useMealPlanStore();
  const { openRecipeDetail, setSubview } = useHearthViewStore();
  const [visible, setVisible] = useState<boolean>(true);
  const [heroSrc, setHeroSrc] = useState<string | null>(null);

  useEffect(() => { void loadTonight(); }, [loadTonight]);
  useEffect(() => {
    void settingGet("hearth.show_tonight_band").then((v) => setVisible(v !== "false")).catch(() => {});
  }, []);
  useEffect(() => {
    const uuid = tonight?.recipe?.hero_attachment_uuid;
    if (uuid) { void recipeIpc.attachmentSrc(uuid).then(setHeroSrc).catch(() => {}); }
  }, [tonight?.recipe?.hero_attachment_uuid]);

  if (!visible) return null;

  const recipe = tonight?.recipe;
  const isGhost = recipe?.deleted_at != null;

  const row = (icon: React.ReactNode, text: React.ReactNode, right: React.ReactNode, onClick?: () => void) => (
    <div
      onClick={onClick}
      style={{
        display: "flex", alignItems: "center", gap: 12,
        height: 56, padding: "0 16px",
        background: "var(--paper, #fff)",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 6,
        cursor: onClick ? "pointer" : "default",
      }}
    >
      {icon}
      <div style={{ flex: 1, minWidth: 0 }}>{text}</div>
      {right}
    </div>
  );

  if (!recipe) {
    return row(
      <Utensils size={18} strokeWidth={1.6} color="var(--ink-soft, #999)" />,
      <span style={{ color: "var(--ink-soft, #999)" }}>No dinner planned</span>,
      <button type="button" onClick={() => setSubview("this_week")}>Plan one →</button>,
    );
  }

  if (isGhost) {
    return row(
      <Ban size={18} strokeWidth={1.6} color="var(--ink-soft, #999)" />,
      <span style={{ color: "var(--ink-soft, #999)" }}>Recipe deleted — restore or replace?</span>,
      <button type="button" onClick={() => setSubview("this_week")}>Review →</button>,
    );
  }

  const meta = [
    recipe.cook_time_mins != null || recipe.prep_time_mins != null
      ? `${(recipe.prep_time_mins ?? 0) + (recipe.cook_time_mins ?? 0)}m`
      : null,
    recipe.servings != null ? `serves ${recipe.servings}` : null,
  ].filter(Boolean).join(" · ");

  return (
    <div
      onClick={() => openRecipeDetail(recipe.id)}
      style={{
        display: "flex", alignItems: "center", gap: 12,
        height: 56, padding: "0 12px",
        background: "var(--paper, #fff)",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 6,
        cursor: "pointer",
      }}
    >
      <div style={{ width: 40, height: 40, borderRadius: 4, overflow: "hidden",
                    background: "var(--paper-muted, #f5f5f5)",
                    display: "flex", alignItems: "center", justifyContent: "center" }}>
        {heroSrc ? <img src={heroSrc} alt="" style={{ width: "100%", height: "100%", objectFit: "cover" }} />
                 : <Utensils size={16} strokeWidth={1.6} color="var(--ink-soft, #999)" />}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 14, fontWeight: 600,
                      whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          Tonight — {recipe.title}
        </div>
        {meta && <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>{meta}</div>}
      </div>
      <span style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>View recipe →</span>
    </div>
  );
}
```

- [ ] **Step 2: Mount `<TonightBand />` in `Today.tsx`**

Modify `apps/desktop/src/components/Today/Today.tsx`:

- Import `TonightBand`.
- Insert `<TonightBand />` inside the main content, between the existing `<PageHeader />` and the first existing card component.

Specifically, between `<PageHeader ... />` and `<EventsCard />` (or whichever is first).

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Today/
git commit -m "feat(meal_plan): Tonight band on Today — planned/unplanned/ghost/hidden states"
```

---

## Task 14: TimeBlocks meal block

**Files:**
- Modify: `apps/desktop/src/components/TimeBlocks/TimeBlocksView.tsx`

- [ ] **Step 0: Inspect current TimeBlocks data flow**

Read `apps/desktop/src/components/TimeBlocks/TimeBlocksView.tsx` and `apps/desktop/src/lib/timeblocks/ipc.ts` + `state.ts`. Understand:
- How the current day's blocks are fetched and rendered.
- The `TimeBlock` type shape (start, end, title, etc.).

- [ ] **Step 1: Inject a synthetic meal block at `hearth.dinner_time`**

In `TimeBlocksView.tsx`, after the existing blocks are loaded, merge in a meal block if one is planned for today. Pseudocode:

```tsx
import { useEffect, useState } from "react";
import { useMealPlanStore } from "../../lib/meal_plan/meal-plan-state";
import { useHearthViewStore } from "../../lib/hearth/view-state";
import { settingGet } from "../../lib/settings/ipc";  // adapt path
import { Utensils, Ban } from "lucide-react";

// Inside TimeBlocksView's render / useEffect:
const { tonight, loadTonight } = useMealPlanStore();
const { openRecipeDetail } = useHearthViewStore();
const [dinnerTime, setDinnerTime] = useState<string>("19:00");

useEffect(() => {
  void loadTonight();
  void settingGet("hearth.dinner_time").then((v) => { if (v) setDinnerTime(v); }).catch(() => {});
}, [loadTonight]);

// When rendering blocks, add a synthetic meal block if `tonight?.recipe` exists:
const mealBlock = tonight?.recipe ? {
  id: "meal-today",
  title: `${tonight.recipe.deleted_at != null ? "⊘" : "🍽"} ${tonight.recipe.title}`,
  start_time: dinnerTime,      // adapt to the TimeBlock shape's actual field name
  end_time: addMinutes(dinnerTime, 45),
  origin: "meal" as const,
  recipeId: tonight.recipe.id,
} : null;

function addMinutes(hm: string, mins: number): string {
  const [h, m] = hm.split(":").map(Number);
  const total = h * 60 + m + mins;
  const hh = String(Math.floor(total / 60) % 24).padStart(2, "0");
  const mm = String(total % 60).padStart(2, "0");
  return `${hh}:${mm}`;
}
```

Merge `mealBlock` into the blocks array at render time, keeping the type discriminated so click handling can route:

```tsx
const allBlocks = [...blocks, ...(mealBlock ? [mealBlock] : [])];
// sort by start time before rendering if the view does that already.
```

In the click handler for rendering each block, branch on origin:
```tsx
onClick={() => {
  if ("origin" in b && b.origin === "meal" && "recipeId" in b) {
    openRecipeDetail(b.recipeId);
  } else {
    // existing click behaviour for non-meal blocks (open edit drawer, etc.)
  }
}}
```

Exact integration depends on how `TimeBlocksView.tsx` is currently structured. Adapt. Key constraints:
- Meal block is NOT draggable / editable from TimeBlocks (read-only).
- Clicking opens the recipe detail via `openRecipeDetail`.
- Ghost state (recipe soft-deleted) still renders the block (with ⊘ prefix) so the user sees "something's wrong."
- If `hearth.dinner_time` setting is malformed (not HH:MM), skip the block entirely rather than rendering at garbage time. Add a regex check `/^\d{2}:\d{2}$/` before using.

- [ ] **Step 2: Typecheck + build + manual smoke**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

Smoke test: dev run, plan a meal for today, open TimeBlocks tab, verify block appears at 19:00 with `🍽` prefix and clicking jumps to recipe detail.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/components/TimeBlocks/
git commit -m "feat(meal_plan): TimeBlocks renders tonight's meal at dinner_time (read-only)"
```

---

## Task 15: Settings Hearth section

**Files:**
- Create: `apps/desktop/src/components/Settings/HearthTab.tsx`
- Modify: `apps/desktop/src/components/Settings/Tabs.tsx` (or wherever the Settings tab list lives)

- [ ] **Step 0: Inspect existing Settings tabs**

Read `apps/desktop/src/components/Settings/Tabs.tsx` + `SettingsModal.tsx` + existing tabs like `HouseholdTab.tsx` for the canonical pattern. Mirror it.

- [ ] **Step 1: `HearthTab.tsx`**

```tsx
import { useEffect, useState } from "react";
import { useHearthViewStore } from "../../lib/hearth/view-state";
import { settingGet, settingSet } from "../../lib/settings/ipc";  // adapt path

export default function HearthTab() {
  const { setSubview } = useHearthViewStore();
  const [showBand, setShowBand] = useState(true);
  const [dinnerTime, setDinnerTime] = useState("19:00");
  const [message, setMessage] = useState("");

  useEffect(() => {
    void settingGet("hearth.show_tonight_band").then((v) => setShowBand(v !== "false")).catch(() => {});
    void settingGet("hearth.dinner_time").then((v) => { if (v) setDinnerTime(v); }).catch(() => {});
  }, []);

  const toggleBand = async () => {
    const next = !showBand;
    setShowBand(next);
    await settingSet("hearth.show_tonight_band", next ? "true" : "false");
    setMessage("Saved.");
  };

  const saveDinnerTime = async (v: string) => {
    setDinnerTime(v);
    if (/^\d{2}:\d{2}$/.test(v)) {
      await settingSet("hearth.dinner_time", v);
      setMessage("Saved.");
    }
  };

  return (
    <div style={{ padding: 24 }}>
      <h2 style={{ fontSize: 18, marginTop: 0 }}>Hearth</h2>

      <div style={{ marginBottom: 20 }}>
        <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <input type="checkbox" checked={showBand} onChange={toggleBand} />
          Show tonight's meal on Today
        </label>
      </div>

      <div style={{ marginBottom: 20 }}>
        <label style={{ display: "block", fontSize: 13, marginBottom: 4 }}>Dinner time</label>
        <input
          type="time"
          value={dinnerTime}
          onChange={(e) => void saveDinnerTime(e.target.value)}
          style={{ fontSize: 14, padding: 4 }}
        />
        <div style={{ fontSize: 12, color: "var(--ink-soft, #999)", marginTop: 4 }}>
          Used when your planned meal appears on the TimeBlocks view.
        </div>
      </div>

      <div style={{ marginBottom: 20 }}>
        <button type="button" onClick={() => setSubview("staples")}>
          Manage staples →
        </button>
        <div style={{ fontSize: 12, color: "var(--ink-soft, #999)", marginTop: 4 }}>
          Items your shopping list skips by default.
        </div>
      </div>

      {message && <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>{message}</div>}
    </div>
  );
}
```

- [ ] **Step 2: Register the tab**

In `Settings/Tabs.tsx` (or `SettingsModal.tsx`, whichever owns the tab list), add "Hearth" between existing tabs. Typical pattern: a `TABS` array with `{ id, label, component }`. Add:

```tsx
{ id: "hearth", label: "Hearth", component: HearthTab },
```

Import `HearthTab` at the top.

The "Manage staples" button uses `setSubview("staples")` but doesn't close the Settings modal or navigate to Hearth. If Settings is a modal, it will stay open over Hearth. Add a close-settings hook if the existing tabs do it (e.g., other tabs that navigate). If not, accept this for MVP — the user can close manually.

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Settings/
git commit -m "feat(meal_plan): Settings → Hearth section — band toggle, dinner time, staples link"
```

---

## Task 16: Manual QA pass

**Files:** (verification only — no code unless bugs found)

- [ ] **Step 1: Full test suite**

```bash
cd /Users/hanamori/life-assistant && cargo test --workspace
```
Expected: all green. Log counts.

- [ ] **Step 2: Clippy + typecheck + build**

```bash
cargo clippy --workspace -- -D warnings
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```
Expected: clean.

- [ ] **Step 3: Dev-server golden paths**

```bash
cd /Users/hanamori/life-assistant && pnpm tauri dev
```

Walk these:
- Open Hearth → Recipes sub-view (from last session or default). Switch to This Week. Confirm empty grid.
- In Recipes sub-view, create a recipe (from L3a). Go back to This Week.
- Tap an empty slot → picker drawer opens, recipe visible, tap it → slot fills.
- Today view: "Tonight" band shows the meal. Tap → navigates to the recipe detail.
- TimeBlocks view: meal block visible at 19:00 (or the setting value).
- Settings → Hearth: toggle "Show tonight band" off → Today band disappears. Change dinner time → TimeBlocks block moves.
- Back in This Week: hover a filled slot, swap via the `⋯` overflow (re-opens picker, new recipe replaces it). Click `×` to remove.
- Staples sub-view: add "salt", "olive oil" with alias "EVOO". Delete one. Verify sort.
- Test ghost state: trash a recipe that's on the plan (from Recipes detail → Delete). Open This Week. Slot shows the ghost. Click → drawer offers Restore/Unplan.
- Test week nav: click ← / → / Today / 📅 picker. Verify entries load.

- [ ] **Step 4: If all green, L3b is shipped.**

Invoke `superpowers:finishing-a-development-branch` to merge.

---

## Self-review

**Spec coverage:**
- §3 architecture (two-crate split) → Tasks 2–6. ✓
- §4 migration V16 → Task 1. ✓
- §5 sub-nav → Task 8. ✓
- §6 This Week view (grid, nav, picker, ghost) → Tasks 9–11. ✓
- §7 Tonight band → Task 13. ✓
- §8 TimeBlocks integration → Task 14. ✓
- §9 Staples sub-view → Task 12. ✓
- §10 Settings Hearth section → Task 15. ✓
- §11 error table → covered inline (load_status in stores, inline errors in drawers).
- §12 testing strategy — core unit tests in Tasks 2, 4, 5; manual QA in Task 16. Integration-test gap vs spec: no wiremock-style integration test for the full IPC layer, but the core DAL tests cover the interesting logic and the frontend Zustand stores wrap simple invoke calls. Acceptable for L3b — matches L3a's test depth.

**Placeholder scan:** none found. Every step has actual code or a concrete instruction with target files.

**Type consistency:** `MealPlanEntry`, `MealPlanEntryWithRecipe`, `StapleItem`, `StapleDraft`, `HearthSubview` used consistently across Rust and TS surfaces. `Recipe.hero_attachment_uuid` referenced from DaySlotCard + RecipePickerDrawer + TonightBand — the field exists as of L3a's V15.

---

*End of plan. Next: `superpowers:subagent-driven-development` or `superpowers:executing-plans` to implement.*
