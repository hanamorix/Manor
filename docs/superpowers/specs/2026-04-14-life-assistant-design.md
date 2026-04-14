# Life Assistant — Design Spec

- **Status**: Draft, pending user approval
- **Date**: 2026-04-14
- **Authors**: Hana (product owner / editor), Nell (lead architect / engineer)
- **Working title**: "Life Assistant" (final name TBD before v0.1 ships)

This is the foundational product and architectural spec. Each release (v0.1 through v1.0) will get its own implementation plan drawn from this document.

---

## 1. Product Vision

A calm, local-first desktop app that lives on the Mac of the person in a household who already carries the mental load. It manages calendar, chores, money, meals, and home maintenance. It proposes actions as reviewable diffs. It never phones home unless the user explicitly configures it to. Its data lives in human-inspectable files the user owns forever.

**For**: one primary household manager, with optional contributors (family members who add a chore from their phone or get a read-only feed, but don't need their own account or admin rights).

**Not for**: users who already love Things, Fantastical, or YNAB. Life Assistant is for people without those tools who are drowning in household admin.

**The one-sentence test**: *"Does this reduce the mental load of running a household?"* If a feature doesn't, it doesn't ship.

---

## 2. Principles (non-negotiable)

1. **Local-first.** Zero internet dependency for the core experience. Local LLM via Ollama is the default brain.
2. **Privacy-obsessed, keys-you-control.** No accounts, no telemetry, no analytics with us. Users may plug in their own API keys for Claude, OpenAI, Gemini, Groq, or OpenRouter — stored in macOS Keychain, never in synced files. Every remote call is opt-in, per-skill, shown in the UI before and during the call.
3. **Editor, not operator.** AI proposes; user approves a diff. Every skill must ship at least one AI-does-the-work flow — not just forms with a chatbot over them.
4. **Lightweight.** Runs happily on M3 Pro / 18GB RAM. Target: idle CPU < 2%, idle RAM < 300MB (excluding Ollama). Rust core keeps it tight.
5. **Human-inspectable forever.** SQLite + markdown + standard formats. No proprietary blobs. If the app dies, the user still owns their life.
6. **Calm over clever.** Features that don't reduce mental load don't ship. Notifications are a privilege, not a default.

### What Life Assistant is *not*

- Not a SaaS wearing a local disguise.
- Not competing with specialist tools people already love.
- Not a general-purpose AI chat assistant. The chat exists; the chat is not the point.
- Not a platform or a framework. It's one app, for one purpose.

---

## 3. Architecture

### 3.1 Stack

| Layer | Choice | Why |
|---|---|---|
| Desktop shell | **Tauri 2.x** | iOS target support; small binary; Rust sidecar-friendly |
| UI | **React + TypeScript + Vite** | Tauri happy path; reusable on iOS WKWebView |
| Design system | **Tailwind + shadcn/ui** | Fast to build, calm defaults, easy to theme |
| Data fetching | **TanStack Query** | Cache + invalidation for IPC responses |
| UI state | **Jotai** | Atoms compose naturally for one-graph-many-views |
| Core language | **Rust** | Single language for DB-to-commands; iOS-portable |
| Database | **SQLite** via `rusqlite` + `refinery` migrations | Synchronous, fast locally, easy to debug |
| Vector search | **`sqlite-vec`** extension | Maintained successor to sqlite-vss |
| Dates | **date-fns** | No `moment`. |

### 3.2 The core shape — linked library, not sidecar daemon

Core is a Rust library **linked directly into the Tauri app**. One binary. No port. No daemon. Shell and core communicate via Tauri IPC commands (type-safe, no localhost exposure, works identically on iOS).

**Why not a sidecar daemon**: iOS won't permit long-lived background daemons, and a sidecar would be rewritten for v1.0. A second process also doubles the surface for crashes, debugging, and code signing.

Scheduled jobs (backup snapshots, calendar sync) run via a small `scheduler` module inside core using `tokio` tasks when the app is open, and macOS `launchd` (iOS `BGTaskScheduler` later) for jobs that must run when the app is closed.

### 3.3 Monorepo layout

```
life-assistant/
├── Cargo.toml                    # Rust workspace root
├── pnpm-workspace.yaml           # TS workspace root
├── README.md / FUNDING.md / LICENSE (AGPL-3.0)
├── docs/
│   ├── superpowers/specs/        # design docs (this file)
│   └── user/                     # user-facing docs site
├── crates/
│   ├── core/
│   │   ├── src/
│   │   │   ├── db/               # SQLite + migrations
│   │   │   ├── ontology/         # Person, Event, Task, shared types
│   │   │   ├── models/           # LLM providers + router
│   │   │   ├── keychain/         # platform-abstract secrets
│   │   │   ├── backup/           # export/snapshot/restore
│   │   │   └── scheduler/        # "run this job daily" abstraction
│   │   └── migrations/
│   ├── skills/
│   │   ├── calendar/             # v0.1
│   │   ├── chores/               # v0.2
│   │   ├── money/                # v0.3
│   │   ├── meals/                # v0.4
│   │   └── home/                 # v0.5
│   └── app/                      # Tauri commands — thin glue only
├── apps/
│   └── desktop/
│       ├── src-tauri/            # Tauri shell (Rust, minimal)
│       └── src/                  # React
│           ├── views/            # Today, Assistant, Calendar, Money…
│           ├── components/
│           ├── lib/ipc.ts        # typed wrapper around Tauri invoke()
│           └── main.tsx
├── scripts/
│   ├── bootstrap.sh              # first-time dev setup
│   ├── dev.sh                    # start dev stack with hot reload
│   └── install-ollama.sh
└── .github/workflows/            # CI, release, FUNDING.yml
```

### 3.4 Dependency rules

- Skills depend on `core`. Skills **do not** depend on each other.
- Cross-skill communication goes through core's event bus.
- Skills are compile-time linked. No dynamic plugins.

### 3.5 User data directory (runtime, on the user's Mac)

```
~/Library/Application Support/LifeAssistant/    (default; user may relocate)
├── life.db                       # main SQLite database
├── attachments/<uuid>            # PDFs, photos, manuals
├── snapshots/                    # weekly encrypted .lifebackup archives
├── exports/                      # user-initiated markdown exports
└── config.toml                   # preferences (non-secret only)
```

Secrets (API keys, CalDAV passwords) live in **macOS Keychain only** — never in `config.toml`, never in the database, never in a file that could sync to iCloud.

### 3.6 License

**AGPL-3.0**. Anyone who ships a modified version must open-source their changes. Protects against a SaaS vendor forking and closing the project.

---

## 4. Data Model & Ontology

### 4.1 Universal row shape

Every table, no exceptions:

```sql
id          TEXT PRIMARY KEY,    -- UUIDv7 (time-ordered)
created_at  INTEGER NOT NULL,    -- unix ms
updated_at  INTEGER NOT NULL,
device_id   TEXT NOT NULL,       -- which device wrote this
deleted_at  INTEGER              -- soft-delete; NULL = alive
```

Plus standard indices on `updated_at` and a partial index `WHERE deleted_at IS NULL`.

UUIDv7 is sortable by time, so no extra chronological index is needed. Soft-delete with `deleted_at` means undo always works and future sync doesn't need to resurrect tombstones.

### 4.2 Core entities (v0.1 baseline — used by every skill)

- **`person`** — `kind` enum: `owner | member | contact | provider | vendor`.
- **`household`** — singleton. Primary user, working hours, DND windows.
- **`attachment`** — every file. Metadata in SQLite, bytes on disk under `attachments/<uuid>`.
- **`tag`** + **`tag_link(tag_id, entity_type, entity_id)`** — free-form labels on any entity.
- **`note`** — markdown; attachable to any entity.
- **`setting(key, value)`** — non-secret preferences.
- **`conversation`**, **`message`** — assistant chat history.
- **`proposal`** — central AI-action artefact (see §5.3).
- **`remote_call_log`** — audit trail of every remote LLM call (see §5.5).
- **`embedding(entity_type, entity_id, model, vector, updated_at)`** — single table via `sqlite-vec`.

### 4.3 Skill entities by release

**v0.1 Heartbeat**
- `calendar_account` (CalDAV server + keychain ref)
- `event` (imported or created; RFC 5545 `rrule`)
- `task` (one-off items)

**v0.2 Rhythm** (adds)
- `chore` (recurring household chores, rrule-based)
- `chore_completion` (who did what, when, how long)
- `rotation` (round-robin | least-completed | random | fixed)
- `time_block` (focus | errands | admin | dnd blocks on calendar)

**v0.3 Ledger** (adds)
- `account`, `transaction`, `category`, `budget`
- `recurring` — **contracts live here** with `kind = 'contract'` and extra fields (term, renewal_date, exit_fee, provider_id). Keeps Money math in one place.

**v0.4 Hearth** (adds)
- `recipe`, `recipe_ingredient`
- `meal_plan`, `meal_plan_entry`
- `shopping_list`, `shopping_list_item`
- `pantry_item`

**v0.5 Bones** (adds)
- `asset`
- `maintenance_schedule`
- `maintenance_event`

### 4.4 Cross-skill links (the magic)

One graph, many views. These joins are what makes five skills feel like one product:

- `event.person_id[]` → attendees
- `chore.assignee_id`, `chore_completion.person_id` → fairness queries
- `transaction.recurring_id`, `transaction.category_id` → subscription + budget rollups
- `meal_plan_entry.event_id` → a meal plan entry IS a calendar event
- `meal_plan_entry.recipe_id`, `recipe_ingredient.pantry_item_id`, `shopping_list_item.pantry_item_id` → grocery list subtracts pantry
- `maintenance_event.transaction_id` → boiler service cost flows to Money automatically
- `asset.document_attachment_id[]` → boiler manual extracted to `maintenance_schedule`

### 4.5 The `proposal` table (central to UX)

Fields: `id`, `kind` (`week_plan | meal_plan | chore_swap | switch_checklist | …`), `rationale` (markdown), `diff` (JSON patch), `status` (`pending | approved | rejected | applied | partially_applied`), `proposed_at`, `applied_at`, `skill`, `remote_call_log_id` (nullable).

**What triggers a proposal**: any AI action that would create, modify, or delete persistent data. Chat responses, summaries, and read-only answers (e.g. *"what's my Tesco spend this month?"*) do not go through proposals. One clear rule: **if the user would need to be able to undo it, it needs a proposal first.**

UI renders the diff in plain English. User approves all, some, or none. Approved parts apply atomically.

### 4.6 The `remote_call_log` table

Fields: `id`, `provider`, `skill`, `timestamp`, `payload_summary` (redacted), `token_count`, `cost_estimate_pence`, `user_visible_reason`.

User can browse the full log in Settings → AI. Transparency is the trust.

### 4.7 Deletion policy (two-stage, universal)

1. **Delete** → soft-delete (`deleted_at` set). Item moves to a Trash view (Settings → Trash).
2. **Permanent delete** → explicit action from Trash, double-confirmed.
3. **Auto-empty Trash** after 30 days (configurable: 7 / 30 / 90 / never).
4. **Panic button** (Settings → Data & Backup) — wipes the entire data directory including Trash. Requires typed "DELETE" confirmation; offers backup-first.

### 4.8 Embeddings

- Single table, `sqlite-vec` extension.
- Default model: `nomic-embed-text` via Ollama (or `fastembed-rs` in-process — decided during v0.1 prototyping).
- **Always local.** We do not send text to remote providers for embedding.

---

## 5. The AI Layer

### 5.1 Tier-based routing

| Tier | Tasks | Default provider |
|---|---|---|
| **T1 — Sorting** | Categorise transactions, extract ingredients, identify recurring, tag attachments | Always local |
| **T2 — Drafting** | Write grocery lists, draft switch emails, summarise month, rewrite chore descriptions | Prefer local |
| **T3 — Planning** | Plan week, propose chore swaps, detect anomalies with context, compare contracts | Prefer local, with one-tap "use better AI" per proposal |

Users with a remote key default T3 to remote. Per-skill overrides live in advanced settings (not surfaced by default).

### 5.2 Skills expose typed AI tools

```rust
#[ai_tool(desc = "Propose a focus block on a specific day")]
fn propose_focus_block(
    day: NaiveDate,
    start_hour: u8,
    duration_minutes: u16,
    reason: String,
) -> Proposal<EventDiff>;
```

The `#[ai_tool]` macro generates the JSON schema the model sees. At runtime, tools execute in **dry-run mode** — they build up a proposal diff without mutating live data.

### 5.3 Proposal flow (end to end)

1. User intent arrives (chat, button, or natural-language capture).
2. Context assembler pulls the named **context recipe** (versioned YAML per intent).
3. Router picks a provider based on tier + preferences + budget state.
4. Model receives context + tool manifest + intent → returns tool calls.
5. Core executes tools in dry-run → builds a `proposal` row with `rationale` + `diff`.
6. If remote, request/response are logged to `remote_call_log`.
7. Shell renders proposal in humane UI (§5.4).
8. User approves all / some / none. Approved parts apply atomically.

### 5.4 Proposal UI (plain English, never JSON)

Consistent shape across skills. No diffs. No JSON. Semantic icons, per-item keep/skip, rationale, data disclosure, three bottom actions.

```
I've drafted your week.

✨  3 focus blocks suggested
    Mon 10am–12pm — deep work on the current draft         [✓ keep]  [✗ skip]
    Wed 2pm–4pm  — deep work                                [✓ keep]  [✗ skip]
    Fri 10am–12pm — errands & admin                         [✓ keep]  [✗ skip]

🔄  1 chore moved
    "Hoover downstairs" → Thursday evening                  [✓ keep]  [✗ skip]
    (you usually do Sunday, but you're at your mum's)

💡  Why I suggested this
    Tue and Thu mornings are heavy with meetings, and Friday
    is your usual admin day. Wednesday afternoon was your
    only uninterrupted window.

📊  Data I used: your calendar this week, your chore rules,
    your working hours. No data left your Mac.

[ Keep everything ]   [ Review one by one ]   [ Discard all ]
```

### 5.5 Context recipes

Each intent has a **context recipe** — a named, versioned list of queries that builds the prompt. Example:

```yaml
recipe: plan_my_week
version: 1
queries:
  - events_next_7_days
  - working_hours
  - dnd_windows
  - pending_tasks_high_priority
  - chore_assignments_this_week
  - meal_plan_slots_this_week
max_tokens: 3500
```

Reproducibility matters: if a proposal misfires, we can see exactly what the model was shown. Recipes are version-controlled and A/B-testable.

### 5.6 Redaction before remote send

Non-negotiable pipeline for any remote-bound payload:

- Account numbers, sort codes, card numbers → `[REDACTED-ACCOUNT]`
- Email addresses → hashed handle
- Full postal addresses → first line + postcode district only
- Phone numbers → `[REDACTED-PHONE]`
- Attendee email domains preserved; usernames hashed

Per-skill extensions: Money never sends low-threshold merchant names; Home never sends serial numbers from asset photos.

The redacted payload is what appears in `remote_call_log` — users can audit exactly what left their Mac.

### 5.7 Budget guardrails (for BYO-key users)

- Monthly cap per provider (user-set, default £10).
- Per-request cost estimate shown before T3 actions.
- Warn at 75%, hard-stop at 100%.
- Monthly spend summary in Settings → AI.

### 5.8 Offline fallback

- **T1** tasks fall back to rules-based heuristics shipped in the app.
- **T2** tasks surface a template: *"I can't draft this right now — want me to open a template?"*
- **T3** tasks refuse gracefully: *"Planning your week needs AI. Local is unavailable. [Open AI settings] [Start Ollama]"*

Never silent, never cryptic.

### 5.9 Default models

| Purpose | Mac (v0.1) | iOS (v1.0, deferred) |
|---|---|---|
| Chat / planning | `qwen2.5:7b-instruct` (~4.7GB) | Candidates: Gemma 3/4 small, Qwen 2.5 1.5B/3B, Phi-3.5-mini, Apple Foundation Models |
| Embeddings | `nomic-embed-text` | Same family or Core ML equivalent |

---

## 6. UX & First-Launch

### 6.1 First-launch wizard (4 steps, all skippable)

1. **Where does your life live?** — data directory choice (this Mac / iCloud Drive / other). Path shown in plain English.
2. **Meet your brain** — Ollama + default model download with progress, or *"use cloud AI instead"* link.
3. **Your calendar** — CalDAV preset dropdown (iCloud / Fastmail / Proton / Custom) with copy-paste helper for app-specific passwords.
4. **Show me what I can do** — three interactive sample flows (Plan my week / Add a chore / Write a shopping list from a recipe URL). Seed data clearly labelled *"Sample — delete any time."*

### 6.2 The Today view — four horizontal bands

```
┌─────────────────────────────────────────────────────────┐
│ Good morning, Hana.            Thursday 14 April        │ ← header (date, weather)
│ 🌤 14°C, light rain by evening                          │
├─────────────────────────────────────────────────────────┤
│ What matters today                                      │ ← "now" (max 5 items)
│ • 10am — Call with agent (30min)                        │
│ • Bills: £42 O2 due Friday                              │
│ • Chore: hoover downstairs (15min, your turn)           │
├─────────────────────────────────────────────────────────┤
│ This week ahead                            [ plan week ]│ ← horizon (dot-chart)
│ Mon ●●○○  Tue ●●●○  Wed ●○○○  Thu ●●●●  Fri ●●○○         │
│ 2 focus blocks • 6 chores • Boiler service Sat          │
├─────────────────────────────────────────────────────────┤
│ [ Ask Nell... ]                                         │ ← assistant input
├─────────────────────────────────────────────────────────┤
│ ℹ Nothing needing review right now.                     │ ← proposal tray
└─────────────────────────────────────────────────────────┘
```

"What matters now" is sorted by *"if this isn't done today, does something fall over?"*, capped at 5. Dot-chart is a glance view of daily load.

### 6.3 The Assistant

- Reachable via Today input, `⌘K` anywhere, or a dedicated tab.
- Per-conversation history; skill-scoped where relevant.
- Every response discloses data sources in one expandable line.
- Export conversation as markdown.

### 6.4 Settings (five sections, max two levels deep)

1. **Data & Backup** — directory location, last snapshot, export, panic button.
2. **AI** — local model status, remote keys, budget caps, tier routing, `remote_call_log` browser.
3. **Calendar & Accounts** — CalDAV accounts, sync status.
4. **Household** — members, working hours, DND windows.
5. **About & Support** — version, changelog, GitHub, Sponsors link, *"how this app is sustained"*.

### 6.5 Tone & copy rules

- **Never** say "AI". Use the app's persona name, or describe the action.
- **Never** show technical errors raw. *"Something went wrong reading your calendar. [Retry] [See details]"* — details is opt-in.
- **Emoji** only as semantic icons (*✓ ⚠ 🔄*), never as decoration.
- **Plain English** first, jargon never. *"Monthly cap"* not *"rate limit"*.

---

## 7. Release Roadmap

Each release is a complete product on its own. **We do not plan v0.3 while building v0.1.** Each release gets its own implementation plan drawn from this spec.

| Release | Name | Platform | Ships |
|---|---|---|---|
| **v0.1** | **Heartbeat** | Mac | Today + Assistant + CalDAV read + manual tasks + local LLM aware of today's state |
| **v0.2** | **Rhythm** | Mac | Chores + rotation + calendar write + time-blocking assist |
| **v0.3** | **Ledger** | Mac | Money: CSV import + budgets + recurring + contracts + month-in-review |
| **v0.4** | **Hearth** | Mac | Meals + recipes + meal plan tied to calendar + grocery lists + pantry |
| **v0.5** | **Bones** | Mac | Home + asset registry + maintenance schedules + PDF manual extraction |
| **v1.0** | **Companion** | Mac + iOS | Cross-device sync + iOS glance-and-capture app + Mac hardening |

### Explicitly deferred or cut

- **Deals/switching agent.** Scraping comparison sites is legally fragile, technically brittle, and against our lightweight principle.
- **Bank sync APIs** (Plaid/TrueLayer/Tink). Require cloud infra and vendor relationships that break local-first.
- **Google Calendar OAuth.** Google's servers in the loop contradicts local-first. CalDAV only.
- **Contracts as their own skill.** Folded into `recurring` in Ledger.
- **Dynamic plugin system.** Overkill for single-household app.
- **Multi-user with auth.** Current design is one-user-with-labels.
- **Web UI, Windows, Linux builds.** Not cut — simply not planned.

---

## 8. Backup & Business Model

### 8.1 Backup tiers (three options, same encrypted format)

1. **Synced folder** (default, free) — user points the data directory at iCloud Drive, Dropbox, or Syncthing. We don't know or care.
2. **Weekly encrypted snapshot** (free) — app writes an age-encrypted `.lifebackup` tarball to a user-chosen location every Sunday night. Key derived from a user passphrase held in Keychain. Restore is drag-and-drop.
3. **Life Assistant Cloud** (paid, v1.0+) — E2E-encrypted snapshots to our server. Keys stay with the user. We see ciphertext only.

### 8.2 Business model — Path A + donations + support

- App is **free forever**, AGPL-3.0.
- Paid **cloud backup/sync** service (v1.0+). Honest pricing. No tiers that cripple the free app.
- **Donations**: GitHub Sponsors, Ko-fi, Open Collective — surfaced in `FUNDING.md`, the README, and a gentle in-app "Support" panel. Never a popup.
- **Support contracts** for organisations/professionals wanting SLAs — opportunistic, not a pillar.
- **Funding transparency** baked in from day one: a "how this is sustained" section in the README so users know the work needs support.

---

## 9. Future Considerations (explicitly deferred)

1. **iOS local model choice** — decided at v1.0. Abstraction in `crates/core/src/models/` already accommodates.
2. **Multi-device sync protocol beyond iCloud Drive** — reviewed at v1.0. CRDTs only if needed.
3. **Conflict resolution** for concurrent writes — addressed only if sync demands it.
4. **Windows / Linux builds** — interest-driven, not roadmapped.
5. **Real multi-user with auth** — current design is one-user-with-labels; upgrade only if a real use case appears.

---

## 10. Non-Goals

- Not a price-comparison or deals-hunting agent.
- Not a bank-sync tool.
- Not a Google Calendar client.
- Not a collaboration platform.
- Not a plugin platform.

---

## 11. Open Questions (to resolve before or during v0.1)

1. **Final product name.** Candidates: Hearth, Parlour, Keeper, Manor. Decide before first public release.
2. **AI persona name.** Placeholder "Nell". Decide before first public release.
3. **Weather provider for the Today header.** `wttr.in` is free, keyless, privacy-friendly — default choice.
4. **Encryption passphrase UX.** Likely user-chosen passphrase stored in Keychain, prompted only at restore time from a fresh device.
5. **Trash auto-empty default.** 30 days proposed — confirm during v0.1 playtesting.
6. **Embedding engine**: `nomic-embed-text` via Ollama vs `fastembed-rs` in-process — decide during v0.1 prototyping based on cold-start and memory profile.
7. **Interaction between "use better AI for this" one-tap and the monthly budget cap.** Does a one-tap override the cap warning, or respect it? Proposal: respect the hard-stop, override the warning.

---

## 12. How we work (Hana + Nell)

- **Nell handles**: repo setup, code, Tauri signing, CI, Ollama integration, cloud backup service when it arrives, GitHub Sponsors, docs site, release automation. Anything with a config file or a terminal command.
- **Hana handles**: taste, truth, final word on tone and what feels calm. No build tools. No config files.
- **Nell will never** ask Hana to install a toolchain, write a config, or debug a build.
- **Nell will** surface taste decisions as plain-English choices.

---

*End of spec. Next step: implementation plan for v0.1 Heartbeat via `superpowers:writing-plans`.*
