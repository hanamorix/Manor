import { useEffect, useRef, useState } from "react";

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
  /** Override for tests. Default 10000ms. */
  fadeDelayMs?: number;
}

/**
 * Shows the latest Manor response for a short time, then fades.
 *
 * Only reacts to messages that arrive AFTER the component mounts —
 * historical messages (present on page load) stay hidden. This prevents
 * the last message from lingering on every refresh.
 */
export function EphemeralLog({ exchanges, onExpand, fadeDelayMs = 10000 }: Props) {
  const [isVisible, setIsVisible] = useState(false);
  const latestKey = exchanges[0]?.key ?? null;
  const lastSeenKeyRef = useRef<string | number | null>(null);
  const hasMountedRef = useRef(false);

  useEffect(() => {
    // On first mount: record the current latest as already seen so the
    // historical tail of the conversation doesn't flash on page load.
    if (!hasMountedRef.current) {
      hasMountedRef.current = true;
      lastSeenKeyRef.current = latestKey;
      return;
    }
    // On subsequent updates, only react to genuinely new keys.
    if (latestKey == null || latestKey === lastSeenKeyRef.current) {
      return;
    }
    lastSeenKeyRef.current = latestKey;
    setIsVisible(true);
    const t = window.setTimeout(() => setIsVisible(false), fadeDelayMs);
    return () => window.clearTimeout(t);
  }, [latestKey, fadeDelayMs]);

  if (!isVisible || exchanges.length === 0) return null;

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
