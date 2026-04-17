# Phase 6 Flat Design System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate Manor's UI from the legacy iMessage-token + Nunito + bordered-card system to the Flat-Notion direction — SF Pro + SF Mono, Lucide icons, monochrome palette with auto light/dark, document-flat section layouts, HIG-flat ConversationDrawer.

**Architecture:** Token-layer foundation first (`styles.css` rewrite + dep swap), then mechanical token migration across 40 files, then Phase-5 dark gradient migration, then icon language rollout + anti-pattern surgery across every view, then Assistant rebuild (delete `BubbleLayer`, restyle `ConversationDrawer`), then a11y + hygiene pass. All work on `feature/phase-6-design-system` branch in `.worktrees/phase-6-design-system`.

**Tech Stack:** React 18, TypeScript, Vite, Tauri 2, Zustand (no changes). Adds `lucide-react`. Removes `@fontsource/nunito`.

---

## Spec reference

Implements `docs/superpowers/specs/2026-04-17-phase-6-flat-design-system.md`. Updated design bible at `.impeccable.md`.

## File Map

| Path | Status | What it does |
|---|---|---|
| `apps/desktop/src/styles.css` | Rewrite | Tokens (light + dark palette, type scale, radii, motion), global element styles, reduced-motion block |
| `apps/desktop/src/main.tsx` | Modify | Remove nunito imports |
| `apps/desktop/package.json` | Modify | Remove `@fontsource/nunito`, add `lucide-react` |
| `apps/desktop/src/components/Settings/styles.ts` | Modify | Replace hardcoded hex + rgba + legacy tokens with new token refs |
| `apps/desktop/src/components/Nav/Sidebar.tsx` | Modify | Manor logo → solid with Lucide Home icon; nav items → Lucide icons |
| `apps/desktop/src/components/Today/HeaderCard.tsx` | Modify | Apply page-header pattern with Lucide Sun icon |
| `apps/desktop/src/components/Today/Today.tsx` | Modify | Retire card-stack layout; adopt document-flat section pattern |
| `apps/desktop/src/components/Today/EventsCard.tsx` | Modify | Section-introducer pattern, Lucide Calendar icon |
| `apps/desktop/src/components/Today/TasksCard.tsx` | Modify | Section-introducer pattern, Lucide ListTodo icon |
| `apps/desktop/src/components/Today/ChoresCard.tsx` | Modify | Section-introducer + Lucide checkbox icons; add visible Skip button |
| `apps/desktop/src/components/Today/TimeBlocksCard.tsx` | Modify | Remove border-left stripes; add Lucide kind icons |
| `apps/desktop/src/components/Today/ProposalBanner.tsx` | Modify | Token swap, no iMessage tints |
| `apps/desktop/src/components/Today/RenewalAlertsCard.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Today/TaskRow.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Today/Toast.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Today/InputPill.tsx` | Modify | Token swap + width-animation → scaleX |
| `apps/desktop/src/components/Ledger/LedgerView.tsx` | Modify | Page-header pattern with Lucide Wallet |
| `apps/desktop/src/components/Ledger/SummaryCard.tsx` | Modify | Dark gradient → solid `--action-bg`; width transition → scaleX |
| `apps/desktop/src/components/Ledger/MonthReviewPanel.tsx` | Modify | Token swap; in/out color-only indicator gets shape/label cue |
| `apps/desktop/src/components/Ledger/TransactionFeed.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Ledger/TransactionRow.tsx` | Modify | Rounded-square category chip → inline Lucide icon; income hex → token |
| `apps/desktop/src/components/Ledger/BudgetSheet.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Ledger/RecurringSection.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Ledger/ContractsSection.tsx` | Modify | Token swap + keyboard handler on clickable row |
| `apps/desktop/src/components/Ledger/AddRecurringDrawer.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Ledger/AddContractDrawer.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Ledger/CsvImportDrawer.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Chores/ChoresView.tsx` | Modify | Page-header pattern with Lucide Sparkles |
| `apps/desktop/src/components/Chores/ChoreDrawer.tsx` | Modify | Token swap; tab active underline → --ink |
| `apps/desktop/src/components/TimeBlocks/TimeBlocksView.tsx` | Modify | Page-header pattern with Lucide LayoutGrid |
| `apps/desktop/src/components/Settings/SettingsModal.tsx` | Modify | Remove backdrop-filter glassmorphism; token swap |
| `apps/desktop/src/components/Settings/Tabs.tsx` | Modify | Underline color → --ink |
| `apps/desktop/src/components/Settings/AccountRow.tsx` | Modify | Token swap; remove `background: "white"` sites |
| `apps/desktop/src/components/Settings/AddAccountForm.tsx` | Modify | Token swap; `:focus-visible` replaces `outline: none` |
| `apps/desktop/src/components/Settings/CalendarsTab.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Settings/BankAccountsSection.tsx` | Modify | Token swap (minor) |
| `apps/desktop/src/components/Settings/BankAccountRow.tsx` | Modify | Dark palette (`#1a1a2e` / `#3a2a12`) → `--surface` / rust-replacement tint |
| `apps/desktop/src/components/Settings/ConnectBankDrawer.tsx` | Modify | `#16213e` → `--paper`; all dark-mode hex → tokens |
| `apps/desktop/src/components/Settings/AiTab.tsx` | Modify | Token swap + width-animation → scaleX |
| `apps/desktop/src/components/Settings/DataBackupTab.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Settings/HouseholdTab.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Assistant/BubbleLayer.tsx` | **Delete** | iMessage bubbles retired |
| `apps/desktop/src/components/Assistant/ConversationDrawer.tsx` | Modify | Restyle: hairline-separated messages with role labels, no bubbles |
| `apps/desktop/src/components/Assistant/UnreadBadge.tsx` | Modify | Token swap |
| `apps/desktop/src/components/Wizard/Wizard.tsx` | Modify | Token swap; progress width animation → scaleX; `#fff` → `--paper` |
| `apps/desktop/src/components/Wizard/styles.ts` | Modify | Replace hardcoded styles with new tokens |
| `apps/desktop/src/components/Safety/*.tsx` | Modify | Token swap |
| `apps/desktop/src/App.tsx` | Modify | Loading-state `#888` → `--ink-soft` |
| `apps/desktop/src/components/Today/SampleDataBanner.tsx` | Modify | `#d90` → token |

A new shared file:

| Path | Status | What it does |
|---|---|---|
| `apps/desktop/src/lib/ui/PageHeader.tsx` | Create | Shared page-header component: `[icon] [title]` + optional subtitle / metadata |
| `apps/desktop/src/lib/ui/SectionLabel.tsx` | Create | Shared section-introducer: `[icon] [label]` |
| `apps/desktop/src/lib/ui/Button.tsx` | Create | `.btn-primary` / `.btn-secondary` variants with Lucide icon support |

---

## Task 1: Foundation — styles.css + deps

**Files:**
- Modify: `apps/desktop/src/styles.css` (full rewrite)
- Modify: `apps/desktop/src/main.tsx` (remove nunito imports)
- Modify: `apps/desktop/package.json` (swap deps)

- [ ] **Step 1: Swap dependencies**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop
npm uninstall @fontsource/nunito
npm install lucide-react
```

- [ ] **Step 2: Rewrite `apps/desktop/src/styles.css`**

Replace the entire file contents with:

```css
:root {
  color-scheme: light dark;

  /* Typography */
  --font-body: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
  --font-mono: ui-monospace, "SF Mono", "Commit Mono", "JetBrains Mono", monospace;

  /* Type scale */
  --text-xs:  0.75rem;
  --text-sm:  0.8125rem;
  --text-md:  0.875rem;
  --text-lg:  1rem;
  --text-xl:  1.375rem;
  --text-2xl: 1.75rem;

  /* Radii */
  --radius-sm: 4px;
  --radius-md: 5px;
  --radius-lg: 6px;

  /* Motion */
  --ease-out: cubic-bezier(0.2, 0.8, 0.2, 1);
  --duration-fast: 120ms;
  --duration-med:  200ms;
}

/* Light palette */
:root,
:root[data-theme="light"] {
  --paper:             #fcfcfc;
  --surface:           #ffffff;
  --ink:               #1f1f1f;
  --ink-soft:          #6b6b6b;
  --ink-faint:         #8a8a8a;
  --hairline:          #efefef;
  --hairline-strong:   #e0e0e0;
  --scrim:             rgba(0, 0, 0, 0.25);

  --action-bg:               #1f1f1f;
  --action-fg:               #ffffff;
  --action-secondary-border: #e0e0e0;
}

/* Dark palette — auto */
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --paper:             #191919;
    --surface:           #1c1c1e;
    --ink:               #e8e8e8;
    --ink-soft:          #9a9a9a;
    --ink-faint:         #636366;
    --hairline:          #2a2a2a;
    --hairline-strong:   #3a3a3a;
    --scrim:             rgba(0, 0, 0, 0.55);

    --action-bg:               #e8e8e8;
    --action-fg:               #191919;
    --action-secondary-border: #3a3a3a;
  }
}

/* Explicit dark override — future setting */
:root[data-theme="dark"] {
  --paper:             #191919;
  --surface:           #1c1c1e;
  --ink:               #e8e8e8;
  --ink-soft:          #9a9a9a;
  --ink-faint:         #636366;
  --hairline:          #2a2a2a;
  --hairline-strong:   #3a3a3a;
  --scrim:             rgba(0, 0, 0, 0.55);
  --action-bg:               #e8e8e8;
  --action-fg:               #191919;
  --action-secondary-border: #3a3a3a;
}

/* ============================================ */
/* SHIM — legacy iMessage token aliases.        */
/* Allows unmigrated components to keep         */
/* compiling during Phase 2. Removed in Task 5. */
/* ============================================ */
:root {
  --imessage-blue:  var(--action-bg);
  --imessage-green: var(--action-bg);
  --imessage-red:   var(--ink);
  --radius-pill:    999px; /* still used by a few genuine pills */
  --shadow-sm: 0 1px 2px rgba(20, 20, 30, 0.06), 0 1px 3px rgba(20, 20, 30, 0.04);
  --shadow-md: 0 4px 10px rgba(20, 20, 30, 0.08), 0 2px 6px rgba(20, 20, 30, 0.05);
  --shadow-lg: 0 12px 28px rgba(20, 20, 30, 0.12), 0 4px 10px rgba(20, 20, 30, 0.08);
}

html, body {
  font-family: var(--font-body);
  font-size: var(--text-md);
  color: var(--ink);
  background: var(--paper);
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
  margin: 0;
}

#root { min-height: 100vh; }

.num, time, [data-num] {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

/* Existing keyframes retained unchanged for now; typingBounce replaced in Phase 5 */
@keyframes bubbleIn     { from { opacity: 0; transform: translateY(4px); } to { opacity: 1; transform: translateY(0); } }
@keyframes drawerIn     { from { opacity: 0; transform: translateX(24px); } to { opacity: 1; transform: translateX(0); } }
@keyframes bannerIn     { from { opacity: 0; transform: translateY(-8px); } to { opacity: 1; transform: translateY(0); } }
@keyframes settingsIn   { from { opacity: 0; transform: scale(0.98); } to { opacity: 1; transform: scale(1); } }
@keyframes typingBounce { 0%, 60%, 100% { transform: translateY(0); } 30% { transform: translateY(-5px); } }

/* Reduced motion — non-negotiable */
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

- [ ] **Step 3: Update `apps/desktop/src/main.tsx`**

Delete these three lines at the top:

```tsx
import "@fontsource/nunito/400.css";
import "@fontsource/nunito/600.css";
import "@fontsource/nunito/700.css";
```

Leave the rest of the file (`import App`, `import "./styles.css"`, `ReactDOM.createRoot` etc.) untouched.

- [ ] **Step 4: Verify build**

Run: `cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -5`
Expected: no type errors.

Run: `cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system && cargo build --workspace 2>&1 | tail -3`
Expected: clean build (unchanged — no Rust touched).

Run: `cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npx vite build 2>&1 | tail -10`
Expected: frontend bundles successfully.

- [ ] **Step 5: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system
git add apps/desktop/src/styles.css apps/desktop/src/main.tsx apps/desktop/package.json apps/desktop/package-lock.json
git commit -m "feat(ui): Phase 6 foundation — tokens, type stack, reduced motion, lucide-react"
```

---

## Task 2: Shared UI primitives — PageHeader, SectionLabel, Button

**Files:**
- Create: `apps/desktop/src/lib/ui/PageHeader.tsx`
- Create: `apps/desktop/src/lib/ui/SectionLabel.tsx`
- Create: `apps/desktop/src/lib/ui/Button.tsx`
- Create: `apps/desktop/src/lib/ui/index.ts` (barrel)

- [ ] **Step 1: Write `PageHeader.tsx`**

Create `apps/desktop/src/lib/ui/PageHeader.tsx`:

```tsx
import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface Props {
  icon: LucideIcon;
  title: string;
  subtitle?: ReactNode;
  meta?: ReactNode;
}

export function PageHeader({ icon: Icon, title, subtitle, meta }: Props) {
  return (
    <header
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "baseline",
        marginBottom: 18,
        paddingBottom: 18,
      }}
    >
      <div>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <Icon size={22} strokeWidth={1.8} color="var(--ink)" />
          <h1
            style={{
              fontSize: "var(--text-xl)",
              fontWeight: 600,
              letterSpacing: "-0.015em",
              margin: 0,
              color: "var(--ink)",
            }}
          >
            {title}
          </h1>
        </div>
        {subtitle && (
          <div
            className="num"
            style={{
              fontSize: "var(--text-xs)",
              color: "var(--ink-soft)",
              marginTop: 2,
              marginLeft: 32,
            }}
          >
            {subtitle}
          </div>
        )}
      </div>
      {meta && (
        <div style={{ fontSize: "var(--text-xs)", color: "var(--ink-soft)" }}>
          {meta}
        </div>
      )}
    </header>
  );
}
```

- [ ] **Step 2: Write `SectionLabel.tsx`**

```tsx
import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface Props {
  icon: LucideIcon;
  children: ReactNode;
  action?: ReactNode;
}

export function SectionLabel({ icon: Icon, children, action }: Props) {
  return (
    <header
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: 8,
        marginBottom: 8,
        color: "var(--ink-soft)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <Icon size={14} strokeWidth={1.8} />
        <span style={{ fontSize: "var(--text-xs)", fontWeight: 500 }}>
          {children}
        </span>
      </div>
      {action}
    </header>
  );
}
```

- [ ] **Step 3: Write `Button.tsx`**

```tsx
import type { LucideIcon } from "lucide-react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  icon?: LucideIcon;
  children: ReactNode;
}

export function Button({
  variant = "primary",
  icon: Icon,
  children,
  style,
  ...rest
}: Props) {
  const base = {
    fontFamily: "inherit",
    fontSize: "var(--text-xs)",
    padding: "6px 11px",
    borderRadius: "var(--radius-md)",
    cursor: "pointer",
    fontWeight: 500,
    display: "inline-flex",
    alignItems: "center",
    gap: 5,
  } as const;

  const variantStyle =
    variant === "primary"
      ? {
          border: "none",
          background: "var(--action-bg)",
          color: "var(--action-fg)",
        }
      : {
          border: "1px solid var(--action-secondary-border)",
          background: "transparent",
          color: "var(--ink)",
        };

  return (
    <button {...rest} style={{ ...base, ...variantStyle, ...style }}>
      {Icon && <Icon size={12} strokeWidth={2.2} />}
      {children}
    </button>
  );
}
```

- [ ] **Step 4: Write barrel `index.ts`**

```ts
export { PageHeader } from "./PageHeader";
export { SectionLabel } from "./SectionLabel";
export { Button } from "./Button";
```

- [ ] **Step 5: Verify TS**

Run: `cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -5`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/lib/ui/
git commit -m "feat(ui): shared PageHeader + SectionLabel + Button primitives"
```

---

## Task 3: Legacy token audit script

**Files:**
- Create: `apps/desktop/scripts/audit-legacy-tokens.sh`

This script is the verification oracle for Phase 2. Re-run after each migration commit.

- [ ] **Step 1: Create the script**

```bash
mkdir -p apps/desktop/scripts
```

Create `apps/desktop/scripts/audit-legacy-tokens.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

echo "=== Legacy tokens remaining ==="
echo

echo "-- --imessage-* refs --"
grep -rn "imessage-blue\|imessage-green\|imessage-red" apps/desktop/src --include="*.tsx" --include="*.ts" --include="*.css" 2>/dev/null | grep -v "audit-legacy-tokens\|styles\.css:" | wc -l | tr -d ' '

echo
echo "-- Phase-5 dark hex --"
grep -rn "#1a1a2e\|#16213e\|#3a2a12" apps/desktop/src --include="*.tsx" --include="*.ts" 2>/dev/null | wc -l | tr -d ' '

echo
echo "-- Pure #fff / #000 in inline styles --"
grep -rn '"#fff"\|"#000"\|"#FFF"\|"#000000"\|"#ffffff"\|"white"\|"black"' apps/desktop/src --include="*.tsx" 2>/dev/null | grep -v "audit-legacy-tokens" | wc -l | tr -d ' '

echo
echo "-- Cool rgba neutrals --"
grep -rn "rgba(0,\s*0,\s*0" apps/desktop/src --include="*.tsx" --include="*.ts" 2>/dev/null | wc -l | tr -d ' '

echo
echo "-- Nunito references --"
grep -rn "Nunito\|@fontsource/nunito" apps/desktop/src --include="*.tsx" --include="*.ts" --include="*.css" 2>/dev/null | wc -l | tr -d ' '

echo
echo "-- border-left > 1px --"
grep -rn "borderLeft.*[2-9]px solid\|border-left.*[2-9]px solid" apps/desktop/src --include="*.tsx" --include="*.ts" --include="*.css" 2>/dev/null | wc -l | tr -d ' '

echo
echo "-- backdrop-filter decorative --"
grep -rn "backdropFilter\|backdrop-filter" apps/desktop/src --include="*.tsx" --include="*.ts" --include="*.css" 2>/dev/null | wc -l | tr -d ' '
```

- [ ] **Step 2: Make executable, run baseline**

```bash
chmod +x apps/desktop/scripts/audit-legacy-tokens.sh
./apps/desktop/scripts/audit-legacy-tokens.sh
```

Expected baseline counts (pre-migration):
- `--imessage-*` refs: ~50+
- Phase-5 dark hex: ~10
- Pure `#fff` / `#000`: ~30+
- Cool rgba neutrals: ~15+
- Nunito references: 0 (already removed)
- `border-left > 1px`: ~1 (TimeBlocksCard)
- `backdrop-filter`: ~1 (SettingsModal)

Record these numbers in the commit message.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/scripts/audit-legacy-tokens.sh
git commit -m "feat(tools): legacy-token audit script for migration verification"
```

---

## Task 4: Token migration — Today surface (pass 1)

**Files (all under `apps/desktop/src/components/Today/`):**
- Modify: `EventsCard.tsx`
- Modify: `TasksCard.tsx`
- Modify: `ProposalBanner.tsx`
- Modify: `RenewalAlertsCard.tsx`
- Modify: `TaskRow.tsx`
- Modify: `Toast.tsx`
- Modify: `InputPill.tsx`
- Modify: `HeaderCard.tsx`
- Modify: `SampleDataBanner.tsx`

Mechanical find-replace pass — the shim in `styles.css` keeps existing calls compiling, but we swap to semantic tokens now so Phase 4 (icon rollout) touches each file once cleanly.

- [ ] **Step 1: Apply substitutions to each file**

For every file listed above, apply these literal replacements in inline style strings and style objects:

| Find | Replace with |
|---|---|
| `var(--imessage-blue)` | `var(--ink)` |
| `var(--imessage-green)` | `var(--ink)` |
| `var(--imessage-red)` | `var(--ink)` |
| `"#fff"` / `"#ffffff"` | `"var(--paper)"` (for backgrounds) OR `"var(--action-fg)"` (for text on primary buttons) |
| `"#000"` / `"#000000"` | `"var(--ink)"` |
| `"white"` | `"var(--paper)"` (bg) / `"var(--action-fg)"` (button text) |
| `"black"` | `"var(--ink)"` |
| `"#aaa"` / `"#bbb"` / `"#888"` | `"var(--ink-soft)"` |
| `"#ccc"` / `"#ddd"` | `"var(--hairline-strong)"` |
| `"#eee"` / `"#f5f5f5"` | `"var(--hairline)"` |
| `"rgba(0,0,0,0.55)"` / `"rgba(0,0,0,0.65)"` | `"var(--ink-soft)"` |
| `"rgba(0,0,0,0.35)"` | `"var(--ink-faint)"` |
| `"rgba(20,20,30,0.08)"` | `"var(--hairline)"` |
| `"rgba(20,20,30,0.12)"` or similar scrims | `"var(--scrim)"` |
| `"#d90"` | `"var(--ink)"` (SampleDataBanner — monochrome, banner bg does the work) |

**Special cases per file:**

- `InputPill.tsx:33` — `transition: "width 150ms ease"` → leave for Task 21; token pass only
- `Toast.tsx:26` — `color: "white"` on dark bg → `color: "var(--action-fg)"`, wrap bg in `var(--action-bg)` (dark block stays dark in light theme — the toast is a deliberate inverted surface)
- `HeaderCard.tsx:54,55,67` — `"rgba(0,0,0,0.55)"` → `"var(--ink-soft)"`
- `EventsCard.tsx:80` — event times using `--imessage-blue` — stays `var(--ink)` but add `fontFamily: "var(--font-mono)"` (task list: anticipate Phase 4 but put the change here while you're in the file)

- [ ] **Step 2: Verify TS + build**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
```
Expected: clean.

- [ ] **Step 3: Run audit oracle**

```bash
./apps/desktop/scripts/audit-legacy-tokens.sh
```
Expected: `--imessage-*` count and pure-hex counts drop by ~15 from baseline.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Today/
git commit -m "refactor(ui): token migration — Today surface"
```

---

## Task 5: Token migration — Ledger surface (pass 1)

**Files (all under `apps/desktop/src/components/Ledger/`):**
- Modify: `MonthReviewPanel.tsx`
- Modify: `TransactionFeed.tsx`
- Modify: `TransactionRow.tsx`
- Modify: `BudgetSheet.tsx`
- Modify: `RecurringSection.tsx`
- Modify: `ContractsSection.tsx`
- Modify: `AddRecurringDrawer.tsx`
- Modify: `AddContractDrawer.tsx`
- Modify: `CsvImportDrawer.tsx`

- [ ] **Step 1: Apply the same substitution table as Task 4**

Use the same find/replace mappings. Extra per-file specifics:

- `MonthReviewPanel.tsx:49,55,65` — income/outgoing color-only indicator: leave token swap here (all → `var(--ink)`) but ADD a prefix character: income gets `"+ "` prefix, outgoing gets `"− "` prefix (Unicode minus `U+2212`, not hyphen). Net gets `"= "`. Enforces shape-cue accessibility requirement.
- `TransactionFeed.tsx:105` — `"#aaa"` → `"var(--ink-soft)"`
- `TransactionRow.tsx:82` — `"#bbb"` → `"var(--ink-soft)"`
- `TransactionRow.tsx:93` — `"#2BB94A"` income color → `"var(--ink)"`, but also add `fontWeight: 600` to compensate for lost color differentiation (shape/weight cue)
- `TransactionRow.tsx:4–15` `CATEGORY_COLORS` pastel table — DELETE entirely (Task 13 replaces with Lucide icons). For now replace every usage with `"var(--hairline)"` as a placeholder; icon rollout in Task 13 will remove the usage.
- `AddRecurringDrawer.tsx:310` / `AddContractDrawer.tsx:367` / `CsvImportDrawer.tsx:408` — primary button `color: "#fff"` → `color: "var(--action-fg)"`, `background: "var(--imessage-blue)"` → `background: "var(--action-bg)"`

- [ ] **Step 2: TS check + audit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
./apps/desktop/scripts/audit-legacy-tokens.sh
```
Expected: `--imessage-*` count drops further.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/components/Ledger/
git commit -m "refactor(ui): token migration — Ledger surface; income/outgoing gets +/− shape cue"
```

---

## Task 6: Token migration — Settings, Chores, TimeBlocks, Wizard, Nav, Safety, App, Assistant

**Files:**
- Modify: `apps/desktop/src/components/Settings/*.tsx` + `styles.ts` (13 files)
- Modify: `apps/desktop/src/components/Chores/*.tsx` (2 files)
- Modify: `apps/desktop/src/components/TimeBlocks/*.tsx` (2 files)
- Modify: `apps/desktop/src/components/Wizard/*.tsx` + `styles.ts` (6 files)
- Modify: `apps/desktop/src/components/Nav/Sidebar.tsx` (1 file)
- Modify: `apps/desktop/src/components/Safety/*.tsx` (3 files)
- Modify: `apps/desktop/src/components/Assistant/UnreadBadge.tsx` (1 file)
- Modify: `apps/desktop/src/App.tsx`

Same substitution table as Task 4. Work through each directory in order, commit per directory.

- [ ] **Step 1: Settings — migrate `styles.ts` first**

`apps/desktop/src/components/Settings/styles.ts` is imported by ~8 other Settings files. Migrate it first:

- `TEXT_MUTED: "rgba(0,0,0,0.55)"` → `TEXT_MUTED: "var(--ink-soft)"`
- `TEXT_SECONDARY: "rgba(0,0,0,0.65)"` → `TEXT_SECONDARY: "var(--ink-soft)"`
- `settingsCard: { background: "#fff", ... }` → `background: "var(--surface)"`
- `settingsListRow: { background: "#fff", ... }` → `background: "var(--surface)"`
- `dangerButton: { background: "var(--imessage-red)", color: "#fff", ... borderRadius: "var(--radius-pill)" }` → `background: "var(--ink)"`, `color: "var(--action-fg)"`, `borderRadius: "var(--radius-md)"`
- Any remaining pure `#fff` / `#000` → tokens per table

- [ ] **Step 2: Settings — per-file migrations**

Apply substitution table + these specifics:
- `SettingsModal.tsx:37` — `backdropFilter: "blur(2px)"` → DELETE the property; keep the scrim
- `Tabs.tsx:34` — `borderBottom: active ? "2px solid var(--imessage-blue)" : ...` → `"2px solid var(--ink)"`
- `AccountRow.tsx:89,136,151` — `background: "white"` → `"var(--surface)"`
- `ConnectBankDrawer.tsx:104` — `background: "#16213e"` → `"var(--paper)"`, `color` swaps to `"var(--ink)"`
- `ConnectBankDrawer.tsx:355` — `color: "#000"` → `"var(--ink)"`
- `BankAccountRow.tsx:34` — `background: expired ? "#3a2a12" : "#1a1a2e"` → `expired ? "var(--hairline)" : "var(--surface)"` with `border: "1px solid var(--hairline-strong)"` added and an inline rust-ish emphasis handled via text weight, NOT color
- `AiTab.tsx:198` — width animation stays for now (Task 22)

- [ ] **Step 3: Chores, TimeBlocks, Wizard, Nav, Safety**

Apply the standard substitution table. Notable:
- `ChoreDrawer.tsx:64,69,73` — tab color / button — all → `var(--ink)`
- `ChoresView.tsx:44,49` — same
- `TimeBlocksCard.tsx` — keep `borderLeft: "3px solid ..."` untouched for now (Task 12 replaces with kind icons)
- `Wizard.tsx:54` — width animation → defer (Task 22)
- `Wizard.tsx:76` — `background: "#fff"` → `"var(--surface)"`
- `Wizard/styles.ts` — primary button → `var(--ink)` bg / `var(--action-fg)` text
- `Sidebar.tsx:23` — `background: "linear-gradient(135deg, #FFC15C 0%, #FF8800 100%)"` → `"var(--ink)"` (solid; icon swap happens in Task 14)
- `Sidebar.tsx:47` — nav icon colors → `active ? "var(--ink)" : "var(--ink-faint)"`

- [ ] **Step 4: App.tsx + remaining**

- `App.tsx:58` — `color: "#888"` → `"var(--ink-soft)"`
- `UnreadBadge.tsx` — any color refs → tokens

- [ ] **Step 5: TS + audit + commit**

Commit per subdirectory for focused history:

```bash
git add apps/desktop/src/components/Settings/
git commit -m "refactor(ui): token migration — Settings surface"

git add apps/desktop/src/components/Chores/ apps/desktop/src/components/TimeBlocks/
git commit -m "refactor(ui): token migration — Chores + TimeBlocks"

git add apps/desktop/src/components/Wizard/
git commit -m "refactor(ui): token migration — Wizard"

git add apps/desktop/src/components/Nav/ apps/desktop/src/components/Safety/ apps/desktop/src/components/Assistant/UnreadBadge.tsx apps/desktop/src/App.tsx
git commit -m "refactor(ui): token migration — Nav, Safety, App shell"
```

After each commit, run the audit oracle. The `--imessage-*` count should be near zero by the end. Pure-hex count should approach zero.

- [ ] **Step 6: Final verification — full audit**

```bash
./apps/desktop/scripts/audit-legacy-tokens.sh
```

Expected after all Task 6 subcommits:
- `--imessage-*` refs: 0–3 (stragglers only)
- Phase-5 dark hex: 3 (SummaryCard still carries it — Task 7)
- Pure `#fff`/`#000`: 0
- Cool rgba: 0
- Nunito: 0
- `border-left > 1px`: 1 (TimeBlocksCard — Task 12)
- `backdrop-filter`: 0

---

## Task 7: Dark panels — SummaryCard migration

**Files:**
- Modify: `apps/desktop/src/components/Ledger/SummaryCard.tsx`

- [ ] **Step 1: Replace the gradient logic**

At `SummaryCard.tsx:9,11–13`, find the three gradient states. They currently look something like:

```tsx
const gradient = pct < 75
  ? "linear-gradient(135deg, #1a1a2e 0%, #16213e 100%)"
  : pct < 100
  ? "linear-gradient(135deg, #2e1a1a 0%, #3e2116 100%)"
  : "linear-gradient(135deg, #2e1a2e 0%, #3e1616 100%)";
```

Replace with a single solid:

```tsx
const cardBg = "var(--action-bg)";
const cardFg = "var(--action-fg)";
```

Apply `background: cardBg`, `color: cardFg` to the card root. The month-total number gets `fontFamily: "var(--font-mono)", fontVariantNumeric: "tabular-nums"`, `fontWeight: 600`.

- [ ] **Step 2: Replace the `progressColor()` helper**

Currently returns `"white"` for normal state. Change to:

```tsx
const progressColor = () => "var(--action-fg)";
```

- [ ] **Step 3: Migrate the progress bar**

At `SummaryCard.tsx:88`, `transition: "width 0.3s"` → defer width fix to Task 22 (one place, batched). Leave the rest of the progress styling using `cardFg` for the fill.

- [ ] **Step 4: Replace emoji-in-chrome pills**

At `SummaryCard.tsx:112`, alert-category pills currently use `🔴` / `⚠️` as emoji content. Replace with text-only pills: the category name becomes the content; the pill state is signaled via `color` alone between `var(--action-fg)` (normal) and `rgba(var(--action-fg), 0.7)` (muted). The over/under distinction is already conveyed by the progress-bar fill and the summary numeric text weight.

- [ ] **Step 5: Text content using `color: "white"`**

Replace all `color: "white"` in this file with `color: "var(--action-fg)"` (same for text-on-card elements).

- [ ] **Step 6: TS + audit + commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
./apps/desktop/scripts/audit-legacy-tokens.sh
```
Expected: Phase-5 dark hex count now 2 (ConnectBankDrawer + BankAccountRow).

```bash
git add apps/desktop/src/components/Ledger/SummaryCard.tsx
git commit -m "refactor(ledger): SummaryCard migrates dark gradient to monochrome action bg + shape-cue pills"
```

---

## Task 8: Dark panels — ConnectBankDrawer + BankAccountRow final migration

**Files:**
- Modify: `apps/desktop/src/components/Settings/ConnectBankDrawer.tsx`
- Modify: `apps/desktop/src/components/Settings/BankAccountRow.tsx`

Most of the work was done in Task 6. Final sweep here.

- [ ] **Step 1: ConnectBankDrawer — pick up any remaining dark-mode hex**

Grep: `grep -n "#[0-9a-fA-F]\{3,6\}" apps/desktop/src/components/Settings/ConnectBankDrawer.tsx`

Expected: zero hex remaining. If any, swap to tokens per the table.

- [ ] **Step 2: BankAccountRow — verify expired state reads clearly**

Expired row currently differentiated by `#3a2a12` bg. After Task 6 migration it uses `var(--hairline)` bg. Verify the state is still distinguishable by adding:

```tsx
style={{
  ...baseStyle,
  background: expired ? "var(--hairline)" : "var(--surface)",
  borderColor: expired ? "var(--ink-soft)" : "var(--hairline-strong)",
  // expired rows get a mono-weight emphasis on their status text:
}}
```

And the "expires in N days" / "expired" text line adds `fontWeight: 600` when expired.

- [ ] **Step 3: Audit + commit**

```bash
./apps/desktop/scripts/audit-legacy-tokens.sh
```
Expected: Phase-5 dark hex count = 0.

```bash
git add apps/desktop/src/components/Settings/ConnectBankDrawer.tsx apps/desktop/src/components/Settings/BankAccountRow.tsx
git commit -m "refactor(settings): Phase-5 dark hex fully retired; expired state uses weight + tone cue"
```

---

## Task 9: Remove legacy token shim

**Files:**
- Modify: `apps/desktop/src/styles.css`

At this point the audit should show 0 `--imessage-*` call-sites. Delete the shim so future regressions break loudly.

- [ ] **Step 1: Run audit to confirm zero call-sites**

```bash
./apps/desktop/scripts/audit-legacy-tokens.sh
```

Expected: `--imessage-*` refs = 0 (excluding `styles.css` definition). If not 0, fix the stragglers in whichever file(s) the grep reveals before proceeding.

- [ ] **Step 2: Delete the shim block**

In `apps/desktop/src/styles.css`, remove the entire "SHIM" block (comments + the three `--imessage-*` definitions + `--shadow-*` definitions that used rgba with no hue). Keep `--radius-pill` for now (legitimate uses remain). Re-define shadows in OKLCH-like warm form:

```css
/* Shadows — warm, subtle */
:root {
  --shadow-sm: 0 1px 2px rgba(31, 31, 31, 0.06), 0 1px 3px rgba(31, 31, 31, 0.04);
  --shadow-md: 0 4px 10px rgba(31, 31, 31, 0.08), 0 2px 6px rgba(31, 31, 31, 0.05);
  --shadow-lg: 0 12px 28px rgba(31, 31, 31, 0.12), 0 4px 10px rgba(31, 31, 31, 0.08);
}
```

(Shadows intentionally tinted to `#1f1f1f` base, same as `--ink` — no pure black.)

- [ ] **Step 3: Verify build**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npx vite build 2>&1 | tail -10
```
Expected: clean. If the build errors on a stale `--imessage-*` reference, fix it in the offending file and re-run Step 1.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/styles.css
git commit -m "refactor(ui): delete legacy --imessage-* shim tokens; shadows tint to ink"
```

---

## Task 10: Page-header rollout — all top-level views

**Files:**
- Modify: `apps/desktop/src/components/Today/Today.tsx` (imports + usage)
- Modify: `apps/desktop/src/components/Today/HeaderCard.tsx`
- Modify: `apps/desktop/src/components/Ledger/LedgerView.tsx`
- Modify: `apps/desktop/src/components/Chores/ChoresView.tsx`
- Modify: `apps/desktop/src/components/TimeBlocks/TimeBlocksView.tsx`

- [ ] **Step 1: Today — replace HeaderCard with PageHeader**

`HeaderCard.tsx` currently renders a big card with date/weather. Retire that card treatment. In `Today.tsx`, replace `<HeaderCard />` with:

```tsx
import { Sun } from "lucide-react";
import { PageHeader } from "../../lib/ui";

// inside render:
<PageHeader
  icon={Sun}
  title="Today"
  subtitle={formatLongDate(new Date())}  // "Tuesday, 7 April"
  meta={
    <>
      <span data-num>{events.length}</span> events ·{" "}
      <span data-num>{tasks.length}</span> tasks
    </>
  }
/>
```

Delete `HeaderCard.tsx` entirely (its responsibilities are now split: date goes to PageHeader subtitle, weather moves to a dedicated Weather section in a later landmark — not Phase 6 scope).

- [ ] **Step 2: Ledger — page header**

`LedgerView.tsx` currently has an ad-hoc title. Replace with:

```tsx
import { Wallet } from "lucide-react";
import { PageHeader } from "../../lib/ui";

<PageHeader
  icon={Wallet}
  title="Ledger"
  subtitle={formatMonth(currentMonth)}  // "April 2026"
/>
```

- [ ] **Step 3: Chores — page header**

```tsx
import { Sparkles } from "lucide-react";
import { PageHeader } from "../../lib/ui";

<PageHeader icon={Sparkles} title="Chores" />
```

- [ ] **Step 4: TimeBlocks — page header**

```tsx
import { LayoutGrid } from "lucide-react";
import { PageHeader } from "../../lib/ui";

<PageHeader icon={LayoutGrid} title="Time blocks" />
```

- [ ] **Step 5: TS + commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
```
Expected: clean.

```bash
git add apps/desktop/src/components/Today/ apps/desktop/src/components/Ledger/LedgerView.tsx apps/desktop/src/components/Chores/ChoresView.tsx apps/desktop/src/components/TimeBlocks/TimeBlocksView.tsx
git commit -m "feat(ui): page-header pattern applied to Today / Ledger / Chores / TimeBlocks; HeaderCard retired"
```

---

## Task 11: Section-label rollout — convert UPPERCASE labels to icon + normal-case

**Files:**
- Modify: `apps/desktop/src/components/Today/EventsCard.tsx`
- Modify: `apps/desktop/src/components/Today/TasksCard.tsx`
- Modify: `apps/desktop/src/components/Today/ChoresCard.tsx`
- Modify: `apps/desktop/src/components/Today/TimeBlocksCard.tsx`
- Modify: `apps/desktop/src/components/Chores/ChoresView.tsx`
- Modify: `apps/desktop/src/components/TimeBlocks/TimeBlocksView.tsx`
- Modify: `apps/desktop/src/components/Ledger/LedgerView.tsx` (sections within)
- Modify: `apps/desktop/src/components/Ledger/RecurringSection.tsx`
- Modify: `apps/desktop/src/components/Ledger/ContractsSection.tsx`

Every file currently has a `sectionHeader` inline style object with `textTransform: "uppercase"`, `letterSpacing: 0.6`, `fontSize: 11`. Replace with `<SectionLabel icon={…}>…</SectionLabel>`.

- [ ] **Step 1: Apply per file**

Canonical mappings per the spec:

| File | Section label | Lucide icon |
|---|---|---|
| `EventsCard.tsx` | "Events" | `Calendar` |
| `TasksCard.tsx` | "Tasks" | `ListTodo` |
| `ChoresCard.tsx` | "Chores" | `Sparkles` |
| `TimeBlocksCard.tsx` | "Time blocks" | `LayoutGrid` |
| `ChoresView.tsx` (list sections) | dynamic | `Sparkles` |
| `RecurringSection.tsx` | "Recurring" | `RefreshCw` |
| `ContractsSection.tsx` | "Contracts" | `FileText` |

Example migration (EventsCard):

```tsx
// Delete:
const sectionHeader = {
  fontSize: 11,
  textTransform: "uppercase" as const,
  letterSpacing: 0.6,
  fontWeight: 700,
  color: "rgba(0,0,0,0.55)",
  marginBottom: 8,
};
// ...
<div style={sectionHeader}>EVENTS</div>

// Replace with:
import { Calendar } from "lucide-react";
import { SectionLabel } from "../../lib/ui";
// ...
<SectionLabel icon={Calendar}>Events</SectionLabel>
```

- [ ] **Step 2: Retire local `sectionHeader` style objects**

Each file had its own inline `sectionHeader` — delete them. If the same object is reused elsewhere in the file, confirm `SectionLabel` covers the use or extend the primitive in Task 2 (but it should suffice).

- [ ] **Step 3: TS + commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
git add apps/desktop/src/components/
git commit -m "feat(ui): section-label pattern with Lucide icons replaces UPPERCASE headers"
```

---

## Task 12: TimeBlocksCard — kill side-stripe borders, add kind icons

**Files:**
- Modify: `apps/desktop/src/components/Today/TimeBlocksCard.tsx`

- [ ] **Step 1: Remove `borderLeft: "3px solid ..."` from block pill rows**

Find the per-block render. Replace:

```tsx
style={{ borderLeft: `3px solid ${KIND_COLOR[kind]}`, ... }}
```

with:

```tsx
style={{ display: "flex", alignItems: "center", gap: 8, borderBottom: "1px solid var(--hairline)", padding: "6px 0", ... }}
```

- [ ] **Step 2: Replace `KIND_COLOR` with `KIND_ICON`**

Delete the `KIND_COLOR` map. Add:

```tsx
import { Target, Inbox, Coffee, Zap } from "lucide-react";
import type { LucideIcon } from "lucide-react";

const KIND_ICON: Record<string, LucideIcon> = {
  focus: Target,
  admin: Inbox,
  break: Coffee,
  deep: Zap,
};
```

- [ ] **Step 3: Render the icon inline with the label**

At the render for each block pill, prepend:

```tsx
{(() => {
  const Icon = KIND_ICON[kind] ?? Target;
  return <Icon size={14} strokeWidth={1.8} color="var(--ink-soft)" />;
})()}
```

(Or extract to a local `BlockKindIcon` sub-component for cleanliness — see Task 2 patterns.)

- [ ] **Step 4: Audit**

```bash
./apps/desktop/scripts/audit-legacy-tokens.sh
```
Expected: `border-left > 1px` count = 0.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/Today/TimeBlocksCard.tsx
git commit -m "refactor(today): TimeBlocksCard — side-stripe border replaced with kind icon"
```

---

## Task 13: TransactionRow — rounded-square chip to inline Lucide category icon

**Files:**
- Modify: `apps/desktop/src/components/Ledger/TransactionRow.tsx`

- [ ] **Step 1: Delete `CATEGORY_COLORS` (pastel-background table)**

Remove `CATEGORY_COLORS` entirely. Remove the 32×32 rounded-square icon container `<div style={{ width: 32, height: 32, borderRadius: 9, background: CATEGORY_COLORS[...], ... }}>`. The category will be represented by a Lucide icon inline with the merchant/description text.

- [ ] **Step 2: Add `CATEGORY_ICON` map**

```tsx
import {
  ShoppingBag, UtensilsCrossed, Bus, Zap, CreditCard,
  Pill, Shirt, Music, CircleDashed, TrendingUp,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

const CATEGORY_ICON: Record<number, LucideIcon> = {
  1:  ShoppingBag,      // Groceries
  2:  UtensilsCrossed,  // Eating Out
  3:  Bus,              // Transport
  4:  Zap,              // Utilities
  5:  CreditCard,       // Subscriptions
  6:  Pill,             // Health
  7:  Shirt,            // Shopping
  8:  Music,            // Entertainment
  9:  CircleDashed,     // Other
  10: TrendingUp,       // Income
};
```

- [ ] **Step 3: Render inline**

Restructure the row as:

```tsx
<li
  role="button"
  tabIndex={0}
  onClick={onClick}
  onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onClick?.(); }}
  style={{
    display: "grid",
    gridTemplateColumns: "14px 1fr auto auto",
    alignItems: "center",
    gap: 10,
    padding: "8px 0",
    borderBottom: "1px solid var(--hairline)",
  }}
>
  {(() => {
    const Icon = CATEGORY_ICON[tx.category_id ?? 9] ?? CircleDashed;
    return <Icon size={14} strokeWidth={1.8} color="var(--ink-soft)" />;
  })()}
  <span style={{ fontSize: "var(--text-md)" }}>{tx.merchant ?? tx.description}</span>
  <time className="num" style={{ fontSize: "var(--text-xs)", color: "var(--ink-soft)" }}>
    {formatShortDate(tx.date)}
  </time>
  <span
    className="num"
    style={{
      fontSize: "var(--text-md)",
      fontWeight: tx.amount_pence > 0 ? 600 : 400,
      color: "var(--ink)",
    }}
  >
    {formatAmount(tx.amount_pence)}
  </span>
</li>
```

(Income is distinguished by `fontWeight: 600`, not color.)

- [ ] **Step 4: TS + commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
git add apps/desktop/src/components/Ledger/TransactionRow.tsx
git commit -m "refactor(ledger): TransactionRow — inline Lucide category icon replaces pastel chip"
```

---

## Task 14: Sidebar Nav — solid logo + Lucide navigation

**Files:**
- Modify: `apps/desktop/src/components/Nav/Sidebar.tsx`

- [ ] **Step 1: Replace Manor logo orange gradient**

At line 23, replace:

```tsx
background: "linear-gradient(135deg, #FFC15C 0%, #FF8800 100%)"
```

with a solid square containing an inset Lucide `Home` icon:

```tsx
import { Home } from "lucide-react";
// ...
<div
  style={{
    width: 32,
    height: 32,
    background: "var(--ink)",
    borderRadius: "var(--radius-sm)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  }}
>
  <Home size={18} strokeWidth={1.8} color="var(--paper)" />
</div>
```

- [ ] **Step 2: Replace emoji nav items with Lucide icons**

Current: nav entries render emoji (`💰`, `📅`, `🧹`, `🧱`, `⚙️`, etc.) as text. Replace each with its Lucide equivalent:

```tsx
import {
  LayoutDashboard,  // Today
  Wallet,           // Ledger
  Calendar,         // Calendar
  Sparkles,         // Chores
  LayoutGrid,       // TimeBlocks
  MessageSquare,    // Assistant (if nav entry exists)
  Settings as SettingsIcon,  // Settings
} from "lucide-react";

const NAV_ICONS: Record<string, LucideIcon> = {
  today: LayoutDashboard,
  ledger: Wallet,
  calendar: Calendar,
  chores: Sparkles,
  timeblocks: LayoutGrid,
  settings: SettingsIcon,
};
```

For each nav entry, render:

```tsx
<Icon size={20} strokeWidth={1.8} color={active ? "var(--ink)" : "var(--ink-faint)"} />
```

- [ ] **Step 3: Active state — left marker instead of color stripe**

Replace any border-left stripe or background tint used for active state with a 2px left marker:

```tsx
<div
  style={{
    position: "absolute",
    left: 0,
    top: "50%",
    transform: "translateY(-50%)",
    width: 2,
    height: active ? 16 : 0,
    background: "var(--ink)",
    transition: "height var(--duration-fast) var(--ease-out)",
  }}
/>
```

(Set the parent `<li>` to `position: relative`.)

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Nav/Sidebar.tsx
git commit -m "feat(nav): Sidebar — solid logo + Lucide nav icons + left-marker active state"
```

---

## Task 15: Primary/secondary button rollout

**Files (any file with inline-styled buttons):**
- Modify: `apps/desktop/src/components/Today/TasksCard.tsx`
- Modify: `apps/desktop/src/components/Today/ChoresCard.tsx`
- Modify: `apps/desktop/src/components/Today/ProposalBanner.tsx`
- Modify: `apps/desktop/src/components/Ledger/MonthReviewPanel.tsx`
- Modify: `apps/desktop/src/components/Ledger/RecurringSection.tsx`
- Modify: `apps/desktop/src/components/Ledger/AddRecurringDrawer.tsx`
- Modify: `apps/desktop/src/components/Ledger/AddContractDrawer.tsx`
- Modify: `apps/desktop/src/components/Ledger/CsvImportDrawer.tsx`
- Modify: `apps/desktop/src/components/Settings/ConnectBankDrawer.tsx`
- Modify: `apps/desktop/src/components/Settings/AddAccountForm.tsx`
- Modify: `apps/desktop/src/components/Chores/ChoreDrawer.tsx`
- Modify: `apps/desktop/src/components/Wizard/Wizard.tsx`

- [ ] **Step 1: Replace inline primary buttons with `<Button variant="primary">`**

For each file, find primary action buttons (previously iMessage-blue background, now `--action-bg` from Task 4/5/6). Replace:

```tsx
<button
  style={{
    background: "var(--action-bg)",
    color: "var(--action-fg)",
    padding: "8px 14px",
    borderRadius: "var(--radius-md)",
    fontWeight: 500,
    ...
  }}
  onClick={handleSubmit}
>
  Save
</button>
```

with:

```tsx
import { Button } from "../../lib/ui";
// ...
<Button variant="primary" onClick={handleSubmit} icon={Plus}>
  Add
</Button>
```

Icon choice per common action (imported from `lucide-react`):
- "Add / Create" → `Plus`
- "Save / Confirm" → `Check`
- "Delete" → `Trash2`
- "Cancel / Close" → `X`
- "Next / Continue" → `ArrowRight`
- "Back" → `ArrowLeft`
- "Import" → `Upload`
- "Review month" → `BookOpen`
- "Ask Nell" → `MessageSquare`

- [ ] **Step 2: Replace secondary buttons with `<Button variant="secondary">`**

Same pattern. Cancel, Close, tertiary actions become `variant="secondary"`.

- [ ] **Step 3: TS check**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/
git commit -m "feat(ui): primary + secondary button rollout with Lucide icons across all drawers/forms"
```

---

## Task 16: ChoresCard — visible Skip button + keyboard handler

**Files:**
- Modify: `apps/desktop/src/components/Today/ChoresCard.tsx`

- [ ] **Step 1: Add visible Skip button on hover/focus**

Each chore row currently supports right-click to skip (discoverable only via title tooltip). Add a visible Skip action:

```tsx
import { useState } from "react";
import { Check, SkipForward } from "lucide-react";

function ChoreRow({ chore, onComplete, onSkip }: Props) {
  const [hover, setHover] = useState(false);

  return (
    <li
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      onFocus={() => setHover(true)}
      onBlur={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "6px 0",
        borderBottom: "1px solid var(--hairline)",
      }}
    >
      <button
        onClick={onComplete}
        onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onComplete(); }}
        style={{ border: "none", background: "none", padding: 0, cursor: "pointer" }}
        aria-label={`Complete ${chore.title}`}
      >
        <Check size={14} strokeWidth={1.8} color="var(--ink-soft)" />
      </button>
      <span style={{ fontSize: "var(--text-md)", flex: 1 }}>{chore.title}</span>
      {(hover || /* focused */ false) && (
        <button
          onClick={onSkip}
          style={{
            border: "none",
            background: "none",
            color: "var(--ink-soft)",
            fontSize: "var(--text-xs)",
            cursor: "pointer",
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
          }}
          aria-label={`Skip ${chore.title}`}
        >
          <SkipForward size={12} strokeWidth={1.8} />
          Skip
        </button>
      )}
    </li>
  );
}
```

- [ ] **Step 2: Keep the right-click as secondary path**

The existing `onContextMenu` handler stays, but it's no longer the only path. Remove the `title="Right-click to skip"` attribute since the behaviour is now discoverable.

- [ ] **Step 3: TS + commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
git add apps/desktop/src/components/Today/ChoresCard.tsx
git commit -m "feat(today): ChoresCard — visible Skip button on hover/focus; keyboard-reachable"
```

---

## Task 17: ContractsSection — keyboard handler on clickable row

**Files:**
- Modify: `apps/desktop/src/components/Ledger/ContractsSection.tsx`

- [ ] **Step 1: Add `tabIndex={0}` and `onKeyDown`**

At line ~186 where the contract row `<div>` has `onClick`, add:

```tsx
<div
  role="button"
  tabIndex={0}
  onClick={() => handleContractClick(c.id)}
  onKeyDown={(e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      handleContractClick(c.id);
    }
  }}
  style={{ /* existing */ }}
>
```

- [ ] **Step 2: Commit**

```bash
git add apps/desktop/src/components/Ledger/ContractsSection.tsx
git commit -m "feat(ledger): ContractsSection — keyboard activation on clickable row"
```

---

## Task 18: Delete BubbleLayer

**Files:**
- Delete: `apps/desktop/src/components/Assistant/BubbleLayer.tsx`
- Modify: any file importing `BubbleLayer` — find with grep

- [ ] **Step 1: Find BubbleLayer imports**

```bash
grep -rn "BubbleLayer" apps/desktop/src --include="*.tsx" --include="*.ts"
```

Typical caller: `App.tsx` (or similar root). Each caller needs its usage removed — `<BubbleLayer />` out of the render, import out of the head.

- [ ] **Step 2: Remove callers**

For each caller, delete:
- The `import { BubbleLayer } from …` line
- The `<BubbleLayer />` JSX usage
- Any nearby prop-passing infrastructure that only served `BubbleLayer`

- [ ] **Step 3: Delete the file**

```bash
rm apps/desktop/src/components/Assistant/BubbleLayer.tsx
```

- [ ] **Step 4: Remove typingBounce keyframe**

In `apps/desktop/src/styles.css`, delete:

```css
@keyframes typingBounce { 0%, 60%, 100% { transform: translateY(0); } 30% { transform: translateY(-5px); } }
```

Add `typingPulse`:

```css
@keyframes typingPulse { 0%, 100% { opacity: 0.35; } 50% { opacity: 1; } }
```

(Used in the new ConversationDrawer — Task 19.)

- [ ] **Step 5: TS + commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
```
Expected: clean. If typescript complains about an orphaned import in a caller, remove it.

```bash
git add -A apps/desktop/src/components/Assistant/ apps/desktop/src/styles.css apps/desktop/src/App.tsx
git commit -m "feat(assistant): delete BubbleLayer; replace typingBounce with typingPulse"
```

---

## Task 19: ConversationDrawer restyle — hairline-separated messages with role labels

**Files:**
- Modify: `apps/desktop/src/components/Assistant/ConversationDrawer.tsx`

- [ ] **Step 1: Retire iMessage-bubble message rendering**

Find the JSX that renders each message as a gradient bubble. Replace with:

```tsx
import { MessageSquare, User } from "lucide-react";

function Message({ role, text }: { role: "user" | "assistant"; text: string }) {
  const Icon = role === "user" ? User : MessageSquare;
  const label = role === "user" ? "You" : "Nell";

  return (
    <div style={{ marginBottom: 18 }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          color: "var(--ink-soft)",
          fontSize: "var(--text-xs)",
          fontWeight: 500,
          marginBottom: 4,
        }}
      >
        <Icon size={14} strokeWidth={1.8} />
        <span>{label}</span>
      </div>
      <div
        style={{
          fontSize: "var(--text-md)",
          color: "var(--ink)",
          lineHeight: 1.55,
          paddingBottom: 14,
          borderBottom: "1px solid var(--hairline)",
        }}
      >
        {text}
      </div>
    </div>
  );
}
```

Message list wraps these in a scrollable container — same shape as before.

- [ ] **Step 2: Replace typing indicator**

Old `typingBounce` dots → opacity pulse with the new keyframe:

```tsx
function TypingDots() {
  return (
    <div style={{ display: "inline-flex", gap: 4, alignItems: "center", color: "var(--ink-soft)" }}>
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          style={{
            width: 5,
            height: 5,
            borderRadius: "50%",
            background: "currentColor",
            animation: `typingPulse 1.2s ${i * 0.15}s infinite var(--ease-out)`,
          }}
        />
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Restyle textarea + send button**

```tsx
import { Send } from "lucide-react";
import { Button } from "../../lib/ui";

<div style={{ display: "flex", gap: 6, alignItems: "flex-end", padding: "12px 14px", borderTop: "1px solid var(--hairline)" }}>
  <textarea
    value={input}
    onChange={(e) => setInput(e.target.value)}
    placeholder="Message Nell…"
    rows={1}
    style={{
      flex: 1,
      resize: "none",
      fontFamily: "inherit",
      fontSize: "var(--text-md)",
      color: "var(--ink)",
      background: "var(--surface)",
      border: "1px solid var(--hairline-strong)",
      borderRadius: "var(--radius-md)",
      padding: "8px 10px",
    }}
    onFocus={(e) => { e.currentTarget.style.outline = "2px solid var(--ink)"; }}
    onBlur={(e) => { e.currentTarget.style.outline = "none"; }}
  />
  <Button variant="primary" icon={Send} onClick={handleSend}>
    Send
  </Button>
</div>
```

(Task 21 replaces the inline onFocus/onBlur pattern with `:focus-visible` CSS — for now this inline approach keeps it accessible.)

- [ ] **Step 4: Drawer shell**

Drawer panel uses `background: "var(--surface)"`, `border-left: 1px solid var(--hairline-strong)`, no drop shadow. Scrim uses `var(--scrim)`.

- [ ] **Step 5: TS + commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
git add apps/desktop/src/components/Assistant/ConversationDrawer.tsx
git commit -m "feat(assistant): ConversationDrawer — hairline-separated messages with role labels; typing pulse; Lucide send"
```

---

## Task 20: `:focus-visible` replaces `outline: none`

**Files:**
- Modify: `apps/desktop/src/components/Assistant/ConversationDrawer.tsx`
- Modify: `apps/desktop/src/components/Today/InputPill.tsx`
- Modify: `apps/desktop/src/components/Today/TasksCard.tsx`
- Modify: `apps/desktop/src/components/Settings/AddAccountForm.tsx`
- Modify: `apps/desktop/src/styles.css`

- [ ] **Step 1: Add global focus-visible styles**

Append to `apps/desktop/src/styles.css`:

```css
/* Focus visible — keyboard-reachable everywhere */
:focus {
  outline: none;
}
:focus-visible {
  outline: 2px solid var(--ink);
  outline-offset: 2px;
  border-radius: var(--radius-sm);
}
button:focus-visible,
[role="button"]:focus-visible,
a:focus-visible {
  outline-offset: 3px;
}
input:focus-visible,
textarea:focus-visible,
select:focus-visible {
  outline: 2px solid var(--ink);
  outline-offset: 0;
  border-color: var(--ink);
}
```

- [ ] **Step 2: Remove inline `outline: "none"` from each file**

Search for `outline: "none"` across the four files and delete. The new CSS handles focus for keyboard users.

- [ ] **Step 3: Remove any inline `onFocus`/`onBlur` outline manipulation**

E.g., the pattern added in Task 19 Step 3 — revert to plain `style={…}` without inline focus styles, since the global CSS now covers it.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/styles.css apps/desktop/src/components/
git commit -m "a11y(ui): global :focus-visible outlines replace inline outline:none sites"
```

---

## Task 21: SettingsModal — remove glassmorphism backdrop-filter

**Files:**
- Modify: `apps/desktop/src/components/Settings/SettingsModal.tsx`

Already addressed in Task 6 token pass — verify here that no `backdrop-filter` survives anywhere.

- [ ] **Step 1: Audit**

```bash
./apps/desktop/scripts/audit-legacy-tokens.sh
```
Expected: `backdrop-filter` count = 0. If any remain, grep to find and delete:

```bash
grep -rn "backdrop-filter\|backdropFilter" apps/desktop/src
```

- [ ] **Step 2: Confirm scrim is intact**

`SettingsModal.tsx:37` area should still render `background: "var(--scrim)"` on the backdrop — only the `backdropFilter: "blur(…)"` property is gone.

- [ ] **Step 3: Commit (if any changes)**

```bash
git add apps/desktop/src/components/Settings/SettingsModal.tsx
git commit -m "a11y(settings): SettingsModal — glassmorphism backdrop-filter removed; scrim retained" || echo "already clean"
```

---

## Task 22: Width-animations → `transform: scaleX()`

**Files:**
- Modify: `apps/desktop/src/components/Today/InputPill.tsx`
- Modify: `apps/desktop/src/components/Ledger/SummaryCard.tsx`
- Modify: `apps/desktop/src/components/Settings/AiTab.tsx`
- Modify: `apps/desktop/src/components/Wizard/Wizard.tsx`

- [ ] **Step 1: SummaryCard progress bar**

Currently:

```tsx
<div style={{ width: `${pct}%`, transition: "width 0.3s" }} />
```

Replace with:

```tsx
<div
  style={{
    width: "100%",
    transform: `scaleX(${Math.min(pct, 100) / 100})`,
    transformOrigin: "left",
    transition: "transform var(--duration-med) var(--ease-out)",
    height: 6,
    background: "var(--action-fg)",
    borderRadius: 3,
  }}
/>
```

- [ ] **Step 2: Wizard progress bar**

Same pattern as above. At `Wizard.tsx:54`, replace the width-animating progress bar.

- [ ] **Step 3: AiTab progress indicator**

At `AiTab.tsx:198`, same pattern.

- [ ] **Step 4: InputPill — expansion on focus**

InputPill currently animates `width: 220px → 320px` on focus. Switch to a max-width + transform combo:

```tsx
<input
  style={{
    width: 320,
    transform: focused ? "scaleX(1)" : "scaleX(0.6875)",  /* 220/320 */
    transformOrigin: "right",
    transition: "transform var(--duration-fast) var(--ease-out)",
    ...
  }}
/>
```

(An acceptable alternative: leave `width` fixed at 320, forgo the animation. Do this if the transform approach causes text-clip issues.)

- [ ] **Step 5: TS + commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
git add apps/desktop/src/components/
git commit -m "perf(ui): width-animations converted to transform:scaleX — no layout thrash"
```

---

## Task 23: Final verification + audit re-run

**Files:**
- None modified; this is verification.

- [ ] **Step 1: Run the audit oracle**

```bash
./apps/desktop/scripts/audit-legacy-tokens.sh
```

Expected all zeros except:
- Pure `"white"` / `"black"` string refs may exist in legitimate places (e.g., CSS `color: white` in the new focus-visible block is impossible because we use `var(--ink)` — so this should be zero too).

If any count is non-zero, fix the offenders and re-run.

- [ ] **Step 2: Run TS + vite build**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system/apps/desktop && npm run tsc 2>&1 | tail -3
npx vite build 2>&1 | tail -10
```

Expected: zero TS errors, clean frontend bundle.

- [ ] **Step 3: Cargo build (unchanged — sanity)**

```bash
cd /Users/hanamori/life-assistant/.worktrees/phase-6-design-system && cargo build --workspace 2>&1 | tail -3
```

Expected: clean (no Rust touched, so this is just to confirm the worktree isn't damaged).

- [ ] **Step 4: Manual acceptance**

Launch the app (`npm run tauri dev` from `apps/desktop/`) and verify:
- Today view renders with Lucide sun icon, monochrome, document-flat sections, SF Pro body, SF Mono numbers
- Ledger view renders with Wallet page icon, transactions use inline category icons, SummaryCard is a monochrome dark-on-paper block
- Sidebar Nav uses Lucide icons with ink left-marker on active
- Settings opens without glassmorphism blur; drawers use flat surfaces
- Assistant drawer opens with hairline-separated messages (no bubbles)
- Switch macOS between Light and Dark appearance — Manor auto-switches with no reload
- Enable "Reduce Motion" in System Settings → animations collapse instantly
- Keyboard-tab through Today — every focusable control shows the ink outline
- No `console.error` / `console.warn` noise in the browser devtools

- [ ] **Step 5: Final commit**

```bash
git add -u
git commit -m "chore(ui): Phase 6 verification passed — audit clean, manual acceptance ok" --allow-empty
```

---

## Self-Review

**1. Spec coverage:**

- ✅ Palette tokens (light + dark) — Task 1
- ✅ Typography + type scale — Task 1
- ✅ Radii — Task 1
- ✅ Motion tokens + reduced-motion — Task 1 + Task 22
- ✅ `color-scheme: light dark` — Task 1
- ✅ `prefers-color-scheme` auto-switch — Task 1
- ✅ Explicit `[data-theme]` override hooks — Task 1
- ✅ `lucide-react` dep — Task 1
- ✅ Remove `@fontsource/nunito` — Task 1
- ✅ PageHeader + SectionLabel + Button primitives — Task 2
- ✅ Token migration across 40 files — Tasks 4, 5, 6
- ✅ Delete legacy shim — Task 9
- ✅ SummaryCard dark gradient → monochrome — Task 7
- ✅ ConnectBankDrawer + BankAccountRow — Task 6 + Task 8
- ✅ Page-header pattern applied per view — Task 10
- ✅ Section-label pattern per view — Task 11
- ✅ TimeBlocksCard kind icons — Task 12
- ✅ TransactionRow Lucide icons — Task 13
- ✅ Sidebar Nav solid + Lucide — Task 14
- ✅ Primary/secondary buttons — Task 15
- ✅ ChoresCard Skip disclosure — Task 16
- ✅ ContractsSection keyboard handler — Task 17
- ✅ Delete BubbleLayer — Task 18
- ✅ ConversationDrawer restyle — Task 19
- ✅ typingPulse replaces typingBounce — Task 18 + 19
- ✅ `:focus-visible` replaces `outline: none` — Task 20
- ✅ SettingsModal backdrop-filter removed — Task 6 + verified Task 21
- ✅ Width → scaleX — Task 22
- ✅ Final acceptance — Task 23

Every acceptance criterion from the spec maps to at least one task.

**2. Placeholder scan:**

No "TBD", "TODO", or "handle edge cases" placeholders in the task steps. Every code block is complete. Every command has expected output.

**3. Type consistency:**

- `PageHeader` props (`icon`, `title`, `subtitle`, `meta`) match usage in Task 10
- `SectionLabel` props (`icon`, `children`, `action`) match usage in Task 11
- `Button` props (`variant`, `icon`, `children`) match usage in Task 15 + Task 19
- `KIND_ICON` (Task 12) and `CATEGORY_ICON` (Task 13) use consistent `Record<K, LucideIcon>` shape
- `LucideIcon` type imported from `lucide-react` at every call site

**Pre-flight notes for executor:**

1. Branch + worktree setup (`using-git-worktrees` skill) before Task 1 — the plan assumes `/Users/hanamori/life-assistant/.worktrees/phase-6-design-system/` exists and is on `feature/phase-6-design-system`.
2. Token migration tasks (4, 5, 6) are the most tedious — use the audit oracle after each to keep momentum. The audit script is the progress bar.
3. The shim in Task 1 Step 2 is load-bearing for Tasks 2–8. Don't delete it early (Task 9 is the right moment).
4. `HeaderCard.tsx` is deleted in Task 10 — if any other caller imports it, grep + remove before deletion.
5. Some files (`SampleDataBanner`, `Safety/*`) are lightly touched; if a file has no hex/hex-named/legacy tokens, it may need zero changes — the substitution table only bites when there's something to replace.
