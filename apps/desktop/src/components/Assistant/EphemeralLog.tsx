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
