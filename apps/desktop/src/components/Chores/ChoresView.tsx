import { useEffect, useState } from "react";
import { Sparkles, Plus } from "lucide-react";
import { useChoresStore } from "../../lib/chores/state";
import { listAllChores, checkChoreFairness, type Chore } from "../../lib/chores/ipc";
import { PageHeader, SectionLabel, Button } from "../../lib/ui";
import ChoreDrawer from "./ChoreDrawer";

const pageStyle: React.CSSProperties = {
  maxWidth: 760,
  margin: "0 auto",
  padding: "24px 24px 120px",
};

const sectionStyle: React.CSSProperties = {
  marginBottom: 22,
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
  fontSize: "var(--text-xs)",
  padding: "2px 8px",
  borderRadius: "var(--radius-sm)",
  background: daysAway <= 0 ? "var(--paper-muted)" : "var(--paper-muted)",
  color: daysAway <= 0 ? "var(--ink-danger)" : "var(--ink-soft)",
  fontWeight: 500,
});

const fairnessBanner: React.CSSProperties = {
  background: "rgba(255,193,92,0.12)",
  borderRadius: "var(--radius-md)",
  padding: "10px 14px",
  marginBottom: 12,
  fontSize: 13,
  color: "var(--ink-soft)",
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
      <PageHeader icon={Sparkles} title="Chores" />

      {fairnessNudges.map((n) => (
        <div key={n.chore_id} style={fairnessBanner}>
          <span>
            <b>{n.person_name}</b> hasn't done <b>{n.chore_title}</b> in {n.days_ago} days — might be worth a nudge.
          </span>
          <button
            onClick={() => dismissFairnessNudge(n.chore_id)}
            style={{ float: "right", background: "transparent", border: "none", color: "var(--ink-faint)", cursor: "pointer", fontSize: 12 }}
          >
            Dismiss
          </button>
        </div>
      ))}

      <section style={sectionStyle}>
        <SectionLabel icon={Sparkles}>Due soon</SectionLabel>
        {dueSoon.length === 0 ? (
          <p style={{ color: "var(--ink-faint)", fontSize: 13, margin: 0 }}>Nothing in the next 7 days.</p>
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
        <SectionLabel icon={Sparkles}>All chores</SectionLabel>
        {allChores.length === 0 ? (
          <p style={{ color: "var(--ink-faint)", fontSize: 13, margin: 0 }}>No chores yet — add your first one.</p>
        ) : (
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {[...allChores].sort((a, b) => a.title.localeCompare(b.title)).map((c) => (
              <li key={c.id} style={rowStyle} onClick={() => setEditing(c)}>
                <span style={{ fontSize: 18 }}>{c.emoji}</span>
                <span style={{ flex: 1, fontSize: 14 }}>{c.title}</span>
                <span style={{ fontSize: 11, color: "var(--ink-faint)" }}>{c.rotation === "none" ? "" : c.rotation}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <Button variant="primary" icon={Plus} onClick={() => setCreating(true)}>Add chore</Button>

      {creating && <ChoreDrawer chore={null} onClose={() => setCreating(false)} />}
      {editing && <ChoreDrawer chore={editing} onClose={() => setEditing(null)} />}
    </div>
  );
}
