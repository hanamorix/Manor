import { useState } from "react";
import { Sparkles, Check, SkipForward } from "lucide-react";
import { useChoresStore } from "../../lib/chores/state";
import { completeChore, skipChore } from "../../lib/chores/ipc";
import { SectionLabel } from "../../lib/ui";

const manageLink: React.CSSProperties = {
  background: "transparent",
  border: "none",
  color: "var(--ink)",
  fontWeight: 600,
  fontSize: "var(--text-xs)",
  cursor: "pointer",
  padding: 0,
};

const rowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "10px 4px",
  cursor: "pointer",
  borderRadius: "var(--radius-lg)",
  transition: "background 0.15s",
};

const emptyStyle: React.CSSProperties = {
  padding: "10px 4px",
  fontSize: "var(--text-sm)",
  color: "var(--ink-faint)",
};

export default function ChoresCard() {
  const chores = useChoresStore((s) => s.choresDueToday);
  const removeFromDueToday = useChoresStore((s) => s.removeFromDueToday);
  const upsertChore = useChoresStore((s) => s.upsertChore);
  const [hoveredId, setHoveredId] = useState<number | null>(null);

  async function onComplete(id: number) {
    const updated = await completeChore(id, null);
    removeFromDueToday(id);
    upsertChore(updated);
  }

  async function onSkip(id: number, e?: React.MouseEvent) {
    e?.preventDefault();
    const updated = await skipChore(id);
    removeFromDueToday(id);
    upsertChore(updated);
  }

  return (
    <section style={{ marginBottom: 22 }} aria-label="Chores">
      <SectionLabel
        icon={Sparkles}
        action={
          <button style={manageLink} onClick={() => {}}>
            Manage →
          </button>
        }
      >
        Chores
      </SectionLabel>
      {chores.length === 0 ? (
        <div style={emptyStyle}>All clear today 🧹</div>
      ) : (
        <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
          {chores.map((c) => {
            const isHovered = hoveredId === c.id;
            return (
              <li
                key={c.id}
                style={{
                  ...rowStyle,
                  background: isHovered ? "var(--paper-muted)" : "transparent",
                }}
                onMouseEnter={() => setHoveredId(c.id)}
                onMouseLeave={() => setHoveredId(null)}
                onFocus={() => setHoveredId(c.id)}
                onBlur={() => setHoveredId(null)}
                onContextMenu={(e) => onSkip(c.id, e)}
              >
                <button
                  onClick={() => onComplete(c.id)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      onComplete(c.id);
                    }
                  }}
                  aria-label={`Complete ${c.title}`}
                  style={{
                    border: "none",
                    background: "none",
                    padding: 0,
                    cursor: "pointer",
                    color: "var(--ink-soft)",
                    display: "inline-flex",
                    alignItems: "center",
                    flexShrink: 0,
                  }}
                >
                  <Check size={14} strokeWidth={1.8} />
                </button>
                <span style={{ fontSize: 18, flexShrink: 0 }}>{c.emoji}</span>
                <span style={{ flex: 1, fontSize: "var(--text-md)", color: "var(--ink)" }}>{c.title}</span>
                {isHovered && (
                  <button
                    onClick={(e) => { e.stopPropagation(); onSkip(c.id); }}
                    aria-label={`Skip ${c.title}`}
                    style={{
                      border: "none",
                      background: "none",
                      color: "var(--ink-soft)",
                      fontSize: "var(--text-xs)",
                      cursor: "pointer",
                      display: "inline-flex",
                      alignItems: "center",
                      gap: 4,
                      padding: "2px 4px",
                      flexShrink: 0,
                    }}
                  >
                    <SkipForward size={12} strokeWidth={1.8} />
                    Skip
                  </button>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
