# Manor — Project Instructions

Local-first household management app for Hana. Tauri 2 desktop on macOS, Rust (manor-core + manor-app) + React 18 + TypeScript frontend. AGPL-3.0. Single-user, private-by-design.

## Design Context

The canonical design direction lives in [`.impeccable.md`](./.impeccable.md) at the project root. Every Impeccable skill (`impeccable craft`, `shape`, `critique`, `polish`, `audit`, etc.) reads that file before starting work. The summary below mirrors the key decisions so other skills — writing-plans, brainstorming, reviewing — have the context without loading the full file.

### At a glance

- **Direction:** Cottage journal. Cream paper, ink text, ivy + rust accents, ornamental typography, hand-lettered accents for dates and pullquotes. Monospace for numbers.
- **Personality:** cottagecore, handmade, quiet. Journal, not dashboard.
- **Display type:** Marcellus (free) or PP Right Serif Narrow (paid). **Never:** Inter, Fraunces, Playfair, DM Sans, IBM Plex, Space Grotesk, Outfit.
- **Body:** macOS system stack (SF Pro). Keep native.
- **Accents:** Caveat for handwritten moments; Commit Mono / JetBrains Mono for numbers.
- **Palette:** OKLCH-tinted warm neutrals (`paper`, `oat`, `ink`, `ink-soft`, `hairline`) + ivy, rust, butter, plum accents. No pure black/white. No cool grays.
- **Dark panels:** Used only for emphasis (SummaryCard-style pullquotes). Not a theme.
- **Motion:** Minimal. `ease-out-quart`/`ease-out-expo` only. No bounce. Reduced-motion respected.

### Anti-references (what Manor must not look like)

SaaS dashboards, AI tech-bro (cyan glow / purple gradients), consumer iOS cloneware (rounded-card nesting + bounces), corpo productivity (Inter + gray).

### Anti-patterns (enforced by `audit`)

No gradient text, no coloured side-stripe borders, no glassmorphism decoration, no icons-above-every-heading, no reflex fonts (see list in `.impeccable.md`), no pure-black/pure-white, no bounce/elastic easing, no modals when a side panel works.

### Current migration state

Phase 2-3 components carry legacy iMessage tokens (`--imessage-blue` etc.) and Phase 5 introduced a dark `#1a1a2e → #16213e` gradient on SummaryCard. Both should migrate opportunistically toward the OKLCH warm-dark palette when touched. No retrofit-all-at-once — new surfaces commit to Cottage Journal; old ones get refactored when they're in flight anyway.

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
