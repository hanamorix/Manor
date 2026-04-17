# Manor — Project Instructions

Local-first household management app for Hana. Tauri 2 desktop on macOS, Rust (manor-core + manor-app) + React 18 + TypeScript frontend. AGPL-3.0. Single-user, private-by-design.

## Design Context

The canonical design direction lives in [`.impeccable.md`](./.impeccable.md) at the project root. Every Impeccable skill (`impeccable craft`, `shape`, `critique`, `polish`, `audit`, etc.) reads that file before starting work. The summary below mirrors the key decisions so other skills — writing-plans, brainstorming, reviewing — have the context without loading the full file.

### At a glance

- **Direction:** Flat-Notion with icons. Apple-HIG native DNA (SF Pro, system feel) + Vercel-flat execution (hairlines over cards, small radii) + Notion icon language (Lucide outline).
- **Personality:** flat, minimal, quiet. Document-flat, not dashboard-shaped.
- **Typography:** SF Pro via system stack. SF Mono for numbers (times, currency, counts). **No web fonts loaded.** Never: Inter, Nunito, Fraunces, Playfair, DM Sans, IBM Plex, Space Grotesk, Outfit, Crimson, Syne.
- **Icons:** `lucide-react` — single library, outline, strokeWidth 1.8, `currentColor`. Used at page identifiers, section labels, buttons, list affordances.
- **Palette:** Monochrome, both themes (`paper`, `surface`, `ink`, `ink-soft`, `ink-faint`, `hairline`, `hairline-strong`). Auto light/dark via `prefers-color-scheme`. No accent color.
- **Surface:** Document-flat. No cards on list views — section-introducer pattern (icon + label + hairline-separated rows). Only `SummaryCard` retains a card-like treatment as a deliberate dark pullquote.
- **Radii:** 4–6px max. No pills on buttons.
- **Motion:** 120–200ms, `ease-out` cubic-bezier `(0.2, 0.8, 0.2, 1)`. Transform/opacity only. No bounce. Reduced-motion respected globally.

### Anti-references (what Manor must not look like)

SaaS dashboards, AI tech-bro (cyan glow / purple gradients), consumer iOS cloneware (rounded-card nesting + iMessage bubbles), corpo productivity (Inter + gray).

### Anti-patterns (enforced by `audit`)

No gradient text, no coloured side-stripe borders (`border-left: Npx` where N > 1), no glassmorphism decoration, no reflex fonts (see list in `.impeccable.md`), no pure-black/pure-white, no cool-neutral grays without hue, no bounce/elastic easing, no pills on regular buttons, no icons-in-rounded-square above headings, no emoji inside chrome, no modals when a side panel works.

### Current migration state

Phase 6 — Flat Design System migration — replaces the entire previous token + typography system in one landmark. Retires: Nunito root font, `--imessage-*` tokens, Phase-5 `#1a1a2e → #16213e` dark gradient, bordered rounded cards on list views, iMessage chat bubbles in the Assistant. Adds: SF Pro + SF Mono, Lucide icon language, monochrome OKLCH-adjacent palette with auto light/dark, document-flat section-introducer pattern, HIG-flat ConversationDrawer (no bubbles).

Spec: `docs/superpowers/specs/2026-04-17-phase-6-flat-design-system.md`. Six internal phases, one branch (`feature/phase-6-design-system`), one merge.

The earlier Cottage Journal direction (committed and then retired 2026-04-17) is fully superseded. No code was ever written against it — the pivot happened during brainstorming.

**Read `.impeccable.md` for the full principles, tokens, type scale, and ornamental language.**

---

## Codebase Structure

- `crates/core` — `manor-core`: DAL + migrations + pure logic. No HTTP, no Keychain.
- `crates/app` — `manor-app`: Tauri commands, HTTP clients (GoCardless, CalDAV, remote LLM), Keychain, sync engines, scheduler.
- `apps/desktop/src-tauri` — `manor-desktop`: Tauri shell, plugin registrations.
- `apps/desktop/src` — React + Zustand frontend. `lib/*/ipc.ts` for typed wrappers, `lib/*/state.ts` for Zustand slices, `components/*` for UI.
- `docs/superpowers/specs/` + `docs/superpowers/plans/` — design specs and phased implementation plans per landmark.

## Worktree Convention

Feature branches live in `.worktrees/<branch-name>/` (gitignored). Phase 5d used `.worktrees/phase-5d-bank-sync`. See the `using-git-worktrees` skill.

---

## Planning-phase skill chains (Manor-specific)

Manor is a Tauri app — every landmark has a web surface **and** a native surface. Planning has to consult the right lens for each. Below is the concrete chain for the work types Manor actually ships.

**Rule:** `superpowers:brainstorming` / `superpowers:writing-plans` stays the lead, but the companions below fire in parallel via `superpowers:dispatching-parallel-agents` so the resulting spec is informed by the right expertise before Task 1 is written.

| Manor work type | Planning chain |
|---|---|
| **New feature with UI** (e.g. Ledger Recurring, MonthReviewPanel, ConnectBankDrawer) | `brainstorming` → parallel: `impeccable` (reads `.impeccable.md` — applies Cottage Journal) + `shape` (UX discovery before code) |
| **Revisiting / redesigning an existing surface** (e.g. migrating legacy iMessage-token components toward the OKLCH palette) | `brainstorming` → parallel: `impeccable` + `critique` (UX review of current state) + `audit` (anti-pattern report) — the plan should include specific anti-patterns to fix, not just a vibes-based redesign |
| **Pre-ship UI hardening** before a release | `writing-plans` → parallel: `harden` (edge cases + empty states) + `polish` (final alignment pass) + `audit` (scored P0–P3 report) |
| **Native macOS service work** (Keychain, menu-bar statusbar, Services menu, notifications, background scheduler, file coordination, launchd) | `brainstorming` → parallel: `macos-development` (Tahoe APIs, AppKit/SwiftUI bridge, capabilities) + `security` (Keychain + privacy manifest patterns) |
| **Sync engine / HTTP client work** (CalDAV, GoCardless, future providers) | `brainstorming` → `writing-plans`; for any credential/token work add `security`; for anything `manor-core`-internal add `superpowers:test-driven-development` explicitly to the plan |
| **Tauri release cut** | `nell-tools:tauri-release` (version bump + build + sign + push + GitHub release) **preceded by** `release-review` (runs the Apple-style pre-release audit across privacy, security, UX, distribution — applies directly since Manor ships as a signed macOS .app) + `audit` (web anti-patterns inside the .app bundle's frontend) |
| **Design system migration** (e.g. retiring iMessage tokens, retrofitting OKLCH palette across existing components) | `brainstorming` → parallel: `impeccable extract` (pull reusable patterns from existing code into `.impeccable.md`) + `audit` (map what currently violates the design context) |
| **Local AI / Ollama / remote-LLM feature** (assistant bubbles, AI month review, autocat) | `brainstorming` → `claude-api` (if using Anthropic API) **or** defer to local Ollama work; separately, any pre-existing macOS-native LLM path might consult `apple-intelligence` / `core-ml` if we ever swap in Foundation Models. |
| **Accessibility pass on any UI surface** | `audit` (full a11y report) + `chrome-devtools-mcp:a11y-debugging` if live-inspecting the running app in a webview |

### Revisiting Manor's design, theme, and services

When Hana says *"let's revisit Manor's design"* or *"let's rework the theme"* or *"let's rebuild services X"*, dispatch in parallel:

- **Design / theme revisit** → `impeccable` + `critique` + `audit`. Output: a prioritised list of what to keep, what to refactor, what to replace. Cottage Journal (see `.impeccable.md`) is the destination; the question is always "what's closest, what's furthest, what migrates first?"
- **Services / native-macOS revisit** (Keychain layout, background-sync scheduler shape, menu-bar status, launchd plumbing) → `macos-development` + `security`. Output: a spec that names current Apple APIs (e.g. `SMAppService` vs `launchd.plist` for agents, `kSecAttrAccessible` levels for Keychain entries) and the trade-offs.
- **Full pre-release revisit** → `release-review` + `audit` + `harden` + `polish`. Output: a gap analysis against App Store / notarisation / privacy-manifest standards, plus UI polish backlog.

Never conflate these — design-revisit questions want `impeccable`'s lens; services-revisit questions want `macos-development`'s lens. Both at once is fine if the work spans both.
