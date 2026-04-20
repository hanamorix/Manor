import { useEffect, useRef } from "react";
import { MessageSquare, User, Minimize2 } from "lucide-react";
import type { Message as AssistantMessage, Role } from "../../lib/assistant/ipc";

interface Props {
  isOpen: boolean;
  messages: AssistantMessage[];
  onCollapse: () => void;
}

function MessageRow({ role, content }: { role: Role; content: string }) {
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
