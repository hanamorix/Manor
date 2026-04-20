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
