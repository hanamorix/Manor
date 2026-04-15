# Phase 3a — Today View + Manual Tasks (with the first proposal flow)

**Date:** 2026-04-15
**Phase:** v0.1 Heartbeat, Phase 3a (first sub-phase of Phase 3 to ship; honors the `task` schema contract from the 3c spec)
**Status:** Spec — ready to plan

---

## 1. Summary

Manor's main window stops being a placeholder. The Today view becomes the first real surface — a stacked-card layout showing today's events (empty until 3b) and tasks. Hana can add, edit, complete, and delete tasks directly. Manor can *propose* task additions through Ollama tool calling; proposals appear as a calm amber banner at the top of the view for review/approve/reject. A direct slash command (`/task <title>`) bypasses Manor entirely for power-user immediacy.

This is the first phase that introduces a mutable user-data feature, so it doubles as the first real test of the **proposal abstraction** scaffolded in Phase 2. Only the `add_task` proposal kind is wired in 3a; `complete_task`, `delete_task`, etc. are deferred to a later "Manor edits" phase.

---

## 2. Goals & non-goals

### 2.1 Goals

- **Today view** replaces the placeholder text in `App.tsx`, laid out as 3 stacked cards (Header, Events, Tasks), with bottom padding reserved so the avatar never overlaps content
- **`task` table** per the 3c spec §5.1 contract (no surprises later for 3c)
- **Manual task CRUD:** add (inline input revealed by "+ Add"), edit (click title → inline rename), complete (checkbox → strike + fade after 4s with undo), delete (hover → trash icon → soft-confirm)
- **Two creation paths:**
  - **Direct:** `/task <title>` slash command in the pill, OR the inline "+ Add" affordance — both create a task immediately, no proposal
  - **Manor-routed:** Ollama tool call → backend creates a `proposal` row of kind `add_task` → banner appears at the top of the Today view → user approves (task created) or rejects (proposal marked rejected)
- **Banner pattern** for proposals — stacks if multiple pending, persists across drawer-open/close and app restarts, never auto-dismisses
- **Empty states** for Events ("no calendar connected — coming next phase") and Tasks ("nothing on your plate — type /task or click + Add to add one")

### 2.2 Non-goals (explicit YAGNI)

| Item | Where it lives instead |
|---|---|
| CalDAV / event sync | 3b |
| Manor *reading* tasks (in system prompt) | 3c |
| `complete_task` / `delete_task` / `edit_task` proposal kinds | Future "Manor edits" phase |
| Tool-use for any non-task action | Same |
| Task description / priority / tags | Add when needed by a real feature |
| Recurring tasks / chores | Phase 4 Rhythm |
| Drag-to-reorder / due-date dragging | Not v0.1 |
| Done-today archive view | Defer — completion is fade-and-forget for v0.1 |
| Editing a task's due date in the UI | All UI-created tasks default to today; Manor's tool calls may pass a date. Inline editing of dates is deferred |
| Multi-select on tasks | Defer |
| Keyboard shortcuts beyond Enter / Esc on inputs | Defer |
| Stale-proposal dedup | Defer; user can reject duplicates manually |

---

## 3. Architecture

### 3.1 Rust backend (touched crates)

| Crate / module | Change |
|---|---|
| `manor-core/migrations/V2__task.sql` | New file: `task` table per 3c §5.1 + index |
| `manor-core/src/assistant/task.rs` | New module: `Task` type + CRUD functions |
| `manor-core/src/assistant/proposal.rs` | Extend: `approve_add_task` (transactional) + `reject` |
| `manor-app/src/assistant/tools.rs` | New module: tool-schema definitions (just `add_task` for 3a) |
| `manor-app/src/assistant/ollama.rs` | Extend `ChatRequest` to include a `tools` array; parse `tool_calls` from final NDJSON message; add `StreamChunk::Proposal(i64)` variant |
| `manor-app/src/assistant/prompts.rs` | Append a short addition explaining Manor's tool-use boundaries (only propose, never claim done) |
| `manor-app/src/assistant/commands.rs` | Modify `send_message`: call Ollama with tools declared. On tool calls, create proposals + emit `StreamChunk::Proposal`. Add new commands listed in §3.3. (Slash parsing is client-side only — see §6.) |

### 3.2 Frontend (under `apps/desktop/src/`)

```
components/Today/
  Today.tsx              # top-level: header card + Events card + Tasks card
  HeaderCard.tsx         # date + day label + live "now" badge
  EventsCard.tsx         # empty state for 3a; populated in 3b
  TasksCard.tsx          # list + "+ Add" reveal + inline new-task input
  TaskRow.tsx            # checkbox + title + hover edit/delete
  ProposalBanner.tsx     # stacked banners at top, reads from useTodayStore.pendingProposals
lib/today/
  state.ts               # zustand slice: tasks[], pendingProposals[]
  ipc.ts                 # typed wrappers for the new Tauri commands
  slash.ts               # client-side slash detection (only — no server-side parser)
lib/layout.ts            # AVATAR_FOOTPRINT_PX constant; consumed by Avatar + Today's <main>
```

Modified:

- `App.tsx` — render `<ProposalBanner />` + `<Today />` instead of placeholder
- `Assistant/Assistant.tsx` — slash detection in `handleSubmit` (if `/task ...`, route to direct add instead of `sendMessage`); same for `ConversationDrawer`'s submit
- `lib/assistant/ipc.ts` — add `Proposal` variant to the `StreamChunk` type
- `lib/assistant/state.ts` — no changes (tasks + proposals live in their own store)

### 3.3 Tauri IPC contract additions

| Command | Shape | Purpose |
|---|---|---|
| `list_tasks() -> Vec<Task>` | query | Today view's tasks card hydrates from this. Implemented as `task::list_today_open(conn, today_local_iso())` so the view shows only tasks due today or with no due date |
| `add_task(title: String, due_date: Option<String>) -> Task` | mutation | Direct add (slash command or "+ Add"); returns the new row |
| `complete_task(id: i64) -> ()` | mutation | Checkbox click |
| `undo_complete_task(id: i64) -> ()` | mutation | Click within the 4s window |
| `update_task(id: i64, title: String) -> ()` | mutation | Inline rename |
| `delete_task(id: i64) -> ()` | mutation | Trash icon (after soft-confirm) |
| `list_proposals(status: Option<String>) -> Vec<Proposal>` | query | Banner reads `status = 'pending'` |
| `approve_proposal(id: i64) -> Vec<Task>` | mutation | Apply diff; returns refreshed task list |
| `reject_proposal(id: i64) -> ()` | mutation | Mark rejected, no apply |

### 3.4 State management

Separate `useTodayStore` (zustand) from the existing `useAssistantStore`. Cleaner boundaries; we can merge later if cross-surface state proves painful.

### 3.5 No new dependencies

Tauri 2's `Channel` plumbing handles the new `StreamChunk::Proposal` variant for free. Everything else uses workspace deps already declared in Phase 1 / 2.

---

## 4. Today view layout

Top-level structure — a vertical-flow main area, padded so the avatar's footprint never overlaps content.

```
<main>
  <ProposalBanner />          ← stacked amber banners, max-height ~30vh, scrolls if many
  <HeaderCard />              ← date + day label + live "now" badge
  <EventsCard />              ← events list (empty in 3a)
  <TasksCard />               ← tasks list + "+ Add" affordance
</main>
```

### 4.1 Container styling

- Window width up to 1100px (default Tauri shell); main area centered with `max-width: 760px` so cards don't sprawl on wider windows
- Padding: `24px` left/right/top, `140px` bottom (avatar 96 + 16 corner gap + 28 breathing room)
- Vertical gap between cards: `12px`

### 4.2 Card styling (shared)

- White background (`var(--paper)`)
- 1px hairline border (`var(--hairline)`)
- `border-radius: var(--radius-lg)` (16px)
- Subtle shadow (`var(--shadow-sm)`)
- Padding `16px 18px`

### 4.3 HeaderCard

- Day label, large + bold: `Today` (h1, 22px, weight 700)
- Date subtitle: `Tuesday, 15 April` (13px, muted)
- Right-aligned: a small "now" badge showing local time (`14:32 BST`, monospace), refreshes every 60s via `setInterval` in the component (cleared on unmount)

### 4.4 EventsCard (3a empty state)

- Section header: `Events` (uppercase 11px tracked, muted)
- Empty body: `No calendar connected. Coming next phase.` in muted text, italics

### 4.5 TasksCard — populated state

- Section header row: `Tasks · {N} open` on the left, `+ Add` link on the right (right-aligned via flex `justify-content: space-between`)
- Vertical list of `<TaskRow>` components, gap `4px`
- When "+ Add" clicked: an inline `<input>` appears below the last row, autofocused; type a title + Enter to commit (calls `add_task`); commit clears the input and keeps it focused for a follow-up; Esc cancels and hides

### 4.6 TasksCard — empty state

- Section header: `Tasks` (no count)
- Body: `Nothing on your plate. Type /task or click + Add to add one.` (muted, single line)
- "+ Add" link still in the header, clickable

### 4.7 Avatar collision discipline

- The 140px bottom padding on `<main>` is computed from the avatar's actual rendered size (`96 + 16 + 28`)
- If the avatar size ever changes, the padding constant must follow — captured as a single source of truth: `export const AVATAR_FOOTPRINT_PX = 140` in `lib/layout.ts` (new file), referenced by both `<Avatar>` (via existing constant lookup) and `<main>` (for its bottom padding). Prevents drift.

---

## 5. Task management — interaction details

### 5.1 TaskRow default state

```
[☐] Reply to Miriam                              ← title takes flex 1
```

- Checkbox: 18×18, 1.5px border, radius 4. Hairline color; on hover the border deepens.
- Title: 14px, line-height 1.4, vertically centered with the checkbox baseline.
- Row padding: `6px 4px`.
- Cursor: `default` over the row, `pointer` over the checkbox and (on hover) over the action icons.

### 5.2 TaskRow hover state

```
[☐] Reply to Miriam                       [✎] [🗑]
```

- Pencil + trash icons appear right-aligned on hover (opacity 0 → 0.6 over 100ms)
- Icon click area is 24×24
- Pencil → switch row to edit state
- Trash → soft-confirm: first click highlights the row red briefly + waits 1s for second click to confirm. If no second click, returns to default.

### 5.3 TaskRow edit state

```
[☐] [Reply to Miriam_______________________]    ← inline input, replaces title text
```

- Click the title text (or the pencil icon) → title becomes an `<input type="text">` with `value` set to current title, autofocused, cursor at end
- Enter saves (calls `update_task(id, title)`), Esc cancels and reverts
- Click outside the row also commits (acts as Enter)
- Empty title on commit → revert (don't allow blanking)

### 5.4 TaskRow completion flow

1. Click checkbox → optimistic update: row gets `class: completing`
2. Visual: checkbox fills green with white check, title gets strikethrough + opacity 0.5
3. Backend: `complete_task(id)` called (sets `completed_at = now`)
4. 4-second window: clicking the row again calls `undo_complete_task(id)` (sets `completed_at = NULL`), row reverts to default
5. After 4s with no undo: row fades out (200ms opacity transition) and unmounts; `useTodayStore` removes it from `tasks` array

### 5.5 Due dates in 3a — explicit non-feature

- The DB column exists (3c contract requires it)
- Direct creation paths (`/task` slash, "+ Add" inline input) default `due_date` to today's local date
- The UI in 3a does **not** expose editing or even displaying the due date — every task in the view is implicitly "today" because that's the view's filter
- Manor's `add_task` tool call **may** pass a `due_date` (e.g., the user said "remind me to call mum tomorrow"). The proposal banner shows the date as part of the diff preview; once approved, the task is created with that date and won't appear in today's list (it'll show up the day it's due, post-3c when the system filters by today)
- Editing existing dates: deferred. If you need to change a date, delete + re-add. If this hurts in practice, add an inline date affordance later.

### 5.6 Direct add via "+ Add"

1. Click "+ Add" link in the Tasks card header
2. An inline input slides in below the last task (or at the top if list is empty), autofocused
3. Placeholder: `New task title`
4. Enter → calls `add_task(title, today)` → backend inserts → input clears, stays focused for follow-up adds
5. Esc → cancels and hides the input
6. Click outside → commits if non-empty, hides if empty

### 5.7 Optimistic UI

All mutations update `useTodayStore` immediately, then the IPC call confirms. On error (rare for SQLite), the store reverts and a small toast appears at the bottom-center: `Couldn't save: <reason>`.

---

## 6. Slash commands

### 6.1 Syntax for v0.1

```
/task <title>
```

That's the only command. No flags, no due-date suffix (deferred per §5.5). Anything after `/task ` (with the space) up to the next newline is the title.

### 6.2 Where it's parsed

**Client-side**, in both `InputPill` (the corner pill) and `ConversationDrawer`'s textarea. Both submit handlers call a small `parseSlash(text)` helper:

```ts
type SlashCommand =
  | { type: "task"; title: string }
  | { type: "unknown"; raw: string };

parseSlash("/task pick up prescription")
  // → { type: "task", title: "pick up prescription" }
parseSlash("/banana split")
  // → { type: "unknown", raw: "/banana split" }
parseSlash("hello")
  // → null  (not a slash command)
```

- If the result is a `task` command, the submit handler calls `addTask(title, today)` IPC directly. **No call to `send_message`. No turn in chat history.** Manor never sees it.
- If `unknown`, fall through to `send_message` as normal — Manor will see "/banana split" in her chat history. (She can respond with "I don't know that command yet" or whatever the model decides.)
- If `null`, it's a regular message — fall through to `send_message`.

### 6.3 UI feedback for successful `/task`

A small toast at the bottom-center of the window: `Added: <title>` for 2s. The Tasks card refreshes immediately (optimistic — toast fires on store update, not IPC ack).

### 6.4 No server-side slash parsing in 3a

If the client somehow misses a slash command (it won't, both inputs use the same helper), Manor just sees a literal `/task X` message in her chat — graceful degradation. Server-side parsing can be added later if a third input surface appears that bypasses the client helper.

### 6.5 Future commands (out of scope, illustrative)

- `/event <title> at <time>` — would land in 3b
- `/help` — generic listing
- `/clear` — wipe conversation

The parser's `unknown` branch + Manor's natural-language fallback handles everything else.

---

## 7. Manor's tool-use flow (Ollama function calling)

### 7.1 Tool declaration

In new module `manor-app/src/assistant/tools.rs`:

```rust
pub fn add_task_tool() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "add_task",
            "description": "Propose adding a task to the user's task list. \
                            Use when the user asks you to remember, remind, or \
                            track something. Do not claim to have added it; the \
                            user must approve the proposal first.",
            "parameters": {
                "type": "object",
                "required": ["title"],
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "A short imperative — e.g. 'Pick up prescription'"
                    },
                    "due_date": {
                        "type": "string",
                        "description": "Optional. ISO date, format YYYY-MM-DD. Omit for today."
                    }
                }
            }
        }
    })
}

pub fn all_tools() -> Vec<serde_json::Value> {
    vec![add_task_tool()]
}
```

### 7.2 Outgoing chat request shape

Extend `ChatRequest` in `ollama.rs`:

```rust
#[derive(Debug, Clone, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
    tools: &'a [serde_json::Value],   // NEW
}
```

`commands::send_message` passes `tools::all_tools().as_slice()` so Ollama knows what's callable.

### 7.3 Incoming NDJSON parsing

Extend `OllamaChunkMessage`:

```rust
#[derive(Debug, Clone, Deserialize)]
struct OllamaChunkMessage {
    #[allow(dead_code)]
    role: Option<String>,
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,   // NEW
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaToolCall {
    function: OllamaToolFunction,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaToolFunction {
    name: String,
    arguments: serde_json::Value,
}
```

For `qwen2.5:7b-instruct`, tool calls arrive in the final `done: true` message (not chunked across the stream). Parser collects them and processes after stream ends.

### 7.4 New StreamChunk variant

In `ollama.rs`:

```rust
pub enum StreamChunk {
    Started(i64),
    Token(String),
    Proposal(i64),    // NEW — proposal row id
    Done,
    Error(ErrorCode),
}
```

### 7.5 End-of-stream handling in commands

```rust
// after the stream loop ends, before sending Done:
for tool_call in collected_tool_calls {
    match tool_call.function.name.as_str() {
        "add_task" => {
            let args: AddTaskArgs = serde_json::from_value(tool_call.function.arguments)?;
            let proposal_id = proposal::insert(&conn, NewProposal {
                kind: "add_task",
                rationale: &assistant_content_so_far,  // Manor's text
                diff_json: &serde_json::to_string(&args)?,
                skill: "tasks",
            })?;
            on_event.send(StreamChunk::Proposal(proposal_id)).await?;
        }
        unknown => {
            tracing::warn!("ignoring unknown tool call: {unknown}");
        }
    }
}
on_event.send(StreamChunk::Done).await?;
```

### 7.6 System prompt addition

In `prompts.rs`:

```rust
pub const SYSTEM_PROMPT: &str = concat!(
    /* existing prompt unchanged */,
    "\n\n",
    "You can propose changes to the user's data using the tools provided. ",
    "When you call a tool, the change is *proposed* — the user reviews and ",
    "approves before it takes effect. Do not say 'I added' or 'I did' — say ",
    "'I'd like to add' or 'shall I…?' instead. The proposal banner will ",
    "show them what you suggested."
);
```

### 7.7 Frontend handling of `Proposal` chunk

In `Assistant.tsx`'s `onEvent`:

```ts
} else if (chunk.type === "Proposal") {
  void listProposals("pending").then((proposals) => {
    useTodayStore.getState().setPendingProposals(proposals);
  });
}
```

`<ProposalBanner>` subscribes to `useTodayStore((s) => s.pendingProposals)` and renders one banner per pending proposal.

---

## 8. Proposal lifecycle + review UI

### 8.1 Lifecycle (extends the Phase 2 scaffold)

```
pending  ──approve_proposal──> applied
   │
   └────reject_proposal───────> rejected
```

No further states. `partially_applied` is reserved for proposals with multi-op diffs (Phase 4+). For 3a, every proposal has exactly one op (`add_task`).

### 8.2 `approve_proposal(id)` (Rust, transactional)

```rust
pub fn approve_proposal(conn: &Connection, id: i64) -> Result<Vec<Task>> {
    let tx = conn.unchecked_transaction()?;
    let row: ProposalRow = /* SELECT * FROM proposal WHERE id = ?1 AND status = 'pending' */;
    match row.kind.as_str() {
        "add_task" => {
            let args: AddTaskArgs = serde_json::from_str(&row.diff)?;
            let due_date = args.due_date.unwrap_or_else(today_local_date_string);
            task::insert(&tx, &args.title, Some(&due_date), Some(id))?;
            tx.execute(
                "UPDATE proposal SET status = 'applied', applied_at = ?1 WHERE id = ?2",
                params![now_secs(), id],
            )?;
        }
        unknown => anyhow::bail!("unknown proposal kind: {unknown}"),
    }
    tx.commit()?;
    task::list_open(conn)
}
```

Returns the refreshed task list so the frontend can update in one round trip.

### 8.3 `reject_proposal(id)`

Simple `UPDATE proposal SET status = 'rejected' WHERE id = ?1 AND status = 'pending'`. No diff applied.

### 8.4 `<ProposalBanner>` component (frontend)

Lives at the top of `<main>` (above `<HeaderCard>`), and ALSO renders inside `<ConversationDrawer>` at the top of the body area. Both read from the same `useTodayStore.pendingProposals` slice — actions in either place dismiss from both.

### 8.5 Per-banner shape

```
┌──────────────────────────────────────────────────────────────┐
│ [PROPOSAL]  Add task: Book dentist appointment      [✓] [✗]  │
│             Manor: "you mentioned it just now"               │
└──────────────────────────────────────────────────────────────┘
```

- Background: warm amber gradient (`linear-gradient(180deg, #FFF3D6, #FFE4A8)`)
- Border: 1px `#FFC15C`
- Border-radius: `var(--radius-md)` (12px)
- Padding: `10px 14px`
- Layout: flex row — badge + content (flex 1) + actions
- **Badge** (`PROPOSAL`): orange (`#FF8800`), white text, 10px tracked
- **Content (flex 1):**
  - Top line: bold 13px, the diff summary (e.g. `Add task: Book dentist appointment` — derived from the proposal's `kind` + parsed `diff`)
  - Bottom line: 11px muted, prefixed `Manor:` then the rationale in quotes (truncated to 1 line with ellipsis if >120 chars)
- **Actions:** two icon buttons
  - ✓ Approve — `--imessage-green`, white text, ~28×24px, calls `approveProposal(id)`
  - ✗ Reject — light gray, calls `rejectProposal(id)`
  - Both immediately remove the banner (optimistic) and refresh tasks on success
- Banner-to-banner gap: `8px`. Multiple pending proposals stack vertically with the oldest at the top
- Entry animation: `bannerIn 200ms ease-out` — fade + slide down from -8px

### 8.6 Empty rationale handling

If Manor's content was empty (model jumped straight to a tool call without text), the bottom line shows `Manor proposed this from your message` instead of an empty quote.

### 8.7 Stacking limit

No hard cap. If 5 proposals pile up, the banner stack scrolls (the surrounding `<main>` becomes scrollable; banners themselves stay readable).

### 8.8 Persistence

Proposals live in the DB. On app restart, `<ProposalBanner>` calls `listProposals("pending")` on first mount and re-renders any unhandled banners. Killing Manor doesn't lose pending proposals.

### 8.9 Dedup (out of scope for v0.1)

If Manor proposes "Add task: Pick up prescription" twice, two banners stack. The user can reject the duplicate. Proposal-side dedup (e.g., refuse to insert if there's a pending proposal with identical diff) is a nice-to-have; defer.

---

## 9. Data model

### 9.1 Migration `V2__task.sql`

New file at `crates/core/migrations/V2__task.sql`:

```sql
CREATE TABLE task (
  id              INTEGER PRIMARY KEY,
  title           TEXT    NOT NULL,
  due_date        TEXT    NULL,        -- 'YYYY-MM-DD' in local TZ
  completed_at    INTEGER NULL,        -- unix seconds
  created_at      INTEGER NOT NULL,    -- unix ms
  proposal_id     INTEGER NULL REFERENCES proposal(id)
);

CREATE INDEX idx_task_open_due ON task (completed_at, due_date);
```

Verbatim from 3c §5.1 plus the `proposal_id` column for traceability — when a task is created via a proposal, that proposal's id is recorded.

### 9.2 Existing `proposal` table

No schema changes. Phase 2 already shipped it. 3a populates rows of `kind = 'add_task'`.

### 9.3 Task data-access functions

In `crates/core/src/assistant/task.rs`:

| Function | Signature | Purpose |
|---|---|---|
| `insert(conn, title, due_date, proposal_id?)` | → `Result<i64>` | Create a new task (returns row id) |
| `list_open(conn)` | → `Result<Vec<Task>>` | All tasks where `completed_at IS NULL`, ordered by `(due_date IS NULL), due_date, created_at`. **Used by 3c** for the prompt context (Manor needs awareness of upcoming + no-date open tasks too). |
| `list_today_open(conn, today_iso)` | → `Result<Vec<Task>>` | Open tasks where `due_date IS NULL OR due_date = ?1`. **Used by 3a's `list_tasks` IPC** so the Today view filters out tasks due in the future. |
| `complete(conn, id)` | → `Result<()>` | Set `completed_at = now()` |
| `undo_complete(conn, id)` | → `Result<()>` | Set `completed_at = NULL` (called by the 4s undo window) |
| `update_title(conn, id, title)` | → `Result<()>` | Rename |
| `delete(conn, id)` | → `Result<()>` | Hard delete |

### 9.4 `Task` struct (Rust)

Mirrors message's serde shape — derives `Serialize, Deserialize, Clone, Debug, PartialEq, Eq`. Fields:

```rust
pub struct Task {
    pub id: i64,
    pub title: String,
    pub due_date: Option<String>,    // "YYYY-MM-DD" or None
    pub completed_at: Option<i64>,   // unix seconds
    pub created_at: i64,             // unix ms
    pub proposal_id: Option<i64>,
}
```

### 9.5 Frontend `Task` type

In `lib/today/ipc.ts`:

```ts
export interface Task {
  id: number;
  title: string;
  due_date: string | null;
  completed_at: number | null;
  created_at: number;
  proposal_id: number | null;
}
```

### 9.6 `useTodayStore` zustand slice

In `lib/today/state.ts`:

```ts
interface TodayStore {
  tasks: Task[];
  pendingProposals: Proposal[];

  setTasks: (t: Task[]) => void;
  upsertTask: (t: Task) => void;
  removeTask: (id: number) => void;
  setPendingProposals: (p: Proposal[]) => void;
  removeProposal: (id: number) => void;
}
```

---

## 10. Testing strategy

### 10.1 Rust unit tests

| Module | Tests |
|---|---|
| `manor-core::assistant::task` | `insert_returns_id_and_persists`; `list_open_excludes_completed`; `list_open_orders_by_due_date_then_created_at`; `complete_then_undo_round_trip`; `update_title_persists`; `delete_removes_row` |
| `manor-core::assistant::proposal` (extended) | `approve_add_task_creates_task_and_marks_applied`; `approve_already_applied_is_noop`; `reject_marks_rejected_without_applying` |
| `manor-app::assistant::ollama` (extended) | New wiremock test: `streams_content_then_tool_call` — Ollama responds with content + a final `tool_calls` entry, assert `StreamChunk::Token` events then `StreamChunk::Proposal(_)` then `StreamChunk::Done` |

### 10.2 Rust integration test

None for 3a beyond the wiremock test in §10.1. The new IPC commands (`add_task`, `complete_task`, etc.) are thin wrappers over the data layer covered in §10.1; manual smoke (§10.4) catches end-to-end issues.

### 10.3 Frontend vitest

Under `apps/desktop/src/lib/today/`:

| File | Tests |
|---|---|
| `slash.test.ts` | Mirror of the Rust slash parser tests; same inputs, same outputs (cross-check parity) |
| `state.test.ts` | `setTasks_replaces_array`; `upsertTask_replaces_by_id_or_appends`; `removeTask_drops_by_id`; `setPendingProposals`; `removeProposal` |
| `task-completion.test.ts` | Fake-timer test for the 4s undo window — assert that completing a task then immediately undoing within 4s reverts state, and that completing without undo unmounts after 4s |

### 10.4 Manual smoke test (end-to-end, last task in the plan)

1. Open Manor → Today view shows `Tuesday 15 April`, Events card empty placeholder, Tasks card empty placeholder
2. Click "+ Add" → input appears → type "Test task one" + Enter → row appears with checkbox
3. Type `/task Test task two` in the pill + Enter → small toast `Added: Test task two`, row appears in Tasks card
4. Click checkbox on Test task one → strikethrough + dim, then row vanishes after 4s
5. Hover over Test task two → pencil + trash icons fade in
6. Click pencil → title becomes inline input, type "Renamed" + Enter → row updates
7. Click trash → row turns red briefly, click trash again within 1s → row removed
8. In the pill, type "remind me to book a dentist appointment" + Enter
9. Manor streams a reply ("I'd like to add that for you") then a `PROPOSAL` banner appears at the top of Today view
10. Click ✓ on the banner → banner removes itself, "Book dentist appointment" appears in Tasks card
11. Send another message that should propose a task; click ✗ on the new banner → banner removes, no task added
12. Quit Manor + relaunch → if a banner was left pending, it reappears; tasks persist

---

## 11. Phase 3a completion criteria

- [ ] `cargo test --workspace --all-targets` green (existing 17 + new ones from §10.1, §10.2)
- [ ] `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `pnpm tsc` clean
- [ ] `pnpm --filter manor-desktop test` green (existing 8 + new ones from §10.3)
- [ ] CI on the feature branch's PR is green
- [ ] Manual smoke (12-step list in §10.4) — every step works
- [ ] PR merged to main
- [ ] Tag `phase-3a-today-tasks-complete` on the merge commit

---

## 12. Open questions

None. Every behaviour is specified.

---

*End of spec. Implementation plan via `superpowers:writing-plans` once Hana approves.*
