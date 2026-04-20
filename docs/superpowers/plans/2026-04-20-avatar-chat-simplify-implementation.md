# Avatar + Chat Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Simplify Manor's assistant persona UI — replace the 5-state expression-swapping avatar with a single static image, swap the 320px bottom-right input pill for a centered square-cornered bottom bar, replace the right-side conversation drawer with an inline expandable scrollable panel, and add a fading recent-exchange log above the input.

**Architecture:** Four component touches to `apps/desktop/src/components/Assistant/`: simplify `Avatar.tsx` to a single `<img>`, introduce `ChatDock.tsx` (replaces `InputPill.tsx`), `ChatHistoryPanel.tsx` (replaces `ConversationDrawer.tsx`), and new `EphemeralLog.tsx`. `Assistant.tsx` host refactors to mount the new trio + own `isHistoryOpen` state; it also strips all `setAvatarState(...)` calls because the avatar is now static. The store's `avatarState` field + `expressions.ts` mapping utility are deleted.

**Tech Stack:** React + TypeScript + Zustand, existing assistant IPC + message store, vitest + React Testing Library.

**Spec:** `docs/superpowers/specs/2026-04-20-avatar-chat-simplify-design.md`

---

## Prerequisite — user action before execution

**Hana drops `manor_page.png` into `apps/desktop/src/assets/avatars/` before Task 4 runs.** Task 4 imports it; without the file the frontend build fails.

If the file isn't present when Task 4 starts, escalate — don't create a placeholder.

---

## File structure

### Modified (4 files)
- `apps/desktop/src/components/Assistant/Avatar.tsx` — simplified from 53 → ~30 lines; removes expression dependency.
- `apps/desktop/src/components/Assistant/Assistant.tsx` — swaps imports, removes all `setAvatarState(...)` calls, mounts new components, owns `isHistoryOpen` state.
- `apps/desktop/src/lib/assistant/state.ts` — removes `avatarState`, `setAvatarState`, `drawerOpen`, `setDrawerOpen` fields (ConversationDrawer is gone; history state moves to Assistant.tsx local state).

### Created (4 files)
- `apps/desktop/src/components/Assistant/ChatDock.tsx`
- `apps/desktop/src/components/Assistant/ChatHistoryPanel.tsx`
- `apps/desktop/src/components/Assistant/EphemeralLog.tsx`
- `apps/desktop/src/components/Assistant/__tests__/` directory + 4 test files

### Deleted (8 files)
- `apps/desktop/src/components/Assistant/InputPill.tsx`
- `apps/desktop/src/components/Assistant/ConversationDrawer.tsx`
- `apps/desktop/src/lib/assistant/expressions.ts`
- `apps/desktop/src/assets/avatars/content.png`
- `apps/desktop/src/assets/avatars/smile.png`
- `apps/desktop/src/assets/avatars/questioning.png`
- `apps/desktop/src/assets/avatars/laughing.png`
- `apps/desktop/src/assets/avatars/confused.png`

### Added asset
- `apps/desktop/src/assets/avatars/manor_page.png` (Hana provides)

---

## Task 1: EphemeralLog component

**Files:**
- Create: `apps/desktop/src/components/Assistant/EphemeralLog.tsx`
- Test: `apps/desktop/src/components/Assistant/__tests__/EphemeralLog.test.tsx`

- [ ] **Step 1: Create the component**

Write `apps/desktop/src/components/Assistant/EphemeralLog.tsx`:

```tsx
import { useEffect, useState } from "react";

export interface Exchange {
  userText: string;
  assistantText: string;
  /** Unique key — typically the assistant message id, or a monotonic counter. */
  key: string | number;
}

interface Props {
  /** Most recent first. Expected up to 2 entries; component renders up to 2. */
  exchanges: Exchange[];
  onExpand: () => void;
  /** Override for tests. Default 10000ms. */
  fadeDelayMs?: number;
}

export function EphemeralLog({ exchanges, onExpand, fadeDelayMs = 10000 }: Props) {
  const [isVisible, setIsVisible] = useState(false);
  const latestKey = exchanges[0]?.key ?? null;

  useEffect(() => {
    if (latestKey == null) {
      setIsVisible(false);
      return;
    }
    setIsVisible(true);
    const t = window.setTimeout(() => setIsVisible(false), fadeDelayMs);
    return () => window.clearTimeout(t);
  }, [latestKey, fadeDelayMs]);

  if (!isVisible || exchanges.length === 0) return null;

  const visible = exchanges.slice(0, 2);

  return (
    <button
      type="button"
      onClick={onExpand}
      aria-label="Expand conversation history"
      style={{
        display: "block",
        textAlign: "left",
        width: "100%",
        padding: "6px 10px",
        marginBottom: 6,
        background: "rgba(249, 249, 247, 0.85)",
        border: "1px solid var(--hairline, #eee)",
        borderRadius: 4,
        fontSize: 12,
        lineHeight: 1.6,
        cursor: "pointer",
        transition: "opacity 400ms ease",
      }}
    >
      {visible.map((ex) => (
        <div key={ex.key}>
          <div style={{ color: "var(--ink-soft, #666)" }}>
            <strong>You:</strong> {ex.userText}
          </div>
          <div style={{ color: "var(--ink, #333)" }}>
            <strong>Manor:</strong> {ex.assistantText}
          </div>
        </div>
      ))}
    </button>
  );
}
```

- [ ] **Step 2: Write failing tests**

Write `apps/desktop/src/components/Assistant/__tests__/EphemeralLog.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup, act } from "@testing-library/react";
import { EphemeralLog } from "../EphemeralLog";

describe("EphemeralLog", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  it("renders nothing when exchanges are empty", () => {
    const { container } = render(
      <EphemeralLog exchanges={[]} onExpand={vi.fn()} />,
    );
    expect(container.textContent).toBe("");
  });

  it("renders last two exchanges with user + Manor labels", () => {
    render(
      <EphemeralLog
        exchanges={[
          { userText: "latest question", assistantText: "latest answer", key: 2 },
          { userText: "older question", assistantText: "older answer", key: 1 },
        ]}
        onExpand={vi.fn()}
      />,
    );
    expect(screen.getByText("latest question")).toBeInTheDocument();
    expect(screen.getByText("latest answer")).toBeInTheDocument();
    expect(screen.getByText("older question")).toBeInTheDocument();
  });

  it("caps at 2 exchanges even if more are passed", () => {
    render(
      <EphemeralLog
        exchanges={[
          { userText: "q3", assistantText: "a3", key: 3 },
          { userText: "q2", assistantText: "a2", key: 2 },
          { userText: "q1", assistantText: "a1", key: 1 },
        ]}
        onExpand={vi.fn()}
      />,
    );
    expect(screen.queryByText("q1")).toBeNull();
  });

  it("fades out after the configured delay", () => {
    render(
      <EphemeralLog
        exchanges={[{ userText: "hi", assistantText: "hello", key: 1 }]}
        onExpand={vi.fn()}
        fadeDelayMs={5000}
      />,
    );
    expect(screen.getByText("hi")).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(5001);
    });
    expect(screen.queryByText("hi")).toBeNull();
  });

  it("resets the fade timer when a new exchange arrives", () => {
    const { rerender } = render(
      <EphemeralLog
        exchanges={[{ userText: "first", assistantText: "first-reply", key: 1 }]}
        onExpand={vi.fn()}
        fadeDelayMs={5000}
      />,
    );
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    rerender(
      <EphemeralLog
        exchanges={[{ userText: "second", assistantText: "second-reply", key: 2 }]}
        onExpand={vi.fn()}
        fadeDelayMs={5000}
      />,
    );
    act(() => {
      vi.advanceTimersByTime(3000); // total 6000ms from first — but timer reset
    });
    expect(screen.getByText("second")).toBeInTheDocument();
  });

  it("calls onExpand when the log is clicked", () => {
    const onExpand = vi.fn();
    render(
      <EphemeralLog
        exchanges={[{ userText: "hi", assistantText: "hello", key: 1 }]}
        onExpand={onExpand}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Expand conversation history/ }));
    expect(onExpand).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 3: Run tests**

```
cd apps/desktop
pnpm test EphemeralLog
```
Expected: 6 PASS.

- [ ] **Step 4: Type-check**

```
pnpm tsc --noEmit
```
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/Assistant/EphemeralLog.tsx \
        apps/desktop/src/components/Assistant/__tests__/EphemeralLog.test.tsx
git commit -m "feat(assistant): EphemeralLog — fading last-exchange card"
```

---

## Task 2: ChatDock component

**Files:**
- Create: `apps/desktop/src/components/Assistant/ChatDock.tsx`
- Test: `apps/desktop/src/components/Assistant/__tests__/ChatDock.test.tsx`

- [ ] **Step 1: Create the component**

Write `apps/desktop/src/components/Assistant/ChatDock.tsx`:

```tsx
import { forwardRef, useState, KeyboardEvent } from "react";
import { Maximize2 } from "lucide-react";

interface Props {
  onSubmit: (content: string) => void;
  onExpand: () => void;
  /** When true, the dock hides (preserves existing InputPill behaviour
   *  when transient bubbles are present). */
  hidden?: boolean;
}

/**
 * Centered bottom input bar. Replaces InputPill.
 * Spans the window width minus ~88px on the right (avatar column)
 * and a small gap on the left. Owns its own input value.
 */
const ChatDock = forwardRef<HTMLInputElement, Props>(
  ({ onSubmit, onExpand, hidden = false }, ref) => {
    const [value, setValue] = useState("");

    const handleKey = (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        const trimmed = value.trim();
        if (trimmed.length === 0) return;
        onSubmit(trimmed);
        setValue("");
      } else if (e.key === "Escape") {
        (e.target as HTMLInputElement).blur();
      }
    };

    if (hidden) return null;

    return (
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          height: 32,
          padding: "0 10px",
          background: "var(--paper, #fff)",
          border: "1px solid var(--hairline, #d5d5d5)",
          borderRadius: 4,
          boxShadow: "0 1px 2px rgba(0,0,0,0.04)",
        }}
      >
        <input
          ref={ref}
          type="text"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKey}
          placeholder="Say something…"
          aria-label="Say something"
          style={{
            flex: 1,
            height: "100%",
            border: "none",
            outline: "none",
            background: "transparent",
            fontSize: "var(--text-md, 14px)",
            fontFamily: "inherit",
            color: "var(--ink, #333)",
          }}
        />
        <button
          type="button"
          onClick={onExpand}
          aria-label="Expand conversation history"
          title="Expand conversation"
          style={{
            background: "transparent",
            border: "none",
            padding: 4,
            cursor: "pointer",
            color: "var(--ink-soft, #888)",
            display: "flex",
            alignItems: "center",
          }}
        >
          <Maximize2 size={14} strokeWidth={1.8} />
        </button>
      </div>
    );
  },
);

ChatDock.displayName = "ChatDock";

export default ChatDock;
```

- [ ] **Step 2: Write failing tests**

Write `apps/desktop/src/components/Assistant/__tests__/ChatDock.test.tsx`:

```tsx
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import ChatDock from "../ChatDock";

describe("ChatDock", () => {
  afterEach(() => cleanup());

  it("renders the placeholder", () => {
    render(<ChatDock onSubmit={vi.fn()} onExpand={vi.fn()} />);
    expect(screen.getByPlaceholderText("Say something…")).toBeInTheDocument();
  });

  it("submits on Enter and clears the input", () => {
    const onSubmit = vi.fn();
    render(<ChatDock onSubmit={onSubmit} onExpand={vi.fn()} />);
    const input = screen.getByPlaceholderText("Say something…") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "hello" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).toHaveBeenCalledWith("hello");
    expect(input.value).toBe("");
  });

  it("does not submit when the trimmed value is empty", () => {
    const onSubmit = vi.fn();
    render(<ChatDock onSubmit={onSubmit} onExpand={vi.fn()} />);
    const input = screen.getByPlaceholderText("Say something…");
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("calls onExpand when the expand icon is clicked", () => {
    const onExpand = vi.fn();
    render(<ChatDock onSubmit={vi.fn()} onExpand={onExpand} />);
    fireEvent.click(screen.getByLabelText("Expand conversation history"));
    expect(onExpand).toHaveBeenCalledTimes(1);
  });

  it("renders nothing when hidden", () => {
    const { container } = render(
      <ChatDock onSubmit={vi.fn()} onExpand={vi.fn()} hidden />,
    );
    expect(container.textContent).toBe("");
  });

  it("blurs the input on Escape", () => {
    render(<ChatDock onSubmit={vi.fn()} onExpand={vi.fn()} />);
    const input = screen.getByPlaceholderText("Say something…") as HTMLInputElement;
    input.focus();
    expect(document.activeElement).toBe(input);
    fireEvent.keyDown(input, { key: "Escape" });
    expect(document.activeElement).not.toBe(input);
  });
});
```

- [ ] **Step 3: Run tests**

```
cd apps/desktop
pnpm test ChatDock
```
Expected: 6 PASS.

- [ ] **Step 4: Type-check**

```
pnpm tsc --noEmit
```
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/Assistant/ChatDock.tsx \
        apps/desktop/src/components/Assistant/__tests__/ChatDock.test.tsx
git commit -m "feat(assistant): ChatDock — centered rectangular input bar"
```

---

## Task 3: ChatHistoryPanel component

**Files:**
- Create: `apps/desktop/src/components/Assistant/ChatHistoryPanel.tsx`
- Test: `apps/desktop/src/components/Assistant/__tests__/ChatHistoryPanel.test.tsx`

### Context

Port the existing `Message` sub-component + the scrollable-list + pending-message-indicator logic from `ConversationDrawer.tsx`. Do NOT try to refactor `ConversationDrawer.tsx` — it gets deleted in Task 5. Copy just the parts we need.

Read `apps/desktop/src/components/Assistant/ConversationDrawer.tsx` before starting — particularly the `Message` sub-component (around line 17-51) and the scroll-to-bottom effect — so the ported code matches the existing visual style.

- [ ] **Step 1: Create the component**

Write `apps/desktop/src/components/Assistant/ChatHistoryPanel.tsx`:

```tsx
import { useEffect, useRef } from "react";
import { MessageSquare, User, Minimize2 } from "lucide-react";
import type { Message as AssistantMessage } from "../../lib/assistant/ipc";

interface Props {
  isOpen: boolean;
  messages: AssistantMessage[];
  onCollapse: () => void;
}

function MessageRow({ role, content }: { role: "user" | "assistant"; content: string }) {
  const Icon = role === "user" ? User : MessageSquare;
  const label = role === "user" ? "You" : "Nell";
  return (
    <div style={{ marginBottom: 14 }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          color: "var(--ink-soft, #888)",
          fontSize: "var(--text-xs, 11px)",
          fontWeight: 500,
          marginBottom: 4,
        }}
      >
        <Icon size={12} strokeWidth={1.8} />
        <span>{label}</span>
      </div>
      <div
        style={{
          fontSize: "var(--text-md, 14px)",
          color: "var(--ink, #333)",
          lineHeight: 1.55,
          paddingBottom: 10,
          borderBottom: "1px solid var(--hairline, #eee)",
          whiteSpace: "pre-wrap",
        }}
      >
        {content}
      </div>
    </div>
  );
}

export default function ChatHistoryPanel({ isOpen, messages, onCollapse }: Props) {
  const panelRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Outside click closes the panel.
  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node | null;
      if (panelRef.current && target && !panelRef.current.contains(target)) {
        onCollapse();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [isOpen, onCollapse]);

  // Escape closes the panel.
  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCollapse();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [isOpen, onCollapse]);

  // Auto-scroll to bottom on new messages or when opening.
  useEffect(() => {
    if (!isOpen) return;
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [isOpen, messages.length]);

  if (!isOpen) return null;

  return (
    <div
      ref={panelRef}
      role="dialog"
      aria-modal="false"
      aria-label="Conversation history"
      style={{
        display: "flex",
        flexDirection: "column",
        background: "var(--paper, #fff)",
        border: "1px solid var(--hairline, #ddd)",
        borderRadius: 4,
        boxShadow: "0 4px 12px rgba(0,0,0,0.06)",
        maxHeight: "calc(100vh - 180px)",
        marginBottom: 6,
        overflow: "hidden",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "8px 12px",
          borderBottom: "1px solid var(--hairline, #eee)",
        }}
      >
        <span
          style={{
            fontSize: 11,
            textTransform: "uppercase",
            letterSpacing: "0.5px",
            color: "var(--ink-soft, #888)",
          }}
        >
          Conversation
        </span>
        <button
          type="button"
          onClick={onCollapse}
          aria-label="Collapse conversation history"
          style={{
            background: "transparent",
            border: "none",
            padding: 4,
            cursor: "pointer",
            color: "var(--ink-soft, #888)",
            display: "flex",
            alignItems: "center",
          }}
        >
          <Minimize2 size={14} strokeWidth={1.8} />
        </button>
      </div>
      <div
        ref={scrollRef}
        style={{
          flex: 1,
          overflowY: "auto",
          padding: "12px 14px",
        }}
      >
        {messages.length === 0 ? (
          <div style={{ color: "var(--ink-soft, #888)", fontSize: 13, fontStyle: "italic" }}>
            No conversation yet. Type below to start.
          </div>
        ) : (
          messages.map((m) => <MessageRow key={m.id} role={m.role} content={m.content} />)
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Verify `Message` import from ipc**

The `Message` type lives in `apps/desktop/src/lib/assistant/ipc.ts`. Confirm the shape has `{ id: number, role: "user" | "assistant", content: string, ... }` — matching what the existing `ConversationDrawer.tsx` consumes. If the field names differ (e.g. `role` vs `sender`), adjust the `MessageRow` props accordingly.

- [ ] **Step 3: Write failing tests**

Write `apps/desktop/src/components/Assistant/__tests__/ChatHistoryPanel.test.tsx`:

```tsx
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import ChatHistoryPanel from "../ChatHistoryPanel";
import type { Message } from "../../../lib/assistant/ipc";

const msg = (id: number, role: "user" | "assistant", content: string): Message => ({
  id,
  conversation_id: 1,
  role,
  content,
  created_at: id,
  seen: true,
  proposal_id: null,
});

describe("ChatHistoryPanel", () => {
  afterEach(() => cleanup());

  it("renders nothing when isOpen is false", () => {
    const { container } = render(
      <ChatHistoryPanel
        isOpen={false}
        messages={[msg(1, "user", "hi")]}
        onCollapse={vi.fn()}
      />,
    );
    expect(container.textContent).toBe("");
  });

  it("renders messages when open", () => {
    render(
      <ChatHistoryPanel
        isOpen
        messages={[
          msg(1, "user", "hello manor"),
          msg(2, "assistant", "hello hana"),
        ]}
        onCollapse={vi.fn()}
      />,
    );
    expect(screen.getByText("hello manor")).toBeInTheDocument();
    expect(screen.getByText("hello hana")).toBeInTheDocument();
  });

  it("shows empty placeholder when messages is empty", () => {
    render(
      <ChatHistoryPanel isOpen messages={[]} onCollapse={vi.fn()} />,
    );
    expect(screen.getByText(/No conversation yet/)).toBeInTheDocument();
  });

  it("calls onCollapse when the ⤡ icon is clicked", () => {
    const onCollapse = vi.fn();
    render(
      <ChatHistoryPanel
        isOpen
        messages={[msg(1, "user", "hi")]}
        onCollapse={onCollapse}
      />,
    );
    fireEvent.click(screen.getByLabelText("Collapse conversation history"));
    expect(onCollapse).toHaveBeenCalledTimes(1);
  });

  it("calls onCollapse on Escape key", () => {
    const onCollapse = vi.fn();
    render(
      <ChatHistoryPanel
        isOpen
        messages={[msg(1, "user", "hi")]}
        onCollapse={onCollapse}
      />,
    );
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onCollapse).toHaveBeenCalledTimes(1);
  });

  it("calls onCollapse on outside mousedown", () => {
    const onCollapse = vi.fn();
    render(
      <>
        <div data-testid="outside">outside target</div>
        <ChatHistoryPanel
          isOpen
          messages={[msg(1, "user", "hi")]}
          onCollapse={onCollapse}
        />
      </>,
    );
    fireEvent.mouseDown(screen.getByTestId("outside"));
    expect(onCollapse).toHaveBeenCalledTimes(1);
  });

  it("does NOT call onCollapse when clicking inside the panel", () => {
    const onCollapse = vi.fn();
    render(
      <ChatHistoryPanel
        isOpen
        messages={[msg(1, "user", "hi inside")]}
        onCollapse={onCollapse}
      />,
    );
    fireEvent.mouseDown(screen.getByText("hi inside"));
    expect(onCollapse).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 4: Run tests**

```
cd apps/desktop
pnpm test ChatHistoryPanel
```
Expected: 7 PASS.

- [ ] **Step 5: Type-check**

```
pnpm tsc --noEmit
```
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/components/Assistant/ChatHistoryPanel.tsx \
        apps/desktop/src/components/Assistant/__tests__/ChatHistoryPanel.test.tsx
git commit -m "feat(assistant): ChatHistoryPanel — inline expandable scrollable history"
```

---

## Task 4: Avatar simplification

**Files:**
- Modify: `apps/desktop/src/components/Assistant/Avatar.tsx`
- Test: `apps/desktop/src/components/Assistant/__tests__/Avatar.test.tsx`

**Prerequisite check:** Before starting, confirm `apps/desktop/src/assets/avatars/manor_page.png` exists. If not, escalate — don't proceed.

```bash
ls -la apps/desktop/src/assets/avatars/manor_page.png
```

- [ ] **Step 1: Rewrite `Avatar.tsx`**

Replace the entire contents of `apps/desktop/src/components/Assistant/Avatar.tsx`:

```tsx
import manorPage from "../../assets/avatars/manor_page.png";

const NATURAL_RATIO = 274 / 400; // intrinsic w/h ratio preserved from old avatars

interface AvatarProps {
  /** Rendered height in px. Width is computed from the avatar's natural aspect ratio. */
  height?: number;
  onClick?: () => void;
}

export default function Avatar({ height = 72, onClick }: AvatarProps) {
  const width = Math.round(height * NATURAL_RATIO);

  const img = (
    <img
      src={manorPage}
      alt="Manor"
      width={width}
      height={height}
      style={{
        width,
        height,
        transform: "scaleX(-1)",
        userSelect: "none",
        pointerEvents: "none",
      }}
      draggable={false}
    />
  );

  if (!onClick) return img;

  return (
    <button
      onClick={onClick}
      aria-label="Open conversation with Manor"
      style={{
        border: "none",
        background: "transparent",
        padding: 0,
        cursor: "pointer",
        display: "inline-block",
        lineHeight: 0,
      }}
    >
      {img}
    </button>
  );
}
```

- [ ] **Step 2: Write failing tests**

Write `apps/desktop/src/components/Assistant/__tests__/Avatar.test.tsx`:

```tsx
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import Avatar from "../Avatar";

describe("Avatar", () => {
  afterEach(() => cleanup());

  it("renders an img with the Manor alt text", () => {
    render(<Avatar />);
    expect(screen.getByAltText("Manor")).toBeInTheDocument();
  });

  it("respects the height prop and computes width from the aspect ratio", () => {
    render(<Avatar height={100} />);
    const img = screen.getByAltText("Manor") as HTMLImageElement;
    expect(img.getAttribute("height")).toBe("100");
    // NATURAL_RATIO = 274/400 = 0.685 → width ≈ 69
    expect(img.getAttribute("width")).toBe("69");
  });

  it("defaults to height 72 when no prop is given", () => {
    render(<Avatar />);
    const img = screen.getByAltText("Manor") as HTMLImageElement;
    expect(img.getAttribute("height")).toBe("72");
  });

  it("wraps in a button when onClick is provided and fires on click", () => {
    const onClick = vi.fn();
    render(<Avatar onClick={onClick} />);
    fireEvent.click(screen.getByRole("button", { name: /Open conversation with Manor/ }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("renders img-only (no button) when onClick is absent", () => {
    render(<Avatar />);
    expect(screen.queryByRole("button")).toBeNull();
  });
});
```

- [ ] **Step 3: Run tests**

```
cd apps/desktop
pnpm test Avatar.test
```
Expected: 5 PASS.

- [ ] **Step 4: Type-check + verify full suite**

```
pnpm tsc --noEmit
pnpm test
```

Note: `tsc` may still complain about `Assistant.tsx` importing `expressionFor` / setting `setAvatarState` — those are broken until Task 5 cleans them up. If so, the Avatar tests themselves still pass in isolation. That's expected; commit Task 4 and move to Task 5 immediately.

If the frontend test runner also reports broken imports from `Assistant.tsx`-consuming tests, those likewise resolve in Task 5.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/Assistant/Avatar.tsx \
        apps/desktop/src/components/Assistant/__tests__/Avatar.test.tsx
git commit -m "feat(assistant): simplify Avatar to single manor_page.png"
```

---

## Task 5: Assistant.tsx wiring + state cleanup + file deletions

**Files:**
- Modify: `apps/desktop/src/components/Assistant/Assistant.tsx`
- Modify: `apps/desktop/src/lib/assistant/state.ts`
- Delete: `apps/desktop/src/components/Assistant/InputPill.tsx`
- Delete: `apps/desktop/src/components/Assistant/ConversationDrawer.tsx`
- Delete: `apps/desktop/src/lib/assistant/expressions.ts`
- Delete: `apps/desktop/src/assets/avatars/{content,smile,questioning,laughing,confused}.png`

### Context

`Assistant.tsx` currently imports `InputPill` + `ConversationDrawer`, calls `setAvatarState(...)` in 6 places (focus, submit start, thinking, speaking, idle, error), and mounts the conversation drawer via `useAssistantStore().drawerOpen`. This task rewires it end-to-end.

The store currently owns:
- `avatarState: AssistantState` + `setAvatarState` — REMOVE.
- `drawerOpen: boolean` + `setDrawerOpen` — REMOVE (panel open state moves to local state in `Assistant.tsx`).
- Everything else (messages, bubbles, unread count, etc.) — KEEP.

- [ ] **Step 1: Read the current Assistant.tsx + state.ts**

```bash
cat apps/desktop/src/components/Assistant/Assistant.tsx
cat apps/desktop/src/lib/assistant/state.ts
```

Familiarise yourself with the current shape. Every `setAvatarState(...)` call gets deleted. Every reference to `drawerOpen`/`setDrawerOpen` gets deleted or replaced with local state.

- [ ] **Step 2: Update `state.ts` — remove avatarState and drawerOpen**

In `apps/desktop/src/lib/assistant/state.ts`:

- Remove the import of `AssistantState` from `./expressions` (expressions.ts is being deleted in Step 6).
- Remove the `avatarState` field from the store interface + its initial value.
- Remove the `setAvatarState` action.
- Remove the `drawerOpen` field + its initial value.
- Remove the `setDrawerOpen` action.

After editing, the store should still have: `messages`, `transientBubbles`, `unreadCount`, and their associated actions (`hydrateMessages`, `enqueueBubble`, `appendBubbleContent`, `setBubbleTtl`, `beginAssistantMessage`, `appendAssistantToken`, `endAssistantMessage`, `addUserMessage`, `setUnreadCount`).

Run `pnpm tsc --noEmit` to surface every consumer that imports the removed fields. Then update each consumer — expected touch points:
- `Assistant.tsx` — all `setAvatarState(...)` calls + the `setDrawerOpen` call.
- Any file importing `AssistantState` from `expressions.ts` (shouldn't be any outside Avatar.tsx which was rewritten in Task 4, but check).
- Possibly a `BubbleLayer` or similar that imports `expressions.ts` for the `LAUGHING` PNG — if so, that import becomes dead and can be removed alongside.

If any consumer is a bigger refactor than expected, report BLOCKED — don't forge ahead.

- [ ] **Step 3: Rewrite `Assistant.tsx`**

Replace `apps/desktop/src/components/Assistant/Assistant.tsx` with:

```tsx
import { useEffect, useMemo, useRef, useState } from "react";
import Avatar from "./Avatar";
import ChatDock from "./ChatDock";
import ChatHistoryPanel from "./ChatHistoryPanel";
import { EphemeralLog, type Exchange } from "./EphemeralLog";
import UnreadBadge from "./UnreadBadge";
import { useAssistantStore } from "../../lib/assistant/state";
import { sendMessage, getUnreadCount, listMessages } from "../../lib/assistant/ipc";
import type { StreamChunk, Message as AssistantMessage } from "../../lib/assistant/ipc";
import { parseSlash } from "../../lib/today/slash";
import { addTask, listTasks, listProposals } from "../../lib/today/ipc";
import { addTransaction } from "../../lib/ledger/ipc";
import { useTodayStore } from "../../lib/today/state";
import { useOverlayStore } from "../../lib/overlay/state";
import { useSettingsStore } from "../../lib/settings/state";

function newBubbleId() {
  return Math.random().toString(36).slice(2, 10);
}

const MENU_WIDTH_PX = 70; // mirrors the main menu width; used for ChatDock left edge.
const AVATAR_COLUMN_PX = 104; // avatar column width + margin; ChatDock stops this far from the right.

export default function Assistant() {
  const dockRef = useRef<HTMLInputElement>(null);

  const enqueueBubble = useAssistantStore((s) => s.enqueueBubble);
  const appendBubbleContent = useAssistantStore((s) => s.appendBubbleContent);
  const transientBubbles = useAssistantStore((s) => s.transientBubbles);
  const beginAssistantMessage = useAssistantStore((s) => s.beginAssistantMessage);
  const appendAssistantToken = useAssistantStore((s) => s.appendAssistantToken);
  const endAssistantMessage = useAssistantStore((s) => s.endAssistantMessage);
  const addUserMessage = useAssistantStore((s) => s.addUserMessage);
  const setBubbleTtl = useAssistantStore((s) => s.setBubbleTtl);
  const setUnreadCount = useAssistantStore((s) => s.setUnreadCount);
  const hydrateMessages = useAssistantStore((s) => s.hydrateMessages);
  const messages = useAssistantStore((s) => s.messages);

  const setTodayTasks = useTodayStore((s) => s.setTasks);
  const setPendingProposals = useTodayStore((s) => s.setPendingProposals);

  // NEW: panel open state — local to Assistant, not in the store.
  const [isHistoryOpen, setIsHistoryOpen] = useState(false);

  // Initial load: hydrate recent messages + unread count.
  useEffect(() => {
    void (async () => {
      const msgs = await listMessages(100, 0);
      hydrateMessages(msgs);
      const n = await getUnreadCount();
      setUnreadCount(n);
    })();
  }, [hydrateMessages, setUnreadCount]);

  // Global ⌘/ focuses the dock.
  useEffect(() => {
    let lastFire = 0;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "/") {
        const now = Date.now();
        if (now - lastFire < 150) return;
        lastFire = now;
        e.preventDefault();
        dockRef.current?.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  // Auto-focus the dock when the panel opens.
  useEffect(() => {
    if (isHistoryOpen) dockRef.current?.focus();
  }, [isHistoryOpen]);

  // Derive last 2 exchanges for EphemeralLog from the messages store.
  const lastTwoExchanges = useMemo<Exchange[]>(() => {
    return extractExchanges(messages).slice(-2).reverse();
  }, [messages]);

  const handleSubmit = async (content: string) => {
    const slash = parseSlash(content);
    if (slash?.type === "task") {
      try {
        const task = await addTask(slash.title);
        useTodayStore.getState().upsertTask(task);
        useTodayStore.getState().showToast(`Added: ${slash.title}`);
        return;
      } catch (e) {
        enqueueBubble({
          id: newBubbleId(),
          kind: "error",
          content: `Couldn't add task: ${String(e)}`,
          messageId: null,
          ttlMs: 7000,
        });
        return;
      }
    }
    if (slash?.type === "spent") {
      try {
        const now = new Date();
        now.setHours(0, 0, 0, 0);
        await addTransaction({
          amountPence: slash.amountPence,
          currency: "GBP",
          description: slash.description,
          date: Math.floor(now.getTime() / 1000),
        });
        enqueueBubble({
          id: newBubbleId(),
          kind: "assistant",
          content: `Added: ${slash.description} (£${(Math.abs(slash.amountPence) / 100).toFixed(2)})`,
          messageId: null,
          ttlMs: 6000,
        });
        return;
      } catch (e) {
        enqueueBubble({
          id: newBubbleId(),
          kind: "error",
          content: `Couldn't add transaction: ${String(e)}`,
          messageId: null,
          ttlMs: 7000,
        });
        return;
      }
    }
    // Unknown slashes fall through to normal chat.

    const userBubbleId = newBubbleId();
    enqueueBubble({
      id: userBubbleId,
      kind: "user",
      content,
      messageId: null,
      ttlMs: 10000,
    });
    addUserMessage({
      id: -Date.now(),
      conversation_id: 1,
      role: "user",
      content,
      created_at: Date.now(),
      seen: true,
      proposal_id: null,
    });

    let assistantDbId: number | null = null;
    const assistantBubbleId = newBubbleId();

    const onEvent = (chunk: StreamChunk) => {
      if (chunk.type === "Started") {
        assistantDbId = chunk.value;
        beginAssistantMessage(assistantDbId);
        enqueueBubble({
          id: assistantBubbleId,
          kind: "assistant",
          content: "",
          messageId: assistantDbId,
          ttlMs: 120000,
        });
      } else if (chunk.type === "Token") {
        if (assistantDbId === null) {
          assistantDbId = -Date.now();
          beginAssistantMessage(assistantDbId);
          enqueueBubble({
            id: assistantBubbleId,
            kind: "assistant",
            content: "",
            messageId: assistantDbId,
            ttlMs: 12000,
          });
        }
        appendAssistantToken(chunk.value);
        appendBubbleContent(assistantBubbleId, chunk.value);
      } else if (chunk.type === "Proposal") {
        void listProposals("pending").then(setPendingProposals);
        void listTasks().then(setTodayTasks);
      } else if (chunk.type === "Done") {
        endAssistantMessage();
        setBubbleTtl(assistantBubbleId, 8000);
        void getUnreadCount().then(setUnreadCount);
      } else if (chunk.type === "Error") {
        const errorMessage =
          chunk.value === "OllamaUnreachable"
            ? "I can't reach Ollama. Is it running?"
            : chunk.value === "ModelMissing"
              ? "I need the model `qwen2.5:7b-instruct`. Run `./scripts/install-ollama.sh`."
              : chunk.value === "Interrupted"
                ? "The reply was interrupted — check Ollama."
                : "Something went wrong. Check the logs.";
        enqueueBubble({
          id: newBubbleId(),
          kind: "error",
          content: errorMessage,
          messageId: null,
          ttlMs: 12000,
        });
      }
    };

    try {
      await sendMessage(content, onEvent);
    } catch (e) {
      enqueueBubble({
        id: newBubbleId(),
        kind: "error",
        content: `IPC error: ${String(e)}`,
        messageId: null,
        ttlMs: 7000,
      });
    }
  };

  const overlayCount = useOverlayStore((s) => s.count);
  const settingsOpen = useSettingsStore((s) => s.modalOpen);
  const minimized = overlayCount > 0 || settingsOpen;

  const dockHidden = transientBubbles.length > 0;

  return (
    <>
      {/* Bottom bar — ChatDock centered, full width minus menu + avatar columns. */}
      <div
        style={{
          position: "fixed",
          left: MENU_WIDTH_PX + 16,
          right: AVATAR_COLUMN_PX,
          bottom: 16,
          zIndex: 999,
          pointerEvents: dockHidden && !isHistoryOpen ? "none" : "auto",
        }}
      >
        {!isHistoryOpen && (
          <EphemeralLog
            exchanges={lastTwoExchanges}
            onExpand={() => setIsHistoryOpen(true)}
          />
        )}
        <ChatHistoryPanel
          isOpen={isHistoryOpen}
          messages={messages}
          onCollapse={() => setIsHistoryOpen(false)}
        />
        <ChatDock
          ref={dockRef}
          onSubmit={handleSubmit}
          onExpand={() => setIsHistoryOpen(true)}
          hidden={dockHidden}
        />
      </div>

      {/* Avatar — unchanged corner position, shrinks on overlay. */}
      <div
        style={{
          position: "fixed",
          bottom: 16,
          right: 16,
          display: "flex",
          flexDirection: "column",
          alignItems: "flex-end",
          gap: 8,
          zIndex: 1000,
          transform: minimized ? "translate(8px, 8px)" : "translate(0, 0)",
          transition: "transform var(--duration-med) var(--ease-out)",
        }}
      >
        {!minimized && <UnreadBadgeWithAnchor />}
        <Avatar
          height={minimized ? 40 : 72}
          onClick={() => setIsHistoryOpen((v) => !v)}
        />
      </div>
    </>
  );
}

/** Pairs consecutive user → assistant messages into exchanges. Tolerates
 *  trailing user messages (pending response) by dropping them — the
 *  EphemeralLog should only show completed exchanges. */
function extractExchanges(messages: AssistantMessage[]): Exchange[] {
  const out: Exchange[] = [];
  for (let i = 0; i < messages.length - 1; i++) {
    const a = messages[i];
    const b = messages[i + 1];
    if (a.role === "user" && b.role === "assistant") {
      out.push({ userText: a.content, assistantText: b.content, key: b.id });
      i++; // skip past the paired assistant message
    }
  }
  return out;
}

function UnreadBadgeWithAnchor() {
  return (
    <div style={{ position: "relative" }}>
      <div style={{ position: "absolute", top: -6, right: -6 }}>
        <UnreadBadge />
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Delete retired files**

```bash
git rm apps/desktop/src/components/Assistant/InputPill.tsx
git rm apps/desktop/src/components/Assistant/ConversationDrawer.tsx
git rm apps/desktop/src/lib/assistant/expressions.ts
git rm apps/desktop/src/assets/avatars/content.png
git rm apps/desktop/src/assets/avatars/smile.png
git rm apps/desktop/src/assets/avatars/questioning.png
git rm apps/desktop/src/assets/avatars/laughing.png
git rm apps/desktop/src/assets/avatars/confused.png
```

- [ ] **Step 5: Run tsc — fix any remaining dangling imports**

```
cd apps/desktop
pnpm tsc --noEmit
```

Any errors that pop up are dangling imports from files that referenced deleted modules. Fix them one by one. Common suspects:
- `BubbleLayer.tsx` or similar — if it imports `LAUGHING` from expressions, remove that import + usage.
- Any test file that imports from `ConversationDrawer.tsx` — rewrite or delete.
- Any test file that imports from `InputPill.tsx` — same.

- [ ] **Step 6: Run full test suite**

```
pnpm test
```
Expected: all tests PASS. The deleted components had no dedicated tests (verified during planning — ConversationDrawer had no test file); any tests that previously covered InputPill / ConversationDrawer behaviour are now covered by the new ChatDock / ChatHistoryPanel tests.

If a test file that imports a deleted module exists and isn't trivially fixable, report BLOCKED.

- [ ] **Step 7: Build**

```
pnpm build
```
Expected: success.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(assistant): swap InputPill/ConversationDrawer for ChatDock/Panel, drop expression states"
```

---

## Task 6: Full green battery + manual QA + merge handoff

- [ ] **Step 1: Full test + lint battery**

From the worktree root:

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cd apps/desktop && pnpm tsc --noEmit && pnpm test && pnpm build
cd ..
```

All must pass. Record test counts.

- [ ] **Step 2: Manual QA scenario**

`cd apps/desktop && pnpm tauri dev`.

1. Avatar renders `manor_page.png` bottom-right at 72px, mirrored, single image. Does NOT change on any interaction.
2. Type "hello" + Enter. Message sends. `EphemeralLog` card appears above the input showing the user message + Manor's streaming reply.
3. Wait 10 seconds with no new activity — card fades out.
4. Send another message. Card reappears immediately; fade timer reset.
5. Click the ⤢ icon on the right of the input. Panel expands upward. Scrolls if history is long enough.
6. Click anywhere in main content (Today/Hearth/Bones/menu). Panel closes.
7. Open panel again → press Escape. Panel closes.
8. Resize the window shorter. Panel respects `max-height: calc(100vh - 180px)`; scrollbar handles overflow.
9. Open an AssetDetail drawer (or any overlay). Avatar shrinks to 40px. Bottom bar visibility respects existing overlay-minimize rules.
10. Stop Ollama locally (or send a message when it's already unreachable). Error bubble appears; input value preserved for retry.

Report which steps pass and which fail.

- [ ] **Step 3: Report ready-to-merge**

Collect:
1. Branch history: `git log --oneline main..HEAD` — expect ~6 commits.
2. Test counts: Rust workspace + frontend.
3. Build result + bundle size.
4. Manual QA outcome.
5. Any notes/concerns.

- [ ] **Step 4: Do NOT merge**

Merge + worktree cleanup are user-driven. Surface the branch state and wait for authorization.

---

## Definition of done recap

- `manor_page.png` lives in `apps/desktop/src/assets/avatars/` (Hana drops this in before execution).
- `Avatar.tsx` renders the single image; 5 old state PNGs + `expressions.ts` removed; `avatarState` field + `setAvatarState` removed from the store.
- `ChatDock.tsx` renders centered at bottom, spans menu → avatar column, square-cornered, ~32px tall.
- `EphemeralLog.tsx` shows last up-to-2 exchanges, fades after 10s, clicks expand.
- `ChatHistoryPanel.tsx` expands, scrolls, closes on ⤡ / outside-click / Escape.
- `InputPill.tsx` + `ConversationDrawer.tsx` deleted; all imports updated.
- `Assistant.tsx` host owns `isHistoryOpen` state; all `setAvatarState(...)` calls removed.
- 4 new RTL test files pass; existing frontend tests still pass.
- `pnpm tsc --noEmit` clean. `pnpm test` green. `pnpm build` green.
- Manual QA walked through.

---

*End of plan.*
