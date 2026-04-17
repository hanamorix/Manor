# Phase 6 — Flat Design System Migration

**Landmark:** Phase 6 — Design system migration (Flat-Notion direction)
**Date:** 2026-04-17
**Status:** Design complete, awaiting user review before plan
**Supersedes:** The Cottage Journal direction in the pre-existing `.impeccable.md` (that direction is retired in this landmark).

## Context

Manor's current UI is a mix of legacy iMessage tokens (Phase 2–3), a Phase-5 dark-blue gradient (`#1a1a2e → #16213e`), Nunito as the root font, and bordered rounded cards on every section. An audit scored this 3/10 against the previous design direction and found 58 violations across 10 surfaces (1 P0, 31 P1, 17 P2, 9 P3). The top anti-patterns ranged from banned side-stripe borders on TimeBlocksCard to literal iMessage chat bubbles in the Assistant.

A design revisit produced four candidate directions (Linear, Vercel, Apple HIG, Notion-modern). Hana picked **Apple HIG native DNA with Vercel-flat execution**, then refined to a **Notion-flat with icons** variant (v3 mockup) — document-flat (no cards), Lucide outline icons at every section + action, monochrome (no accent color), auto light/dark via `prefers-color-scheme`, SF Pro everywhere + SF Mono for numbers.

## Decisions

| Question | Decision |
|---|---|
| Overall direction | **Flat-Notion** — document-flat surface, Lucide icon language, monochrome, SF Pro + SF Mono, auto light/dark. Not Cottage Journal. Not a web-font project. |
| Surface treatment | No cards. Sections separated by Lucide icon + normal-case label + subtle hairlines + whitespace. The v2 subtle-card iteration was rejected after v3 showed icons alone give enough definition. |
| Appearance | Both light and dark. Auto-switch via `prefers-color-scheme`. Both palettes defined in parallel in `styles.css`. |
| Accent | Monochrome. Primary button is ink-on-paper in light, paper-on-ink in dark. No blue, no system accent. Weight and contrast do hierarchy. A single muted accent may be added later if the all-mono feel proves too stark in use — not ruled out, not in scope now. |
| Section labels | Normal case, `font-weight: 500`, 12px, muted tint (`#6b6b6b` light / `#9a9a9a` dark). No uppercase tracking. Paired with a 14px outline icon. |
| Page title | 22px, weight 600, letter-spacing `-0.015em`. Page-icon to the left (Lucide, 22px, strokeWidth 1.8). Date beneath the title in SF Mono. |
| Typography | SF Pro (`-apple-system` stack) body; SF Mono (`ui-monospace` stack) for numbers, times, currency, IDs. **No web fonts loaded** — `@fontsource/nunito` removed, no replacement. `font-variant-numeric: tabular-nums` applied everywhere numeric. |
| Icon library | `lucide-react`. 1500+ outline icons, tree-shakeable, `currentColor` fill, consistent strokeWidth 1.8. Used across section headers, buttons, page identifiers, list affordances, navigation sidebar. Single library — no mixing. |
| Radii | 4–6px maximum. Buttons 5px. Cards (where retained — SummaryCard pullquote panel only) 6px. Inputs 5px. Pills removed from general usage. `--radius-pill: 999px` survives only for genuine pill components (if any). |
| Assistant treatment | **HIG-flat sidebar drawer.** Keep the existing `ConversationDrawer` side-panel structural pattern. Remove iMessage bubbles. Messages render as left-aligned paragraphs with small role labels ("You" / "Nell") in muted color, separated by hairlines. No gradient, no tail radii. `BubbleLayer` is deleted. |
| Numbers | SF Mono everywhere they appear — Today events, TimeBlocks, Ledger, SummaryCard, budget counts, day countdowns, renewal days-remaining, transaction IDs. Uniform treatment across the app. |
| Shipping | One landmark, six internal phases, one merge. Branch `feature/phase-6-design-system`. ~25 commits, comparable to Phase 5d. |

## 1. Architecture

### 1.1 Foundation — tokens + type + motion (Phase 1)

All design tokens live in `apps/desktop/src/styles.css`. Components read via `var(--…)` exclusively — no hardcoded hex anywhere in component files.

```css
:root {
  color-scheme: light dark;

  /* Typography */
  --font-body: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
  --font-mono: ui-monospace, "SF Mono", "Commit Mono", "JetBrains Mono", monospace;

  /* Type scale (fixed rem) */
  --text-xs:  0.75rem;   /* 12px — labels, metadata */
  --text-sm:  0.8125rem; /* 13px — list rows */
  --text-md:  0.875rem;  /* 14px — body */
  --text-lg:  1rem;      /* 16px — emphasis */
  --text-xl:  1.375rem;  /* 22px — page titles */
  --text-2xl: 1.75rem;   /* 28px — rare hero */

  /* Radii */
  --radius-sm: 4px;   /* tight surfaces */
  --radius-md: 5px;   /* buttons, inputs */
  --radius-lg: 6px;   /* cards, drawers */

  /* Motion */
  --ease-out: cubic-bezier(0.2, 0.8, 0.2, 1);
  --duration-fast: 120ms;
  --duration-med:  200ms;
}

/* Light palette — applies when system is light or no preference */
:root,
:root[data-theme="light"] {
  --paper:        #fcfcfc;   /* page bg, warm-white */
  --surface:      #ffffff;   /* rare card bg (SummaryCard only) */
  --ink:          #1f1f1f;   /* primary text */
  --ink-soft:     #6b6b6b;   /* labels, metadata, icons */
  --ink-faint:    #8a8a8a;   /* disabled, line-through rest state */
  --hairline:     #efefef;
  --hairline-strong: #e0e0e0; /* button borders, drawer edges */
  --scrim:        rgba(0, 0, 0, 0.25);

  --action-bg:        #1f1f1f; /* primary button bg */
  --action-fg:        #ffffff;
  --action-secondary-border: #e0e0e0;
}

/* Dark palette */
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --paper:        #191919;
    --surface:      #1c1c1e;
    --ink:          #e8e8e8;
    --ink-soft:     #9a9a9a;
    --ink-faint:    #636366;
    --hairline:     #2a2a2a;
    --hairline-strong: #3a3a3a;
    --scrim:        rgba(0, 0, 0, 0.55);

    --action-bg:        #e8e8e8;
    --action-fg:        #191919;
    --action-secondary-border: #3a3a3a;
  }
}

/* Explicit dark override (future setting: user forces dark) */
:root[data-theme="dark"] {
  /* same as @media block above */
}

html, body {
  font-family: var(--font-body);
  font-size: var(--text-md);
  color: var(--ink);
  background: var(--paper);
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
}

.num, time, [data-num] {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

### 1.2 Icon language — `lucide-react`

Add `lucide-react` (~npm dep) with tree-shaking.

Conventions:
- **Page icons** (top of every view): 22px, `strokeWidth={1.8}`, inherits text color
- **Section icons** (next to labels): 14px, `strokeWidth={1.8}`, tinted `--ink-soft`
- **Button icons**: 12px, `strokeWidth={2.2}`, inherits button text color
- **Row affordance icons** (checkboxes, chevrons): 14px, `strokeWidth={1.8}`

Canonical icon-per-surface map (subject to minor adjustment):

| Surface | Icon | Import |
|---|---|---|
| Today (page) | `Sun` | `import { Sun } from 'lucide-react'` |
| Ledger (page) | `Wallet` | `Wallet` |
| Chores (page) | `Sparkles` | `Sparkles` |
| TimeBlocks (page) | `LayoutGrid` | `LayoutGrid` |
| Settings (page) | `Settings` | `Settings` |
| Events (section) | `Calendar` | `Calendar` |
| Chores list (section) | `CheckSquare` | `CheckSquare` |
| Ledger summary (section) | `CircleDollarSign` or `Wallet` | — |
| Tasks (section) | `ListTodo` | `ListTodo` |
| Assistant / Nell | `MessageSquare` | `MessageSquare` |
| Add buttons | `Plus` | `Plus` |
| Close / dismiss | `X` | `X` |
| Expand / collapse | `ChevronDown` / `ChevronRight` | — |
| Checkbox unchecked | `Square` | `Square` |
| Checkbox checked | `CheckSquare` | `CheckSquare` |

All icons use `currentColor` fill so they tint with their parent text colour automatically. No colored icons, no emoji in chrome.

### 1.3 Component treatment — before / after

**Before:** Every view stacks bordered rounded cards at 10–12px radius with small-caps section labels in tiny tracking.

**After:** Every view is a vertical document. Sections are introduced by a row of `[14px outline icon]  Section name` at `--ink-soft`, followed by the list. Rows use 6px vertical padding, 1px `--hairline` bottom borders between items, no left padding (flush with the page margin). Generous 22–26px vertical space between sections. No card backgrounds.

**The one surface that keeps a card-like treatment:** `SummaryCard` in Ledger. It's a deliberate dark pullquote panel. Under the new system it becomes a 6px-radius panel with `background: var(--action-bg)` (dark even in light mode — emphasis inversion) and `color: var(--action-fg)`. No gradient, no shadow, just a solid warm-dark block with the month's total on it.

### 1.4 Assistant redesign

`BubbleLayer.tsx` deleted. `ConversationDrawer.tsx` restyled (not structurally rewritten):

- Panel layout unchanged — right-side drawer, backdrop scrim, tab order unchanged
- Message rendering changes:
  - No bubble shape. No gradient. No tail.
  - Messages render as left-aligned paragraphs with a small `[14px MessageSquare icon] Nell` / `[14px User icon] You` role label above each message
  - Role label color: `--ink-soft`
  - Message body color: `--ink`
  - Vertical spacing: 14px between messages, 6px between label and body
  - A 1px `--hairline` separates every message from the next
- Input field at bottom stays as textarea, restyled: `border-radius: 5px`, `1px solid var(--hairline-strong)`, `background: var(--surface)`, `padding: 8px 10px`, SF Pro, `font-size: var(--text-md)`
- Send button uses Lucide `Send` icon (12px, within a primary-action button, same treatment as other primary actions)

The conversation thread *feels* like a note exchange, not a chat app.

### 1.5 Phases

| Phase | What lands | Rough commits |
|---|---|---|
| **1. Foundation** | `styles.css` token rewrite; kill `@fontsource/nunito`; add `lucide-react`; `color-scheme: light dark`; `prefers-reduced-motion` | 1 |
| **2. Token migration** | Remove `--imessage-*` tokens and all call-sites; replace hardcoded `#fff`/`#000`/raw hex in inline styles with token refs | 2–3 |
| **3. Dark panels** | `SummaryCard`, `ConnectBankDrawer`, `BankAccountRow` retire Phase-5 blues; migrate to `var(--action-bg)` / `var(--paper)` appropriately | 3 |
| **4. Icon language rollout + anti-pattern surgery** | Every view gets Lucide page + section icons; `TimeBlocksCard` side-stripes → kind dot/label; `TransactionRow` rounded-square icon → inline Lucide category icon + emoji removed; `Sidebar.tsx` orange gradient → solid; `SettingsModal` glassmorphism removed; chrome emojis (🔴, ⚠️) → typographic/icon equivalents; `Tabs.tsx` underline color | 6–7 |
| **5. Assistant rebuild** | Delete `BubbleLayer.tsx`; restyle `ConversationDrawer.tsx` (no bubbles, hairline-separated messages, role labels, Lucide icons); delete `typingBounce` keyframe (add opacity-pulse `typingPulse` instead) | 4–5 |
| **6. A11y + hygiene** | Keyboard handlers on `role="button"` divs (ContractsSection); Skip disclosure on ChoresCard right-click; `:focus-visible` replaces `outline:none` (4 sites); width animations → `transform: scaleX()` (4 sites) | 3–4 |

Total: ~22 commits.

## 2. Database / Data

No schema changes. Pure UI refactor.

## 3. Dependencies

| Change | Package |
|---|---|
| Remove | `@fontsource/nunito` (all weights) |
| Add | `lucide-react` (tree-shakes on import) |
| No-op | React, Zustand, Tauri 2 — unchanged |

No new backend dependencies.

## 4. Specific component changes

### 4.1 Page shell (`Today.tsx`, `LedgerView.tsx`, `ChoresView.tsx`, `TimeBlocksView.tsx`, `Settings`, etc.)

Every top-level view gets a uniform page header:

```tsx
<div className="view">
  <header className="page-header">
    <div className="page-title">
      <Sun size={22} strokeWidth={1.8} />
      <h1>Today</h1>
    </div>
    <time className="num">{formatDate(today)}</time>
  </header>

  {/* sections below — no cards, just section introducers + lists */}
</div>
```

CSS for the page header becomes part of `styles.css` as utility classes (or a `.view.css` module — decide in Phase 1).

### 4.2 Section introducer (shared pattern, repeat per section)

```tsx
<section className="section">
  <header className="section-label">
    <Calendar size={14} strokeWidth={1.8} />
    <span>Events</span>
  </header>
  <ul className="rows">
    {events.map((e) => <EventRow key={e.id} {...e} />)}
  </ul>
</section>
```

`.section-label` styling:
```css
.section-label {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--ink-soft);
  font-size: var(--text-xs);
  font-weight: 500;
  margin-bottom: 8px;
}
.section { margin-bottom: 22px; }
.rows > li { padding: 6px 0; border-bottom: 1px solid var(--hairline); }
.rows > li:last-child { border-bottom: none; }
```

### 4.3 Buttons

Primary:
```css
.btn-primary {
  font-family: inherit;
  font-size: var(--text-xs);
  padding: 6px 11px;
  border: none;
  border-radius: var(--radius-md);
  background: var(--action-bg);
  color: var(--action-fg);
  cursor: pointer;
  font-weight: 500;
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
```

Secondary:
```css
.btn-secondary {
  /* same as primary but: */
  background: transparent;
  color: var(--ink);
  border: 1px solid var(--action-secondary-border);
}
```

### 4.4 TransactionRow restyle

Before: 32×32 rounded-square with pastel bg + emoji + label.
After: inline Lucide category icon (14px) + emoji-free merchant name + right-aligned SF Mono amount. Category indicated by icon shape, not coloured background.

```tsx
<li className="transaction-row">
  <ShoppingBag size={14} strokeWidth={1.8} className="cat-icon" />
  <span className="merchant">TESCO STORES</span>
  <time className="num date">7 Apr</time>
  <span className="num amount">-£12.40</span>
</li>
```

Category icons (Lucide): `ShoppingBag` (Groceries), `UtensilsCrossed` (Eating Out), `Bus` (Transport), `Zap` (Utilities), `CreditCard` (Subscriptions), `Pill` (Health), `Shirt` (Shopping), `Music` (Entertainment), `CircleDashed` (Other), `TrendingUp` (Income).

### 4.5 TimeBlocksCard restyle

Before: each block pill has `border-left: 3px solid <kind-color>`.
After: each block pill has a 6px circular dot (Lucide `Circle` filled) to the left of the kind label. Dot color from `--ink` (same for all kinds) + subtle `--hairline-strong` row border.

Kind differentiation moves to Lucide icon per kind:
- `focus` → `Target`
- `admin` → `Inbox`
- `break` → `Coffee`
- `deep` → `Zap`

Icon replaces the color stripe entirely.

### 4.6 Sidebar (Nav) restyle

Before: Manor logo is an orange radial-gradient square; nav icons render as emoji.
After: Manor logo is a solid 4px-radius square with `background: var(--ink)`, an inset Lucide `Home` icon in `--paper`. Nav items use Lucide icons (`LayoutDashboard`, `Wallet`, `Calendar`, `Sparkles`, `Settings`, etc.) in `--ink-soft`, transitioning to `--ink` on active state. No color accent — active state shown by slightly stronger `color` + a 2px left marker in `--ink`.

## 5. Acceptance criteria

Phase 6 is complete when:

- [ ] `@fontsource/nunito` removed from dependencies; no web fonts loaded
- [ ] `lucide-react` is the sole icon library; all emoji-in-chrome replaced
- [ ] `styles.css` defines all palette tokens in OKLCH/hex per this spec and defines both light + dark via `prefers-color-scheme`
- [ ] No component file contains hardcoded `#fff`, `#000`, or raw hex as inline styles (verified via grep)
- [ ] `--imessage-blue`, `--imessage-green`, `--imessage-red` tokens are deleted; no call sites remain
- [ ] `@media (prefers-reduced-motion: reduce)` block exists in `styles.css`; no per-component motion overrides
- [ ] All primary actions use `.btn-primary`; all secondary actions use `.btn-secondary`; no inline-styled buttons remain
- [ ] Every top-level view (`Today`, `LedgerView`, `ChoresView`, `TimeBlocksView`, Settings tabs) uses the page-header pattern with a Lucide page icon
- [ ] `BubbleLayer.tsx` is deleted; `ConversationDrawer.tsx` renders messages as hairline-separated paragraphs with role labels
- [ ] `TimeBlocksCard` has no `border-left: Npx solid` where N > 1 (verified via grep)
- [ ] `TransactionRow` uses Lucide category icons, no pastel rounded-square backgrounds
- [ ] All `outline: none` CSS either has a paired `:focus-visible` rule or is removed
- [ ] Width-animating transitions (`SummaryCard`, `InputPill`, `AiTab`, `Wizard`) converted to `transform: scaleX()`
- [ ] The `typingBounce` keyframe is replaced by `typingPulse` (opacity-only); covered by global reduced-motion block
- [ ] `ContractsSection`'s clickable row has `tabIndex={0}` + `onKeyDown` handler
- [ ] `ChoresCard` exposes a visible "Skip" action alongside the right-click (discoverability fix)
- [ ] The audit (re-run after landing) returns ≤5 findings, all P3
- [ ] Manual acceptance: the app auto-switches light/dark when macOS system preference changes, no reload needed

## 6. Out of scope

- Accent color. The app ships fully monochrome. Adding a muted accent is a potential follow-up once Hana has lived with the monochrome UI for a week.
- Dark-mode-only tuning beyond the parallel palette. Same component shapes in both modes.
- Any new feature work. This is pure refactor.
- Marketing surfaces. Manor has none.
- Custom font files (serif, handwritten, distinctive mono). All deferred.
- Animation polish beyond reducing bounce. Staggered reveals, hero motion — out.
- Ornamental typographic language (`✦`, `❦`, `~ the morning ~`). Retired with Cottage Journal.
- macOS-native chrome changes (title bar style, traffic-light positioning). If needed, a future landmark.

## 7. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Monochrome feels too stark, flagging as "too austere" | Spec allows adding one muted accent token later without structural change (tokens are layered). Make the call after ≥1 week of use. |
| `lucide-react` bundle size | Tree-shakes on named imports — only the icons actually used ship. Expected <30KB added. Verified in Phase 1 build check. |
| Dark mode text contrast on `#191919` background | `#e8e8e8` on `#191919` is ~13:1 contrast — well above WCAG AA (4.5:1) and AAA (7:1). Verified in `audit` re-run. |
| User relies on `--imessage-*` token muscle-memory during migration window | Keep the tokens defined as aliases to new tokens during Phase 2 (transient shim). Remove only after Phase 2 complete. |
| Many view files to touch — partial-merge risk | Single landmark, single PR, one branch. No partial migration visible in `main`. |

## 8. Migration notes from Cottage Journal

The earlier `.impeccable.md` (committed 2026-04-17, earlier today) documented the Cottage Journal direction — cream paper, ink + ivy/rust accents, Marcellus display + Caveat handwritten, ornamental typography. That direction is retired. The same file is rewritten in this landmark to reflect the Flat-Notion direction. No code was ever written against Cottage Journal — the pivot happens before implementation starts.

Phase-5 dark gradient blues and Phase 2-3 iMessage tokens are still migrated exactly the same way — the target palette is just different.
