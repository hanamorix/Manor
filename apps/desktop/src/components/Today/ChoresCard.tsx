import { useChoresStore } from "../../lib/chores/state";
import { completeChore, skipChore } from "../../lib/chores/ipc";

const cardStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
};

const sectionHeader: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  margin: 0,
  marginBottom: 8,
  fontSize: 11,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)",
  fontWeight: 700,
};

const manageLink: React.CSSProperties = {
  background: "transparent",
  border: "none",
  color: "var(--ink)",
  fontWeight: 600,
  fontSize: 12,
  cursor: "pointer",
  padding: 0,
  letterSpacing: 0,
  textTransform: "none",
};

const rowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "10px 4px",
  cursor: "pointer",
  borderRadius: 8,
  transition: "background 0.15s",
};

const emptyStyle: React.CSSProperties = {
  padding: "10px 4px",
  fontSize: 13,
  color: "rgba(20,20,30,0.5)",
};

export default function ChoresCard() {
  const chores = useChoresStore((s) => s.choresDueToday);
  const removeFromDueToday = useChoresStore((s) => s.removeFromDueToday);
  const upsertChore = useChoresStore((s) => s.upsertChore);

  async function onComplete(id: number) {
    const updated = await completeChore(id, null);
    removeFromDueToday(id);
    upsertChore(updated);
  }

  async function onSkip(e: React.MouseEvent, id: number) {
    e.preventDefault();
    const updated = await skipChore(id);
    removeFromDueToday(id);
    upsertChore(updated);
  }

  return (
    <section style={cardStyle} aria-label="Chores">
      <header style={sectionHeader}>
        <span>Chores</span>
        <button style={manageLink} onClick={() => {}}>
          Manage →
        </button>
      </header>
      {chores.length === 0 ? (
        <div style={emptyStyle}>All clear today 🧹</div>
      ) : (
        <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
          {chores.map((c) => (
            <li
              key={c.id}
              style={rowStyle}
              role="button"
              tabIndex={0}
              onClick={() => onComplete(c.id)}
              onContextMenu={(e) => onSkip(e, c.id)}
              onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onComplete(c.id); }}
              onMouseEnter={(e) => { e.currentTarget.style.background = "rgba(20,20,30,0.04)"; }}
              onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}
              title="Click to complete · Right-click to skip"
            >
              <span style={{ fontSize: 18 }}>{c.emoji}</span>
              <span style={{ flex: 1, fontSize: 14, color: "var(--ink)" }}>{c.title}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
