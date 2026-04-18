# L3a Recipe Library — Design Spec

- **Date**: 2026-04-18
- **Landmark**: v0.4 Hearth → L3a (first sub-landmark)
- **Status**: Approved design. Implementation plan next.
- **Authors**: Hana (product), Nell (architect)
- **Roadmap**: `specs/2026-04-18-v0.4-hearth-roadmap.md`

## 1. Purpose

Ship Manor's first step into meal planning: a standalone recipe library you can browse, create, edit, and populate quickly by pasting URLs from recipe sites. L3a ships alone — useful as "a place for my recipes" without any meal plan, shopping list, or LLM reshuffle yet.

This spec covers L3a only. Meal plan (L3b), shopping list (L3c), and meal ideas reshuffle (L3d) have their own specs.

## 2. Scope decisions (locked during brainstorm)

| Area | Decision |
|---|---|
| **Recipe data model** | Structured-lite — title, servings, prep_time, cook_time, instructions (markdown), ingredients as structured rows (quantity_text, ingredient_name, note), source URL, hero image. No nutrition, no unit normalisation, no difficulty. |
| **URL import** | Hybrid: schema.org JSON-LD first, LLM fallback. Parse-path badge in UI. |
| **Import UX** | Preview-first — fetch + parse, show editable preview, user hits Save to persist. Nothing lands in the library until confirmed. |
| **LLM fallback routing** | Local Ollama first; silent fall-through to remote tier if Ollama unreachable (respecting existing Landmark 2 tier routing + budget caps). |
| **Images** | One hero image per recipe. Reuses shared `attachment` infrastructure. Auto-grabbed from `og:image` / schema.org on import; user-replaceable. |
| **Navigation** | New top-level `Hearth` tab introduced now, Lucide `UtensilsCrossed` icon. L3a populates it with only the recipe library view; L3b–L3d add sub-views later. |
| **Search/filter MVP** | Title substring search + tag filter only. No prep-time or cook-time filter. |
| **Deletion** | Reuses existing trash pattern — `deleted_at` timestamp, 30-day auto-empty cascades to ingredients + unlinks attachments. |

## 3. Architecture

**Two-crate split (follows existing Manor pattern):**

- **`crates/core/src/recipe/`** — pure data layer.
  - `mod.rs` — module root + `Recipe` / `RecipeIngredient` / `RecipeDraft` types.
  - `dal.rs` — CRUD functions against SQLite (list, get, create, update, soft-delete, restore).
  - `import/mod.rs` — import orchestrator: `preview(url) -> ImportPreview`.
  - `import/fetch.rs` — HTTP fetch with size/timeout/MIME guards.
  - `import/jsonld.rs` — schema.org Recipe parser.
  - `import/llm.rs` — LLM-extraction fallback, structured-output prompt, validation.
  - `import/image.rs` — hero image download + attachment staging.
  - No Tauri bindings. Testable with `cargo test` against in-memory SQLite + wiremock.

- **`crates/app/src/recipe/`** — thin Tauri command layer.
  - `mod.rs` — IPC handler registration.
  - `commands.rs` — `recipe_list`, `recipe_get`, `recipe_create`, `recipe_update`, `recipe_delete`, `recipe_restore`, `recipe_import_preview`, `recipe_import_commit`, `recipe_import_cancel`.
  - Logic lives in `manor-core`; this crate is glue only.

**Reused infrastructure (no changes needed):**
- `attachment` table + storage for hero images.
- `tag` + `tag_link` for recipe tagging.
- `trash`-pattern `deleted_at` + the shipped 30-day sweeper (needs `recipe` added to its table registry).
- `OllamaClient` and `RemoteLlmClient` for LLM fallback — tier routing, budget caps, redaction pipeline, `remote_call_log` audit all inherited from Landmark 2.

## 4. Schema (migration V14)

```sql
CREATE TABLE recipe (
  id             TEXT PRIMARY KEY,
  title          TEXT NOT NULL,
  servings       INTEGER,
  prep_time_mins INTEGER,
  cook_time_mins INTEGER,
  instructions   TEXT NOT NULL,      -- markdown
  source_url     TEXT,
  source_host    TEXT,                -- for badge UX ("from bbcgoodfood.com")
  import_method  TEXT,                -- 'manual' | 'jsonld' | 'llm' | 'llm_remote'
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL,
  deleted_at     INTEGER               -- trash support; NULL = not trashed
);
CREATE INDEX idx_recipe_deleted ON recipe(deleted_at);
CREATE INDEX idx_recipe_title   ON recipe(title COLLATE NOCASE);

CREATE TABLE recipe_ingredient (
  id              TEXT PRIMARY KEY,
  recipe_id       TEXT NOT NULL REFERENCES recipe(id) ON DELETE CASCADE,
  position        INTEGER NOT NULL,   -- ordering within recipe
  quantity_text   TEXT,                -- "2", "1 tbsp", "a pinch"
  ingredient_name TEXT NOT NULL,       -- "onions", "garlic clove"
  note            TEXT                 -- "finely diced", "optional"
);
CREATE INDEX idx_ri_recipe ON recipe_ingredient(recipe_id, position);
```

Trash sweeper registration update: add `recipe` table name to the existing sweeper registry so `deleted_at > now - 30d` rows purge automatically. Cascade via FK deletes ingredient rows; a post-purge hook soft-deletes attachments where `entity_type='recipe'` and `entity_id` matches the purged row (same attachment sweep pattern used by existing trash hooks).

## 5. URL import pipeline

```
User pastes URL → clicks Import
    │
    ▼
[1] Tauri: recipe_import_preview(url)
    │
    ▼
[2] crates/core::recipe::import::fetch::fetch_html(url)
    • reqwest GET, 10s timeout, UA "Manor/0.4 (+https://manor.app)"
    • Accept: text/html only → else ImportError::NotHtml
    • Max body 2 MB hard cap → else ImportError::TooLarge
    │
    ▼
[3] Parse attempt 1 — JSON-LD (crates/core::recipe::import::jsonld)
    • scraper crate → all <script type="application/ld+json"> blocks
    • serde_json::Value walk: find @type "Recipe" (or @graph entry)
    • Map to ImportedRecipe → import_method = 'jsonld'
    • Return on success, skip to [5]
    │
    ▼
[4] Parse attempt 2 — LLM fallback (crates/core::recipe::import::llm)
    • HTML → readable text via readability (or simple tag-strip)
    • Truncate to 4096 chars
    • Try local Ollama first (existing OllamaClient::complete with structured prompt)
    •   On timeout (>30s) OR connection refused → fall through to RemoteLlmClient
    •   On remote budget exceeded / rate-limited / tier disabled → ImportError::ExtractionFailed
    • Parse JSON response; if invalid, retry once with reminder prompt; then ExtractionFailed
    • import_method = 'llm' (local) or 'llm_remote' (remote tier)
    │
    ▼
[5] Hero image — recipe::import::image::fetch_hero()
    • Try og:image meta, then schema.org recipe.image, then first <img> in main content
    • Download (10s timeout, 5 MB cap, jpg/png/webp only) to attachments/<uuid>.<ext>
    • Create attachment row with entity_type='recipe' but entity_id=NULL (staged)
    • On failure: continue without image, add parse_note
    │
    ▼
[6] Return ImportPreview { recipe_draft, image_attachment_id?, import_method, parse_notes[] }
    • Frontend renders preview drawer, user edits, clicks Save
    │
    ▼
[7] Tauri: recipe_import_commit(edited_recipe, image_attachment_id?)
    • Transaction: insert recipe + ingredient rows + tag_links
    • If image_attachment_id provided: UPDATE attachment SET entity_id = recipe.id
    • Return recipe_id to frontend; it navigates to detail view
```

**Cancel path:** `recipe_import_cancel(image_attachment_id)` — unlinks and deletes any staged attachment so no orphan files. Called by the drawer's unmount/cancel handler.

### 5.1 `ImportedRecipe` shape

```rust
pub struct ImportedRecipe {
    pub title: String,
    pub servings: Option<i32>,
    pub prep_time_mins: Option<i32>,
    pub cook_time_mins: Option<i32>,
    pub instructions: String,              // markdown, numbered steps
    pub ingredients: Vec<IngredientLine>,
    pub source_url: String,
    pub source_host: String,
    pub import_method: ImportMethod,
    pub parse_notes: Vec<String>,          // user-visible warnings
    pub hero_image_url: Option<String>,
}

pub struct IngredientLine {
    pub quantity_text: Option<String>,
    pub ingredient_name: String,
    pub note: Option<String>,
}

pub enum ImportMethod { Manual, JsonLd, Llm, LlmRemote }
```

### 5.2 LLM extraction prompt

```
You extract structured recipe data from webpage text. Output JSON with this exact shape:
{
  "title": str,
  "servings": int|null,
  "prep_time_mins": int|null,
  "cook_time_mins": int|null,
  "instructions": str (markdown, numbered steps),
  "ingredients": [
    {"quantity_text": str|null, "ingredient_name": str, "note": str|null}
  ]
}
If a field is not clearly stated, use null. Do not fabricate quantities.
Output ONLY the JSON, no prose before or after.

Webpage content:
<truncated readable text>
```

The `RemoteLlmClient` call tags the intent as `recipe_import` so budget attribution + the `remote_call_log` row categorise correctly.

## 6. UI structure

### 6.1 Navigation

- New top-level tab **Hearth** inserted between `Ledger` and `Assistant`.
- Icon: Lucide `UtensilsCrossed`, stroke 1.8, size 22 (per Flat-Notion design system).
- Active-state styling reuses existing `NavItem` component — no new chrome.

### 6.2 Hearth landing (L3a = only view)

Recipe library list.

- **Header:** title "Recipes" (left) + action buttons (right): `+ New` (opens empty edit drawer) and `↓ Import URL` (opens import drawer).
- **Filter bar:** title-search input (debounced 200ms, SQL `LIKE '%q%'` with `COLLATE NOCASE`) + tag filter dropdown (multi-select, OR-match).
- **Grid:** responsive 4-column on desktop, 2 on narrow — `grid-template-columns: repeat(auto-fit, minmax(240px, 1fr))`.
- **Card:**
  - Hero image (4:3 aspect, flat placeholder if none — Lucide `ImageOff` centred on a `--paper-muted` background).
  - Title (1-line clamp, `--text-base`, `--weight-semibold`).
  - Meta strip: `{prep+cook}m · {servings}p` (null-safe — skip parts that are null).
  - Tag chips (max 2 visible + `+N` overflow).
- **Empty state:** centred message "Your recipe collection is empty." + two buttons `+ New recipe` / `↓ Import from URL`.

### 6.3 Recipe detail view

- **Top bar:** `← Back`, `Edit`, `Delete` actions.
- **Hero image:** 16:9 max-height block at top of content, flat placeholder if none.
- **Title:** `--text-2xl --weight-semibold`.
- **Meta line:** `{prep}m prep · {cook}m cook · serves {n}` (null-safe).
- **Tags:** row of chips below meta line.
- **Source badge:** small monochrome line "Source: {host}" + import-method badge ("Parsed from structured data" / "AI-extracted — please review") when non-manual.
- **Ingredients section:** `<ul>` with square bullet markers (not checkboxes — tick state lives on shopping list in L3c, not recipes).
- **Instructions section:** rendered markdown (reuse existing `Markdown` component from Today/notes).

### 6.4 Edit / New drawer

Right-side slide-in drawer (same pattern as `ConnectBankDrawer`). Fields:

- Title (required)
- Servings / Prep min / Cook min (optional integers)
- Hero image: current thumbnail + `Replace` / `Remove` buttons (opens native file picker or accepts drag-drop)
- Ingredients: sortable list of rows `[quantity_text] [ingredient_name] [note] [×]`, `+ Add ingredient` footer
- Instructions: markdown textarea with tab-indent preserved
- Tags: autocomplete chip input backed by existing `tag_link` autocomplete
- Source URL (optional freeform field — prefilled if edit came from import)
- Buttons: `Cancel` / `Save`

### 6.5 Import drawer

Right-side slide-in drawer. Two stages within one drawer:

**Stage 1 — URL input:**
- Input: URL field + Fetch button.
- Below: empty preview pane.

**Stage 2 — preview (after Fetch):**
- Import-method badge near the top ("Parsed from structured data" / "AI-extracted — please review" / "Partial data — please check everything").
- All fields editable — same layout as the edit drawer (6.4).
- `parse_notes[]` rendered as small warning list above the title.
- Buttons: `Cancel` (calls `recipe_import_cancel` to clean up staged attachment) / `Save to library` (calls `recipe_import_commit`).

Mid-fetch: Fetch button shows spinner, URL field becomes read-only until response or cancel.

## 7. Error handling

| Error | User sees | Recovery |
|---|---|---|
| URL malformed | "That doesn't look like a web address." | Edit URL, retry |
| Fetch timeout (>10s) | "Couldn't reach that URL." | Retry button |
| Non-HTML content | "That URL isn't a web page." | Manual-entry form with URL prefilled |
| Body > 2 MB | "Page too large to import." | Manual entry |
| JSON-LD missing + Ollama down + remote unavailable | "Couldn't extract this recipe." | Manual-entry form; URL prefilled in Source URL field |
| LLM returned invalid JSON (after 1 retry) | "Couldn't extract this recipe." | Manual entry |
| Image download fails | Recipe saves without image, soft warning in parse_notes | User adds image later via edit |
| DB write fails on commit | Drawer stays open, inline error, transaction rolled back | User retries Save |
| Attachment stage leak (app crash before commit) | Orphaned file in `attachments/` dir | Explicit Cancel cleans up via `recipe_import_cancel`. For crash-orphans: a startup sweep (added in L3a) removes `attachment` rows where `entity_type='recipe'` AND `entity_id IS NULL` AND `created_at < now - 24h`, then deletes the corresponding files. |

## 8. Testing strategy

### 8.1 Unit (crates/core)

- `import::jsonld::parse()` — golden tests against 8 site fixtures (HTML files in `crates/core/tests/fixtures/recipe/`): BBC Good Food, NYT Cooking, Serious Eats, Allrecipes, Bon Appétit, Delicious, Jamie Oliver, Ottolenghi. Fixtures stripped to essential structure (~5 KB each).
- `import::jsonld` edge cases: `@graph` recipe, `Recipe` nested inside `WebPage`, malformed JSON, multiple `Recipe` blocks (take first), missing required fields (title).
- `import::llm::extract()` — tests with a stubbed `LlmClient` trait covering: valid JSON response, malformed JSON (retried once, then errors), null-tolerant optional fields, non-numeric quantity strings preserved, empty ingredients array.
- `dal` CRUD — insert, fetch, update (with `updated_at` bump), soft-delete, restore, list filters, list with trash excluded by default.
- Trash sweep — recipe with `deleted_at > now - 30d` purges, cascades to ingredients (via FK), unlinks attachments.

### 8.2 Integration (crates/app)

- `recipe_import_preview` end-to-end with wiremock serving fixture pages:
  - JSON-LD happy path → returns preview with `import_method='jsonld'`.
  - No JSON-LD + local LLM mock returning valid JSON → `import_method='llm'`.
  - No JSON-LD + local unreachable + remote mock → `import_method='llm_remote'`.
  - Non-HTML content type → error surface.
  - Body > 2 MB → error surface.
- `recipe_import_commit` persists correctly — recipe + ingredients + tag_links + attachment link in one transaction, rolls back on any failure.
- `recipe_import_cancel` removes staged attachment file and row.
- Full delete → sweep cycle: create recipe, `recipe_delete`, advance clock 31 days, run sweeper, assert recipe + ingredients + attachment all gone.

### 8.3 Frontend (minimal — matches existing pattern)

- Import drawer: fetch → preview → edit → save flow against mocked IPC (RTL).
- Library list: search debounce + tag filter composition (RTL).
- Snapshot-free — visual verification by manual QA per Manor's existing frontend-test convention.

## 9. Out of scope for L3a

Listed to prevent drift:

- Meal plan (date → recipe assignment) — **L3b**.
- Shopping list — **L3c**.
- Meal ideas reshuffle — **L3d**.
- Multiple images / step photos per recipe.
- Recipe scaling ("make this for 6 instead of 4").
- Nutritional information / calorie counting.
- Recipe export / share / backup-outside-Manor.
- Full-text ingredient search.
- Filter by prep/cook time or servings.
- Import from PDF / screenshot / photo of a cookbook page.
- Bulk import (multiple URLs at once).
- Duplicate detection on import (same URL or similar title).
- Per-ingredient unit normalisation (grams ↔ cups).

## 10. Definition of done

- Migration V14 runs on fresh install and existing dev DBs without issue.
- `Hearth` tab renders in nav, Lucide icon, Flat-Notion styling.
- Recipe list: search + tag filter + empty state + grid cards all working.
- Recipe detail view renders all fields with null-safe meta, import-method badge, source host.
- New/Edit drawer persists recipe + ingredients + tags + image correctly.
- Import drawer: URL paste → Fetch → preview (all three import_method paths exercised) → Save → recipe appears in library.
- Cancel paths: import drawer, edit drawer, delete confirmation all clean up state without orphans.
- Delete → Trash (via existing trash UI) → Restore round-trips correctly.
- 30-day auto-purge cascades ingredients + unlinks attachments.
- Unit tests: 8 JSON-LD fixtures green, LLM mock paths green, CRUD green.
- Integration tests: wiremock import scenarios green, commit/cancel/sweep green.
- No TypeScript errors. `cargo test` green in workspace. Frontend renders without console errors.

---

*End of L3a design spec. Next: implementation plan.*
