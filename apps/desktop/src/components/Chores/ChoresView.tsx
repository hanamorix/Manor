import { useEffect, useState } from "react";
import { useChoresStore } from "../../lib/chores/state";
import { listAllChores, checkChoreFairness, type Chore } from "../../lib/chores/ipc";
import ChoreDrawer from "./ChoreDrawer";

const pageStyle: React.CSSProperties = {
  maxWidth: 760,
  margin: "0 auto",
  padding: "24px 24px 120px",
};

const sectionStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
  marginBottom: 12,
};

const headerStyle: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 700,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)",
  marginBottom: 10,
};

const rowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "10px 4px",
  borderBottom: "1px solid var(--hairline)",
  cursor: "pointer",
};

const dueBadge = (daysAway: number): React.CSSProperties => ({
  fontSize: 11,
  padding: "2px 8px",
  borderRadius: 999,
  background: daysAway <= 0 ? "rgba(255,59,48,0.1)" : "rgba(20,20,30,0.05)",
  color: daysAway <= 0 ? "var(--imessage-red)" : "rgba(20,20,30,0.55)",
  fontWeight: 600,
});

const addBtn: React.CSSProperties = {
  background: "var(--imessage-blue)",
  color: "white",
  border: "none",
  borderRadius: 999,
  padding: "10px 20px",
  fontSize: 14,
  fontWeight: 600,
  cursor: "pointer",
  marginTop: 12,
};

const fairnessBanner: React.CSSProperties = {
  background: "rgba(255,193,92,0.12)",
  borderRadius: "var(--radius-md)",
  padding: "10px 14px",
  marginBottom: 12,
  fontSize: 13,
  color: "rgba(20,20,30,0.7)",
};

function daysUntil(ms: number): number {
  return Math.round((ms - Date.now()) / 86_400_000);
}

function formatDueBadge(days: number, nextDueMs: number): string {
  if (days < 0) return `${-days}d overdue`;
  if (days === 0) return "Due today";
  if (days === 1) return "Tomorrow";
  if (days < 7) return `In ${days}d`;
  return new Date(nextDueMs).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export default function ChoresView() {
  const allChores = useChoresStore((s) => s.allChores);
  const setAllChores = useChoresStore((s) => s.setAllChores);
  const fairnessNudges = useChoresStore((s) => s.fairnessNudges);
  const setFairnessNudges = useChoresStore((s) => s.setFairnessNudges);
  const dismissFairnessNudge = useChoresStore((s) => s.dismissFairnessNudge);

  const [editing, setEditing] = useState<Chore | null>(null);
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    void listAllChores().then(setAllChores);
    void checkChoreFairness().then(setFairnessNudges);
  }, [setAllChores, setFairnessNudges]);

  const dueSoon = allChores
    .filter((c) => daysUntil(c.next_due) <= 7)
    .sort((a, b) => a.next_due - b.next_due);

  return (
    <div style={pageStyle}>
      <h1 style={{ fontSize: 24, fontWeight: 700, margin: "0 0 16px" }}>Chores</h1>

      {fairnessNudges.map((n) => (
        <div key={n.chore_id} style={fairnessBanner}>
          <span>
            <b>{n.person_name}</b> hasn't done <b>{n.chore_title}</b> in {n.days_ago} days — might be worth a nudge.
          </span>
          <button
            onClick={() => dismissFairnessNudge(n.chore_id)}
            style={{ float: "right", background: "transparent", border: "none", color: "rgba(20,20,30,0.5)", cursor: "pointer", fontSize: 12 }}
          >
            Dismiss
          </button>
        </div>
      ))}

      <section style={sectionStyle}>
        <h2 style={headerStyle}>Due soon</h2>
        {dueSoon.length === 0 ? (
          <p style={{ color: "rgba(20,20,30,0.5)", fontSize: 13, margin: 0 }}>Nothing in the next 7 days.</p>
        ) : (
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {dueSoon.map((c) => {
              const days = daysUntil(c.next_due);
              return (
                <li key={c.id} style={rowStyle} onClick={() => setEditing(c)}>
                  <span style={{ fontSize: 18 }}>{c.emoji}</span>
                  <span style={{ flex: 1, fontSize: 14 }}>{c.title}</span>
                  <span style={dueBadge(days)}>{formatDueBadge(days, c.next_due)}</span>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <section style={sectionStyle}>
        <h2 style={headerStyle}>All chores</h2>
        {allChores.length === 0 ? (
          <p style={{ color: "rgba(20,20,30,0.5)", fontSize: 13, margin: 0 }}>No chores yet — add your first one.</p>
        ) : (
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {[...allChores].sort((a, b) => a.title.localeCompare(b.title)).map((c) => (
              <li key={c.id} style={rowStyle} onClick={() => setEditing(c)}>
                <span style={{ fontSize: 18 }}>{c.emoji}</span>
                <span style={{ flex: 1, fontSize: 14 }}>{c.title}</span>
                <span style={{ fontSize: 11, color: "rgba(20,20,30,0.45)" }}>{c.rotation === "none" ? "" : c.rotation}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <button style={addBtn} onClick={() => setCreating(true)}>+ Add chore</button>

      {creating && <ChoreDrawer chore={null} onClose={() => setCreating(false)} />}
      {editing && <ChoreDrawer chore={editing} onClose={() => setEditing(null)} />}
    </div>
  );
}
