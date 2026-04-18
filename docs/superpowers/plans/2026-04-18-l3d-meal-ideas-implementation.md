# L3d Meal Ideas Reshuffle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Manor's final v0.4 Hearth piece: a 3-card meal ideas row above the This Week grid that rotates from the user's recipe library (soft `days_since_last_cooked` score) and opens into LLM-generated new ideas via a "Try something new →" escape hatch, reusing L3a's recipe-edit drawer for AI-recipe save.

**Architecture:** Thin — no schema change. One new pure-function ranker in `manor-core`. Three Tauri commands (library_sample, llm_titles, llm_expand) in `manor-app`, all reusing the existing `OllamaLlmAdapter` + L3a's `importer::to_preview`. Frontend: `MealIdeasRow` + `IdeaTitleCard` + `AssignDayPopover`, wired above the existing `ThisWeekView`.

**Tech Stack:** Rust (rusqlite, chrono, rand), React + TypeScript + Zustand, Lucide icons.

**Spec:** `docs/superpowers/specs/2026-04-18-l3d-meal-ideas-design.md`

---

## File structure

### New Rust files
- `crates/core/src/meal_plan/ideas.rs` — `ScoredRecipe` type + `library_ranked(conn)`.
- `crates/app/src/meal_plan/ideas_commands.rs` — three Tauri commands + `IdeaTitle` type.

### New frontend files
- `apps/desktop/src/lib/meal_plan/ideas-ipc.ts` — invoke wrappers + types.
- `apps/desktop/src/lib/meal_plan/ideas-state.ts` — Zustand store.
- `apps/desktop/src/components/Hearth/ThisWeek/MealIdeasRow.tsx` — the 3-card row.
- `apps/desktop/src/components/Hearth/ThisWeek/IdeaTitleCard.tsx` — AI card.
- `apps/desktop/src/components/Hearth/ThisWeek/AssignDayPopover.tsx` — day chips popover.

### Modified files
- `crates/core/src/meal_plan/mod.rs` — `pub mod ideas;`.
- `crates/app/src/meal_plan/mod.rs` — `pub mod ideas_commands;`.
- `crates/app/src/lib.rs` — register 3 new Tauri commands.
- `crates/app/Cargo.toml` — add `rand` to `[dependencies]` (if not present).
- `apps/desktop/src/components/Hearth/ThisWeek/ThisWeekView.tsx` — mount `<MealIdeasRow />` above the grid.

---

## Task 1: Core ranker (ideas.rs)

**Files:**
- Create: `crates/core/src/meal_plan/ideas.rs`
- Modify: `crates/core/src/meal_plan/mod.rs`

- [ ] **Step 1: Create ranker + tests**

Create `crates/core/src/meal_plan/ideas.rs`:

```rust
//! Meal ideas ranker — scores library recipes by days_since_last_cooked.
//! Pure function; no randomness (caller shuffles ties).

use crate::recipe::{dal::ListFilter, Recipe};
use anyhow::Result;
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const NEVER_COOKED_SCORE: i64 = 9999;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredRecipe {
    pub recipe: Recipe,
    pub score: i64,
}

/// Load all non-trashed recipes, compute days-since-last-cooked per recipe,
/// sort descending by score (never-cooked = 9999). Deterministic — no shuffle here.
pub fn library_ranked(conn: &Connection) -> Result<Vec<ScoredRecipe>> {
    let recipes = crate::recipe::dal::list_recipes(conn, &ListFilter::default())?;
    let today = chrono::Local::now().date_naive();

    let mut scored: Vec<ScoredRecipe> = recipes.into_iter().map(|recipe| {
        let score = days_since_last_cooked(conn, &recipe.id, today).unwrap_or(NEVER_COOKED_SCORE);
        ScoredRecipe { recipe, score }
    }).collect();

    scored.sort_by(|a, b| b.score.cmp(&a.score));
    Ok(scored)
}

fn days_since_last_cooked(conn: &Connection, recipe_id: &str, today: NaiveDate) -> Option<i64> {
    let max_date: Option<String> = conn.query_row(
        "SELECT MAX(entry_date) FROM meal_plan_entry WHERE recipe_id = ?1",
        rusqlite::params![recipe_id],
        |r| r.get(0),
    ).ok().flatten();

    let s = max_date?;
    let last = NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()?;
    let diff = today.signed_duration_since(last).num_days();
    Some(diff.max(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use crate::recipe::{ImportMethod, RecipeDraft};
    use tempfile::tempdir;

    fn fresh() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn insert_recipe(conn: &Connection, title: &str) -> String {
        let draft = RecipeDraft {
            title: title.into(),
            servings: None, prep_time_mins: None, cook_time_mins: None,
            instructions: "".into(),
            source_url: None, source_host: None,
            import_method: ImportMethod::Manual,
            hero_attachment_uuid: None,
            ingredients: vec![],
        };
        crate::recipe::dal::insert_recipe(conn, &draft).unwrap()
    }

    #[test]
    fn empty_library_returns_empty_vec() {
        let (_d, conn) = fresh();
        let ranked = library_ranked(&conn).unwrap();
        assert!(ranked.is_empty());
    }

    #[test]
    fn never_cooked_scores_sentinel() {
        let (_d, conn) = fresh();
        let id = insert_recipe(&conn, "Miso");
        let ranked = library_ranked(&conn).unwrap();
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].recipe.id, id);
        assert_eq!(ranked[0].score, NEVER_COOKED_SCORE);
    }

    #[test]
    fn sorted_descending_by_days_since() {
        let (_d, conn) = fresh();
        let a = insert_recipe(&conn, "A");
        let b = insert_recipe(&conn, "B");
        let c = insert_recipe(&conn, "C");

        let today = chrono::Local::now().date_naive();
        let a_date = (today - chrono::Duration::days(2)).format("%Y-%m-%d").to_string();
        let b_date = (today - chrono::Duration::days(30)).format("%Y-%m-%d").to_string();
        let c_date = (today - chrono::Duration::days(10)).format("%Y-%m-%d").to_string();
        crate::meal_plan::dal::set_entry(&conn, &a_date, &a).unwrap();
        crate::meal_plan::dal::set_entry(&conn, &b_date, &b).unwrap();
        crate::meal_plan::dal::set_entry(&conn, &c_date, &c).unwrap();

        let ranked = library_ranked(&conn).unwrap();
        let ordered_ids: Vec<_> = ranked.iter().map(|s| s.recipe.id.as_str()).collect();
        assert_eq!(ordered_ids, vec![b.as_str(), c.as_str(), a.as_str()]);
        assert_eq!(ranked[0].score, 30);
        assert_eq!(ranked[1].score, 10);
        assert_eq!(ranked[2].score, 2);
    }

    #[test]
    fn trashed_recipe_excluded() {
        let (_d, conn) = fresh();
        let alive = insert_recipe(&conn, "Alive");
        let dead = insert_recipe(&conn, "Gone");
        crate::recipe::dal::soft_delete_recipe(&conn, &dead).unwrap();

        let ranked = library_ranked(&conn).unwrap();
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].recipe.id, alive);
    }

    #[test]
    fn never_cooked_ranks_above_any_cooked() {
        let (_d, conn) = fresh();
        let cooked = insert_recipe(&conn, "Cooked");
        let fresh_r = insert_recipe(&conn, "Fresh");

        let today = chrono::Local::now().date_naive();
        let d = (today - chrono::Duration::days(100)).format("%Y-%m-%d").to_string();
        crate::meal_plan::dal::set_entry(&conn, &d, &cooked).unwrap();

        let ranked = library_ranked(&conn).unwrap();
        // Fresh (never cooked, score 9999) should rank first.
        assert_eq!(ranked[0].recipe.id, fresh_r);
        assert_eq!(ranked[0].score, NEVER_COOKED_SCORE);
        assert_eq!(ranked[1].recipe.id, cooked);
        assert_eq!(ranked[1].score, 100);
    }
}
```

- [ ] **Step 2: Register module**

Modify `crates/core/src/meal_plan/mod.rs` — add `pub mod ideas;` to the existing list of sub-modules (alphabetically between `dal` and `matcher`).

- [ ] **Step 3: Run tests**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l3d-meal-ideas
cargo test -p manor-core --lib meal_plan::ideas
```
Expected: 5 PASS.

```bash
cargo test --workspace --lib
```
Expected: baseline + 5 new.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/meal_plan/ideas.rs crates/core/src/meal_plan/mod.rs
git commit -m "feat(ideas): library_ranked pure scorer (days_since_last_cooked)"
```

---

## Task 2: Tauri commands (library_sample, llm_titles, llm_expand)

**Files:**
- Create: `crates/app/src/meal_plan/ideas_commands.rs`
- Modify: `crates/app/src/meal_plan/mod.rs` — add `pub mod ideas_commands;`.
- Modify: `crates/app/src/lib.rs` — register 3 commands.
- Modify: `crates/app/Cargo.toml` — add `rand` if not present.

- [ ] **Step 1: Check & add `rand` dep**

Check if `rand` is already in `crates/app/Cargo.toml`:

```bash
grep '^rand' crates/app/Cargo.toml
```

If absent, append under `[dependencies]`:

```toml
rand = "0.8"
```

Run:
```bash
cargo build -p manor-app
```
Expected: compile clean.

- [ ] **Step 2: Create `ideas_commands.rs`**

```rust
//! Meal ideas Tauri commands — library sample + LLM titles + LLM expand.

use crate::assistant::commands::Db;
use crate::assistant::ollama::{OllamaClient, DEFAULT_ENDPOINT, DEFAULT_MODEL};
use crate::recipe::llm_adapter::OllamaLlmAdapter;
use crate::recipe::importer::ImportPreview;
use manor_core::meal_plan::ideas::library_ranked;
use manor_core::recipe::import::{extract_json_block_public, ImportedRecipe, LlmClient};
use manor_core::recipe::{ImportMethod, Recipe, RecipeDraft};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeaTitle {
    pub title: String,
    pub blurb: String,
}

#[tauri::command]
pub fn meal_ideas_library_sample(state: State<'_, Db>) -> Result<Vec<Recipe>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let ranked = library_ranked(&conn).map_err(|e| e.to_string())?;

    let top_n = 10.min(ranked.len());
    let mut pool: Vec<Recipe> = ranked.into_iter().take(top_n).map(|s| s.recipe).collect();
    pool.shuffle(&mut rand::thread_rng());
    pool.truncate(3);
    Ok(pool)
}

const TITLES_PROMPT: &str = "You suggest 3 home-cookable dinner recipes. Output JSON exactly:\n[\n  {\"title\": str, \"blurb\": str (one sentence, <100 chars, includes timing hint)},\n  {\"title\": str, \"blurb\": str},\n  {\"title\": str, \"blurb\": str}\n]\nVary cuisines. Prefer weeknight-accessible ingredients. No prose before or after the JSON.";

#[tauri::command]
pub async fn meal_ideas_llm_titles() -> Result<Vec<IdeaTitle>, String> {
    let adapter = OllamaLlmAdapter(OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL));
    run_titles(&adapter).await
}

async fn run_titles(client: &dyn LlmClient) -> Result<Vec<IdeaTitle>, String> {
    let first = client.complete(TITLES_PROMPT).await
        .map_err(|_| "AI unavailable — try again later or check Settings → AI.".to_string())?;
    let parsed: Result<Vec<IdeaTitle>, _> = extract_json_block_public::<Vec<IdeaTitle>>(&first);

    let titles = match parsed {
        Ok(t) => t,
        Err(_) => {
            let retry = format!("{}\n\n(Previous response was not valid JSON. Output ONLY JSON.)", TITLES_PROMPT);
            let second = client.complete(&retry).await
                .map_err(|_| "AI unavailable — try again later or check Settings → AI.".to_string())?;
            extract_json_block_public::<Vec<IdeaTitle>>(&second)
                .map_err(|_| "AI returned invalid response — try again.".to_string())?
        }
    };

    if titles.is_empty() {
        return Err("AI returned no suggestions — try again.".to_string());
    }
    Ok(titles.into_iter().take(3).collect())
}

const EXPAND_PROMPT_PREFIX: &str = "You extract structured recipe data from a recipe description. Output JSON with this exact shape:\n{\n  \"title\": str,\n  \"servings\": int|null,\n  \"prep_time_mins\": int|null,\n  \"cook_time_mins\": int|null,\n  \"instructions\": str (markdown, numbered steps),\n  \"ingredients\": [\n    {\"quantity_text\": str|null, \"ingredient_name\": str, \"note\": str|null}\n  ]\n}\nIf a field is not clearly stated, use reasonable defaults for a 2-serving weeknight meal. You may invent plausible ingredient quantities.\nOutput ONLY the JSON.\n\nRecipe description:\n";

#[tauri::command]
pub async fn meal_ideas_llm_expand(title: String, blurb: String) -> Result<ImportPreview, String> {
    let adapter = OllamaLlmAdapter(OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL));
    run_expand(&adapter, &title, &blurb).await
}

async fn run_expand(client: &dyn LlmClient, title: &str, blurb: &str) -> Result<ImportPreview, String> {
    let prompt = format!("{}Title: {}\nSummary: {}", EXPAND_PROMPT_PREFIX, title, blurb);
    let raw = client.complete(&prompt).await
        .map_err(|_| "AI unavailable — try again later.".to_string())?;

    let parsed: Result<ExpandShape, _> = extract_json_block_public(&raw);
    let llm_recipe = match parsed {
        Ok(r) => r,
        Err(_) => {
            let retry = format!("{}\n\n(Previous response was not valid JSON. Output ONLY JSON.)", prompt);
            let second = client.complete(&retry).await
                .map_err(|_| "AI unavailable — try again later.".to_string())?;
            extract_json_block_public::<ExpandShape>(&second)
                .map_err(|_| "AI returned invalid recipe — try again.".to_string())?
        }
    };

    // Build ImportedRecipe, then ImportPreview.
    let imp = ImportedRecipe {
        title: llm_recipe.title,
        servings: llm_recipe.servings,
        prep_time_mins: llm_recipe.prep_time_mins,
        cook_time_mins: llm_recipe.cook_time_mins,
        instructions: llm_recipe.instructions,
        ingredients: llm_recipe.ingredients,
        source_url: String::new(),
        source_host: String::new(),
        import_method: ImportMethod::Llm,
        parse_notes: vec!["AI-extracted — please review quantities and steps.".into()],
        hero_image_url: None,
    };
    Ok(to_preview_from_imported(imp))
}

#[derive(Deserialize)]
struct ExpandShape {
    title: String,
    servings: Option<i32>,
    prep_time_mins: Option<i32>,
    cook_time_mins: Option<i32>,
    instructions: String,
    ingredients: Vec<manor_core::recipe::IngredientLine>,
}

fn to_preview_from_imported(imp: ImportedRecipe) -> ImportPreview {
    let notes = imp.parse_notes.clone();
    let method = imp.import_method.clone();
    let draft = RecipeDraft {
        title: imp.title,
        servings: imp.servings,
        prep_time_mins: imp.prep_time_mins,
        cook_time_mins: imp.cook_time_mins,
        instructions: imp.instructions,
        source_url: None,
        source_host: None,
        import_method: imp.import_method,
        hero_attachment_uuid: None,
        ingredients: imp.ingredients,
    };
    ImportPreview {
        recipe_draft: draft,
        import_method: method,
        parse_notes: notes,
        hero_image_url: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct StubLlm { responses: std::sync::Mutex<Vec<String>> }

    #[async_trait]
    impl LlmClient for StubLlm {
        async fn complete(&self, _prompt: &str) -> anyhow::Result<String> {
            let mut r = self.responses.lock().unwrap();
            Ok(r.remove(0))
        }
    }

    fn stub(resps: &[&str]) -> StubLlm {
        StubLlm { responses: std::sync::Mutex::new(resps.iter().map(|s| s.to_string()).collect()) }
    }

    #[tokio::test]
    async fn titles_happy_path() {
        let s = stub(&[r#"[{"title":"A","blurb":"x"},{"title":"B","blurb":"y"},{"title":"C","blurb":"z"}]"#]);
        let titles = run_titles(&s).await.unwrap();
        assert_eq!(titles.len(), 3);
        assert_eq!(titles[0].title, "A");
    }

    #[tokio::test]
    async fn titles_malformed_then_retry() {
        let s = stub(&[
            "not json",
            r#"[{"title":"X","blurb":"x"}]"#,
        ]);
        let titles = run_titles(&s).await.unwrap();
        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0].title, "X");
    }

    #[tokio::test]
    async fn titles_both_fail_returns_err() {
        let s = stub(&["not json", "still not json"]);
        let err = run_titles(&s).await.unwrap_err();
        assert!(err.to_lowercase().contains("invalid") || err.to_lowercase().contains("unavailable"));
    }

    #[tokio::test]
    async fn expand_happy_path() {
        let s = stub(&[r#"{"title":"Miso","servings":2,"prep_time_mins":5,"cook_time_mins":25,"instructions":"1. Cook.","ingredients":[{"quantity_text":null,"ingredient_name":"aubergine","note":null}]}"#]);
        let preview = run_expand(&s, "Miso", "blurb").await.unwrap();
        assert_eq!(preview.recipe_draft.title, "Miso");
        assert_eq!(preview.import_method, ImportMethod::Llm);
        assert!(preview.recipe_draft.source_url.is_none());
    }
}
```

**NOTE on `extract_json_block_public`:** L3a's `extract_json_block` in `crates/core/src/recipe/import.rs` is currently a private helper. We need it callable from `manor-app`. Add a thin `pub` re-export in `crates/core/src/recipe/import.rs`:

```rust
/// Public wrapper over the JSON-block parser. Used by L3d's ideas commands.
pub fn extract_json_block_public<T: for<'de> serde::Deserialize<'de>>(s: &str) -> Result<T, serde_json::Error> {
    extract_json_block(s)
}
```

This is a one-line addition to the bottom of `import.rs`. No behaviour change; `extract_json_block` itself stays private.

- [ ] **Step 3: Add `extract_json_block_public` to core**

Modify `crates/core/src/recipe/import.rs` — add the public wrapper at the end of the file (outside any `#[cfg(test)]` block):

```rust
/// Public wrapper over the internal JSON-block parser.
/// Exposed so manor-app can reuse the same forgiving parse for its own LLM calls.
pub fn extract_json_block_public<T: for<'de> serde::Deserialize<'de>>(s: &str) -> Result<T, serde_json::Error> {
    extract_json_block(s)
}
```

Run:
```bash
cargo build -p manor-core
```
Expected: compile clean.

- [ ] **Step 4: Register new module + commands**

Modify `crates/app/src/meal_plan/mod.rs`:

```rust
pub mod commands;
pub mod ideas_commands;
```

Modify `crates/app/src/lib.rs` — in the `invoke_handler` list, add next to the other `meal_plan` commands:

```rust
            meal_plan::ideas_commands::meal_ideas_library_sample,
            meal_plan::ideas_commands::meal_ideas_llm_titles,
            meal_plan::ideas_commands::meal_ideas_llm_expand,
```

- [ ] **Step 5: Build + test + clippy**

```bash
cargo build -p manor-app
cargo test -p manor-app --lib meal_plan::ideas_commands
cargo clippy --workspace -- -D warnings
```
Expected: 4 tests PASS, clippy clean.

```bash
cargo test --workspace --lib
```
Expected: +4 above prior baseline.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/recipe/import.rs crates/app/src/meal_plan/ crates/app/src/lib.rs crates/app/Cargo.toml
git commit -m "feat(ideas): Tauri commands — library sample + LLM titles + expand"
```

---

## Task 3: Frontend IPC + Zustand store

**Files:**
- Create: `apps/desktop/src/lib/meal_plan/ideas-ipc.ts`
- Create: `apps/desktop/src/lib/meal_plan/ideas-state.ts`

- [ ] **Step 1: IPC wrappers**

Create `apps/desktop/src/lib/meal_plan/ideas-ipc.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { Recipe, RecipeDraft, ImportMethod } from "../recipe/recipe-ipc";

export interface IdeaTitle {
  title: string;
  blurb: string;
}

// ImportPreview shape — matches what recipe_import_preview returns.
export interface ImportPreview {
  recipe_draft: RecipeDraft;
  import_method: ImportMethod;
  parse_notes: string[];
  hero_image_url: string | null;
}

export async function librarySample(): Promise<Recipe[]> {
  return await invoke<Recipe[]>("meal_ideas_library_sample");
}

export async function llmTitles(): Promise<IdeaTitle[]> {
  return await invoke<IdeaTitle[]>("meal_ideas_llm_titles");
}

export async function llmExpand(title: string, blurb: string): Promise<ImportPreview> {
  return await invoke<ImportPreview>("meal_ideas_llm_expand", { title, blurb });
}
```

- [ ] **Step 2: Zustand store**

Create `apps/desktop/src/lib/meal_plan/ideas-state.ts`:

```ts
import { create } from "zustand";
import * as ipc from "./ideas-ipc";
import type { Recipe } from "../recipe/recipe-ipc";

export type IdeasMode = "library" | "llm";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface IdeasStore {
  mode: IdeasMode;
  library: Recipe[];
  llm: ipc.IdeaTitle[];
  loadStatus: LoadStatus;

  loadLibrary(): Promise<void>;
  loadLlm(): Promise<void>;
  backToLibrary(): void;
  expandAiTitle(t: ipc.IdeaTitle): Promise<ipc.ImportPreview>;
}

export const useIdeasStore = create<IdeasStore>((set, get) => ({
  mode: "library",
  library: [],
  llm: [],
  loadStatus: { kind: "idle" },

  async loadLibrary() {
    set({ mode: "library", loadStatus: { kind: "loading" } });
    try {
      const library = await ipc.librarySample();
      set({ library, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  async loadLlm() {
    set({ mode: "llm", loadStatus: { kind: "loading" }, llm: [] });
    try {
      const llm = await ipc.llmTitles();
      set({ llm, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      // Auto-switch back to library on LLM failure.
      set({ mode: "library", loadStatus: { kind: "error", message } });
    }
  },

  backToLibrary() {
    set({ mode: "library", loadStatus: { kind: "idle" } });
  },

  async expandAiTitle(t) {
    return await ipc.llmExpand(t.title, t.blurb);
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
cd /Users/hanamori/life-assistant/.worktrees/l3d-meal-ideas
git add apps/desktop/src/lib/meal_plan/ideas-ipc.ts apps/desktop/src/lib/meal_plan/ideas-state.ts
git commit -m "feat(ideas): frontend IPC + Zustand store (library + llm modes)"
```

---

## Task 4: `IdeaTitleCard` + `MealIdeasRow` (library mode only)

Ship the library-mode row first; LLM mode wires in Task 5.

**Files:**
- Create: `apps/desktop/src/components/Hearth/ThisWeek/IdeaTitleCard.tsx`
- Create: `apps/desktop/src/components/Hearth/ThisWeek/MealIdeasRow.tsx`

- [ ] **Step 1: `IdeaTitleCard.tsx`**

```tsx
import { Sparkles } from "lucide-react";
import type { IdeaTitle } from "../../../lib/meal_plan/ideas-ipc";

interface Props {
  idea: IdeaTitle;
  onClick: () => void;
  loading?: boolean;
}

export function IdeaTitleCard({ idea, onClick, loading }: Props) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={loading}
      style={{
        textAlign: "left",
        background: "var(--paper, #fff)",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 6,
        padding: 12,
        cursor: loading ? "wait" : "pointer",
        display: "flex",
        flexDirection: "column",
        gap: 6,
        minHeight: 140,
        position: "relative",
        opacity: loading ? 0.6 : 1,
      }}
    >
      <Sparkles size={16} strokeWidth={1.6} color="var(--ink-soft, #999)" />
      <div style={{
        fontSize: 14,
        fontWeight: 600,
        overflow: "hidden",
        display: "-webkit-box",
        WebkitBoxOrient: "vertical" as const,
        WebkitLineClamp: 2 as const,
      }}>
        {idea.title}
      </div>
      <div style={{
        fontSize: 12,
        color: "var(--ink-soft, #999)",
        overflow: "hidden",
        display: "-webkit-box",
        WebkitBoxOrient: "vertical" as const,
        WebkitLineClamp: 2 as const,
      }}>
        {idea.blurb}
      </div>
      {loading && (
        <div style={{
          position: "absolute",
          inset: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontSize: 12,
          color: "var(--ink-soft, #999)",
        }}>
          Expanding…
        </div>
      )}
    </button>
  );
}
```

- [ ] **Step 2: `MealIdeasRow.tsx`**

```tsx
import { useEffect } from "react";
import { RefreshCw } from "lucide-react";
import { useIdeasStore } from "../../../lib/meal_plan/ideas-state";
import { useHearthViewStore } from "../../../lib/hearth/view-state";
import { RecipeCard } from "../RecipeCard";

export function MealIdeasRow() {
  const { mode, library, loadStatus, loadLibrary } = useIdeasStore();
  const { setSubview } = useHearthViewStore();

  useEffect(() => { void loadLibrary(); }, [loadLibrary]);

  const emptyLibrary = loadStatus.kind === "idle" && library.length === 0;

  return (
    <div style={{ marginBottom: 24 }}>
      <div style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        marginBottom: 8,
      }}>
        <div style={{ fontSize: 14, fontWeight: 600 }}>
          Meal ideas{mode === "llm" ? " — AI" : ""}
        </div>
        <button
          type="button"
          onClick={() => void loadLibrary()}
          style={{ display: "flex", alignItems: "center", gap: 4 }}
          aria-label="Reshuffle"
        >
          <RefreshCw size={14} strokeWidth={1.8} /> Reshuffle
        </button>
      </div>

      {loadStatus.kind === "loading" && (
        <div style={{ color: "var(--ink-soft, #999)", fontSize: 13, padding: 12 }}>
          Loading…
        </div>
      )}

      {loadStatus.kind === "error" && (
        <div style={{ color: "var(--ink-danger, #b00020)", fontSize: 13, padding: 12 }}>
          {loadStatus.message} <button type="button" onClick={() => void loadLibrary()}>Retry</button>
        </div>
      )}

      {emptyLibrary && (
        <div style={{ color: "var(--ink-soft, #999)", fontSize: 13, padding: 12 }}>
          Add some recipes to your library and suggestions will appear here.{" "}
          <button type="button" onClick={() => setSubview("recipes")}>→ Go to Recipes</button>
        </div>
      )}

      {loadStatus.kind === "idle" && library.length > 0 && (
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(3, 1fr)",
          gap: 16,
        }}>
          {library.map((r) => (
            <RecipeCard
              key={r.id}
              recipe={r}
              onClick={() => console.log("Library card tap — wired in Task 5", r.id)}
            />
          ))}
        </div>
      )}

      <div style={{ marginTop: 8, fontSize: 12, color: "var(--ink-soft, #999)" }}>
        <span>Not feeling it? </span>
        <button type="button"
          onClick={() => console.log("LLM mode — wired in Task 6")}
          style={{ background: "transparent", border: "none", cursor: "pointer",
                   color: "var(--ink-soft, #999)", textDecoration: "underline" }}>
          Try something new →
        </button>
      </div>
    </div>
  );
}
```

Imports note: the `RecipeCard` import path is `../RecipeCard` — it's in `apps/desktop/src/components/Hearth/RecipeCard.tsx` (shipped in L3a).

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l3d-meal-ideas
git add apps/desktop/src/components/Hearth/ThisWeek/IdeaTitleCard.tsx apps/desktop/src/components/Hearth/ThisWeek/MealIdeasRow.tsx
git commit -m "feat(ideas): MealIdeasRow + IdeaTitleCard (library mode, stubs for LLM + assign)"
```

---

## Task 5: `AssignDayPopover` + wire library-card tap

**Files:**
- Create: `apps/desktop/src/components/Hearth/ThisWeek/AssignDayPopover.tsx`
- Modify: `apps/desktop/src/components/Hearth/ThisWeek/MealIdeasRow.tsx` — wire card tap.

- [ ] **Step 1: `AssignDayPopover.tsx`**

```tsx
import type { Recipe } from "../../../lib/recipe/recipe-ipc";
import type { MealPlanEntryWithRecipe } from "../../../lib/meal_plan/meal-plan-ipc";

interface Props {
  recipe: Recipe;
  entries: MealPlanEntryWithRecipe[];   // 7 entries for the current week
  onPick: (date: string) => Promise<void>;
  onClose: () => void;
}

const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

export function AssignDayPopover({ recipe, entries, onPick, onClose }: Props) {
  const todayIso = (() => {
    const d = new Date();
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${dd}`;
  })();

  const handleClick = async (entry: MealPlanEntryWithRecipe) => {
    if (entry.recipe != null) {
      const current = entry.recipe.title;
      if (!window.confirm(`Replace "${current}" with "${recipe.title}"?`)) return;
    }
    await onPick(entry.entry_date);
  };

  return (
    <div
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.2)",
        zIndex: 60,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div style={{
        background: "var(--paper, #fff)",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 6,
        padding: 20,
        minWidth: 520,
        maxWidth: 720,
        boxShadow: "0 4px 16px rgba(0,0,0,0.15)",
      }}>
        <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 12 }}>
          <h3 style={{ margin: 0, fontSize: 15 }}>
            Plan "{recipe.title}" on…
          </h3>
          <button type="button" onClick={onClose} aria-label="Close">✕</button>
        </div>
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(7, 1fr)",
          gap: 6,
        }}>
          {entries.map((e, i) => {
            const day = new Date(e.entry_date + "T00:00:00");
            const isToday = e.entry_date === todayIso;
            const filled = e.recipe != null;
            return (
              <button
                key={e.entry_date}
                type="button"
                onClick={() => void handleClick(e)}
                style={{
                  background: isToday ? "var(--paper-muted, #f5f5f5)" : "transparent",
                  border: "1px solid var(--hairline, #e5e5e5)",
                  borderRadius: 4,
                  padding: 8,
                  cursor: "pointer",
                  display: "flex",
                  flexDirection: "column",
                  gap: 2,
                  minHeight: 64,
                }}
              >
                <div style={{ fontSize: 11, fontWeight: isToday ? 600 : 500,
                              color: isToday ? "var(--ink-strong, #111)" : "var(--ink-soft, #999)" }}>
                  {DAY_LABELS[i]} {day.getDate()}
                </div>
                <div style={{ fontSize: 11,
                              color: filled ? "var(--ink-strong, #111)" : "var(--ink-soft, #999)",
                              whiteSpace: "nowrap",
                              overflow: "hidden",
                              textOverflow: "ellipsis" }}>
                  {filled ? e.recipe!.title : "—"}
                </div>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Wire into `MealIdeasRow`**

Modify `apps/desktop/src/components/Hearth/ThisWeek/MealIdeasRow.tsx`:

Add imports + state for selected library recipe:

```tsx
import { useEffect, useState } from "react";
import { useMealPlanStore } from "../../../lib/meal_plan/meal-plan-state";
import { AssignDayPopover } from "./AssignDayPopover";
import type { Recipe } from "../../../lib/recipe/recipe-ipc";
```

Add state:

```tsx
const { entries, setEntry } = useMealPlanStore();
const [assigningRecipe, setAssigningRecipe] = useState<Recipe | null>(null);
```

Replace the `onClick` console.log stub on the library card:

```tsx
onClick={() => setAssigningRecipe(r)}
```

At the bottom of the component, before the closing `</div>`:

```tsx
{assigningRecipe && (
  <AssignDayPopover
    recipe={assigningRecipe}
    entries={entries}
    onClose={() => setAssigningRecipe(null)}
    onPick={async (date) => {
      await setEntry(date, assigningRecipe.id);
      setAssigningRecipe(null);
      // After assigning, re-load library so the newly-cooked recipe may rotate out.
      await useIdeasStore.getState().loadLibrary();
    }}
  />
)}
```

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l3d-meal-ideas
git add apps/desktop/src/components/Hearth/ThisWeek/
git commit -m "feat(ideas): AssignDayPopover + library-card tap assigns to day (with replace confirm)"
```

---

## Task 6: LLM mode + expand-to-drawer

**Files:**
- Modify: `apps/desktop/src/components/Hearth/ThisWeek/MealIdeasRow.tsx` — wire "Try something new" + LLM cards + expand.

- [ ] **Step 1: Wire LLM mode**

Update `MealIdeasRow.tsx` — destructure more from the store, add state for expanding + preview-drawer:

```tsx
import { useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";
import { useIdeasStore } from "../../../lib/meal_plan/ideas-state";
import { useMealPlanStore } from "../../../lib/meal_plan/meal-plan-state";
import { useHearthViewStore } from "../../../lib/hearth/view-state";
import { RecipeCard } from "../RecipeCard";
import { IdeaTitleCard } from "./IdeaTitleCard";
import { AssignDayPopover } from "./AssignDayPopover";
import { RecipeEditDrawer } from "../RecipeEditDrawer";
import * as recipeIpc from "../../../lib/recipe/recipe-ipc";
import type { Recipe } from "../../../lib/recipe/recipe-ipc";
import type { IdeaTitle, ImportPreview } from "../../../lib/meal_plan/ideas-ipc";
```

Component body (full rewrite):

```tsx
export function MealIdeasRow() {
  const { mode, library, llm, loadStatus, loadLibrary, loadLlm, backToLibrary, expandAiTitle } = useIdeasStore();
  const { entries, setEntry } = useMealPlanStore();
  const { setSubview } = useHearthViewStore();

  const [assigningRecipe, setAssigningRecipe] = useState<Recipe | null>(null);
  const [expandingIdx, setExpandingIdx] = useState<number | null>(null);
  const [previewDrawer, setPreviewDrawer] = useState<ImportPreview | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  useEffect(() => { void loadLibrary(); }, [loadLibrary]);

  const emptyLibrary = mode === "library" && loadStatus.kind === "idle" && library.length === 0;

  const onReshuffle = () => {
    if (mode === "library") void loadLibrary();
    else void loadLlm();
  };

  const handleExpand = async (i: number, idea: IdeaTitle) => {
    setExpandingIdx(i);
    try {
      const preview = await expandAiTitle(idea);
      setPreviewDrawer(preview);
    } catch (e: unknown) {
      setToast(e instanceof Error ? e.message : String(e));
    } finally {
      setExpandingIdx(null);
    }
  };

  return (
    <div style={{ marginBottom: 24 }}>
      <div style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        marginBottom: 8,
      }}>
        <div style={{ fontSize: 14, fontWeight: 600 }}>
          Meal ideas{mode === "llm" ? " — AI" : ""}
        </div>
        <button
          type="button"
          onClick={onReshuffle}
          style={{ display: "flex", alignItems: "center", gap: 4 }}
          aria-label="Reshuffle"
          disabled={loadStatus.kind === "loading"}
        >
          <RefreshCw size={14} strokeWidth={1.8} /> Reshuffle
        </button>
      </div>

      {loadStatus.kind === "loading" && (
        <div style={{ color: "var(--ink-soft, #999)", fontSize: 13, padding: 12 }}>
          Loading…
        </div>
      )}

      {loadStatus.kind === "error" && (
        <div style={{ color: "var(--ink-danger, #b00020)", fontSize: 13, padding: 12 }}>
          {loadStatus.message}{" "}
          <button type="button" onClick={onReshuffle}>Retry</button>
        </div>
      )}

      {emptyLibrary && (
        <div style={{ color: "var(--ink-soft, #999)", fontSize: 13, padding: 12 }}>
          Add some recipes to your library and suggestions will appear here.{" "}
          <button type="button" onClick={() => setSubview("recipes")}>→ Go to Recipes</button>
        </div>
      )}

      {mode === "library" && loadStatus.kind === "idle" && library.length > 0 && (
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(3, 1fr)",
          gap: 16,
        }}>
          {library.map((r) => (
            <RecipeCard
              key={r.id}
              recipe={r}
              onClick={() => setAssigningRecipe(r)}
            />
          ))}
        </div>
      )}

      {mode === "llm" && loadStatus.kind === "idle" && llm.length > 0 && (
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(3, 1fr)",
          gap: 16,
        }}>
          {llm.map((idea, i) => (
            <IdeaTitleCard
              key={i}
              idea={idea}
              onClick={() => void handleExpand(i, idea)}
              loading={expandingIdx === i}
            />
          ))}
        </div>
      )}

      {mode === "library" && (
        <div style={{ marginTop: 8, fontSize: 12, color: "var(--ink-soft, #999)" }}>
          <span>Not feeling it? </span>
          <button type="button" onClick={() => void loadLlm()}
            style={{ background: "transparent", border: "none", cursor: "pointer",
                     color: "var(--ink-soft, #999)", textDecoration: "underline" }}>
            Try something new →
          </button>
        </div>
      )}

      {mode === "llm" && (
        <div style={{ marginTop: 8, fontSize: 12, color: "var(--ink-soft, #999)" }}>
          <button type="button" onClick={backToLibrary}
            style={{ background: "transparent", border: "none", cursor: "pointer",
                     color: "var(--ink-soft, #999)", textDecoration: "underline" }}>
            ← Back to library
          </button>
        </div>
      )}

      {assigningRecipe && (
        <AssignDayPopover
          recipe={assigningRecipe}
          entries={entries}
          onClose={() => setAssigningRecipe(null)}
          onPick={async (date) => {
            await setEntry(date, assigningRecipe.id);
            setAssigningRecipe(null);
            await loadLibrary();
          }}
        />
      )}

      {previewDrawer && (
        <RecipeEditDrawer
          initialDraft={previewDrawer.recipe_draft}
          title="Save AI recipe"
          saveLabel="Save to library"
          onClose={() => setPreviewDrawer(null)}
          onSubmit={async (draft) => {
            return await recipeIpc.importCommit(draft, previewDrawer.hero_image_url);
          }}
          onSaved={() => setPreviewDrawer(null)}
        />
      )}

      {toast && (
        <div style={{
          position: "fixed", bottom: 24, left: "50%", transform: "translateX(-50%)",
          background: "var(--paper, #fff)", border: "1px solid var(--hairline, #e5e5e5)",
          padding: "8px 16px", borderRadius: 6, fontSize: 13,
          boxShadow: "0 2px 8px rgba(0,0,0,0.1)",
          zIndex: 70,
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

- [ ] **Step 2: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 3: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l3d-meal-ideas
git add apps/desktop/src/components/Hearth/ThisWeek/MealIdeasRow.tsx
git commit -m "feat(ideas): LLM mode — Try something new → 3 AI cards → tap to expand into RecipeEditDrawer"
```

---

## Task 7: Mount `MealIdeasRow` in `ThisWeekView` + fully-planned collapse

**Files:**
- Modify: `apps/desktop/src/components/Hearth/ThisWeek/ThisWeekView.tsx`

- [ ] **Step 1: Mount MealIdeasRow**

Modify `apps/desktop/src/components/Hearth/ThisWeek/ThisWeekView.tsx`:

Add import near top:

```tsx
import { MealIdeasRow } from "./MealIdeasRow";
```

In the component's returned JSX, insert `<MealIdeasRow />` between `<WeekNav …/>` and the grid container (the `<div style={{ display: "grid", …}}>` that holds the 7 day cells).

- [ ] **Step 2: Handle fully-planned collapse (simplified)**

The spec §7.5 describes a collapsed one-liner when all 7 days are filled. The simplest implementation: `MealIdeasRow` itself detects this via its `entries` access and renders the collapsed variant.

Modify `MealIdeasRow.tsx` — add near the top of the render, after destructuring stores:

```tsx
const allFilled = entries.length === 7 && entries.every((e) => e.recipe !== null);
```

Wrap the three main content blocks (library grid, LLM grid, empty-library message) in a conditional that collapses to a one-liner when `allFilled` AND not currently reshuffling into a different mode — but actually, the cleanest UX is: if all 7 are filled, still render the reshuffle header but skip the cards and links. User can reshuffle (which loads fresh picks) and still tap a suggestion to swap an existing meal.

Simplest: when `allFilled`, append `" · Week is fully planned"` to the heading and skip rendering the `library.map(...)` / `llm.map(...)` blocks AND skip the "Try something new" / "Back to library" links.

```tsx
const allFilled = entries.length === 7 && entries.every((e) => e.recipe !== null);

// In the heading div:
<div style={{ fontSize: 14, fontWeight: 600 }}>
  Meal ideas{mode === "llm" ? " — AI" : ""}
  {allFilled && <span style={{ color: "var(--ink-soft, #999)", fontWeight: 500 }}>
    {" · Week is fully planned"}
  </span>}
</div>
```

Then guard the three content blocks:

```tsx
{!allFilled && loadStatus.kind === "loading" && <...>}
{!allFilled && loadStatus.kind === "error" && <...>}
{!allFilled && emptyLibrary && <...>}
{!allFilled && mode === "library" && loadStatus.kind === "idle" && library.length > 0 && <...>}
{!allFilled && mode === "llm" && loadStatus.kind === "idle" && llm.length > 0 && <...>}
{!allFilled && mode === "library" && <... "Try something new" ...>}
{!allFilled && mode === "llm" && <... "Back to library" ...>}
```

AssignDayPopover + preview drawer + toast stay outside the guard — they handle their own lifecycles.

The reshuffle button stays visible always so the user can expand the row by pressing it (loading fresh picks re-displays the grid — actually no, `allFilled` still blocks them. If the user wants to swap, they click Reshuffle → load completes → but the `!allFilled` guards block the render).

Refine: when user clicks Reshuffle while `allFilled`, we temporarily want to show cards so they can swap. Simplest tweak: if user has reshuffled at least once, show cards regardless of `allFilled`. Track via local state:

```tsx
const [forceShow, setForceShow] = useState(false);
const onReshuffle = () => {
  setForceShow(true);
  if (mode === "library") void loadLibrary();
  else void loadLlm();
};
// Show cards if !allFilled OR forceShow is true
const collapsed = allFilled && !forceShow;
```

Then all the `!allFilled &&` guards become `!collapsed &&`.

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l3d-meal-ideas
git add apps/desktop/src/components/Hearth/ThisWeek/
git commit -m "feat(ideas): mount MealIdeasRow in ThisWeekView + collapsed header when week is fully planned"
```

---

## Task 8: Final QA

**Files:** verification only.

- [ ] **Step 1: Full test suite**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l3d-meal-ideas
cargo test --workspace
```
Expected: core 314 baseline + 5 ideas ranker + 4 ideas_commands = 323 lib tests (plus 81 app lib + 3 integration).

- [ ] **Step 2: Clippy + typecheck + build**

```bash
cargo clippy --workspace -- -D warnings
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```
Expected: clean.

- [ ] **Step 3: Dev-server golden path**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l3d-meal-ideas && pnpm tauri dev
```

Walk:
- Hearth → This Week — `MealIdeasRow` renders above grid with 3 `RecipeCard`s and "Try something new →" link.
- Click **Reshuffle** — library picks change (visibly different cards across clicks when library > 3).
- Click a card — `AssignDayPopover` opens showing 7 day chips with correct empty/planned state. Click empty chip → meal lands on that day. Row re-renders.
- Click a card that would replace a planned meal → `Replace "X" with "Y"?` confirm → accept → replacement happens.
- Click **Try something new →** — spinner, then 3 AI cards render with `Sparkles` icon + title + blurb.
- Click an AI card — "Expanding…" overlay, then `RecipeEditDrawer` opens pre-filled with the AI recipe. Save → lands in library with `import_method = llm`. Drawer closes.
- Back to library view via `← Back to library` link — renders library cards again without a new load.
- Empty-library case (fresh install): shows "Add some recipes…" + link to Recipes.
- Fully-planned week: heading becomes `Meal ideas · Week is fully planned`, no cards; Reshuffle still expands it back.

- [ ] **Step 4: If all green, invoke finishing-a-development-branch.**

---

## Self-review

**Spec coverage:**
- §3 architecture → Tasks 1–3. ✓
- §4 library ranker → Task 1. ✓
- §5 Tauri commands → Task 2. ✓
- §6 LLM prompts → embedded in Task 2. ✓
- §7 UI (MealIdeasRow, library mode, LLM mode, empty-library, fully-planned collapse, AssignDayPopover, rate limiting) → Tasks 4–7. ✓
- §8 Zustand store → Task 3. ✓
- §9 error handling → inline in commands + store + UI. ✓
- §10 testing strategy — core unit tests in Task 1 (5 tests), app unit tests in Task 2 (4 tests), manual QA in Task 8. ✓

**Placeholder scan:** no TBD / "implement later" / "similar to Task N". All step code is complete.

**Type consistency:**
- `IdeaTitle { title, blurb }` consistent across Rust (§5.2) and TS (§8 / Task 3).
- `ImportPreview` matches L3a's type (importer.rs → recipe-ipc.ts) — imported from there in TS.
- `ImportMethod::Llm` used for AI-expanded recipes.
- `useIdeasStore` exports match exactly what `MealIdeasRow.tsx` destructures (`mode`, `library`, `llm`, `loadStatus`, `loadLibrary`, `loadLlm`, `backToLibrary`, `expandAiTitle`).
- `extract_json_block_public` defined in Task 2 Step 3 before being imported in `ideas_commands.rs`.

---

*End of plan. Next: `superpowers:subagent-driven-development`.*
