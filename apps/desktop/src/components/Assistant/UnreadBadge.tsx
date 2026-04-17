import { useAssistantStore } from "../../lib/assistant/state";

export default function UnreadBadge() {
  const count = useAssistantStore((s) => s.unreadCount);
  if (count === 0) return null;

  const label = count > 9 ? "9+" : String(count);

  return (
    <div
      aria-label={`${count} unread message${count === 1 ? "" : "s"}`}
      style={{
        minWidth: 20,
        height: 20,
        padding: "0 6px",
        borderRadius: "var(--radius-pill)",
        background: "var(--ink)",
        color: "var(--action-fg)",
        fontSize: 11,
        fontWeight: 700,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        boxShadow: "var(--shadow-sm)",
        pointerEvents: "none",
      }}
    >
      {label}
    </div>
  );
}
