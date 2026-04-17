import { useEffect, useState, useRef } from "react";
import { MessageSquare, User, Send } from "lucide-react";
import { useAssistantStore } from "../../lib/assistant/state";
import { listMessages, markSeen, getUnreadCount } from "../../lib/assistant/ipc";
import { parseSlash } from "../../lib/today/slash";
import { addTask } from "../../lib/today/ipc";
import { useTodayStore } from "../../lib/today/state";
import { Button } from "../../lib/ui";

interface ConversationDrawerProps {
  onSubmit: (content: string) => void;
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function Message({ role, content }: { role: "user" | "assistant"; content: string }) {
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
          whiteSpace: "pre-wrap",
        }}
      >
        {content}
      </div>
    </div>
  );
}

function TypingDots() {
  return (
    <div
      style={{
        display: "inline-flex",
        gap: 4,
        alignItems: "center",
        color: "var(--ink-soft)",
        padding: "6px 0",
      }}
    >
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

// ---------------------------------------------------------------------------
// Main drawer
// ---------------------------------------------------------------------------

export default function ConversationDrawer({ onSubmit }: ConversationDrawerProps) {
  const drawerOpen = useAssistantStore((s) => s.drawerOpen);
  const setDrawerOpen = useAssistantStore((s) => s.setDrawerOpen);
  const messages = useAssistantStore((s) => s.messages);
  const hydrateMessages = useAssistantStore((s) => s.hydrateMessages);
  const setUnreadCount = useAssistantStore((s) => s.setUnreadCount);
  const avatarState = useAssistantStore((s) => s.avatarState);

  const [input, setInput] = useState("");
  const bodyRef = useRef<HTMLDivElement>(null);

  const isGenerating = avatarState === "thinking" || avatarState === "speaking";

  // On drawer open: hydrate messages, mark all unread as seen, reset unread count.
  useEffect(() => {
    if (!drawerOpen) return;
    void (async () => {
      const msgs = await listMessages(100, 0);
      hydrateMessages(msgs);
      const unreadIds = msgs.filter((m) => m.role === "assistant" && !m.seen).map((m) => m.id);
      if (unreadIds.length > 0) await markSeen(unreadIds);
      const n = await getUnreadCount();
      setUnreadCount(n);
    })();
  }, [drawerOpen, hydrateMessages, setUnreadCount]);

  // Auto-scroll to bottom on new messages when drawer is open.
  useEffect(() => {
    if (!drawerOpen) return;
    const el = bodyRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [drawerOpen, messages]);

  // Escape to close.
  useEffect(() => {
    if (!drawerOpen) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") setDrawerOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [drawerOpen, setDrawerOpen]);

  if (!drawerOpen) return null;

  const handleSend = () => {
    const t = input.trim();
    if (t.length === 0) return;
    const slash = parseSlash(t);
    if (slash?.type === "task") {
      void addTask(slash.title).then((task) => {
        useTodayStore.getState().upsertTask(task);
        useTodayStore.getState().showToast(`Added: ${slash.title}`);
      });
      setInput("");
      return;
    }
    onSubmit(t);
    setInput("");
  };

  return (
    <>
      {/* click-away overlay */}
      <div
        onClick={() => setDrawerOpen(false)}
        style={{
          position: "fixed",
          inset: 0,
          background: "var(--scrim)",
          zIndex: 900,
        }}
      />

      <aside
        style={{
          position: "fixed",
          top: 0,
          right: 0,
          bottom: 0,
          width: "min(420px, 45vw)",
          background: "var(--surface)",
          borderLeft: "1px solid var(--hairline-strong)",
          display: "flex",
          flexDirection: "column",
          zIndex: 1001,
          animation: "drawerIn 250ms ease-out",
        }}
      >
        {/* Header */}
        <header
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            padding: "14px 16px",
            borderBottom: "1px solid var(--hairline)",
          }}
        >
          <strong style={{ flex: 1, fontSize: 15 }}>Full Conversation</strong>
          <button
            onClick={() => setDrawerOpen(false)}
            aria-label="Close"
            style={{
              border: "none",
              background: "transparent",
              fontSize: 20,
              cursor: "pointer",
              color: "var(--ink)",
              lineHeight: 1,
            }}
          >
            ×
          </button>
        </header>

        {/* Message list */}
        <div
          ref={bodyRef}
          style={{
            flex: 1,
            overflowY: "auto",
            padding: "16px 18px",
          }}
        >
          {messages.length === 0 && (
            <Message role="assistant" content="Hi, I'm Nell. Ask me anything." />
          )}

          {messages.map((m) => (
            <Message key={m.id} role={m.role as "user" | "assistant"} content={m.content} />
          ))}

          {isGenerating && <TypingDots />}
        </div>

        {/* Input composer */}
        <div
          style={{
            display: "flex",
            gap: 6,
            alignItems: "flex-end",
            padding: "12px 14px",
            borderTop: "1px solid var(--hairline)",
            background: "var(--surface)",
          }}
        >
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
              background: "var(--paper)",
              border: "1px solid var(--hairline-strong)",
              borderRadius: "var(--radius-md)",
              padding: "8px 10px",
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSend();
              }
            }}
          />
          <Button variant="primary" icon={Send} onClick={handleSend}>
            Send
          </Button>
        </div>
      </aside>
    </>
  );
}
