import { useEffect, useState, useRef, KeyboardEvent } from "react";
import { useAssistantStore } from "../../lib/assistant/state";
import { listMessages, markSeen, getUnreadCount } from "../../lib/assistant/ipc";
import { parseSlash } from "../../lib/today/slash";
import { addTask } from "../../lib/today/ipc";
import { useTodayStore } from "../../lib/today/state";

interface ConversationDrawerProps {
  onSubmit: (content: string) => void;
}

export default function ConversationDrawer({ onSubmit }: ConversationDrawerProps) {
  const drawerOpen = useAssistantStore((s) => s.drawerOpen);
  const setDrawerOpen = useAssistantStore((s) => s.setDrawerOpen);
  const messages = useAssistantStore((s) => s.messages);
  const hydrateMessages = useAssistantStore((s) => s.hydrateMessages);
  const setUnreadCount = useAssistantStore((s) => s.setUnreadCount);

  const [value, setValue] = useState("");
  const bodyRef = useRef<HTMLDivElement>(null);

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

  useEffect(() => {
    if (!drawerOpen) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") setDrawerOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [drawerOpen, setDrawerOpen]);

  if (!drawerOpen) return null;

  const submit = () => {
    const t = value.trim();
    if (t.length === 0) return;
    const slash = parseSlash(t);
    if (slash?.type === "task") {
      void addTask(slash.title).then((task) => {
        useTodayStore.getState().upsertTask(task);
        // TODO(Task 14): showToast here too
      });
      setValue("");
      return;
    }
    onSubmit(t);
    setValue("");
  };

  const onInputKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  };

  return (
    <>
      {/* click-away overlay */}
      <div
        onClick={() => setDrawerOpen(false)}
        style={{
          position: "fixed",
          inset: 0,
          background: "transparent",
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
          background: "var(--paper)",
          boxShadow: "var(--shadow-lg)",
          display: "flex",
          flexDirection: "column",
          zIndex: 1001,
          animation: "drawerIn 250ms ease-out",
        }}
      >
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

        <div
          ref={bodyRef}
          style={{
            flex: 1,
            overflowY: "auto",
            padding: 14,
            display: "flex",
            flexDirection: "column",
            gap: 6,
          }}
        >
          {messages.length === 0 && (
            <div
              style={{
                alignSelf: "flex-start",
                background: "var(--imessage-green)",
                color: "white",
                padding: "8px 12px",
                borderRadius: "var(--radius-lg) var(--radius-lg) var(--radius-lg) 4px",
                maxWidth: "80%",
                fontSize: 14,
                boxShadow: "var(--shadow-sm)",
              }}
            >
              Hi, I'm Manor. Ask me anything.
            </div>
          )}

          {messages.map((m) => (
            <div
              key={m.id}
              style={{
                alignSelf: m.role === "user" ? "flex-end" : "flex-start",
                background:
                  m.role === "user" ? "var(--imessage-blue)" : "var(--imessage-green)",
                color: "white",
                padding: "8px 12px",
                borderRadius:
                  m.role === "user"
                    ? "var(--radius-lg) var(--radius-lg) 4px var(--radius-lg)"
                    : "var(--radius-lg) var(--radius-lg) var(--radius-lg) 4px",
                maxWidth: "80%",
                fontSize: 14,
                whiteSpace: "pre-wrap",
                boxShadow: "var(--shadow-sm)",
              }}
            >
              {m.content}
            </div>
          ))}
        </div>

        <footer
          style={{
            padding: 10,
            borderTop: "1px solid var(--hairline)",
            display: "flex",
            gap: 8,
          }}
        >
          <textarea
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={onInputKey}
            placeholder="Say something…"
            rows={2}
            style={{
              flex: 1,
              padding: "8px 12px",
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--hairline)",
              fontFamily: "inherit",
              fontSize: 14,
              resize: "none",
              outline: "none",
            }}
          />
          <button
            onClick={submit}
            style={{
              padding: "8px 14px",
              borderRadius: "var(--radius-md)",
              border: "none",
              background: "var(--imessage-blue)",
              color: "white",
              fontWeight: 600,
              cursor: "pointer",
            }}
          >
            Send
          </button>
        </footer>
      </aside>
    </>
  );
}
