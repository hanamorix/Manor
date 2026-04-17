// Shared inline styles for Settings tabs + Safety panels.
// Keeps every tab consistent with Manor's paper+ink theme — no black boxes.

import type { CSSProperties } from "react";

export const TEXT_MUTED = "var(--ink-soft)";
export const TEXT_SECONDARY = "var(--ink-soft)";

export const settingsCard: CSSProperties = {
  background: "var(--surface)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-sm)",
  padding: 12,
};

export const settingsCardMuted: CSSProperties = {
  background: "var(--paper-muted)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-sm)",
  padding: 10,
};

export const settingsListRow: CSSProperties = {
  padding: 8,
  borderRadius: "var(--radius-sm)",
  background: "var(--surface)",
  border: "1px solid var(--hairline)",
};

export const settingsCodeBlock: CSSProperties = {
  fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
  fontSize: 11,
  color: "var(--ink)",
  background: "var(--paper-muted)",
  border: "1px solid var(--hairline)",
  padding: 8,
  borderRadius: 4,
  overflowX: "auto",
  margin: 0,
};

export const settingsStatusGood: CSSProperties = {
  padding: 10,
  border: "1px solid rgba(52, 199, 89, 0.3)",
  background: "rgba(52, 199, 89, 0.08)",
  borderRadius: "var(--radius-sm)",
};

export const settingsStatusWarn: CSSProperties = {
  padding: 10,
  border: "1px solid rgba(255, 149, 0, 0.35)",
  background: "rgba(255, 149, 0, 0.08)",
  borderRadius: "var(--radius-sm)",
};

export const settingsStatusDanger: CSSProperties = {
  padding: 10,
  border: "1px solid rgba(255, 59, 48, 0.35)",
  background: "rgba(255, 59, 48, 0.08)",
  borderRadius: "var(--radius-sm)",
};

export const dangerButton: CSSProperties = {
  background: "var(--ink)",
  color: "var(--action-fg)",
  border: "none",
  borderRadius: "var(--radius-md)",
  padding: "6px 14px",
  fontSize: 12,
  fontWeight: 600,
  cursor: "pointer",
  fontFamily: "inherit",
};

// Semantic color tokens
export const COLOR_AMBER   = "#b36b00";
export const COLOR_SUCCESS = "var(--ink-success)";
export const COLOR_DANGER  = "var(--ink-danger)";
