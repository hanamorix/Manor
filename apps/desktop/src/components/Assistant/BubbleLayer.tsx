import { useEffect, useRef, useState } from "react";
import { useAssistantStore } from "../../lib/assistant/state";
import type { TransientBubble } from "../../lib/assistant/state";
import { createTtlTimer, TtlTimer } from "../../lib/assistant/bubble-ttl";
import { getUnreadCount, markSeen } from "../../lib/assistant/ipc";

function bubbleColors(kind: TransientBubble["kind"]): {
  background: string;
  color: string;
  alignSelf: "flex-start" | "flex-end";
  borderRadius: string;
  border?: string;
} {
  switch (kind) {
    case "user":
      return {
        // Subtle iMessage-style vertical gradient on the blue
        background: "linear-gradient(180deg, #2683FE 0%, #0866EF 100%)",
        color: "white",
        alignSelf: "flex-end",
        borderRadius: "18px 18px 4px 18px",
      };
    case "assistant":
      return {
        background: "linear-gradient(180deg, #4ED365 0%, #2BB94A 100%)",
        color: "white",
        alignSelf: "flex-start",
        borderRadius: "18px 18px 18px 4px",
      };
    case "error":
      return {
        background: "rgba(255, 59, 48, 0.1)",
        color: "var(--imessage-red)",
        alignSelf: "flex-start",
        borderRadius: "var(--radius-lg) var(--radius-lg) var(--radius-lg) 4px",
        border: "1px solid rgba(255, 59, 48, 0.4)",
      };
  }
}

async function markBubbleSeenIfAssistant(b: TransientBubble, refreshUnread: (n: number) => void) {
  if (b.kind === "assistant" && b.messageId !== null && b.messageId > 0) {
    await markSeen([b.messageId]);
    const n = await getUnreadCount();
    refreshUnread(n);
  }
}

export default function BubbleLayer() {
  const bubbles = useAssistantStore((s) => s.transientBubbles);
  const dismissBubble = useAssistantStore((s) => s.dismissBubble);
  const setDrawerOpen = useAssistantStore((s) => s.setDrawerOpen);
  const setUnreadCount = useAssistantStore((s) => s.setUnreadCount);
  const drawerOpen = useAssistantStore((s) => s.drawerOpen);

  if (drawerOpen) return null;

  return (
    <div
      style={{
        position: "fixed",
        bottom: 130,
        right: 16,
        display: "flex",
        flexDirection: "column",
        gap: 4,
        alignItems: "flex-end",
        maxWidth: 460,
        pointerEvents: "none",
        zIndex: 500,
      }}
    >
      {bubbles.map((b) => (
        <Bubble
          key={b.id}
          bubble={b}
          onDismiss={async () => {
            // Natural fade marks assistant messages as seen too — looking at the
            // bubble for its TTL counts as "saw it". Click-to-open also marks via
            // the onClick path below.
            await markBubbleSeenIfAssistant(b, setUnreadCount);
            dismissBubble(b.id);
          }}
          onClick={async () => {
            await markBubbleSeenIfAssistant(b, setUnreadCount);
            setDrawerOpen(true);
          }}
        />
      ))}
    </div>
  );
}

interface BubbleProps {
  bubble: TransientBubble;
  onDismiss: () => void;
  onClick: () => void;
}

const MAX_BUBBLE_LINES = 6;

function Bubble({ bubble, onDismiss, onClick }: BubbleProps) {
  const timerRef = useRef<TtlTimer | null>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const [overflowing, setOverflowing] = useState(false);

  // Always-current refs so the timer effect doesn't restart on every parent
  // re-render — content updates would otherwise reset the TTL countdown.
  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;

  useEffect(() => {
    const timer = createTtlTimer(bubble.ttlMs, () => {
      void onDismissRef.current();
    });
    timerRef.current = timer;
    timer.start();
    return () => timer.cancel();
  }, [bubble.id, bubble.ttlMs]);

  // Re-measure overflow on every content update (covers streaming).
  useEffect(() => {
    const el = contentRef.current;
    if (!el) return;
    setOverflowing(el.scrollHeight > el.clientHeight + 1);
  }, [bubble.content]);

  const c = bubbleColors(bubble.kind);

  return (
    <div
      role="button"
      tabIndex={0}
      onMouseEnter={() => timerRef.current?.pause()}
      onMouseLeave={() => timerRef.current?.resumeWith(5000)}
      onClick={onClick}
      style={{
        background: c.background,
        color: c.color,
        alignSelf: c.alignSelf,
        borderRadius: c.borderRadius,
        border: c.border,
        padding: "10px 14px",
        fontSize: 15,
        lineHeight: 1.35,
        maxWidth: 420,
        whiteSpace: "pre-wrap",
        wordBreak: "break-word",
        boxShadow: "var(--shadow-md)",
        pointerEvents: "auto",
        cursor: "pointer",
        animation: "bubbleIn 200ms ease-out",
      }}
    >
      <div
        ref={contentRef}
        style={{
          display: "-webkit-box",
          WebkitLineClamp: MAX_BUBBLE_LINES,
          WebkitBoxOrient: "vertical",
          overflow: "hidden",
        }}
      >
        {bubble.content}
      </div>
      {overflowing && (
        <div
          style={{
            marginTop: 6,
            fontSize: 12,
            fontWeight: 700,
            opacity: 0.85,
            letterSpacing: 0.2,
          }}
        >
          See more →
        </div>
      )}
    </div>
  );
}
