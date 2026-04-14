# Phase 2 — The Assistant Shell

**Date:** 2026-04-14
**Phase:** v0.1 Heartbeat, Phase 2 (follows `phase-1-foundation`, precedes Today-and-friends)
**Status:** Spec — ready to plan

---

## 1. Summary

Manor gets a presence — a character who lives in the bottom-right corner of the window, listens, remembers, and responds in ephemeral iMessage-style speech bubbles. She runs entirely on local Ollama (`qwen2.5:7b-instruct`). No household features exist yet; Phase 2 builds the substrate every later feature rides on.

Open Manor → her face (`content` expression) sits in the corner holding her phone. Type "hello" into the pill; she shifts to `smile`, a blue bubble with your message pops up, fades in ~3s; she shifts to `questioning` while Ollama thinks, then back to `smile` as she streams a green bubble reply. The reply bubble fades ~7s after streaming completes. If you weren't looking, a small red badge appears above her right shoulder with the unread count. Click her, a drawer slides out with the full rolling conversation. Press ⌘/ from anywhere → pill takes focus immediately.

---

## 2. Goals

- **Living presence.** She feels like a person in the room, not a chat panel.
- **Frictionless talking.** The always-visible pill + ⌘/ hotkey mean you can start a thought from anywhere in the app without clicking first.
- **Ephemeral by default, recoverable on demand.** Conversations flow in the moment (bubbles pop + fade); you never lose anything (full history is one click away).
- **Substrate for everything later.** Schema, IPC contract, and avatar engine are all built such that Phase 3+ features plug in without redesign.

## 3. Non-goals (explicit YAGNI)

| Item | Deferred to |
|---|---|
| Settings UI (Ollama endpoint / model picker) | Later Heartbeat phase or dedicated Settings phase |
| BYO remote keys, tier routing, `remote_call_log` | Providers phase (post-Heartbeat) |
| Today view, CalDAV read, manual tasks, context-aware LLM | Remaining Heartbeat phases |
| Voice input/output | v1.0 Companion (iOS mic integration natural fit) |
| Multiple threads, daily threads, topic threads | Later Heartbeat or early Rhythm — schema already accommodates via `conversation.id` |
| Real proposals triggered by features | Rhythm / Ledger / Hearth / Bones (each skill creates its own proposal kinds) |
| Menu-bar / system notifications when app is backgrounded | Separate "Ambient Manor" phase or v1.0 |
| Conversation summarisation / compaction | When rolling thread exceeds Ollama context limits in practice |
| Tailwind / headless component library | Add when inline styles get tedious (signal: 3+ components repeating the same layout) |
| Avatar blend-shape animation (à la NellFace) | Reserved if NellFace and Manor ever fold together |

## 4. Architecture

### 4.1 Rust (three existing crates, one added module each)

- **`manor-core`** — new `assistant` module. Types: `Conversation`, `Message`, `Proposal`. SQLite access via `rusqlite` + `refinery` migrations. Pure data layer — no Tauri, no Ollama.
- **`manor-app`** — new `assistant` module. Tauri commands, Ollama HTTP client, streaming pipe.
- **`manor-desktop`** — unchanged responsibility; the `register()` seam in `manor-app` covers command registration.

### 4.2 New workspace dependencies

| Crate | Purpose |
|---|---|
| `rusqlite` | SQLite driver (bundled feature) |
| `refinery` (+ `refinery_core`) | SQL migrations |
| `reqwest` | Async HTTP with streaming for Ollama's `/api/chat` NDJSON endpoint |

No `ollama-rs`. Direct `reqwest` against Ollama's HTTP API — fewer layers, smaller dep surface, we own the wire format.

### 4.3 Tauri IPC contract

| Command | Shape | Purpose |
|---|---|---|
| `send_message(content: String, on_event: Channel<StreamChunk>)` | streams chunks | User sends a message; backend streams Ollama's response back chunk-by-chunk |
| `list_messages(limit: u32, offset: u32) -> Vec<Message>` | query | Drawer scrollback |
| `get_unread_count() -> u32` | query | Drives the badge value |
| `mark_seen(message_ids: Vec<i64>)` | write | Called when drawer opens, or when a bubble is hovered/clicked |

**Streaming transport:** Tauri 2 `Channel<T>` (per-invocation typed streams). Chosen over `emit_all` because `Channel` has cleaner lifecycle, no event-bus leakage between components, and is the idiomatic Tauri 2 pattern.

`StreamChunk` variants:
```rust
enum StreamChunk {
  Token(String),         // token or small text fragment from Ollama
  Done,                  // stream complete
  Error(ErrorCode),      // OllamaUnreachable | ModelMissing | Interrupted | Unknown
}
```

### 4.4 Frontend structure (under `apps/desktop/src/`)

```
components/Assistant/
  Assistant.tsx              # composes everything, owns ⌘/ hotkey binding, mounts once at app root
  Avatar.tsx                 # corner-docked, cross-fades expressions
  BubbleLayer.tsx            # ephemeral bubbles with TTL, iMessage colors
  InputPill.tsx              # always-visible pill, ⌘/ focuses it
  UnreadBadge.tsx            # counter above the avatar's right shoulder
  ConversationDrawer.tsx     # full scrollback, opens from avatar on click
lib/assistant/
  state.ts                   # zustand store
  expressions.ts             # AssistantState → asset mapping
  ipc.ts                     # typed wrappers: sendMessage, listMessages, ...
assets/avatars/
  content.png                # idle
  smile.png                  # listening / speaking
  questioning.png            # thinking
  laughing.png               # proactive delight (reserved)
  confused.png               # error / clarifying
```

Assets sourced from `material/manor_face_*.png`, copied during Phase 2 implementation (keeping `material/` as the originals). Open Peeps / Pablo Stanley aesthetic; Apache 2.0 compatible with Manor's AGPL-3.0.

### 4.5 Client state (zustand)

| Slice | Purpose |
|---|---|
| `conversation` | the rolling thread (single row, auto-created on first boot) |
| `messages[]` | persisted chat history, streamed-in as Ollama responds |
| `transientBubbles[]` | UI-only queue of bubbles-with-TTL (max 3 visible) |
| `unreadCount` | derived: assistant messages where `seen=0` |
| `avatarState` | `idle \| listening \| thinking \| speaking \| confused` |
| `drawerOpen` | boolean |

### 4.6 Design system layer

Pablo Stanley aesthetic implied by Open Peeps triggers a small token layer in `apps/desktop/src/styles.css`:

- CSS vars for radii (`--radius-sm: 8px`, `--radius-md: 12px`, `--radius-lg: 16px`)
- Soft shadows (`--shadow-sm`, `--shadow-md`, `--shadow-lg` — tinted, low-opacity)
- Accent palette: warm neutrals + iMessage blue (`#007AFF`) and green (`#34C759`) for bubbles, iMessage red (`#FF3B30`) for the unread badge
- Typography: Nunito via `@fontsource/nunito` — rounded, friendly, matches the illustration style

Components consume the tokens via inline styles (no Tailwind yet).

## 5. Data model

Three tables, introduced in a single `refinery` migration `V1__initial.sql` living at `crates/core/migrations/`.

### 5.1 `conversation`

```sql
CREATE TABLE conversation (
  id         INTEGER PRIMARY KEY,
  created_at INTEGER NOT NULL,
  title      TEXT    NOT NULL DEFAULT 'Manor'
);
```

Phase 2 only ever holds `id=1`. The table exists so Phase 3+ can add daily/topic threading without a schema migration.

### 5.2 `message`

```sql
CREATE TABLE message (
  id              INTEGER PRIMARY KEY,
  conversation_id INTEGER NOT NULL REFERENCES conversation(id),
  role            TEXT    NOT NULL,               -- 'user' | 'assistant' | 'system'
  content         TEXT    NOT NULL,
  created_at      INTEGER NOT NULL,               -- unix millis (ordering matters)
  seen            INTEGER NOT NULL DEFAULT 0,     -- 0/1 boolean
  proposal_id     INTEGER NULL REFERENCES proposal(id)
);

CREATE INDEX idx_message_conversation_created ON message (conversation_id, created_at);
```

`seen` is only meaningful for `assistant` rows. User messages are inserted with `seen=1`.

### 5.3 `proposal` (scaffolded)

```sql
CREATE TABLE proposal (
  id                  INTEGER PRIMARY KEY,
  kind                TEXT    NOT NULL,             -- 'week_plan' | 'meal_plan' | 'chore_swap' | ...
  rationale           TEXT    NOT NULL,             -- markdown
  diff                TEXT    NOT NULL,             -- JSON patch as string
  status              TEXT    NOT NULL DEFAULT 'pending',  -- pending | approved | rejected | applied | partially_applied
  proposed_at         INTEGER NOT NULL,
  applied_at          INTEGER NULL,
  skill               TEXT    NOT NULL,             -- 'calendar', 'chores', ...
  remote_call_log_id  INTEGER NULL                  -- fkey resolved in Providers phase; column exists now to avoid migration later
);
```

No features produce proposals in Phase 2 — table exists so Phase 3+ can `INSERT` without migrating.

### 5.4 Data-access module layout (in `manor-core/src/assistant/`)

```
mod.rs            # pub re-exports
conversation.rs   # get_or_create_default() -> Conversation
message.rs        # insert, list, mark_seen, unread_count
proposal.rs       # types + insert (unused by features yet)
db.rs             # connection pool, migration runner
```

No ORM. Focused `rusqlite` functions, each independently testable against an in-memory SQLite.

## 6. UI/UX contract

### 6.1 Avatar

- Bottom-right corner of the main window
- ~96×96px visible (head + shoulders; phone crops off the bottom edge naturally)
- Fixed position, `z-index: 1000`
- `transform: scaleX(-1)` so she faces **into** the window (toward bubbles and content)
- Edge padding: 16px bottom, 16px right
- `cursor: pointer` — clicking opens the drawer
- Cross-fade 150ms between expressions (no hard cuts)

### 6.2 Bubble layer

- Bubbles float above and to the left of the avatar
- Max 3 visible simultaneously; a 4th arriving immediately fades the oldest
- **User bubbles:** iMessage blue (`#007AFF`), right-aligned, trailing-bottom pointer
- **Assistant bubbles:** iMessage green (`#34C759`), left-aligned, trailing-bottom pointer
- Entry animation: fade + slide-up 8px over 200ms
- Exit animation: fade + slide-up 4px over 300ms
- TTL: user bubbles live 3s after send; assistant bubbles live 7s after streaming completes
- Hovering a bubble pauses its TTL timer; mouse-out restarts the countdown with 3s remaining
- Clicking a bubble opens the drawer scrolled to that message

### 6.3 Unread badge

- Small circle above the avatar's right shoulder (post-mirror that's the left shoulder visually)
- White text on iMessage-red fill (`#FF3B30`)
- Appears when an assistant bubble's TTL completes without being clicked (or the drawer being opened during its visible window) → message stays `seen=0`. Hover pauses the TTL but does **not** mark seen — only click or drawer-open does.
- Shows the count of unread assistant messages, capped at "9+"
- User bubbles never count as missed
- Clears (count returns to 0, badge hides) when drawer opens — `mark_seen` batched on open

### 6.4 Input pill

- Rounded pill, docked just above the avatar (10px gap)
- Width: 220px default, grows to 320px when focused (150ms ease)
- Placeholder: `Say something…`
- Enter sends; Escape blurs; Shift+Enter inserts newline (drawer input supports multi-line; pill collapses long text by scrolling horizontally until sent)
- Submitting clears the pill and enqueues a blue bubble

### 6.5 Global hotkey

- `⌘/` (Cmd+Slash) focuses the pill from anywhere in Manor
- Active only when Manor has window focus (not a system-wide hotkey — pressing ⌘/ in another app does nothing). Implementation likely uses a React-level `keydown` listener on the root, not Tauri's global shortcut API.
- Debounced to 150ms so double-press doesn't misbehave

### 6.6 Conversation drawer

- Slides out from the avatar's corner, takes ~420px wide × full window height
- Animation: 250ms ease-out on open, 200ms ease-in on close
- **Header:** avatar face at 32px showing current expression + "Manor" name + close button (×)
- **Body:** scrollable chat history, most recent at the bottom, iMessage-style bubbles (same colors, but **persistent** — no TTL inside the drawer)
- **Footer:** full-width multi-line input + send button
- Opening the drawer calls `mark_seen(unread_ids)` → badge clears
- Click outside or press Escape to close
- When drawer is open: floating bubbles, pill, and badge are hidden (drawer IS the conversation UI)

### 6.7 Expression state machine

| State | Asset | Trigger |
|---|---|---|
| `idle` | `content.png` | default, no activity for >2s |
| `listening` | `smile.png` | pill is focused or user is mid-typing |
| `thinking` | `questioning.png` | message sent, awaiting first Ollama token |
| `speaking` | `smile.png` | streaming a response (tokens arriving) |
| `confused` | `confused.png` | error state or stream interruption |

`laughing.png` is **reserved**. Triggered when the response content matches a simple delight heuristic (configured emoji like 🎉 🎊, multiple exclamation marks, or a future LLM-tagged `<delight/>` marker). Phase 2 ships the asset and the hook but uses a minimal heuristic (e.g., response contains 🎉 OR contains three or more exclamation marks).

### 6.8 First-run

- No messages yet → drawer shows a single system-rendered greeting ("Hi, I'm Manor. Ask me anything.") with her `smile` expression in the header
- Avatar visible the instant the app opens; pill placeholder invites first input
- No onboarding flow, no tutorial — she's just there

### 6.9 Error states (visible UX)

| Condition | Expression | Bubble text |
|---|---|---|
| Ollama unreachable (`ECONNREFUSED`) | `confused` | "I can't reach Ollama. Is it running?" |
| Model not pulled (Ollama 404 on model) | `confused` | "I need the model `qwen2.5:7b-instruct`. Run `./scripts/install-ollama.sh`." |
| Stream interrupted mid-response | `confused` | brief; partial content retained in drawer |
| Unknown error | `confused` | "Something went wrong. Check the logs." |

Error bubbles carry a red-tinted border to distinguish from normal replies, but follow the same TTL rules.

## 7. Ollama integration

### 7.1 Transport

- HTTP POST to `http://localhost:11434/api/chat` (Ollama's default local endpoint)
- Streaming mode: Ollama responds with NDJSON — one JSON object per line; each line contains either a token or metadata
- Rust side: `reqwest` with `.bytes_stream()` → line-buffered parser → push `StreamChunk::Token` into the Tauri `Channel` per line
- Frontend: channel `onmessage` appends each token to the in-flight assistant message in zustand; the green bubble reads that state reactively

### 7.2 Configuration (Phase 2 hard-coded; settings UI is YAGNI)

- Endpoint: `http://localhost:11434` — constant in `manor-app/src/assistant/ollama.rs`
- Model: `qwen2.5:7b-instruct` — constant in same file
- System prompt: a single constant in `manor-app/src/assistant/prompts.rs`. Establishes Manor's name, calm-household-assistant role, and explicit instruction to NOT act as Nell (persona hygiene for the public AGPL release)

### 7.3 Context window strategy

- Every `send_message` invocation loads the last 20 messages from SQLite and sends them as the chat history to Ollama
- 20 messages ≪ `qwen2.5:7b-instruct`'s 32k context, so no compaction needed
- When the rolling thread exceeds 20 messages, older messages stay in the database but are not sent to the model. Phase 3+ can add summarisation when context pressure becomes real.

### 7.4 Persistence flow

```
1. User submits text via pill
2. INSERT message (role='user', content=<text>, seen=1)
3. INSERT message (role='assistant', content='', seen=0)         ← placeholder row
4. Load last 20 messages, POST to Ollama with stream=true
5. avatar_state → 'thinking'; first token arrives → 'speaking'
6. For each NDJSON line:
     - push StreamChunk::Token to Channel
     - UPDATE message SET content = content || $token WHERE id = <assistant_row>
7. On stream-end: push StreamChunk::Done, final UPDATE, avatar → 'idle' after 2s
8. Bubble TTL starts (7s) for the assistant message
9. If user hovers/clicks within 7s → mark_seen(<assistant_row_id>)
10. Otherwise: TTL expires, seen stays 0, badge increments
```

Inserting the assistant row **before** streaming means a crash mid-stream still leaves a recoverable partial message in the drawer.

## 8. Installing the Ollama model

`scripts/install-ollama.sh` (added in Phase 1) currently installs the Ollama binary. Phase 2 extends it to also pull the default model:

```bash
ollama pull qwen2.5:7b-instruct
```

Idempotent — re-running the script with the model already pulled is a no-op. This keeps `./scripts/bootstrap.sh` as the single-command first-time setup story for new contributors.

## 9. Testing strategy

- **`manor-core` — unit tests.** Each data-access function (insert, list, mark_seen, unread_count) gets an in-memory SQLite test. Migration runs fresh in `#[cfg(test)]` setup.
- **`manor-app` — Ollama integration.** Mock the Ollama HTTP endpoint with `wiremock` (already in Rust ecosystem). Test: token streaming path, error paths (unreachable, model-missing, interrupt mid-stream).
- **Frontend — component tests minimal.** Vitest + React Testing Library for the state machine (state transitions given mocked events) and the bubble TTL logic. No end-to-end browser tests in Phase 2 — the Phase 1 smoke test pattern (manual dev-shell verification) stays.

## 10. Phase boundaries

Phase 2 ships a Manor that **listens + remembers + responds**. She does NOT yet know about calendar, chores, money, meals, or home. Those are the next phases. Phase 2 is the body; Phase 3 onwards gives her things to know about.

**Phase 2 completion criteria:**

- [ ] Workspace builds (`cargo build --workspace` clean)
- [ ] `cargo test --workspace --all-targets` green (new tests for core + app)
- [ ] `pnpm tsc` clean
- [ ] `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] CI on the feature branch's PR is green
- [ ] Manual smoke: open Manor, type "hello", she replies in a green bubble; close and reopen Manor, open drawer, previous conversation is still there
- [ ] Tag `phase-2-assistant-complete` on the merge commit

## 11. Open questions (none block implementation)

- Final `laughing.png` trigger heuristic — whether emoji-only, emoji+exclamation, or small classifier. Ship with the simple emoji+exclamation check; refine after using her for a week.
- Drawer width on small windows — 420px fixed might be too much on a 1100px window (~38% of width). If uncomfortable, cap drawer to `min(420, window_width * 0.45)` during Phase 2 implementation; no user-visible setting.

---

*End of spec. Next step: implementation plan via `superpowers:writing-plans`.*
