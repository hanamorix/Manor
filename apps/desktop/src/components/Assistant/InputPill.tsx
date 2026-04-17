import { forwardRef, useState, KeyboardEvent } from "react";

interface InputPillProps {
  onSubmit: (content: string) => void;
  onFocus?: () => void;
  onBlur?: () => void;
}

const InputPill = forwardRef<HTMLInputElement, InputPillProps>(
  ({ onSubmit, onFocus, onBlur }, ref) => {
    const [value, setValue] = useState("");
    const [focused, setFocused] = useState(false);

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

    return (
      <div
        style={{
          position: "relative",
          width: 320,
        }}
      >
        <input
          ref={ref}
          type="text"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKey}
          onFocus={() => {
            setFocused(true);
            onFocus?.();
          }}
          onBlur={() => {
            setFocused(false);
            onBlur?.();
          }}
          placeholder="Say something…"
          style={{
            width: "100%",
            padding: "8px 14px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--hairline)",
            background: "var(--paper)",
            fontSize: "var(--text-md)",
            fontFamily: "inherit",
            color: "var(--ink)",
            boxShadow: focused ? "var(--shadow-md)" : "var(--shadow-sm)",
            transition: "box-shadow 150ms ease",
          }}
        />
      </div>
    );
  },
);

InputPill.displayName = "InputPill";
export default InputPill;
