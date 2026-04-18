import { Sparkles } from "lucide-react";
import type { IdeaTitle } from "../../../lib/meal_plan/ideas-ipc";

interface Props {
  idea: IdeaTitle;
  onClick: () => void;
  loading?: boolean;
}

export function IdeaTitleCard({ idea, onClick, loading }: Props) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={loading}
      style={{
        textAlign: "left",
        background: "var(--paper, #fff)",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 6,
        padding: 12,
        cursor: loading ? "wait" : "pointer",
        display: "flex",
        flexDirection: "column",
        gap: 6,
        minHeight: 140,
        position: "relative",
        opacity: loading ? 0.6 : 1,
      }}
    >
      <Sparkles size={16} strokeWidth={1.6} color="var(--ink-soft, #999)" />
      <div style={{
        fontSize: 14,
        fontWeight: 600,
        overflow: "hidden",
        display: "-webkit-box",
        WebkitBoxOrient: "vertical" as const,
        WebkitLineClamp: 2 as const,
      }}>
        {idea.title}
      </div>
      <div style={{
        fontSize: 12,
        color: "var(--ink-soft, #999)",
        overflow: "hidden",
        display: "-webkit-box",
        WebkitBoxOrient: "vertical" as const,
        WebkitLineClamp: 2 as const,
      }}>
        {idea.blurb}
      </div>
      {loading && (
        <div style={{
          position: "absolute",
          inset: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontSize: 12,
          color: "var(--ink-soft, #999)",
        }}>
          Expanding…
        </div>
      )}
    </button>
  );
}
