// Shared inline styles for the first-launch wizard. Keeps the steps consistent
// with Manor's paper/ink design tokens.

import type { CSSProperties } from "react";

export const wizardStatusCardGood: CSSProperties = {
  padding: 12,
  border: "1px solid var(--ink-success)",
  background: "var(--paper-muted)",
  borderRadius: "var(--radius-sm)",
};

export const wizardStatusCardWarn: CSSProperties = {
  padding: 12,
  border: "1px solid var(--ink-soft)",
  background: "var(--ink-soft)",
  borderRadius: "var(--radius-sm)",
};

export const wizardStatusCardMuted: CSSProperties = {
  padding: 12,
  border: "1px solid var(--hairline)",
  background: "var(--paper-muted)",
  borderRadius: "var(--radius-sm)",
};

export const wizardCodeBlock: CSSProperties = {
  display: "block",
  padding: 6,
  background: "var(--paper-muted)",
  border: "1px solid var(--hairline)",
  borderRadius: 4,
  marginTop: 6,
  fontSize: 11,
  fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
  color: "var(--ink)",
};
