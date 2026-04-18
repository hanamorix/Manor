import { Sparkles, FileCode } from "lucide-react";
import type { ImportMethod } from "../../lib/recipe/recipe-ipc";

export function ImportMethodBadge({ method }: { method: ImportMethod }) {
  if (method === "manual") return null;
  const style = {
    display: "inline-flex" as const,
    alignItems: "center" as const,
    gap: 4,
    fontSize: 12,
    color: "var(--ink-soft, #999)",
    padding: "2px 8px",
    borderRadius: 4,
    background: "var(--paper-muted, #f5f5f5)",
  };
  if (method === "jsonld") {
    return (
      <span style={style}>
        <FileCode size={12} strokeWidth={1.8} /> Parsed from structured data
      </span>
    );
  }
  return (
    <span style={style}>
      <Sparkles size={12} strokeWidth={1.8} /> AI-extracted — please review
    </span>
  );
}
