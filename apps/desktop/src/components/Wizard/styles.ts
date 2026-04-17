// Shared inline styles for the first-launch wizard. Keeps the steps consistent
// with Manor's paper/ink design tokens.

import type { CSSProperties } from "react";

export const wizardPrimaryButton: CSSProperties = {
  padding: "8px 18px",
  background: "var(--imessage-blue)",
  color: "#fff",
  border: "none",
  borderRadius: "var(--radius-pill)",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
  fontFamily: "inherit",
};

export const wizardSecondaryButton: CSSProperties = {
  padding: "8px 14px",
  background: "transparent",
  color: "var(--ink)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-pill)",
  fontSize: 13,
  fontWeight: 500,
  cursor: "pointer",
  fontFamily: "inherit",
};

export const wizardStatusCardGood: CSSProperties = {
  padding: 12,
  border: "1px solid rgba(52, 199, 89, 0.3)",
  background: "rgba(52, 199, 89, 0.08)",
  borderRadius: "var(--radius-sm)",
};

export const wizardStatusCardWarn: CSSProperties = {
  padding: 12,
  border: "1px solid rgba(255, 149, 0, 0.35)",
  background: "rgba(255, 149, 0, 0.08)",
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
