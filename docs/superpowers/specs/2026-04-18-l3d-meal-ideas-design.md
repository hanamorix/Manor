# L3d Meal Ideas Reshuffle — Design Spec

- **Date**: 2026-04-18
- **Landmark**: v0.4 Hearth → L3d (fourth and final sub-landmark)
- **Status**: Approved design. Implementation plan next.
- **Authors**: Hana (product), Nell (architect)
- **Roadmap**: `specs/2026-04-18-v0.4-hearth-roadmap.md`
- **Depends on**: L3a Recipe Library (shipped at `bace41e`), L3b Meal Plan + Staples (shipped at `efd5fb7`), Ollama + Remote LLM (shipped in Landmark 2).

## 1. Purpose

Ship the closing piece of v0.4 Hearth: a meal ideas panel that lives at the top of the This Week view and answers *"what should we cook?"* Default source is the user's own recipe library, rotated using a soft "days-since-last-cooked" score. A "Try something new" escape hatch pivots to AI-suggested recipes (title + blurb cards) that expand into the existing L3a import preview drawer when tapped. L3d completes the plan → shop → refresh loop.

## 2. Scope decisions (locked during brainstorm)

| Area | Decision |
|---|---|
| **UI placement** | Card row above the This Week grid. No dedicated Hearth tab. |
| **Library rotation** | Soft scoring: `score = days_since_last_cooked` (9999 if never cooked). Sorted descending. No hard "recently cooked" window. Graceful with thin libraries. |
| **Cards visible** | 3 at a time. Fixed count on all viewport widths. |
| **Reshuffle affordance** | Library by default. "Not feeling it? Try something new →" muted link escalates to LLM on demand. Explicit, not cyclical. |
| **LLM suggestion fidelity** | Titles-first. One call returns 3 `{title, blurb}` cards. Tapping a card fires a second expand call that produces a full `ImportPreview`, rendered in L3a's existing edit drawer. |
| **Caching** | None. Each reshuffle is a fresh call. |
| **Assign-to-day flow** | Tap a library card → popover of 7 day chips for the current week → pick a day → `meal_plan::setEntry`. |
| **Save AI recipe** | Uses L3a's recipe-edit drawer with `onSubmit = importCommit`. No auto-assign-to-day after save. |
| **Rate limiting** | Library reshuffle: no limit. LLM reshuffle + expand: 1-second debounce on button clicks. Remote-tier budget caps inherited from Landmark 2. |
| **Empty states** | Zero recipes → "Add some recipes and suggestions will appear" + link to Recipes. All 7 days filled → collapsed one-liner header. |

## 3. Architecture

Thin landmark. No new core module or schema; most work is frontend.

### 3.1 New Rust files

- `crates/core/src/meal_plan/ideas.rs` — pure ranker function `library_ranked(conn) -> Vec<ScoredRecipe>`.
- `crates/app/src/meal_plan/ideas_commands.rs` — three Tauri commands: `meal_ideas_library_sample`, `meal_ideas_llm_titles`, `meal_ideas_llm_expand`.

### 3.2 New frontend files

- `apps/desktop/src/lib/meal_plan/ideas-ipc.ts` — 3 invoke wrappers.
- `apps/desktop/src/lib/meal_plan/ideas-state.ts` — Zustand store (library-mode state + LLM-mode state + loading/error).
- `apps/desktop/src/components/Hearth/ThisWeek/MealIdeasRow.tsx` — the 3-card row + reshuffle + "Try something new" link.
- `apps/desktop/src/components/Hearth/ThisWeek/IdeaTitleCard.tsx` — card component for AI-mode entries (no hero image, `Sparkles` icon).
- `apps/desktop/src/components/Hearth/ThisWeek/AssignDayPopover.tsx` — 7-day chip picker.

### 3.3 Reused infrastructure

- `manor_core::recipe::dal::list_recipes` — library source.
- `manor_core::meal_plan::dal::set_entry` — assign-to-day writes.
- `manor_core::recipe::import::extract_via_llm` — reused for the expand step with a slightly modified prompt (see §6).
- `manor_core::recipe::import::ImportedRecipe` + `ImportPreview` — same types surface.
- `manor_core::recipe::ImportMethod::Llm` / `LlmRemote` — tag the AI-sourced recipes.
- `manor_app::recipe::importer::to_preview` — reused to map `ImportedRecipe` → `ImportPreview`.
- `OllamaLlmAdapter` (Landmark 2) — instantiated the same way as L3a's import flow. Frontend's existing `RecipeEditDrawer` with `onSubmit` override handles the save path.
- L3a's `RecipeCard.tsx` — rendered as-is for library cards (hero image, title, meta).
- L3a's `ImportMethodBadge.tsx` — renders on the saved AI recipe's detail view (auto-renders because `import_method` is persisted).

### 3.4 No new schema

"Recently cooked" is derived at query time from `meal_plan_entry` via `MAX(entry_date) GROUP BY recipe_id`. No new table, no new column on `recipe`.

### 3.5 No new settings

Ideas is always-on. LLM budget + tier routing inherited from Landmark 2.

## 4. Library ranker

### 4.1 Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredRecipe {
    pub recipe: Recipe,
    pub score: i64,  // days_since_last_cooked (9999 if never cooked)
}
```

### 4.2 Function

```rust
pub fn library_ranked(conn: &Connection) -> Result<Vec<ScoredRecipe>>;
```

### 4.3 Algorithm

1. Load all non-trashed recipes via `recipe::dal::list_recipes(conn, &ListFilter::default())`.
2. For each recipe, compute `days_since_last_cooked`:
   ```sql
   SELECT MAX(entry_date) FROM meal_plan_entry WHERE recipe_id = ?1
   ```
   If NULL → `score = 9999`. Otherwise `score = (today_local - max_date) in days`. Parse both as `chrono::NaiveDate` and subtract.
3. Sort descending by score. Return.

### 4.4 Today's date

`chrono::Local::now().date_naive()`. Matches L3b's `meal_plan_today_get` convention.

### 4.5 Tie-breaking

Deterministic in the core ranker — returns ties in whatever order `list_recipes` produced (which is `ORDER BY created_at DESC`). Random tie-breaking happens at the Tauri command level via a shuffle of the top-10 scored entries — see §5.1. This keeps unit tests stable.

### 4.6 Edge cases

- **Zero recipes** → empty Vec.
- **1 or 2 recipes** → 1 or 2 ScoredRecipes. Frontend renders whatever it gets.
- **All recipes cooked today** → all score 0; `list_recipes` order preserved in ranker; shuffle at command level gives variety per reshuffle.

## 5. Tauri commands

### 5.1 `meal_ideas_library_sample`

```rust
#[tauri::command]
pub fn meal_ideas_library_sample(state: State<'_, Db>) -> Result<Vec<Recipe>, String>;
```

Steps:
1. `library_ranked(&conn)`.
2. Truncate to top 10 by score.
3. Shuffle that slice (using `rand::thread_rng()`) so reshuffle output varies.
4. Return the first 3 as `Vec<Recipe>` (strip the `ScoredRecipe` wrapper — the score isn't useful to the frontend).

### 5.2 `meal_ideas_llm_titles`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeaTitle {
    pub title: String,
    pub blurb: String,
}

#[tauri::command]
pub async fn meal_ideas_llm_titles(
    state: State<'_, Db>,
    app: AppHandle,  // to access the Ollama/remote client wiring
) -> Result<Vec<IdeaTitle>, String>;
```

Steps:
1. Build the `OllamaLlmAdapter` exactly how L3a's `recipe_import_preview` does it.
2. Send the titles prompt (see §6.1).
3. Parse the JSON array response with L3a's forgiving `extract_json_block` helper. One retry on malformed output with a "previous response was not valid JSON" follow-up.
4. Return up to 3 `IdeaTitle` entries (frontend renders however many came back).
5. On total failure → `Err("AI unavailable — try again later or check Settings → AI.")`.

### 5.3 `meal_ideas_llm_expand`

```rust
#[tauri::command]
pub async fn meal_ideas_llm_expand(
    title: String,
    blurb: String,
    state: State<'_, Db>,
    app: AppHandle,
) -> Result<ImportPreview, String>;
```

Steps:
1. Build the `OllamaLlmAdapter` (same as above).
2. Send the expand prompt (see §6.2) with title + blurb in the content slot.
3. Parse as `ImportedRecipe` — same logic as L3a's LLM extract path.
4. Wrap in `ImportPreview` using the existing `importer::to_preview` helper.
5. Override `source_url` to `None` and `source_host` to `None` (AI-sourced recipes have no URL).
6. Return. Frontend feeds this to `RecipeEditDrawer` with `onSubmit = importCommit`.

## 6. LLM prompts

### 6.1 Titles prompt

```
You suggest 3 home-cookable dinner recipes. Output JSON exactly:
[
  {"title": str, "blurb": str (one sentence, <100 chars, includes timing hint)},
  {"title": str, "blurb": str},
  {"title": str, "blurb": str}
]
Vary cuisines. Prefer weeknight-accessible ingredients. No prose before or after the JSON.
```

Budget attribution: `recipe_ideas_titles` intent.

### 6.2 Expand prompt

```
You extract structured recipe data from a recipe description. Output JSON with this exact shape:
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
If a field is not clearly stated, use reasonable defaults for a 2-serving weeknight meal. You may invent plausible ingredient quantities.
Output ONLY the JSON.

Recipe description:
Title: {title}
Summary: {blurb}
```

Budget attribution: `recipe_ideas_expand` intent.

## 7. UI — Meal Ideas Row

### 7.1 Placement

Directly above the week grid in `ThisWeekView`, below `WeekNav`. Full-width within the existing `ThisWeekView` container.

### 7.2 Library mode (default)

```
Meal ideas                                                [ ↻ Reshuffle ]

┌──────────┐  ┌──────────┐  ┌──────────┐
│  hero    │  │  hero    │  │  hero    │
│          │  │          │  │          │
│ Miso     │  │ Lentil   │  │ Thai     │
│ aubergine│  │ dal      │  │ curry    │
│ 45m · 4p │  │ 30m · 2p │  │ 35m · 4p │
└──────────┘  └──────────┘  └──────────┘

Not feeling it? Try something new →

─────────────────────────────────────────

(week grid below)
```

- Heading "Meal ideas" + reshuffle button (Lucide `RefreshCw`) right-aligned.
- 3 cards using L3a's `RecipeCard` component verbatim. Grid: `repeat(3, 1fr)`, gap `16px`, max-width same as the grid.
- Muted link below: `Not feeling it? Try something new →` in `--ink-soft` at `--text-xs`.
- Each card tap → `AssignDayPopover`.

### 7.3 LLM mode

```
Meal ideas — AI                          [ ↻ Reshuffle ]

┌──────────┐  ┌──────────┐  ┌──────────┐
│ ✨       │  │ ✨       │  │ ✨       │
│          │  │          │  │          │
│ Harissa  │  │ Sheet    │  │ Creamy   │
│ chicken  │  │ pan salm │  │ gnocchi  │
│ 35m · 4p │  │ 25m · 2p │  │ 20m · 2p │
│ (blurb)  │  │ (blurb)  │  │ (blurb)  │
└──────────┘  └──────────┘  └──────────┘

← Back to library
```

- Heading becomes "Meal ideas — AI".
- Cards render `IdeaTitleCard`: flat background, `Sparkles` icon top-left, title, blurb (2-line clamp). No hero image. No servings/time meta (LLM didn't produce structured fields at this stage).
- Link below is `← Back to library` (switches store mode back, re-renders library cards without new call).
- Tap a card → loading overlay on that card → `ideas_llm_expand` → opens `RecipeEditDrawer` with `initialDraft = preview.recipe_draft`, `title = "Save AI recipe"`, `saveLabel = "Save to library"`, `onSubmit = importCommit`. Drawer closes on save; recipe lands in library with `import_method = 'llm'` or `'llm_remote'`.
- Cancel in the drawer just closes it; no orphan cleanup needed (nothing was staged).

### 7.4 Empty-library state

When `library_ranked` returns 0:

```
Meal ideas

Add some recipes to your library and suggestions will appear here.
→ Go to Recipes
```

Single line, muted text, link triggers `useHearthViewStore().setSubview("recipes")`.

### 7.5 Fully-planned-week state

When all 7 `entries` from `useMealPlanStore` have a non-null `recipe`:

```
Meal ideas · Week is fully planned                          [ ↻ Reshuffle ]
```

Collapsed single-line header. Reshuffle still functions (user might swap a planned meal). Cards don't render — tapping reshuffle expands the row back to 3-card layout.

### 7.6 Assign-day popover

Anchored below the clicked library card.

```
Plan "Miso aubergine" on…
┌─────┬─────┬─────┬─────┬─────┬─────┬─────┐
│ Mon │ Tue │ Wed │ Thu*│ Fri*│ Sat │ Sun │
│ 20  │ 21  │ 22  │ 23  │ 24  │ 25  │ 26  │
│ —   │ dal │ —   │ —   │ miso│ —   │ —   │
└─────┴─────┴─────┴─────┴─────┴─────┴─────┘
```

- 7 chips, one per day. `*` marks today.
- Each chip shows day name + day-of-month + tiny status label: `—` for empty, truncated recipe title for planned.
- Empty chip click → `meal_plan::setEntry(date, recipe.id)` → popover closes.
- Planned chip click → confirm `window.confirm("Replace <current> with <suggestion>?")` → if yes, same setEntry call (upserts).
- Escape or click-outside → popover closes.

Popover uses the same chrome as L3b's date picker: absolute positioning, `--paper` bg, 1px `--hairline` border, 4px shadow.

### 7.7 Rate limiting

- Library reshuffle button: no debounce — cheap DB sample.
- LLM reshuffle button + card-expand click: 1-second debounce via `setTimeout`-guarded handler. Prevents double-click firing two LLM calls.

## 8. Zustand store

`apps/desktop/src/lib/meal_plan/ideas-state.ts`:

```ts
type Mode = "library" | "llm";
type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface IdeasStore {
  mode: Mode;
  library: Recipe[];
  llm: IdeaTitle[];
  loadStatus: LoadStatus;

  loadLibrary(): Promise<void>;
  loadLlm(): Promise<void>;           // switches mode to "llm" + loads titles
  backToLibrary(): void;              // switches mode back without re-fetching
  expandAiTitle(t: IdeaTitle): Promise<ImportPreview>;
}
```

On first mount, frontend calls `loadLibrary`. Mode never persists across sessions (no setting).

## 9. Error handling

| Error | User sees | Recovery |
|---|---|---|
| Library ranker fails | Row renders "Couldn't load ideas — Retry" | Retry button calls `loadLibrary` |
| Zero recipes | Empty-state message with "→ Go to Recipes" link | User adds a recipe |
| LLM titles both-tiers unreachable | Toast "AI unavailable — try again later or check Settings → AI." + auto-switch back to library mode | User retries later |
| LLM titles returns <3 / malformed (after retry) | Render whatever valid titles came back. If 0, same as unreachable | — |
| LLM expand fails | Inline error on the tapped card + Retry button. Other cards stay clickable | Retry or tap a different card |
| setEntry fails on popover confirm | Popover stays open, inline error | Retry |
| Rate-limit debounce active | Reshuffle button briefly disabled | User waits |

## 10. Testing strategy

### 10.1 Unit — `crates/core/src/meal_plan/ideas.rs`

- `library_ranked` with empty library → empty Vec.
- Single never-cooked recipe → 1 ScoredRecipe, score=9999.
- 3 recipes cooked at different dates → sorted descending, correct day counts.
- Trashed recipe excluded (verified by `list_recipes`'s soft-delete filter).
- All-cooked-today → all score 0, deterministic order.

### 10.2 Unit — `crates/app/src/meal_plan/ideas_commands.rs`

- `meal_ideas_library_sample` with stubbed DAL returning 20 scored recipes → returns exactly 3.
- Same with only 2 available → returns 2.
- `meal_ideas_llm_titles` with a `StubLlm` returning valid 3-item JSON array → parses 3 `IdeaTitle`s.
- `meal_ideas_llm_titles` with malformed JSON + retry returning valid JSON → parses successfully.
- `meal_ideas_llm_titles` with both failures → returns an error.
- `meal_ideas_llm_expand` with `StubLlm` returning valid `ImportedRecipe` JSON → returns an `ImportPreview` with `import_method = Llm`, `source_url = None`.

### 10.3 Frontend

React Testing Library:
- `MealIdeasRow` renders 3 `RecipeCard`s from a mocked store.
- `"Try something new →"` click calls `loadLlm` + switches heading.
- LLM mode renders 3 `IdeaTitleCard`s.
- `"← Back to library"` switches back without re-fetching.
- Empty-library branch renders link.
- Fully-planned-week branch collapses to one line.
- Library card click opens `AssignDayPopover` with 7 correct chips.
- Popover empty-chip click fires `setEntry` with right args.

## 11. Out of scope for L3d

Pinned to prevent drift:

- Dietary filters (vegetarian / gluten-free / etc).
- "Cook again soon" pinning.
- Nutritional balancing across the week.
- Learning from user preferences over time.
- Shopping-list-aware suggestions ("meals that reuse stuff you already need").
- Seasonal awareness.
- Cooking-time-aware filters ("weeknight? <30min only").
- AI-generated hero images for AI cards.
- One-gesture save-and-assign for AI recipes.
- Multi-week planning from suggestions.
- Suggest swapping an already-planned meal proactively.

## 12. Definition of done

- `ideas.rs` ranker shipped with unit tests.
- 3 Tauri commands registered in `manor-app`.
- `MealIdeasRow` renders above the week grid with 3 cards, reshuffle button, and "Try something new →" link.
- Library reshuffle rotates cards (visibly different picks across reshuffles).
- "Try something new" shows 3 AI `IdeaTitleCard`s. Tap expands via LLM into L3a's `RecipeEditDrawer`; save lands the recipe in library with `import_method = 'llm'` (or `'llm_remote'`).
- `AssignDayPopover` shows 7 correct day chips; tapping assigns; replace-confirm works.
- Empty-library and fully-planned-week states render gracefully.
- Error paths (LLM unreachable, malformed response, setEntry failure) surface toasts/inline errors per §9.
- `cargo test --workspace` green. Clippy clean. TypeScript clean. Production build green.
- Manual QA pass: plan a week using a mix of library + AI suggestions end-to-end.

---

*End of L3d design spec. This closes v0.4 Hearth. Next: implementation plan → execute → merge → roadmap marks v0.4 shipped.*
