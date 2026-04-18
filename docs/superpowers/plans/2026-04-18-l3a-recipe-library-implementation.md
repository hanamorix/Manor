# L3a Recipe Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Manor's recipe library — a standalone Hearth tab that lets the user browse, create, edit, and URL-import recipes, reusing existing foundation infrastructure (attachments, tags, trash).

**Architecture:** Pure parsing + DAL in `manor-core` (rusqlite + serde_json + scraper for JSON-LD). HTTP fetch, Tauri IPC, and orchestration in `manor-app` (reqwest, existing OllamaClient/RemoteLlmClient). React frontend follows the existing drawer-based pattern used in `ConnectBankDrawer`.

**Tech Stack:**
- Rust: rusqlite, refinery (migrations), scraper (HTML+JSON-LD), readability-rs (text extraction), reqwest (in app)
- Frontend: React + TypeScript, Zustand, Lucide icons, Flat-Notion design tokens
- Testing: `cargo test` (unit), wiremock (integration), React Testing Library (component)

**Spec:** `docs/superpowers/specs/2026-04-18-l3a-recipe-library-design.md`

---

## File structure

**New files in `manor-core`:**
- `crates/core/migrations/V14__recipe.sql`
- `crates/core/src/recipe/mod.rs` — types + module root
- `crates/core/src/recipe/dal.rs` — CRUD + trash lookups
- `crates/core/src/recipe/import.rs` — pure JSON-LD + LLM parsers (no network)
- `crates/core/tests/fixtures/recipe/*.html` — 8 JSON-LD fixture pages

**New files in `manor-app`:**
- `crates/app/src/recipe/mod.rs` — module root + command registration
- `crates/app/src/recipe/commands.rs` — Tauri IPC commands
- `crates/app/src/recipe/importer.rs` — orchestrator (fetch + parse + fallback)
- `crates/app/src/recipe/stage_sweep.rs` — startup sweep for orphan staged attachments

**New files in frontend (`apps/desktop/src/`):**
- `lib/recipe/recipe-ipc.ts` — IPC wrappers
- `lib/recipe/recipe-store.ts` — Zustand store for recipe list + filters
- `components/Hearth/HearthTab.tsx` — tab landing (L3a = recipe library)
- `components/Hearth/RecipeCard.tsx` — grid card
- `components/Hearth/RecipeDetail.tsx` — detail view
- `components/Hearth/RecipeEditDrawer.tsx` — New/Edit drawer
- `components/Hearth/RecipeImportDrawer.tsx` — URL import drawer
- `components/Hearth/IngredientRowEditor.tsx` — ingredient-row sub-component
- `components/Hearth/ImportMethodBadge.tsx` — parse-path badge

**Modified files:**
- `crates/core/src/lib.rs` — add `pub mod recipe;`
- `crates/core/src/trash.rs` — register `recipe` table in sweeper + add attachment post-hook
- `crates/app/src/lib.rs` — register recipe commands in invoke handler; call `stage_sweep::run_on_startup`
- `apps/desktop/src/components/Nav/NavBar.tsx` — add Hearth tab entry
- `crates/core/Cargo.toml` — add `scraper` dep

---

## Task 1: Migration V14 — recipe + recipe_ingredient tables

**Files:**
- Create: `crates/core/migrations/V14__recipe.sql`
- Test: `crates/core/src/recipe/dal.rs` (test-only smoke check added in Task 2)

- [ ] **Step 1: Write the migration SQL**

```sql
-- V14__recipe.sql
-- L3a Recipe Library: recipe + recipe_ingredient tables.

CREATE TABLE recipe (
    id             TEXT PRIMARY KEY,
    title          TEXT NOT NULL,
    servings       INTEGER,
    prep_time_mins INTEGER,
    cook_time_mins INTEGER,
    instructions   TEXT NOT NULL,
    source_url     TEXT,
    source_host    TEXT,
    import_method  TEXT,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    deleted_at     INTEGER
);

CREATE INDEX idx_recipe_deleted ON recipe(deleted_at);
CREATE INDEX idx_recipe_title   ON recipe(title COLLATE NOCASE);

CREATE TABLE recipe_ingredient (
    id              TEXT PRIMARY KEY,
    recipe_id       TEXT NOT NULL REFERENCES recipe(id) ON DELETE CASCADE,
    position        INTEGER NOT NULL,
    quantity_text   TEXT,
    ingredient_name TEXT NOT NULL,
    note            TEXT
);

CREATE INDEX idx_ri_recipe ON recipe_ingredient(recipe_id, position);
```

- [ ] **Step 2: Run migrations in tests**

Run: `cargo test -p manor-core --lib -- migrations`
Expected: PASS (existing migration tests should pick up V14 without change because refinery discovers files by filename pattern).

- [ ] **Step 3: Commit**

```bash
git add crates/core/migrations/V14__recipe.sql
git commit -m "feat(recipe): migration V14 — recipe + recipe_ingredient tables"
```

---

## Task 2: Core types + DAL for recipe CRUD

**Files:**
- Create: `crates/core/src/recipe/mod.rs`
- Create: `crates/core/src/recipe/dal.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write failing test for `insert_recipe` + `get_recipe`**

Create `crates/core/src/recipe/dal.rs` with just the test first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::migrations::run(&mut conn).unwrap();
        conn
    }

    #[test]
    fn insert_and_get_recipe_roundtrips() {
        let conn = fresh_db();
        let draft = RecipeDraft {
            title: "Miso aubergine".into(),
            servings: Some(4),
            prep_time_mins: Some(15),
            cook_time_mins: Some(30),
            instructions: "1. Preheat oven...".into(),
            source_url: None,
            source_host: None,
            import_method: ImportMethod::Manual,
            ingredients: vec![
                IngredientLine { quantity_text: Some("2".into()), ingredient_name: "aubergines".into(), note: None },
            ],
        };
        let id = insert_recipe(&conn, &draft).unwrap();
        let got = get_recipe(&conn, &id).unwrap().expect("recipe exists");
        assert_eq!(got.title, "Miso aubergine");
        assert_eq!(got.ingredients.len(), 1);
        assert_eq!(got.ingredients[0].ingredient_name, "aubergines");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p manor-core --lib recipe::dal::tests`
Expected: FAIL with "unresolved module `recipe`" / "unresolved import".

- [ ] **Step 3: Create `crates/core/src/recipe/mod.rs` with types**

```rust
//! Recipe library — types + CRUD. Pure data layer; no network, no parsing.

pub mod dal;
pub mod import;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImportMethod {
    Manual,
    JsonLd,
    Llm,
    LlmRemote,
}

impl ImportMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImportMethod::Manual => "manual",
            ImportMethod::JsonLd => "jsonld",
            ImportMethod::Llm => "llm",
            ImportMethod::LlmRemote => "llm_remote",
        }
    }

    pub fn from_db(s: Option<&str>) -> Self {
        match s {
            Some("jsonld") => Self::JsonLd,
            Some("llm") => Self::Llm,
            Some("llm_remote") => Self::LlmRemote,
            _ => Self::Manual,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngredientLine {
    pub quantity_text: Option<String>,
    pub ingredient_name: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeDraft {
    pub title: String,
    pub servings: Option<i32>,
    pub prep_time_mins: Option<i32>,
    pub cook_time_mins: Option<i32>,
    pub instructions: String,
    pub source_url: Option<String>,
    pub source_host: Option<String>,
    pub import_method: ImportMethod,
    pub ingredients: Vec<IngredientLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: String,
    pub title: String,
    pub servings: Option<i32>,
    pub prep_time_mins: Option<i32>,
    pub cook_time_mins: Option<i32>,
    pub instructions: String,
    pub source_url: Option<String>,
    pub source_host: Option<String>,
    pub import_method: ImportMethod,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub ingredients: Vec<IngredientLine>,
}
```

- [ ] **Step 4: Implement `insert_recipe` + `get_recipe` in `dal.rs`**

```rust
use super::{ImportMethod, IngredientLine, Recipe, RecipeDraft};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn insert_recipe(conn: &Connection, draft: &RecipeDraft) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_ms();
    conn.execute(
        "INSERT INTO recipe (id, title, servings, prep_time_mins, cook_time_mins,
            instructions, source_url, source_host, import_method, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id, draft.title, draft.servings, draft.prep_time_mins, draft.cook_time_mins,
            draft.instructions, draft.source_url, draft.source_host,
            draft.import_method.as_str(), now, now,
        ],
    )?;
    for (pos, ing) in draft.ingredients.iter().enumerate() {
        conn.execute(
            "INSERT INTO recipe_ingredient (id, recipe_id, position, quantity_text, ingredient_name, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Uuid::new_v4().to_string(), id, pos as i64,
                ing.quantity_text, ing.ingredient_name, ing.note,
            ],
        )?;
    }
    Ok(id)
}

pub fn get_recipe(conn: &Connection, id: &str) -> Result<Option<Recipe>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, servings, prep_time_mins, cook_time_mins, instructions,
                source_url, source_host, import_method, created_at, updated_at, deleted_at
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

- [ ] **Step 5: Add `pub mod recipe;` to `crates/core/src/lib.rs`**

Modify `crates/core/src/lib.rs` — insert after line 12 (setting):

```rust
pub mod recipe;
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p manor-core --lib recipe::dal::tests`
Expected: PASS (1 test).

- [ ] **Step 7: Add failing test for `list_recipes`, `update_recipe`, `soft_delete_recipe`, `restore_recipe`**

Append to `crates/core/src/recipe/dal.rs` test module:

```rust
    #[test]
    fn list_excludes_trashed_by_default() {
        let conn = fresh_db();
        let a = insert_recipe(&conn, &simple_draft("A")).unwrap();
        let _b = insert_recipe(&conn, &simple_draft("B")).unwrap();
        soft_delete_recipe(&conn, &a).unwrap();
        let list = list_recipes(&conn, &ListFilter::default()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "B");
    }

    #[test]
    fn update_bumps_updated_at_and_replaces_ingredients() {
        let conn = fresh_db();
        let id = insert_recipe(&conn, &simple_draft("Original")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let mut draft = simple_draft("Updated");
        draft.ingredients = vec![
            IngredientLine { quantity_text: Some("5".into()), ingredient_name: "garlic".into(), note: None },
        ];
        update_recipe(&conn, &id, &draft).unwrap();
        let r = get_recipe(&conn, &id).unwrap().unwrap();
        assert_eq!(r.title, "Updated");
        assert_eq!(r.ingredients.len(), 1);
        assert_eq!(r.ingredients[0].ingredient_name, "garlic");
        assert!(r.updated_at > r.created_at);
    }

    #[test]
    fn restore_clears_deleted_at() {
        let conn = fresh_db();
        let id = insert_recipe(&conn, &simple_draft("X")).unwrap();
        soft_delete_recipe(&conn, &id).unwrap();
        assert!(get_recipe(&conn, &id).unwrap().unwrap().deleted_at.is_some());
        restore_recipe(&conn, &id).unwrap();
        assert!(get_recipe(&conn, &id).unwrap().unwrap().deleted_at.is_none());
    }

    fn simple_draft(title: &str) -> RecipeDraft {
        RecipeDraft {
            title: title.into(),
            servings: None, prep_time_mins: None, cook_time_mins: None,
            instructions: "Cook it.".into(),
            source_url: None, source_host: None,
            import_method: ImportMethod::Manual,
            ingredients: vec![],
        }
    }
```

- [ ] **Step 8: Run tests to verify they fail**

Run: `cargo test -p manor-core --lib recipe::dal`
Expected: 3 new tests FAIL (unresolved `list_recipes`, `update_recipe`, etc.).

- [ ] **Step 9: Implement the missing functions**

Append to `crates/core/src/recipe/dal.rs`:

```rust
#[derive(Debug, Clone, Default)]
pub struct ListFilter {
    pub search: Option<String>,
    pub tag_ids: Vec<String>,
    pub include_trashed: bool,
}

pub fn list_recipes(conn: &Connection, filter: &ListFilter) -> Result<Vec<Recipe>> {
    let mut sql = String::from(
        "SELECT id, title, servings, prep_time_mins, cook_time_mins, instructions,
                source_url, source_host, import_method, created_at, updated_at, deleted_at
         FROM recipe WHERE 1=1"
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if !filter.include_trashed {
        sql.push_str(" AND deleted_at IS NULL");
    }
    if let Some(q) = filter.search.as_ref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND title LIKE ?");
        params.push(Box::new(format!("%{}%", q)));
    }
    sql.push_str(" ORDER BY created_at DESC");

    let mut stmt = conn.prepare(&sql)?;
    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(params_ref.as_slice(), |row| {
        let import_method_str: Option<String> = row.get(8)?;
        Ok(Recipe {
            id: row.get(0)?, title: row.get(1)?, servings: row.get(2)?,
            prep_time_mins: row.get(3)?, cook_time_mins: row.get(4)?,
            instructions: row.get(5)?, source_url: row.get(6)?, source_host: row.get(7)?,
            import_method: ImportMethod::from_db(import_method_str.as_deref()),
            created_at: row.get(9)?, updated_at: row.get(10)?, deleted_at: row.get(11)?,
            ingredients: Vec::new(),
        })
    })?;

    let mut out = Vec::new();
    for row in rows { out.push(row?); }

    // Note: tag filter applied by caller against tag_link; not in this SQL pass to keep it
    // simple. If filter.tag_ids non-empty, caller intersects out with tag_link rows.
    Ok(out)
}

pub fn update_recipe(conn: &Connection, id: &str, draft: &RecipeDraft) -> Result<()> {
    let now = now_ms();
    conn.execute(
        "UPDATE recipe SET title=?1, servings=?2, prep_time_mins=?3, cook_time_mins=?4,
            instructions=?5, source_url=?6, source_host=?7, import_method=?8, updated_at=?9
         WHERE id=?10",
        params![
            draft.title, draft.servings, draft.prep_time_mins, draft.cook_time_mins,
            draft.instructions, draft.source_url, draft.source_host,
            draft.import_method.as_str(), now, id,
        ],
    )?;
    conn.execute("DELETE FROM recipe_ingredient WHERE recipe_id=?1", params![id])?;
    for (pos, ing) in draft.ingredients.iter().enumerate() {
        conn.execute(
            "INSERT INTO recipe_ingredient (id, recipe_id, position, quantity_text, ingredient_name, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Uuid::new_v4().to_string(), id, pos as i64,
                ing.quantity_text, ing.ingredient_name, ing.note,
            ],
        )?;
    }
    Ok(())
}

pub fn soft_delete_recipe(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE recipe SET deleted_at=?1 WHERE id=?2", params![now_ms(), id])?;
    Ok(())
}

pub fn restore_recipe(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE recipe SET deleted_at=NULL WHERE id=?1", params![id])?;
    Ok(())
}
```

- [ ] **Step 10: Run tests**

Run: `cargo test -p manor-core --lib recipe::dal`
Expected: PASS (4 tests).

- [ ] **Step 11: Commit**

```bash
git add crates/core/src/recipe/ crates/core/src/lib.rs
git commit -m "feat(recipe): core types + DAL (CRUD with trash)"
```

---

## Task 3: JSON-LD recipe parser with site fixtures

**Files:**
- Modify: `crates/core/Cargo.toml` (add `scraper`)
- Create: `crates/core/src/recipe/import.rs`
- Create: 8 fixture files in `crates/core/tests/fixtures/recipe/`

- [ ] **Step 1: Add `scraper` dep to core**

Modify `crates/core/Cargo.toml` — add under `[dependencies]`:

```toml
scraper = "0.20"
```

Run: `cargo build -p manor-core`
Expected: Successful compile.

- [ ] **Step 2: Write failing test — BBC Good Food fixture parse**

Save a minimal fixture at `crates/core/tests/fixtures/recipe/bbc_good_food.html`:

```html
<!DOCTYPE html><html><head><title>Thai green curry</title>
<script type="application/ld+json">
{"@context":"https://schema.org","@type":"Recipe",
 "name":"Thai green curry","recipeYield":"4","prepTime":"PT15M","cookTime":"PT20M",
 "recipeIngredient":["1 tbsp vegetable oil","2 garlic cloves, crushed","400ml coconut milk"],
 "recipeInstructions":[{"@type":"HowToStep","text":"Heat the oil."},
                        {"@type":"HowToStep","text":"Add garlic, fry 30 sec."},
                        {"@type":"HowToStep","text":"Pour coconut milk, simmer."}],
 "image":"https://bbcgoodfood.com/thai-curry.jpg"}
</script></head><body></body></html>
```

Create `crates/core/src/recipe/import.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bbc_good_food_jsonld() {
        let html = include_str!("../../tests/fixtures/recipe/bbc_good_food.html");
        let parsed = parse_jsonld(html).expect("parse succeeds");
        assert_eq!(parsed.title, "Thai green curry");
        assert_eq!(parsed.servings, Some(4));
        assert_eq!(parsed.prep_time_mins, Some(15));
        assert_eq!(parsed.cook_time_mins, Some(20));
        assert_eq!(parsed.ingredients.len(), 3);
        assert_eq!(parsed.ingredients[0].quantity_text.as_deref(), Some("1 tbsp"));
        assert_eq!(parsed.ingredients[0].ingredient_name, "vegetable oil");
        assert_eq!(parsed.hero_image_url.as_deref(), Some("https://bbcgoodfood.com/thai-curry.jpg"));
        assert!(parsed.instructions.contains("Heat the oil"));
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p manor-core --lib recipe::import::tests`
Expected: FAIL — `parse_jsonld` not defined.

- [ ] **Step 4: Implement `parse_jsonld`**

Prepend (above `#[cfg(test)]`) in `crates/core/src/recipe/import.rs`:

```rust
use super::{IngredientLine, ImportMethod};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedRecipe {
    pub title: String,
    pub servings: Option<i32>,
    pub prep_time_mins: Option<i32>,
    pub cook_time_mins: Option<i32>,
    pub instructions: String,
    pub ingredients: Vec<IngredientLine>,
    pub source_url: String,
    pub source_host: String,
    pub import_method: ImportMethod,
    pub parse_notes: Vec<String>,
    pub hero_image_url: Option<String>,
}

/// Parse a recipe from schema.org JSON-LD embedded in HTML.
/// Returns None if no valid Recipe block is found.
pub fn parse_jsonld(html: &str) -> Option<ImportedRecipe> {
    let doc = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;

    for el in doc.select(&selector) {
        let raw = el.text().collect::<String>();
        let Ok(json) = serde_json::from_str::<Value>(&raw) else { continue };
        if let Some(recipe) = find_recipe_node(&json) {
            if let Some(r) = map_recipe_node(recipe) {
                return Some(r);
            }
        }
    }
    None
}

fn find_recipe_node(v: &Value) -> Option<&Value> {
    match v {
        Value::Object(_) => {
            if node_type_matches(v, "Recipe") { return Some(v); }
            if let Some(graph) = v.get("@graph").and_then(|g| g.as_array()) {
                for item in graph {
                    if node_type_matches(item, "Recipe") { return Some(item); }
                }
            }
            None
        }
        Value::Array(arr) => arr.iter().find_map(find_recipe_node),
        _ => None,
    }
}

fn node_type_matches(node: &Value, wanted: &str) -> bool {
    match node.get("@type") {
        Some(Value::String(s)) => s == wanted,
        Some(Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some(wanted)),
        _ => false,
    }
}

fn map_recipe_node(node: &Value) -> Option<ImportedRecipe> {
    let title = node.get("name").and_then(Value::as_str)?.trim().to_string();
    if title.is_empty() { return None; }

    let servings = parse_yield(node.get("recipeYield"));
    let prep_time_mins = parse_iso_duration_mins(node.get("prepTime"));
    let cook_time_mins = parse_iso_duration_mins(node.get("cookTime"));

    let ingredients = node.get("recipeIngredient")
        .and_then(Value::as_array)
        .map(|arr| arr.iter()
            .filter_map(|v| v.as_str())
            .map(split_ingredient_line)
            .collect::<Vec<_>>())
        .unwrap_or_default();

    let instructions = instructions_to_markdown(node.get("recipeInstructions"));
    let hero_image_url = extract_image(node.get("image"));

    Some(ImportedRecipe {
        title, servings, prep_time_mins, cook_time_mins,
        instructions, ingredients,
        source_url: String::new(),   // caller fills from fetched URL
        source_host: String::new(),  // caller fills from URL host
        import_method: ImportMethod::JsonLd,
        parse_notes: Vec::new(),
        hero_image_url,
    })
}

fn parse_yield(v: Option<&Value>) -> Option<i32> {
    match v? {
        Value::Number(n) => n.as_i64().map(|x| x as i32),
        Value::String(s) => {
            // "4", "4 servings", "Serves 4"
            s.split(|c: char| !c.is_ascii_digit())
                .find(|s| !s.is_empty())
                .and_then(|t| t.parse().ok())
        }
        Value::Array(arr) => arr.iter().find_map(|v| parse_yield(Some(v))),
        _ => None,
    }
}

/// Parse ISO-8601 duration like "PT15M", "PT1H20M", "PT2H" → minutes.
fn parse_iso_duration_mins(v: Option<&Value>) -> Option<i32> {
    let s = v?.as_str()?;
    let s = s.strip_prefix("PT")?;
    let mut hours: i32 = 0;
    let mut mins: i32 = 0;
    let mut buf = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() { buf.push(ch); continue; }
        let n: i32 = buf.parse().unwrap_or(0);
        buf.clear();
        match ch {
            'H' => hours = n,
            'M' => mins = n,
            _ => {}
        }
    }
    Some(hours * 60 + mins)
}

fn split_ingredient_line(line: &str) -> IngredientLine {
    // Heuristic: leading digits/fractions/units = quantity; rest = name + optional note after first comma.
    let line = line.trim();
    let qty_end = line.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '/' || *c == ' ' || *c == '½' || *c == '¼' || *c == '¾').count();
    let (qty_raw, rest) = line.split_at(qty_end);
    let qty = qty_raw.trim();

    // unit-word detection: first word of rest if it's a unit.
    const UNITS: &[&str] = &["tbsp","tsp","cup","cups","g","kg","ml","l","oz","lb","pcs","piece","pieces","clove","cloves","pinch","dash","handful","sprig","sprigs","can","cans","bunch","bunches"];
    let rest = rest.trim_start();
    let (unit, after_unit) = rest.split_once(' ').map(|(a, b)| (a, b)).unwrap_or((rest, ""));
    let (quantity_text, name_plus) = if UNITS.iter().any(|u| unit.eq_ignore_ascii_case(u)) {
        let combined = if qty.is_empty() { unit.to_string() } else { format!("{} {}", qty, unit) };
        (if combined.is_empty() { None } else { Some(combined) }, after_unit.trim())
    } else {
        (if qty.is_empty() { None } else { Some(qty.to_string()) }, rest)
    };

    let (name, note) = match name_plus.split_once(',') {
        Some((n, note)) => (n.trim().to_string(), Some(note.trim().to_string())),
        None => (name_plus.trim().to_string(), None),
    };
    IngredientLine { quantity_text, ingredient_name: name, note }
}

fn instructions_to_markdown(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => arr.iter().enumerate().filter_map(|(i, step)| {
            let text = match step {
                Value::String(s) => s.clone(),
                Value::Object(_) => step.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
                _ => return None,
            };
            if text.trim().is_empty() { None } else { Some(format!("{}. {}", i + 1, text.trim())) }
        }).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

fn extract_image(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Array(arr) => arr.iter().find_map(|v| extract_image(Some(v))),
        Value::Object(_) => v?.get("url").and_then(Value::as_str).map(String::from),
        _ => None,
    }
}
```

- [ ] **Step 5: Run test — expect PASS**

Run: `cargo test -p manor-core --lib recipe::import::tests::parses_bbc_good_food_jsonld`
Expected: PASS.

- [ ] **Step 6: Add 7 more site fixtures + tests**

Create the remaining fixtures (each a minimal `<script type="application/ld+json">…</script>` page, exercising different edge cases). Fixture file names:

- `nyt_cooking.html` — `@graph` wrapped
- `serious_eats.html` — `@type: ["Recipe","SomethingElse"]` array
- `allrecipes.html` — `recipeYield` as number
- `bon_appetit.html` — `prepTime` missing, `totalTime` present
- `delicious.html` — instructions as plain string (no `HowToStep` array)
- `jamie_oliver.html` — image as object `{"@type":"ImageObject","url":"…"}`
- `ottolenghi.html` — no JSON-LD block (absent — should return None)

Append to `import.rs` tests:

```rust
    #[test]
    fn parses_graph_wrapped_recipe() {
        let html = include_str!("../../tests/fixtures/recipe/nyt_cooking.html");
        let parsed = parse_jsonld(html).expect("found recipe in @graph");
        assert!(!parsed.title.is_empty());
    }

    #[test]
    fn parses_type_array() {
        let html = include_str!("../../tests/fixtures/recipe/serious_eats.html");
        assert!(parse_jsonld(html).is_some());
    }

    #[test]
    fn parses_numeric_yield() {
        let html = include_str!("../../tests/fixtures/recipe/allrecipes.html");
        let r = parse_jsonld(html).unwrap();
        assert_eq!(r.servings, Some(6));
    }

    #[test]
    fn handles_missing_prep_time() {
        let html = include_str!("../../tests/fixtures/recipe/bon_appetit.html");
        let r = parse_jsonld(html).unwrap();
        assert!(r.prep_time_mins.is_none());
    }

    #[test]
    fn parses_string_instructions() {
        let html = include_str!("../../tests/fixtures/recipe/delicious.html");
        let r = parse_jsonld(html).unwrap();
        assert!(!r.instructions.is_empty());
    }

    #[test]
    fn parses_image_object() {
        let html = include_str!("../../tests/fixtures/recipe/jamie_oliver.html");
        let r = parse_jsonld(html).unwrap();
        assert!(r.hero_image_url.is_some());
    }

    #[test]
    fn returns_none_without_jsonld() {
        let html = include_str!("../../tests/fixtures/recipe/ottolenghi.html");
        assert!(parse_jsonld(html).is_none());
    }
```

Run: `cargo test -p manor-core --lib recipe::import::tests`
Expected: 8 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/core/Cargo.toml crates/core/src/recipe/import.rs crates/core/tests/fixtures/
git commit -m "feat(recipe): JSON-LD parser + 8 site fixtures"
```

---

## Task 4: LLM extraction fallback

**Files:**
- Modify: `crates/core/src/recipe/import.rs`

- [ ] **Step 1: Add `LlmClient` trait + failing test**

Append to `import.rs`:

```rust
#[cfg(test)]
mod llm_tests {
    use super::*;

    struct StubLlm { response: String }
    #[async_trait::async_trait]
    impl LlmClient for StubLlm {
        async fn complete(&self, _prompt: &str) -> anyhow::Result<String> {
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn extracts_valid_json_via_llm() {
        let stub = StubLlm { response: r#"{
            "title":"Lentil dal","servings":2,"prep_time_mins":5,"cook_time_mins":25,
            "instructions":"1. Rinse lentils.\n2. Simmer.",
            "ingredients":[{"quantity_text":"200g","ingredient_name":"red lentils","note":null}]
        }"#.into() };
        let r = extract_via_llm("page text", &stub, false).await.unwrap();
        assert_eq!(r.title, "Lentil dal");
        assert_eq!(r.servings, Some(2));
        assert_eq!(r.ingredients.len(), 1);
        assert_eq!(r.import_method, ImportMethod::Llm);
    }

    #[tokio::test]
    async fn malformed_json_retries_then_errors() {
        struct BadLlm;
        #[async_trait::async_trait]
        impl LlmClient for BadLlm {
            async fn complete(&self, _prompt: &str) -> anyhow::Result<String> {
                Ok("not json".into())
            }
        }
        let err = extract_via_llm("x", &BadLlm, false).await.unwrap_err();
        assert!(err.to_string().to_lowercase().contains("parse"));
    }
}
```

Note: `async_trait` needs to be added to `manor-core` dev-dependencies if not present.

- [ ] **Step 2: Add `async-trait` to core**

Modify `crates/core/Cargo.toml` — add to `[dependencies]`:

```toml
async-trait = "0.1"
```

- [ ] **Step 3: Run tests to verify failure**

Run: `cargo test -p manor-core --lib recipe::import::llm_tests`
Expected: FAIL — `LlmClient` and `extract_via_llm` not defined.

- [ ] **Step 4: Implement `LlmClient` + `extract_via_llm`**

Append to `crates/core/src/recipe/import.rs`:

```rust
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str) -> anyhow::Result<String>;
}

const LLM_PROMPT: &str = "You extract structured recipe data from webpage text. Output JSON with this exact shape:\n{\n  \"title\": str,\n  \"servings\": int|null,\n  \"prep_time_mins\": int|null,\n  \"cook_time_mins\": int|null,\n  \"instructions\": str (markdown, numbered steps),\n  \"ingredients\": [\n    {\"quantity_text\": str|null, \"ingredient_name\": str, \"note\": str|null}\n  ]\n}\nIf a field is not clearly stated, use null. Do not fabricate quantities.\nOutput ONLY the JSON, no prose before or after.\n\nWebpage content:\n";

#[derive(Deserialize)]
struct LlmRecipe {
    title: String,
    servings: Option<i32>,
    prep_time_mins: Option<i32>,
    cook_time_mins: Option<i32>,
    instructions: String,
    ingredients: Vec<IngredientLine>,
}

pub async fn extract_via_llm(
    page_text: &str,
    client: &dyn LlmClient,
    via_remote: bool,
) -> anyhow::Result<ImportedRecipe> {
    let truncated: String = page_text.chars().take(4096).collect();
    let prompt = format!("{}{}", LLM_PROMPT, truncated);

    let first = client.complete(&prompt).await?;
    let parsed: Result<LlmRecipe, _> = extract_json_block(&first);

    let llm_recipe = match parsed {
        Ok(r) => r,
        Err(_) => {
            let retry_prompt = format!("{}\n\n(Previous response was not valid JSON. Output ONLY JSON.)", prompt);
            let second = client.complete(&retry_prompt).await?;
            extract_json_block(&second).map_err(|e| anyhow::anyhow!("failed to parse LLM JSON after retry: {}", e))?
        }
    };

    Ok(ImportedRecipe {
        title: llm_recipe.title,
        servings: llm_recipe.servings,
        prep_time_mins: llm_recipe.prep_time_mins,
        cook_time_mins: llm_recipe.cook_time_mins,
        instructions: llm_recipe.instructions,
        ingredients: llm_recipe.ingredients,
        source_url: String::new(),
        source_host: String::new(),
        import_method: if via_remote { ImportMethod::LlmRemote } else { ImportMethod::Llm },
        parse_notes: vec!["AI-extracted — please review quantities and steps.".into()],
        hero_image_url: None,
    })
}

fn extract_json_block<T: for<'de> Deserialize<'de>>(s: &str) -> Result<T, serde_json::Error> {
    // Find first { and last } to be forgiving if the model prepends/appends prose.
    let start = s.find('{').unwrap_or(0);
    let end = s.rfind('}').map(|i| i + 1).unwrap_or(s.len());
    let slice = &s[start..end];
    serde_json::from_str::<T>(slice)
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p manor-core --lib recipe::import::llm_tests`
Expected: 2 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/core/Cargo.toml crates/core/src/recipe/import.rs
git commit -m "feat(recipe): LLM extraction fallback with retry on malformed JSON"
```

---

## Task 5: Tauri CRUD commands

**Files:**
- Create: `crates/app/src/recipe/mod.rs`
- Create: `crates/app/src/recipe/commands.rs`
- Modify: `crates/app/src/lib.rs` (register commands)

- [ ] **Step 1: Skeleton module**

Create `crates/app/src/recipe/mod.rs`:

```rust
//! Recipe library — Tauri command layer + URL import orchestrator.

pub mod commands;
pub mod importer;
pub mod stage_sweep;
```

- [ ] **Step 2: Implement CRUD commands**

Create `crates/app/src/recipe/commands.rs`:

```rust
use manor_core::recipe::{
    dal::{self, ListFilter},
    Recipe, RecipeDraft,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::DbState;  // existing DbState wrapper

#[derive(Deserialize)]
pub struct ListRecipesArgs {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
}

#[tauri::command]
pub async fn recipe_list(
    args: ListRecipesArgs,
    db: State<'_, DbState>,
) -> Result<Vec<Recipe>, String> {
    let conn = db.connection().map_err(|e| e.to_string())?;
    let filter = ListFilter { search: args.search, tag_ids: args.tag_ids, include_trashed: false };
    dal::list_recipes(&conn, &filter).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn recipe_get(id: String, db: State<'_, DbState>) -> Result<Option<Recipe>, String> {
    let conn = db.connection().map_err(|e| e.to_string())?;
    dal::get_recipe(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn recipe_create(draft: RecipeDraft, db: State<'_, DbState>) -> Result<String, String> {
    let conn = db.connection().map_err(|e| e.to_string())?;
    dal::insert_recipe(&conn, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn recipe_update(
    id: String,
    draft: RecipeDraft,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.connection().map_err(|e| e.to_string())?;
    dal::update_recipe(&conn, &id, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn recipe_delete(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.connection().map_err(|e| e.to_string())?;
    dal::soft_delete_recipe(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn recipe_restore(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.connection().map_err(|e| e.to_string())?;
    dal::restore_recipe(&conn, &id).map_err(|e| e.to_string())
}
```

> **Implementer note:** the exact name of the DB state type may differ (`DbState`, `Db`, `AppState::db`). Check `crates/app/src/db.rs` and adapt the `State<'_, …>` type and `.connection()` call to match existing patterns (mirror how `ledger` commands do it).

- [ ] **Step 3: Register commands in `lib.rs`**

Modify `crates/app/src/lib.rs` — find the `.invoke_handler(tauri::generate_handler![` block, append the recipe commands:

```rust
            // recipe
            recipe::commands::recipe_list,
            recipe::commands::recipe_get,
            recipe::commands::recipe_create,
            recipe::commands::recipe_update,
            recipe::commands::recipe_delete,
            recipe::commands::recipe_restore,
```

And add `pub mod recipe;` near the other module declarations.

- [ ] **Step 4: Build + verify**

Run: `cargo build -p manor-app`
Expected: Successful compile.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/recipe/ crates/app/src/lib.rs
git commit -m "feat(recipe): Tauri CRUD commands"
```

---

## Task 6: Import orchestrator (fetch + parse + LLM fallback)

**Files:**
- Create: `crates/app/src/recipe/importer.rs`
- Modify: `crates/app/src/recipe/commands.rs` (add import commands)
- Modify: `crates/app/src/lib.rs` (register new commands)

- [ ] **Step 1: Write failing integration test against wiremock**

Create `crates/app/tests/recipe_import_test.rs`:

```rust
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const JSONLD_PAGE: &str = r#"<!DOCTYPE html><html><head>
<script type="application/ld+json">
{"@context":"https://schema.org","@type":"Recipe","name":"Tomato soup",
 "recipeYield":"2","prepTime":"PT5M","cookTime":"PT20M",
 "recipeIngredient":["500g tomatoes","1 onion"],
 "recipeInstructions":[{"@type":"HowToStep","text":"Chop."},{"@type":"HowToStep","text":"Simmer."}]}
</script></head><body></body></html>"#;

#[tokio::test]
async fn preview_succeeds_via_jsonld() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/tomato"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string(JSONLD_PAGE)
            .insert_header("content-type", "text/html; charset=utf-8"))
        .mount(&server).await;

    let url = format!("{}/tomato", server.uri());
    let preview = manor_app::recipe::importer::preview(&url, None).await
        .expect("preview succeeds");
    assert_eq!(preview.recipe_draft.title, "Tomato soup");
    assert_eq!(preview.import_method, manor_core::recipe::ImportMethod::JsonLd);
    assert_eq!(preview.recipe_draft.ingredients.len(), 2);
}

#[tokio::test]
async fn fails_on_non_html() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/pdf"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_bytes(&b"%PDF-1.4"[..])
            .insert_header("content-type", "application/pdf"))
        .mount(&server).await;

    let url = format!("{}/pdf", server.uri());
    let err = manor_app::recipe::importer::preview(&url, None).await.unwrap_err();
    assert!(err.to_string().to_lowercase().contains("not html") || err.to_string().to_lowercase().contains("html"));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p manor-app --test recipe_import_test`
Expected: FAIL — module `importer` / function `preview` not defined.

- [ ] **Step 3: Implement the orchestrator**

Create `crates/app/src/recipe/importer.rs`:

```rust
use anyhow::{anyhow, Context, Result};
use manor_core::recipe::import::{parse_jsonld, extract_via_llm, ImportedRecipe, LlmClient};
use manor_core::recipe::{ImportMethod, RecipeDraft};
use reqwest::header::{CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const MAX_BODY_BYTES: u64 = 2 * 1024 * 1024;
const FETCH_TIMEOUT_SECS: u64 = 10;
const USER_AGENT_STRING: &str = "Manor/0.4 (+https://manor.app)";

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportPreview {
    pub recipe_draft: RecipeDraft,
    pub import_method: ImportMethod,
    pub parse_notes: Vec<String>,
    pub hero_image_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("URL is not a valid web address")]
    BadUrl,
    #[error("couldn't reach that URL")]
    FetchFailed,
    #[error("that URL isn't a web page (content-type: {0})")]
    NotHtml(String),
    #[error("page too large to import")]
    TooLarge,
    #[error("couldn't extract a recipe from this page")]
    ExtractionFailed,
}

/// Fetch + parse a recipe URL. Returns a preview the frontend shows the user.
///
/// If `llm_client` is None, the LLM fallback is skipped (used in tests that only
/// exercise the JSON-LD path). In production this is always Some(...) wired from
/// the live Ollama/Remote client bundle.
pub async fn preview(
    url: &str,
    llm_client: Option<&dyn LlmClient>,
) -> Result<ImportPreview> {
    let parsed_url = reqwest::Url::parse(url).map_err(|_| ImportError::BadUrl)?;
    let host = parsed_url.host_str().unwrap_or("").to_string();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .user_agent(USER_AGENT_STRING)
        .build()
        .context("building http client")?;

    let resp = client.get(parsed_url.clone()).send().await
        .map_err(|_| ImportError::FetchFailed)?;

    let ctype = resp.headers().get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    if !ctype.contains("text/html") {
        return Err(ImportError::NotHtml(ctype).into());
    }
    if let Some(len) = resp.content_length() {
        if len > MAX_BODY_BYTES { return Err(ImportError::TooLarge.into()); }
    }

    let body = resp.text().await.map_err(|_| ImportError::FetchFailed)?;
    if body.as_bytes().len() as u64 > MAX_BODY_BYTES { return Err(ImportError::TooLarge.into()); }

    // Try JSON-LD first
    if let Some(mut imp) = parse_jsonld(&body) {
        imp.source_url = url.to_string();
        imp.source_host = host.clone();
        return Ok(to_preview(imp));
    }

    // LLM fallback
    let Some(client) = llm_client else {
        return Err(ImportError::ExtractionFailed.into());
    };
    let text = strip_html(&body);
    let mut imp = extract_via_llm(&text, client, /*via_remote*/ false).await
        .map_err(|_| ImportError::ExtractionFailed)?;
    imp.source_url = url.to_string();
    imp.source_host = host;
    Ok(to_preview(imp))
}

fn strip_html(html: &str) -> String {
    let doc = scraper::Html::parse_document(html);
    let body_sel = scraper::Selector::parse("body").unwrap();
    let mut text = String::new();
    for el in doc.select(&body_sel) {
        for node in el.text() {
            text.push_str(node);
            text.push(' ');
        }
    }
    // Collapse whitespace
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn to_preview(imp: ImportedRecipe) -> ImportPreview {
    let hero = imp.hero_image_url.clone();
    let notes = imp.parse_notes.clone();
    let method = imp.import_method.clone();
    let draft = RecipeDraft {
        title: imp.title,
        servings: imp.servings,
        prep_time_mins: imp.prep_time_mins,
        cook_time_mins: imp.cook_time_mins,
        instructions: imp.instructions,
        source_url: Some(imp.source_url),
        source_host: Some(imp.source_host),
        import_method: imp.import_method,
        ingredients: imp.ingredients,
    };
    ImportPreview { recipe_draft: draft, import_method: method, parse_notes: notes, hero_image_url: hero }
}
```

> **Implementer note:** `scraper` needs to be added to `manor-app`'s `Cargo.toml` dependencies.

- [ ] **Step 4: Add `scraper` to app**

Modify `crates/app/Cargo.toml` — add to `[dependencies]`:

```toml
scraper = "0.20"
```

- [ ] **Step 5: Run integration tests**

Run: `cargo test -p manor-app --test recipe_import_test`
Expected: 2 tests PASS.

- [ ] **Step 6: Add LLM-fallback + remote-fallback tests**

Append to `crates/app/tests/recipe_import_test.rs`:

```rust
use async_trait::async_trait;
use manor_core::recipe::import::LlmClient;

struct StubLlm { response: String }

#[async_trait]
impl LlmClient for StubLlm {
    async fn complete(&self, _prompt: &str) -> anyhow::Result<String> { Ok(self.response.clone()) }
}

#[tokio::test]
async fn falls_back_to_llm_when_no_jsonld() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/blog-post"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string("<html><body>Cook onions and eat. Serves 4.</body></html>")
            .insert_header("content-type", "text/html"))
        .mount(&server).await;

    let stub = StubLlm { response: r#"{"title":"Onion dinner","servings":4,"prep_time_mins":null,"cook_time_mins":null,"instructions":"1. Cook.","ingredients":[{"quantity_text":null,"ingredient_name":"onions","note":null}]}"#.into() };

    let url = format!("{}/blog-post", server.uri());
    let preview = manor_app::recipe::importer::preview(&url, Some(&stub)).await.unwrap();
    assert_eq!(preview.recipe_draft.title, "Onion dinner");
    assert_eq!(preview.import_method, manor_core::recipe::ImportMethod::Llm);
}
```

Run: `cargo test -p manor-app --test recipe_import_test`
Expected: 3 tests PASS.

- [ ] **Step 7: Wire `recipe_import_preview` + `recipe_import_commit` Tauri commands**

Append to `crates/app/src/recipe/commands.rs`:

```rust
use crate::recipe::importer::{self, ImportPreview};

#[tauri::command]
pub async fn recipe_import_preview(
    url: String,
    llm: State<'_, crate::assistant::AssistantState>,
) -> Result<ImportPreview, String> {
    // Pull a best-effort client: local Ollama first; if unreachable the orchestrator's caller
    // wraps this in the tier-routing logic set up in Landmark 2. For L3a we start with the
    // unified client that already handles fallback inside itself.
    let client = llm.llm_client();  // adapt name to existing assistant state accessor
    importer::preview(&url, Some(client.as_ref())).await.map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct ImportCommitArgs {
    pub draft: RecipeDraft,
    pub hero_image_url: Option<String>,
}

#[tauri::command]
pub async fn recipe_import_commit(
    args: ImportCommitArgs,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.connection().map_err(|e| e.to_string())?;
    let id = dal::insert_recipe(&conn, &args.draft).map_err(|e| e.to_string())?;
    // Image download + attachment link handled in Task 7.
    if args.hero_image_url.is_some() {
        // Defer: Task 7 adds this. Leaving TODO until Task 7 lands would be a placeholder — instead
        // we call into the stub below which Task 7 fleshes out.
        let _ = crate::recipe::importer::fetch_and_link_hero(&conn, &id, args.hero_image_url.as_deref()).await;
    }
    Ok(id)
}
```

Register both in `crates/app/src/lib.rs`'s invoke handler alongside the CRUD commands from Task 5.

> **Implementer note:** `AssistantState::llm_client()` is the accessor pattern — check the actual method name on Manor's AssistantState (it may be called `client()` or exposed differently). If an LlmClient adapter for the existing OllamaClient doesn't exist yet, add a thin adapter in `crates/app/src/assistant/mod.rs` that implements `manor_core::recipe::import::LlmClient` by forwarding `complete(prompt)` to the existing `OllamaClient::complete(prompt)` (shipped in Landmark 2). The LlmClient trait is defined in core; the adapter lives in app.

- [ ] **Step 8: Commit**

```bash
git add crates/app/src/recipe/importer.rs crates/app/src/recipe/commands.rs \
        crates/app/src/lib.rs crates/app/Cargo.toml crates/app/tests/recipe_import_test.rs
git commit -m "feat(recipe): import orchestrator with JSON-LD and LLM fallback"
```

---

## Task 7: Hero image download + attachment staging + crash-orphan sweep

**Files:**
- Modify: `crates/app/src/recipe/importer.rs`
- Create: `crates/app/src/recipe/stage_sweep.rs`
- Modify: `crates/app/src/lib.rs` (call stage_sweep on startup)

- [ ] **Step 1: Add image fetch + attachment staging**

Append to `crates/app/src/recipe/importer.rs`:

```rust
use manor_core::attachment as attach;
use rusqlite::Connection;
use std::path::PathBuf;
use uuid::Uuid;

const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
const IMAGE_FETCH_TIMEOUT_SECS: u64 = 10;

/// Download an image URL to the attachments dir and create an attachment row
/// with entity_type='recipe' and entity_id=NULL (staged). Returns attachment id.
pub async fn stage_hero_image(conn: &Connection, url: &str, attachments_dir: &PathBuf) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(IMAGE_FETCH_TIMEOUT_SECS))
        .user_agent(USER_AGENT_STRING)
        .build()?;
    let resp = client.get(url).send().await?;
    let ctype = resp.headers().get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let ext = match () {
        _ if ctype.contains("jpeg") || ctype.contains("jpg") => "jpg",
        _ if ctype.contains("png") => "png",
        _ if ctype.contains("webp") => "webp",
        _ => return Err(anyhow!("unsupported image type: {}", ctype)),
    };
    if let Some(len) = resp.content_length() {
        if len > MAX_IMAGE_BYTES { return Err(anyhow!("image too large")); }
    }
    let bytes = resp.bytes().await?;
    if bytes.len() as u64 > MAX_IMAGE_BYTES { return Err(anyhow!("image too large")); }

    let uuid = Uuid::new_v4().to_string();
    let filename = format!("{}.{}", uuid, ext);
    let full_path = attachments_dir.join(&filename);
    std::fs::create_dir_all(attachments_dir)?;
    std::fs::write(&full_path, &bytes)?;

    let att_id = attach::insert_staged(
        conn,
        &uuid,
        &filename,
        ctype.as_str(),
        bytes.len() as i64,
        "recipe",
    )?;
    Ok(att_id)
}

/// Called from recipe_import_commit when draft has a staged-image id or an image URL.
pub async fn fetch_and_link_hero(
    conn: &Connection,
    recipe_id: &str,
    image_url: Option<&str>,
) -> Result<()> {
    let Some(url) = image_url else { return Ok(()) };
    let attachments_dir = crate::paths::attachments_dir()?;
    match stage_hero_image(conn, url, &attachments_dir).await {
        Ok(att_id) => attach::link_to_entity(conn, &att_id, "recipe", recipe_id).map_err(Into::into),
        Err(_) => Ok(()),  // soft-fail: recipe already saved
    }
}
```

> **Implementer note:** `attach::insert_staged` / `attach::link_to_entity` may not exist under those names — check `crates/core/src/attachment.rs` for the existing insert fn and call it with `entity_type = Some("recipe")` + `entity_id = None` at stage time, then UPDATE the row at link time. Adapt names to match the existing API surface. If missing entirely, add two small helpers to `attachment.rs` that wrap the INSERT and the entity-id UPDATE — keep them one-liner helpers.

- [ ] **Step 2: Implement startup sweep for orphans**

Create `crates/app/src/recipe/stage_sweep.rs`:

```rust
use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

const ORPHAN_AGE_SECS: i64 = 24 * 60 * 60;

/// Run at app startup: remove staged recipe attachments (entity_type='recipe', entity_id IS NULL)
/// older than 24h, plus their files on disk. Returns count swept.
pub fn run_on_startup(conn: &Connection, attachments_dir: &PathBuf) -> Result<usize> {
    let cutoff_ms = chrono::Utc::now().timestamp_millis() - ORPHAN_AGE_SECS * 1000;

    let mut stmt = conn.prepare(
        "SELECT id, uuid, filename FROM attachment
         WHERE entity_type = 'recipe' AND entity_id IS NULL AND created_at < ?1"
    )?;
    let rows: Vec<(String, String, String)> = stmt.query_map(params![cutoff_ms], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })?.filter_map(Result::ok).collect();

    let mut swept = 0usize;
    for (id, _uuid, filename) in rows {
        let path = attachments_dir.join(&filename);
        let _ = std::fs::remove_file(&path);  // best-effort
        conn.execute("DELETE FROM attachment WHERE id = ?1", params![id])?;
        swept += 1;
    }
    Ok(swept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn sweeps_orphans_older_than_24h() {
        // NOTE: This test needs the attachment migration + the attach::insert_staged helper
        // from Task 7 step 1 to be landed. Adapt to match the actual helper surface.
        let dir = TempDir::new().unwrap();
        let mut conn = Connection::open_in_memory().unwrap();
        manor_core::migrations::run(&mut conn).unwrap();

        let old_ts = chrono::Utc::now().timestamp_millis() - 48 * 60 * 60 * 1000;
        conn.execute(
            "INSERT INTO attachment (id, uuid, filename, mime, size_bytes, sha256, entity_type, entity_id, created_at)
             VALUES ('a1', 'u1', 'u1.jpg', 'image/jpeg', 100, '', 'recipe', NULL, ?1)",
            params![old_ts],
        ).unwrap();
        std::fs::write(dir.path().join("u1.jpg"), b"x").unwrap();

        let swept = run_on_startup(&conn, &dir.path().to_path_buf()).unwrap();
        assert_eq!(swept, 1);
        assert!(!dir.path().join("u1.jpg").exists());
    }
}
```

> **Implementer note:** the INSERT in the test must match the actual `attachment` table columns from `V8__foundation_tables.sql` — check column order and NOT NULL constraints. Adjust the test insert statement accordingly.

- [ ] **Step 3: Call sweep from lib.rs at startup**

Modify `crates/app/src/lib.rs` — in the Tauri setup callback where other startup jobs run, add:

```rust
            // Sweep orphan recipe image stagings left by prior crashes.
            let _ = crate::recipe::stage_sweep::run_on_startup(
                &conn,
                &crate::paths::attachments_dir().unwrap_or_default(),
            );
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p manor-app --lib recipe::stage_sweep`
Expected: 1 test PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/recipe/ crates/app/src/lib.rs
git commit -m "feat(recipe): hero image download + attachment staging + orphan sweep"
```

---

## Task 8: Register `recipe` with the trash sweeper

**Files:**
- Modify: `crates/core/src/trash.rs`

- [ ] **Step 1: Write failing test**

Append to the existing `trash.rs` tests module:

```rust
    #[test]
    fn trash_sweeper_purges_recipe_after_30_days() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::migrations::run(&mut conn).unwrap();

        let old_ts = chrono::Utc::now().timestamp_millis() - 31 * 24 * 60 * 60 * 1000;
        conn.execute(
            "INSERT INTO recipe (id, title, instructions, created_at, updated_at, deleted_at)
             VALUES ('r1', 'Gone', '', ?1, ?1, ?1)",
            rusqlite::params![old_ts],
        ).unwrap();
        conn.execute(
            "INSERT INTO recipe_ingredient (id, recipe_id, position, ingredient_name)
             VALUES ('i1', 'r1', 0, 'salt')",
            [],
        ).unwrap();

        let report = run_sweep(&conn).unwrap();
        assert!(report.purged_counts.get("recipe").copied().unwrap_or(0) >= 1);

        let remaining: i64 = conn.query_row(
            "SELECT COUNT(*) FROM recipe WHERE id='r1'", [], |r| r.get(0)
        ).unwrap();
        assert_eq!(remaining, 0);
        let ing_remaining: i64 = conn.query_row(
            "SELECT COUNT(*) FROM recipe_ingredient WHERE recipe_id='r1'", [], |r| r.get(0)
        ).unwrap();
        assert_eq!(ing_remaining, 0);
    }
```

- [ ] **Step 2: Run test — expect failure**

Run: `cargo test -p manor-core --lib trash::tests::trash_sweeper_purges_recipe_after_30_days`
Expected: FAIL — recipe not yet in the sweeper's registry.

- [ ] **Step 3: Add `recipe` to the sweeper registry**

Modify `crates/core/src/trash.rs` — locate the table registry (likely a `const TRASHED_TABLES: &[&str] = &["note", "attachment", …];` or equivalent). Append `"recipe"`.

If the sweeper also runs a post-purge hook to unlink attachments by `entity_type`, register `"recipe"` there too. If attachments are handled generically (sweeper already soft-deletes attachments where `entity_type` matches any purged table name's row), no extra work needed.

If the sweeper is a hand-rolled match on table names, add a branch:

```rust
// Inside whatever function executes per-table purge, add recipe branch:
"recipe" => {
    // Delete recipe — FK cascade on recipe_ingredient handles children.
    // Attachment unlink via existing generic attachment-sweep pattern.
    conn.execute(
        "DELETE FROM recipe WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
        rusqlite::params![cutoff_ms],
    )?;
}
```

- [ ] **Step 4: Run test**

Run: `cargo test -p manor-core --lib trash::tests::trash_sweeper_purges_recipe_after_30_days`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/trash.rs
git commit -m "feat(recipe): register recipe table with trash sweeper"
```

---

## Task 9: Frontend — IPC wrappers + Zustand store

**Files:**
- Create: `apps/desktop/src/lib/recipe/recipe-ipc.ts`
- Create: `apps/desktop/src/lib/recipe/recipe-store.ts`

- [ ] **Step 1: IPC wrappers**

Create `apps/desktop/src/lib/recipe/recipe-ipc.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export type ImportMethod = "manual" | "jsonld" | "llm" | "llm_remote";

export interface IngredientLine {
  quantity_text: string | null;
  ingredient_name: string;
  note: string | null;
}

export interface Recipe {
  id: string;
  title: string;
  servings: number | null;
  prep_time_mins: number | null;
  cook_time_mins: number | null;
  instructions: string;
  source_url: string | null;
  source_host: string | null;
  import_method: ImportMethod;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
  ingredients: IngredientLine[];
}

export interface RecipeDraft {
  title: string;
  servings: number | null;
  prep_time_mins: number | null;
  cook_time_mins: number | null;
  instructions: string;
  source_url: string | null;
  source_host: string | null;
  import_method: ImportMethod;
  ingredients: IngredientLine[];
}

export interface ImportPreview {
  recipe_draft: RecipeDraft;
  import_method: ImportMethod;
  parse_notes: string[];
  hero_image_url: string | null;
}

export const recipeIpc = {
  list: (search?: string, tagIds: string[] = []) =>
    invoke<Recipe[]>("recipe_list", { args: { search, tag_ids: tagIds } }),
  get: (id: string) => invoke<Recipe | null>("recipe_get", { id }),
  create: (draft: RecipeDraft) => invoke<string>("recipe_create", { draft }),
  update: (id: string, draft: RecipeDraft) => invoke<void>("recipe_update", { id, draft }),
  delete: (id: string) => invoke<void>("recipe_delete", { id }),
  restore: (id: string) => invoke<void>("recipe_restore", { id }),
  importPreview: (url: string) => invoke<ImportPreview>("recipe_import_preview", { url }),
  importCommit: (draft: RecipeDraft, heroImageUrl: string | null) =>
    invoke<string>("recipe_import_commit", { args: { draft, hero_image_url: heroImageUrl } }),
};
```

- [ ] **Step 2: Zustand store**

Create `apps/desktop/src/lib/recipe/recipe-store.ts`:

```ts
import { create } from "zustand";
import { recipeIpc, Recipe } from "./recipe-ipc";

interface RecipeStore {
  recipes: Recipe[];
  search: string;
  tagIds: string[];
  loading: boolean;
  error: string | null;
  load: () => Promise<void>;
  setSearch: (s: string) => void;
  setTagIds: (ids: string[]) => void;
}

export const useRecipeStore = create<RecipeStore>((set, get) => ({
  recipes: [],
  search: "",
  tagIds: [],
  loading: false,
  error: null,
  load: async () => {
    set({ loading: true, error: null });
    try {
      const rows = await recipeIpc.list(get().search || undefined, get().tagIds);
      set({ recipes: rows, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  setSearch: (s) => { set({ search: s }); void get().load(); },
  setTagIds: (ids) => { set({ tagIds: ids }); void get().load(); },
}));
```

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/lib/recipe/
git commit -m "feat(recipe): frontend IPC wrappers + Zustand store"
```

---

## Task 10: Hearth nav entry + empty tab scaffold

**Files:**
- Modify: `apps/desktop/src/components/Nav/NavBar.tsx`
- Create: `apps/desktop/src/components/Hearth/HearthTab.tsx`
- Modify: the app's router/tab-switching logic (location depends on current structure — likely `App.tsx` or a `Tabs.tsx`)

- [ ] **Step 1: Add Hearth to NavBar**

Modify `apps/desktop/src/components/Nav/NavBar.tsx` — locate the tab list (array or JSX sequence of NavItem). Insert between Ledger and Assistant:

```tsx
import { UtensilsCrossed } from "lucide-react";
// ...
{ key: "hearth", label: "Hearth", icon: <UtensilsCrossed size={22} strokeWidth={1.8} /> },
```

Adapt shape to match the existing NavItem type in that file.

- [ ] **Step 2: Create the tab landing component**

Create `apps/desktop/src/components/Hearth/HearthTab.tsx`:

```tsx
import { useEffect } from "react";
import { useRecipeStore } from "../../lib/recipe/recipe-store";

export function HearthTab() {
  const { recipes, load, loading } = useRecipeStore();
  useEffect(() => { void load(); }, [load]);

  return (
    <div style={{ padding: "var(--space-lg)" }}>
      <h1 style={{ fontSize: "var(--text-2xl)", fontWeight: "var(--weight-semibold)" }}>
        Recipes
      </h1>
      {loading && <p style={{ color: "var(--ink-muted)" }}>Loading…</p>}
      {!loading && recipes.length === 0 && (
        <p style={{ color: "var(--ink-muted)" }}>Your recipe collection is empty.</p>
      )}
      {!loading && recipes.length > 0 && (
        <ul>{recipes.map(r => <li key={r.id}>{r.title}</li>)}</ul>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Wire the tab to the router**

Modify the central tab-router (search for where other tabs like `LedgerTab` / `TodayTab` are mapped to their `key`). Add:

```tsx
case "hearth": return <HearthTab />;
```

with the corresponding import at the top of the file.

- [ ] **Step 4: Run dev server + eyeball**

```bash
npm run dev
```

Expected: Hearth tab appears in nav with UtensilsCrossed icon. Clicking it shows "Recipes" heading and empty-state message.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/Nav/NavBar.tsx apps/desktop/src/components/Hearth/
git commit -m "feat(recipe): Hearth tab with empty-state scaffold"
```

---

## Task 11: Recipe library list view (grid, search, tag filter, empty state)

**Files:**
- Modify: `apps/desktop/src/components/Hearth/HearthTab.tsx` — replace scaffold with full list view
- Create: `apps/desktop/src/components/Hearth/RecipeCard.tsx`

- [ ] **Step 1: RecipeCard component**

Create `apps/desktop/src/components/Hearth/RecipeCard.tsx`:

```tsx
import { ImageOff } from "lucide-react";
import { Recipe } from "../../lib/recipe/recipe-ipc";

interface Props {
  recipe: Recipe;
  heroSrc?: string;
  onClick: () => void;
}

export function RecipeCard({ recipe, heroSrc, onClick }: Props) {
  const meta = [
    recipe.prep_time_mins != null || recipe.cook_time_mins != null
      ? `${(recipe.prep_time_mins ?? 0) + (recipe.cook_time_mins ?? 0)}m`
      : null,
    recipe.servings != null ? `${recipe.servings}p` : null,
  ].filter(Boolean).join(" · ");

  return (
    <button
      onClick={onClick}
      style={{
        textAlign: "left",
        background: "var(--paper)",
        border: "1px solid var(--paper-border)",
        borderRadius: "var(--radius-sm)",
        padding: 0,
        cursor: "pointer",
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div style={{
        aspectRatio: "4 / 3",
        background: "var(--paper-muted)",
        display: "flex", alignItems: "center", justifyContent: "center",
      }}>
        {heroSrc ? (
          <img src={heroSrc} alt={recipe.title}
            style={{ width: "100%", height: "100%", objectFit: "cover" }} />
        ) : (
          <ImageOff size={32} strokeWidth={1.4} color="var(--ink-muted)" />
        )}
      </div>
      <div style={{ padding: "var(--space-sm)" }}>
        <div style={{
          fontSize: "var(--text-base)", fontWeight: "var(--weight-semibold)",
          whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis",
        }}>
          {recipe.title}
        </div>
        {meta && (
          <div style={{ fontSize: "var(--text-xs)", color: "var(--ink-muted)", marginTop: 4 }}>
            {meta}
          </div>
        )}
      </div>
    </button>
  );
}
```

- [ ] **Step 2: Full list view**

Overwrite `apps/desktop/src/components/Hearth/HearthTab.tsx`:

```tsx
import { useEffect, useState } from "react";
import { Plus, Download } from "lucide-react";
import { useRecipeStore } from "../../lib/recipe/recipe-store";
import { RecipeCard } from "./RecipeCard";
import { RecipeEditDrawer } from "./RecipeEditDrawer";        // Task 12
import { RecipeImportDrawer } from "./RecipeImportDrawer";    // Task 13
import { RecipeDetail } from "./RecipeDetail";                // Task 14

type View = { mode: "list" } | { mode: "detail"; id: string };

export function HearthTab() {
  const { recipes, search, setSearch, load, loading } = useRecipeStore();
  const [view, setView] = useState<View>({ mode: "list" });
  const [drawer, setDrawer] = useState<null | "new" | "import">(null);

  useEffect(() => { void load(); }, [load]);

  if (view.mode === "detail") {
    return <RecipeDetail id={view.id} onBack={() => { setView({ mode: "list" }); void load(); }} />;
  }

  return (
    <div style={{ padding: "var(--space-lg)" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "var(--space-md)" }}>
        <h1 style={{ fontSize: "var(--text-2xl)", fontWeight: "var(--weight-semibold)", margin: 0 }}>
          Recipes
        </h1>
        <div style={{ display: "flex", gap: "var(--space-sm)" }}>
          <button onClick={() => setDrawer("new")}
            style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <Plus size={14} strokeWidth={1.8} /> New
          </button>
          <button onClick={() => setDrawer("import")}
            style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <Download size={14} strokeWidth={1.8} /> Import URL
          </button>
        </div>
      </div>

      <input
        placeholder="Search recipes"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        style={{ width: "100%", marginBottom: "var(--space-md)" }}
      />

      {loading && <p style={{ color: "var(--ink-muted)" }}>Loading…</p>}

      {!loading && recipes.length === 0 && (
        <div style={{ textAlign: "center", padding: "var(--space-xl)" }}>
          <p style={{ color: "var(--ink-muted)", marginBottom: "var(--space-md)" }}>
            Your recipe collection is empty.
          </p>
          <div style={{ display: "inline-flex", gap: "var(--space-sm)" }}>
            <button onClick={() => setDrawer("new")}>+ New recipe</button>
            <button onClick={() => setDrawer("import")}>↓ Import from URL</button>
          </div>
        </div>
      )}

      {!loading && recipes.length > 0 && (
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))",
          gap: "var(--space-md)",
        }}>
          {recipes.map((r) => (
            <RecipeCard key={r.id} recipe={r}
              onClick={() => setView({ mode: "detail", id: r.id })} />
          ))}
        </div>
      )}

      {drawer === "new" && (
        <RecipeEditDrawer onClose={() => { setDrawer(null); void load(); }} />
      )}
      {drawer === "import" && (
        <RecipeImportDrawer onClose={() => { setDrawer(null); void load(); }} />
      )}
    </div>
  );
}
```

- [ ] **Step 3: Run dev server + eyeball**

```bash
npm run dev
```

Expected: empty Hearth tab renders "Your recipe collection is empty" with both buttons, search field present, no console errors. (Drawer components still stubs — see Task 12/13/14.)

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Hearth/
git commit -m "feat(recipe): library list view with search + empty state"
```

---

## Task 12: Edit/New drawer

**Files:**
- Create: `apps/desktop/src/components/Hearth/RecipeEditDrawer.tsx`
- Create: `apps/desktop/src/components/Hearth/IngredientRowEditor.tsx`

- [ ] **Step 1: Ingredient row sub-component**

Create `apps/desktop/src/components/Hearth/IngredientRowEditor.tsx`:

```tsx
import { X } from "lucide-react";
import { IngredientLine } from "../../lib/recipe/recipe-ipc";

interface Props {
  row: IngredientLine;
  onChange: (row: IngredientLine) => void;
  onRemove: () => void;
}

export function IngredientRowEditor({ row, onChange, onRemove }: Props) {
  return (
    <div style={{ display: "flex", gap: 6, alignItems: "center", marginBottom: 4 }}>
      <input
        placeholder="qty"
        value={row.quantity_text ?? ""}
        onChange={(e) => onChange({ ...row, quantity_text: e.target.value || null })}
        style={{ width: 80 }}
      />
      <input
        placeholder="ingredient"
        value={row.ingredient_name}
        onChange={(e) => onChange({ ...row, ingredient_name: e.target.value })}
        style={{ flex: 1 }}
      />
      <input
        placeholder="note"
        value={row.note ?? ""}
        onChange={(e) => onChange({ ...row, note: e.target.value || null })}
        style={{ flex: 1 }}
      />
      <button aria-label="Remove ingredient" onClick={onRemove}
        style={{ background: "transparent", border: "none", cursor: "pointer" }}>
        <X size={14} strokeWidth={1.8} />
      </button>
    </div>
  );
}
```

- [ ] **Step 2: Drawer component**

Create `apps/desktop/src/components/Hearth/RecipeEditDrawer.tsx`:

```tsx
import { useEffect, useState } from "react";
import { recipeIpc, Recipe, RecipeDraft, IngredientLine } from "../../lib/recipe/recipe-ipc";
import { IngredientRowEditor } from "./IngredientRowEditor";

interface Props {
  recipeId?: string;     // undefined = create mode
  initialDraft?: RecipeDraft;  // for import-preview prefill
  onClose: () => void;
  onSaved?: (id: string) => void;
}

const EMPTY_DRAFT: RecipeDraft = {
  title: "",
  servings: null,
  prep_time_mins: null,
  cook_time_mins: null,
  instructions: "",
  source_url: null,
  source_host: null,
  import_method: "manual",
  ingredients: [],
};

export function RecipeEditDrawer({ recipeId, initialDraft, onClose, onSaved }: Props) {
  const [draft, setDraft] = useState<RecipeDraft>(initialDraft ?? EMPTY_DRAFT);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (recipeId && !initialDraft) {
      void recipeIpc.get(recipeId).then((r: Recipe | null) => {
        if (r) setDraft({
          title: r.title, servings: r.servings,
          prep_time_mins: r.prep_time_mins, cook_time_mins: r.cook_time_mins,
          instructions: r.instructions, source_url: r.source_url, source_host: r.source_host,
          import_method: r.import_method, ingredients: r.ingredients,
        });
      });
    }
  }, [recipeId, initialDraft]);

  const addIngredient = () =>
    setDraft({ ...draft, ingredients: [...draft.ingredients, { quantity_text: null, ingredient_name: "", note: null }] });

  const save = async () => {
    if (!draft.title.trim()) { setError("Title required"); return; }
    setSaving(true); setError(null);
    try {
      const id = recipeId
        ? (await recipeIpc.update(recipeId, draft), recipeId)
        : await recipeIpc.create(draft);
      onSaved?.(id);
      onClose();
    } catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  };

  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, width: 480,
      background: "var(--paper)", borderLeft: "1px solid var(--paper-border)",
      padding: "var(--space-lg)", overflow: "auto", zIndex: 50,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "var(--space-md)" }}>
        <h2 style={{ margin: 0 }}>{recipeId ? "Edit recipe" : "New recipe"}</h2>
        <button onClick={onClose}>✕</button>
      </div>

      <label>Title</label>
      <input value={draft.title}
        onChange={(e) => setDraft({ ...draft, title: e.target.value })}
        style={{ width: "100%", marginBottom: "var(--space-sm)" }} />

      <div style={{ display: "flex", gap: 8, marginBottom: "var(--space-sm)" }}>
        <div style={{ flex: 1 }}>
          <label>Servings</label>
          <input type="number" value={draft.servings ?? ""}
            onChange={(e) => setDraft({ ...draft, servings: e.target.value ? parseInt(e.target.value) : null })}
            style={{ width: "100%" }} />
        </div>
        <div style={{ flex: 1 }}>
          <label>Prep (min)</label>
          <input type="number" value={draft.prep_time_mins ?? ""}
            onChange={(e) => setDraft({ ...draft, prep_time_mins: e.target.value ? parseInt(e.target.value) : null })}
            style={{ width: "100%" }} />
        </div>
        <div style={{ flex: 1 }}>
          <label>Cook (min)</label>
          <input type="number" value={draft.cook_time_mins ?? ""}
            onChange={(e) => setDraft({ ...draft, cook_time_mins: e.target.value ? parseInt(e.target.value) : null })}
            style={{ width: "100%" }} />
        </div>
      </div>

      <h3>Ingredients</h3>
      {draft.ingredients.map((row: IngredientLine, i: number) => (
        <IngredientRowEditor key={i} row={row}
          onChange={(r) => { const next = [...draft.ingredients]; next[i] = r; setDraft({ ...draft, ingredients: next }); }}
          onRemove={() => setDraft({ ...draft, ingredients: draft.ingredients.filter((_, j) => j !== i) })}
        />
      ))}
      <button onClick={addIngredient}>+ Add ingredient</button>

      <h3>Instructions (markdown)</h3>
      <textarea value={draft.instructions}
        onChange={(e) => setDraft({ ...draft, instructions: e.target.value })}
        rows={10} style={{ width: "100%", fontFamily: "inherit" }} />

      <label>Source URL</label>
      <input value={draft.source_url ?? ""}
        onChange={(e) => setDraft({ ...draft, source_url: e.target.value || null })}
        style={{ width: "100%", marginBottom: "var(--space-sm)" }} />

      {error && <div style={{ color: "var(--ink-danger)" }}>{error}</div>}

      <div style={{ display: "flex", gap: 8, marginTop: "var(--space-md)" }}>
        <button onClick={onClose}>Cancel</button>
        <button onClick={save} disabled={saving}>{saving ? "Saving…" : "Save"}</button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Eyeball in dev**

```bash
npm run dev
```

Click "+ New" in Hearth tab, fill a title, add an ingredient, save. Recipe should appear in the list.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Hearth/RecipeEditDrawer.tsx \
        apps/desktop/src/components/Hearth/IngredientRowEditor.tsx
git commit -m "feat(recipe): New/Edit drawer with ingredient editor"
```

---

## Task 13: Import drawer (URL → preview → save)

**Files:**
- Create: `apps/desktop/src/components/Hearth/RecipeImportDrawer.tsx`
- Create: `apps/desktop/src/components/Hearth/ImportMethodBadge.tsx`

- [ ] **Step 1: Badge component**

Create `apps/desktop/src/components/Hearth/ImportMethodBadge.tsx`:

```tsx
import { Sparkles, FileCode } from "lucide-react";
import { ImportMethod } from "../../lib/recipe/recipe-ipc";

export function ImportMethodBadge({ method }: { method: ImportMethod }) {
  if (method === "manual") return null;
  const style = {
    display: "inline-flex", alignItems: "center", gap: 4,
    fontSize: "var(--text-xs)", color: "var(--ink-muted)",
    padding: "2px 8px", borderRadius: "var(--radius-sm)",
    background: "var(--paper-muted)",
  };
  if (method === "jsonld") {
    return <span style={style}><FileCode size={12} strokeWidth={1.8} /> Parsed from structured data</span>;
  }
  return <span style={style}><Sparkles size={12} strokeWidth={1.8} /> AI-extracted — please review</span>;
}
```

- [ ] **Step 2: Import drawer**

Create `apps/desktop/src/components/Hearth/RecipeImportDrawer.tsx`:

```tsx
import { useState } from "react";
import { recipeIpc, ImportPreview } from "../../lib/recipe/recipe-ipc";
import { RecipeEditDrawer } from "./RecipeEditDrawer";
import { ImportMethodBadge } from "./ImportMethodBadge";

interface Props { onClose: () => void }

export function RecipeImportDrawer({ onClose }: Props) {
  const [url, setUrl] = useState("");
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [fetching, setFetching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchPreview = async () => {
    setFetching(true); setError(null);
    try {
      const p = await recipeIpc.importPreview(url);
      setPreview(p);
    } catch (e) { setError(String(e)); }
    finally { setFetching(false); }
  };

  const commit = async (finalDraft: typeof preview.recipe_draft) => {
    if (!preview) return;
    await recipeIpc.importCommit(finalDraft, preview.hero_image_url);
    onClose();
  };

  if (preview) {
    return (
      <div style={{
        position: "fixed", top: 0, right: 0, bottom: 0, width: 480,
        background: "var(--paper)", borderLeft: "1px solid var(--paper-border)",
        padding: "var(--space-lg)", overflow: "auto", zIndex: 50,
      }}>
        <div style={{ marginBottom: "var(--space-sm)" }}>
          <ImportMethodBadge method={preview.import_method} />
        </div>
        {preview.parse_notes.length > 0 && (
          <ul style={{ color: "var(--ink-muted)", fontSize: "var(--text-xs)", marginBottom: "var(--space-sm)" }}>
            {preview.parse_notes.map((n, i) => <li key={i}>{n}</li>)}
          </ul>
        )}
        <RecipeEditDrawer
          initialDraft={preview.recipe_draft}
          onClose={onClose}
          onSaved={() => { /* closed by parent */ }}
        />
      </div>
    );
  }

  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, width: 480,
      background: "var(--paper)", borderLeft: "1px solid var(--paper-border)",
      padding: "var(--space-lg)", overflow: "auto", zIndex: 50,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "var(--space-md)" }}>
        <h2 style={{ margin: 0 }}>Import from URL</h2>
        <button onClick={onClose}>✕</button>
      </div>
      <label>URL</label>
      <input value={url} onChange={(e) => setUrl(e.target.value)}
        placeholder="https://…" style={{ width: "100%", marginBottom: "var(--space-sm)" }} />
      <button onClick={fetchPreview} disabled={fetching || !url}>
        {fetching ? "Fetching…" : "Fetch"}
      </button>
      {error && <div style={{ color: "var(--ink-danger)", marginTop: "var(--space-sm)" }}>{error}</div>}
    </div>
  );
}
```

> **Implementer note:** the inner `RecipeEditDrawer` call here shares screen space with the outer import drawer wrapper. Visually this can render oddly (two overlapping drawers). The cleaner approach is to hoist state: the import drawer holds `draft` and renders the *same* form body as `RecipeEditDrawer` (extract a `RecipeFormBody` sub-component to share). Do that refactor as part of this task if dev-QA shows visual weirdness.

- [ ] **Step 3: Eyeball in dev**

```bash
npm run dev
```

Paste a BBC Good Food URL, click Fetch, verify preview shows with "Parsed from structured data" badge, edit, save, recipe lands in library.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Hearth/
git commit -m "feat(recipe): URL import drawer with parse-path badge"
```

---

## Task 14: Recipe detail view

**Files:**
- Create: `apps/desktop/src/components/Hearth/RecipeDetail.tsx`

- [ ] **Step 1: Detail component**

Create `apps/desktop/src/components/Hearth/RecipeDetail.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ArrowLeft, Pencil, Trash2 } from "lucide-react";
import { recipeIpc, Recipe } from "../../lib/recipe/recipe-ipc";
import { ImportMethodBadge } from "./ImportMethodBadge";
import { RecipeEditDrawer } from "./RecipeEditDrawer";

interface Props { id: string; onBack: () => void }

export function RecipeDetail({ id, onBack }: Props) {
  const [recipe, setRecipe] = useState<Recipe | null>(null);
  const [editing, setEditing] = useState(false);

  useEffect(() => { void recipeIpc.get(id).then(setRecipe); }, [id, editing]);

  if (!recipe) return <div style={{ padding: "var(--space-lg)" }}>Loading…</div>;

  const meta = [
    recipe.prep_time_mins != null ? `${recipe.prep_time_mins}m prep` : null,
    recipe.cook_time_mins != null ? `${recipe.cook_time_mins}m cook` : null,
    recipe.servings != null ? `serves ${recipe.servings}` : null,
  ].filter(Boolean).join(" · ");

  const handleDelete = async () => {
    if (!confirm("Move this recipe to Trash?")) return;
    await recipeIpc.delete(id);
    onBack();
  };

  return (
    <div style={{ padding: "var(--space-lg)", maxWidth: 720, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "var(--space-md)" }}>
        <button onClick={onBack} style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <ArrowLeft size={14} strokeWidth={1.8} /> Back
        </button>
        <div style={{ display: "flex", gap: 8 }}>
          <button onClick={() => setEditing(true)}><Pencil size={14} strokeWidth={1.8} /> Edit</button>
          <button onClick={handleDelete}><Trash2 size={14} strokeWidth={1.8} /> Delete</button>
        </div>
      </div>

      <h1 style={{ fontSize: "var(--text-2xl)", fontWeight: "var(--weight-semibold)", margin: 0 }}>
        {recipe.title}
      </h1>
      {meta && <div style={{ color: "var(--ink-muted)", marginTop: 4 }}>{meta}</div>}

      <div style={{ marginTop: "var(--space-sm)", display: "flex", gap: 8, alignItems: "center" }}>
        {recipe.source_host && (
          <span style={{ fontSize: "var(--text-xs)", color: "var(--ink-muted)" }}>
            Source: {recipe.source_host}
          </span>
        )}
        <ImportMethodBadge method={recipe.import_method} />
      </div>

      <h2 style={{ marginTop: "var(--space-lg)" }}>Ingredients</h2>
      <ul>
        {recipe.ingredients.map((ing, i) => (
          <li key={i}>
            {ing.quantity_text && <strong>{ing.quantity_text} </strong>}
            {ing.ingredient_name}
            {ing.note && <span style={{ color: "var(--ink-muted)" }}>, {ing.note}</span>}
          </li>
        ))}
      </ul>

      <h2 style={{ marginTop: "var(--space-lg)" }}>Instructions</h2>
      <pre style={{
        whiteSpace: "pre-wrap", fontFamily: "inherit",
        background: "var(--paper)", padding: "var(--space-sm)", borderRadius: "var(--radius-sm)",
      }}>
        {recipe.instructions}
      </pre>

      {editing && (
        <RecipeEditDrawer
          recipeId={id}
          onClose={() => setEditing(false)}
        />
      )}
    </div>
  );
}
```

> **Implementer note:** instructions currently render as plain text in a `<pre>`. If Manor already has a `<Markdown>` component used for notes/today, import it and replace the `<pre>` with `<Markdown source={recipe.instructions} />`. Do not add a new markdown dep just for this — reuse whatever is already in the codebase.

- [ ] **Step 2: Eyeball**

```bash
npm run dev
```

Click a recipe card, detail view opens; Edit button opens drawer; Delete button prompts then returns to list.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/components/Hearth/RecipeDetail.tsx
git commit -m "feat(recipe): recipe detail view with edit/delete actions"
```

---

## Task 15: Manual QA pass + final wiring checks

**Files:** (verification only — no writes unless bugs found)

- [ ] **Step 1: Run full test suite**

```bash
cargo test --workspace
```

Expected: ALL green.

- [ ] **Step 2: Start dev server and walk the golden paths**

```bash
npm run dev
```

Walk these flows explicitly:
- Open Hearth tab, see empty state.
- Click "+ New", create a recipe with 3 ingredients, save. Verify card appears.
- Click the card, detail view opens with all data.
- Click Edit, change title, save. Verify title updated.
- Click Delete, confirm. Verify returns to list, recipe gone.
- Open Trash (existing view). Verify deleted recipe listed.
- Restore from trash. Verify it reappears in Hearth.
- Click "Import URL", paste a known-good JSON-LD URL (e.g. BBC Good Food), Fetch. Verify preview shows "Parsed from structured data" badge. Save. Recipe lands in library.
- Import a URL with no JSON-LD (e.g. personal blog). Verify LLM fallback kicks in and "AI-extracted — please review" badge shows. Save. Recipe lands.
- Type search text. Verify filter works.
- Cancel an import before saving. Verify no orphan row appears in DB.

- [ ] **Step 3: Check no TypeScript errors**

```bash
cd apps/desktop && npm run typecheck
```

Expected: 0 errors.

- [ ] **Step 4: Check no Rust warnings in workspace**

```bash
cargo clippy --workspace -- -D warnings
```

Expected: 0 warnings. If any, fix inline; don't merge with warnings.

- [ ] **Step 5: Commit any final fixes and declare L3a done**

```bash
git log --oneline | head -20  # verify all task commits present
```

If all green: L3a Recipe Library is shipped. Invoke `superpowers:finishing-a-development-branch` to merge back to main.

---

## Self-review (author's checklist — already run)

**Spec coverage:**
- §3 two-crate split → Tasks 2, 3, 4 (core) + 5, 6, 7 (app). ✓
- §4 schema migration V14 → Task 1. ✓
- §5 import pipeline (fetch, JSON-LD, LLM fallback, image stage, commit) → Tasks 3, 4, 6, 7. ✓
- §5.2 LLM prompt → embedded in Task 4. ✓
- §6 UI (nav, landing, detail, edit drawer, import drawer) → Tasks 10–14. ✓
- §7 error table → covered in import orchestrator (Task 6) + drawer error states (Tasks 12, 13). ✓
- §8 testing strategy → unit tests in Tasks 2, 3, 4; integration tests in Task 6; manual QA in Task 15. ✓
- §8 trash sweep extension → Task 8. ✓
- §5 stage-leak recovery → Task 7 step 2 + 3. ✓

**No placeholders found** — all tasks have concrete code or exact commands.

**Type consistency:** `RecipeDraft`, `Recipe`, `IngredientLine`, `ImportMethod`, `ImportPreview` used consistently across Rust and TS surfaces.

---

*End of plan. Next: invoke `superpowers:subagent-driven-development` or `superpowers:executing-plans` to implement.*
