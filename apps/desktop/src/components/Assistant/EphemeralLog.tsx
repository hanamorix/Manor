export interface Exchange {
  userText: string;
  assistantText: string;
  /** Unique key — typically the assistant message id, or a monotonic counter. */
  key: string | number;
}

interface Props {
  /** Most recent first. Only the first entry (latest) is rendered. */
  exchanges: Exchange[];
  onExpand: () => void;
  /** Parent controls visibility so the fade timer can start only after
   *  streaming completes (not after it begins). */
  visible: boolean;
}

export function EphemeralLog({ exchanges, onExpand, visible }: Props) {
  if (!visible || exchanges.length === 0) return null;

  const latest = exchanges[0];

  return (
    <button
      type="button"
      onClick={onExpand}
      aria-label="Expand conversation history"
      style={{
        textAlign: "left",
        width: "100%",
        padding: "6px 10px",
        marginBottom: 6,
        background: "rgba(249, 249, 247, 0.85)",
        border: "1px solid var(--hairline, #eee)",
        borderRadius: 4,
        fontSize: 12,
        lineHeight: 1.6,
        color: "var(--ink, #333)",
        cursor: "pointer",
        transition: "opacity 400ms ease",
        display: "-webkit-box",
        WebkitLineClamp: 3,
        WebkitBoxOrient: "vertical",
        overflow: "hidden",
      }}
    >
      {latest.assistantText}
    </button>
  );
}
