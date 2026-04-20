# Avatar + Chat Simplification — Design Spec

- **Date**: 2026-04-20
- **Landmark**: v0.5 post-Bones UI polish
- **Status**: Approved design. Implementation plan next.
- **Authors**: Hana (product), Nell (architect)

## 1. Purpose

Simplify the assistant-persona UI in the Manor desktop app by:

1. Replacing the current 5-state expression-swapping avatar with a single static image (`manor_page.png`).
2. Replacing the bottom-right 320px input pill with a centered, square-cornered long input bar at the bottom of the window.
3. Replacing the right-side slide-in conversation drawer with an inline expandable scrollable history panel that sits above the input.
4. Adding an ephemeral "recent exchange" card that appears above the input after each message and fades after 10 seconds.

The outcome is a terminal / IM feel — type at the bottom, recent response hovers briefly, expand for full history, click outside to collapse.

## 2. Scope decisions (locked during brainstorm)

| Area | Decision |
|---|---|
| **Avatar image** | Single `manor_page.png` (static, no expression states). Keeps mirrored (`scaleX(-1)`) transform + shrink-when-overlay-open behaviour. Remove the 5 old PNGs (`content.png`, `smile.png`, `questioning.png`, `laughing.png`, `confused.png`) + the `expressionFor()` mapping utility. |
| **Avatar position** | Unchanged — bottom-right corner, 16px from both edges, 72px tall default (40px when overlays are open). Click still opens the conversation UI (now the new panel instead of the drawer). |
| **Input bar** | New `ChatDock` component. Centered at the bottom of the window. ~32px tall, 4px corner radius (rectangular, not pill). Horizontal bounds: `left = menu-width + 16px`, `right = 104px` (leaves ~88px for the avatar column + 16px gap). 16px from the window bottom. |
| **Recent exchange display** | New `EphemeralLog` component, sits directly above ChatDock. Shows the last up-to-2 exchanges on a semi-transparent card. Fades to null after 10 seconds of no new activity; fade animation 400ms. Clicking anywhere on the log expands the full history panel. |
| **Full history panel** | New `ChatHistoryPanel` component. Expands upward from above the input. Scrollable vertically (`overflow-y: auto`, `max-height: calc(100vh - 180px)`). Same horizontal bounds as ChatDock — never covers the main menu or the avatar column. Replaces the existing right-side `ConversationDrawer`. |
| **Expand trigger** | Expand icon (⤢) on the right edge of the input bar, OR clicking the EphemeralLog. |
| **Collapse trigger** | ⤡ icon inside the panel header, OR clicking anywhere outside the panel (including the menu + the main content area + the avatar), OR pressing Escape. |
| **Avatar click** | Toggles the panel — opens if closed, closes if open. |
| **Input behaviour** | Single-line `<input>`. Enter submits. Placeholder `"Say something…"`. Auto-focuses when the panel opens. |
| **Transient bubbles** | Existing floating-notification bubbles keep their current behaviour — render over everything (higher z-index than the panel). |

## 3. Architecture

### 3.1 File surface

**New files:**
- `apps/desktop/src/components/Assistant/ChatDock.tsx` — centered input bar.
- `apps/desktop/src/components/Assistant/ChatHistoryPanel.tsx` — expandable scrollable panel.
- `apps/desktop/src/components/Assistant/EphemeralLog.tsx` — fading recent-exchange card.
- `apps/desktop/src/assets/avatars/manor_page.png` — Hana drops this in.

**Modified:**
- `apps/desktop/src/components/Assistant/Avatar.tsx` — single-image rendering, no expression branching.
- `apps/desktop/src/components/Assistant/Assistant.tsx` — swap InputPill → ChatDock; swap ConversationDrawer → ChatHistoryPanel; mount EphemeralLog; own `isHistoryOpen` state.
- `apps/desktop/src/lib/assistant/*.ts` — remove `expressionFor()` + the `expression` state field if unused elsewhere (verify during implementation).

**Deleted:**
- `apps/desktop/src/components/Assistant/InputPill.tsx`
- `apps/desktop/src/components/Assistant/ConversationDrawer.tsx`
- `apps/desktop/src/assets/avatars/content.png`
- `apps/desktop/src/assets/avatars/smile.png`
- `apps/desktop/src/assets/avatars/questioning.png`
- `apps/desktop/src/assets/avatars/laughing.png`
- `apps/desktop/src/assets/avatars/confused.png`

### 3.2 Layout measurements

| Element | Value |
|---|---|
| Avatar default height | 72px (unchanged) |
| Avatar shrunken height (overlay open) | 40px (unchanged) |
| Avatar position | `right: 16px, bottom: 16px` (unchanged) |
| ChatDock height | ~32px |
| ChatDock corner radius | 4px |
| ChatDock horizontal bounds | `left: menu-width + 16px`, `right: 104px` |
| ChatDock vertical offset | `bottom: 16px` |
| EphemeralLog position | directly above ChatDock, same horizontal bounds, 6px gap |
| EphemeralLog card background | `rgba(249, 249, 247, 0.85)` with 1px hairline border, 4px radius |
| EphemeralLog fade timer | 10000ms |
| EphemeralLog fade animation | 400ms CSS transition |
| EphemeralLog max visible exchanges | 2 (one exchange = user message + Manor reply = 2 lines; so up to 4 lines total) |
| ChatHistoryPanel position | above ChatDock, same horizontal bounds, 6px gap |
| ChatHistoryPanel max-height | `calc(100vh - 180px)` |
| Panel entrance animation | 180ms ease-out, height + opacity |
| Panel collapse animation | 180ms ease-in (reverse) |

### 3.3 Component responsibilities

**`Avatar.tsx`** (simplified, ~25 lines down from 53):

```tsx
import manorPage from "../../assets/avatars/manor_page.png";

interface Props {
  height?: number;       // default 72; 40 when shrunken
  onClick: () => void;
}

export function Avatar({ height = 72, onClick }: Props) {
  const width = Math.round(height * (274 / 400));  // preserve aspect ratio
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label="Open conversation"
      style={{
        background: "none",
        border: "none",
        padding: 0,
        cursor: "pointer",
      }}
    >
      <img
        src={manorPage}
        alt=""
        style={{
          width,
          height,
          transform: "scaleX(-1)",
          display: "block",
        }}
      />
    </button>
  );
}
```

**`ChatDock.tsx`** (~80 lines):
- Controlled `<input>`. Value + setValue from parent (or local state if parent doesn't care).
- Enter key submits via `onSubmit(value)` prop. Input clears on success.
- Placeholder `"Say something…"`.
- `ref` forwarded so parent (Assistant) can auto-focus when panel opens.
- Expand icon (⤢) on the right. Calls `onExpand` prop.
- Hides (`display: none`) when `transientBubblePresent` prop is true — preserves current InputPill behaviour.
- Fixed position: `position: fixed, bottom: 16px, left: <menu-width + 16>px, right: 104px, height: 32px`.

**`EphemeralLog.tsx`** (~60 lines):
- Props: `exchanges: Array<{ userText, assistantText, timestamp }>` (last 2), `onExpand: () => void`.
- Internal `isVisible` state. `useEffect` subscribes to `exchanges[0]?.timestamp`; on change → set `isVisible=true` and start a 10s timer that sets `isVisible=false`.
- When `isVisible` is false, renders null.
- Clicking anywhere on the card calls `onExpand`.
- CSS opacity transition on visibility change.

**`ChatHistoryPanel.tsx`** (~120 lines):
- Props: `isOpen: boolean`, `messages: Array<Message>`, `onCollapse: () => void`.
- Reuses the existing message rendering from `ConversationDrawer` (port the JSX + any per-message action handlers verbatim if they exist).
- Header row: "Conversation" label on the left, ⤡ collapse icon on the right.
- Auto-scrolls to bottom on new messages (via `ref` on the scroll container + `scrollTop = scrollHeight` after render).
- Empty-history placeholder: `"No conversation yet. Type below to start."`.
- Click-outside detection: `useEffect` registers a `mousedown` listener on `document` when `isOpen` is true. The handler checks if the click target is inside the panel's ref; if not, calls `onCollapse`. Deregisters on unmount / close.
- Escape key handler: `keydown` listener on document, calls `onCollapse` when key is Escape.
- Has `role="dialog"`, `aria-modal="false"`, `aria-label="Conversation history"`.

**`Assistant.tsx`** (refactored host):
- Owns `isHistoryOpen: boolean`, starts `false`.
- Calculates `transientBubblePresent` from the existing bubble-state source (preserve current logic).
- Renders:
  ```tsx
  <Avatar height={avatarHeight} onClick={() => setIsHistoryOpen(!isHistoryOpen)} />
  <ChatDock
    transientBubblePresent={transientBubblePresent}
    onSubmit={handleSend}
    onExpand={() => setIsHistoryOpen(true)}
    autoFocus={isHistoryOpen}
  />
  {!isHistoryOpen && (
    <EphemeralLog
      exchanges={lastTwoExchanges}
      onExpand={() => setIsHistoryOpen(true)}
    />
  )}
  <ChatHistoryPanel
    isOpen={isHistoryOpen}
    messages={allMessages}
    onCollapse={() => setIsHistoryOpen(false)}
  />
  ```
- Sources `messages`, `lastTwoExchanges`, `handleSend` from the existing assistant state store (same hooks the old InputPill + ConversationDrawer used — reuse verbatim).

### 3.4 State + data flow

The existing assistant store (`lib/assistant/state.ts` or equivalent — confirm during implementation) owns:
- `messages: Message[]` — conversation history (unchanged).
- `inputValue: string` — current input (unchanged, if it exists; otherwise local to ChatDock).
- `send(text): Promise<void>` — send action (unchanged).
- Transient-bubble state (unchanged).

New UI-only state lives in `Assistant.tsx`:
- `isHistoryOpen: boolean` — panel open/closed. Not persisted.

Derived in `Assistant.tsx`:
- `lastTwoExchanges` — `messages.slice(-4)` grouped into exchange pairs (each exchange = user+assistant turn).

### 3.5 Click-outside semantics

Panel closes when the user clicks:
- Main menu / side nav.
- Main content area (Today, Hearth, Bones, Ledger views).
- Avatar (avatar click toggles — so re-clicks close).

Panel does NOT close when the user clicks:
- Inside the panel content area.
- Inside the ChatDock input (they're focusing to keep typing).
- On the EphemeralLog (it expands on click, not closes — but EphemeralLog only renders when panel is closed, so not a conflict).

Escape key closes regardless of focus.

## 4. Error handling

Inherits the existing assistant IPC error surface (Ollama unreachable, send failures). New UI concerns:

- **Send failure in ChatDock**: input retains the value so the user can retry. Small inline error strip appears in the EphemeralLog slot (`"Couldn't send — <reason>"`) until the next successful send.
- **Panel opens with an in-flight message**: panel renders the pending message with a `…` indicator. If the existing store streams tokens, auto-scroll follows.
- **Panel opens with empty history**: renders placeholder `"No conversation yet. Type below to start."`.
- **IPC send errors surface from the store** — ChatDock maps them to strings via the existing error-handling path.

## 5. Edge cases (pinned)

| Case | Behaviour |
|---|---|
| User types, hits Enter, immediately opens panel | Pending message visible in the panel as it streams. EphemeralLog replaced by panel content. |
| User rapidly opens / closes the panel | Fine — synchronous state toggle. Click-outside listener deregisters on close. |
| Transient bubble notification appears while panel is open | Bubble renders over everything (z-index highest). Panel stays open behind it. |
| Avatar clicked while panel is open | Panel closes (toggle behaviour). |
| Very long conversation (100+ messages) | Panel's `overflow-y: auto` scrolls. No pagination — YAGNI for v1. |
| Window resized very small during expanded state | Panel shrinks to `max-height: calc(100vh - 180px)`; scrollbar handles overflow. |
| Message mid-stream when user collapses panel | Stream continues in background. When user reopens, full message is visible. |
| EphemeralLog's 10s timer fires while panel is open | No-op — EphemeralLog doesn't render when panel is open. |
| EphemeralLog fade interrupted by new message | Timer resets; card stays visible. |
| Input has unsent text when user clicks outside | Input value preserved (local state). Only the panel closes. |

## 6. Testing strategy

Small surface. Minimal new tests.

### 6.1 RTL unit tests (`apps/desktop/src/components/Assistant/__tests__/`)

- **`Avatar.test.tsx`** — renders `<img>` with `manor_page.png`; respects `height` prop; `onClick` fires when clicked.
- **`ChatDock.test.tsx`** — placeholder renders; Enter submits via `onSubmit(value)`; input clears after submit; expand icon click calls `onExpand`; `transientBubblePresent=true` hides the dock.
- **`EphemeralLog.test.tsx`** — renders last 2 exchanges; after 10s (vitest fake timers) renders null; click anywhere calls `onExpand`; new message resets the fade timer.
- **`ChatHistoryPanel.test.tsx`** — renders when `isOpen=true`, nothing when false; Escape calls `onCollapse`; outside-click calls `onCollapse`; inside-click does NOT call `onCollapse`; empty-history placeholder renders when `messages.length === 0`.

### 6.2 Manual QA

1. `pnpm tauri dev`.
2. Verify avatar renders `manor_page.png` at 72px, mirrored, bottom-right. No expression changes on interaction.
3. Type "hello" + Enter. EphemeralLog shows user + Manor reply above input.
4. Wait 10s — card fades out.
5. Send another message. Card reappears.
6. Click ⤢ on the input. Panel expands upward, scrollable.
7. Click anywhere in the main content. Panel closes.
8. Hit Escape with panel open. Panel closes.
9. Resize window shorter. Panel respects `max-height`.
10. Open a modal elsewhere (e.g. AssetDetail drawer). Avatar shrinks to 40px (preserved behaviour).
11. Send a message while offline / Ollama unreachable. Error strip appears; input text preserved.

## 7. Out of scope (pinned)

- **Multi-line input** in ChatDock. v1 is single-line. Shift+Enter, auto-grow, etc. — later.
- **Message actions** (copy, retry, delete) in the panel — port verbatim from existing `ConversationDrawer` if present; don't add new ones.
- **Search / jump in history** — scrollback only for v1.
- **Per-message timestamps in EphemeralLog** — panel shows them (if the drawer does); ephemeral doesn't need them.
- **Keyboard shortcut to open/close panel** beyond Escape. No Cmd-/ or similar in v1.
- **Animated expressions / state-driven avatar** — fully removed. Do NOT re-add.
- **Focus trap / full ARIA modal treatment** for the panel. `aria-expanded` on the expand button + `role="dialog"` `aria-modal="false"` on the panel is the full v1 accessibility posture.
- **Persistence of `isHistoryOpen` across sessions** — always starts closed.
- **Theming / dark-mode specific colours** — inherits the existing Manor CSS variable palette.

## 8. Definition of done

- `manor_page.png` lives in `apps/desktop/src/assets/avatars/` (Hana drops this in before implementation begins).
- `Avatar.tsx` simplified to a single-image component; 5 old state PNGs + `expressionFor()` removed; `expression` state field removed if unused elsewhere.
- `ChatDock.tsx` renders centered at bottom with correct horizontal bounds + dimensions.
- `EphemeralLog.tsx` shows last 2 exchanges, fades after 10s, clicks expand.
- `ChatHistoryPanel.tsx` expands, scrolls, closes on ⤡ / outside-click / Escape.
- Old `InputPill.tsx` + `ConversationDrawer.tsx` deleted; all imports updated.
- `Assistant.tsx` host refactored to mount the new components + own `isHistoryOpen`.
- `pnpm tsc --noEmit` clean.
- `pnpm test` green (existing 57 tests + 4 new Assistant test files).
- `pnpm build` green.
- Manual QA script (§6.2) walked through in dev.

---

*End of spec. Next: implementation plan.*
