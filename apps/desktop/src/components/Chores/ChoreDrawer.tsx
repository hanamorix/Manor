import { useState, useEffect } from "react";
import { Check, Plus } from "lucide-react";
import type { Chore, RotationKind } from "../../lib/chores/ipc";
import {
  createChore,
  updateChore,
  deleteChore,
  listChoreCompletions,
  type ChoreCompletion,
} from "../../lib/chores/ipc";
import { useChoresStore } from "../../lib/chores/state";
import { useOverlay } from "../../lib/overlay/state";
import { Button } from "../../lib/ui";

const overlayStyle: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "var(--scrim)",
  zIndex: 1050,
  display: "flex",
  justifyContent: "flex-end",
};

const drawerStyle: React.CSSProperties = {
  width: 420,
  background: "var(--paper)",
  borderLeft: "1px solid var(--hairline)",
  boxShadow: "var(--shadow-lg)",
  padding: 24,
  overflowY: "auto",
  animation: "drawerIn 0.2s ease-out",
};

const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: 11,
  fontWeight: 600,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "var(--ink-soft)",
  marginBottom: 6,
  marginTop: 14,
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "8px 12px",
  borderRadius: "var(--radius-lg)",
  border: "1px solid var(--hairline)",
  fontSize: 14,
  fontFamily: "inherit",
};

const tabBar: React.CSSProperties = {
  display: "flex",
  gap: 4,
  marginBottom: 16,
  borderBottom: "1px solid var(--hairline)",
};

const tabStyle = (active: boolean): React.CSSProperties => ({
  padding: "8px 14px",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
  color: active ? "var(--ink)" : "var(--ink-soft)",
  background: "transparent",
  border: "none",
  borderBottomStyle: "solid",
  borderBottomWidth: 2,
  borderBottomColor: active ? "var(--ink)" : "transparent",
});

const btnDanger: React.CSSProperties = {
  background: "transparent",
  color: "var(--ink)",
  border: "none",
  fontSize: 12,
  cursor: "pointer",
  padding: "6px 0",
  marginTop: 12,
};

const RRULE_PRESETS: { label: string; rrule: string }[] = [
  { label: "Daily", rrule: "FREQ=DAILY" },
  { label: "Every week", rrule: "FREQ=WEEKLY" },
  { label: "Every 2 weeks", rrule: "FREQ=WEEKLY;INTERVAL=2" },
  { label: "Monthly", rrule: "FREQ=MONTHLY" },
];

interface Props {
  chore: Chore | null;
  onClose: () => void;
}

export default function ChoreDrawer({ chore, onClose }: Props) {
  useOverlay();
  const upsertChore = useChoresStore((s) => s.upsertChore);
  const removeChore = useChoresStore((s) => s.removeChore);

  const [tab, setTab] = useState<"details" | "history">("details");
  const [title, setTitle] = useState(chore?.title ?? "");
  const [emoji, setEmoji] = useState(chore?.emoji ?? "🧹");
  const [rrule, setRrule] = useState(chore?.rrule ?? "FREQ=WEEKLY");
  const [rotation, setRotation] = useState<RotationKind>(chore?.rotation ?? "none");
  const [history, setHistory] = useState<ChoreCompletion[]>([]);

  useEffect(() => {
    setHistory([]);
    if (tab === "history" && chore) {
      void listChoreCompletions(chore.id, 20).then(setHistory);
    }
  }, [tab, chore?.id]);

  async function onSave() {
    const trimmed = title.trim();
    if (!trimmed) return;
    if (chore) {
      const updated = await updateChore({
        id: chore.id, title: trimmed, emoji, rrule, rotation,
      });
      upsertChore(updated);
    } else {
      const created = await createChore({
        title: trimmed,
        emoji,
        rrule,
        firstDue: Date.now(),
        rotation,
      });
      upsertChore(created);
    }
    onClose();
  }

  async function onDelete() {
    if (!chore) return;
    await deleteChore(chore.id);
    removeChore(chore.id);
    onClose();
  }

  return (
    <div style={overlayStyle} onClick={onClose}>
      <aside style={drawerStyle} onClick={(e) => e.stopPropagation()}>
        <h2 style={{ margin: "0 0 16px", fontSize: 20, fontWeight: 600 }}>
          {chore ? "Edit chore" : "New chore"}
        </h2>

        {chore && (
          <div style={tabBar}>
            <button style={tabStyle(tab === "details")} onClick={() => setTab("details")}>Details</button>
            <button style={tabStyle(tab === "history")} onClick={() => setTab("history")}>History</button>
          </div>
        )}

        {tab === "details" && (
          <>
            <label style={labelStyle}>Emoji</label>
            <input
              style={{ ...inputStyle, width: 80, fontSize: 22, textAlign: "center" }}
              value={emoji}
              onChange={(e) => setEmoji(e.target.value.slice(0, 4))}
              aria-label="Emoji"
            />

            <label style={labelStyle}>Title</label>
            <input
              style={inputStyle}
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="e.g. Take the bins out"
              aria-label="Title"
            />

            <label style={labelStyle}>Recurrence</label>
            <select
              style={inputStyle}
              value={rrule}
              onChange={(e) => setRrule(e.target.value)}
              aria-label="Recurrence"
            >
              {RRULE_PRESETS.map((p) => (
                <option key={p.rrule} value={p.rrule}>{p.label}</option>
              ))}
            </select>

            <label style={labelStyle}>Rotation</label>
            <select
              style={inputStyle}
              value={rotation}
              onChange={(e) => setRotation(e.target.value as RotationKind)}
              aria-label="Rotation"
            >
              <option value="none">No rotation (single person)</option>
              <option value="round_robin">Round-robin</option>
              <option value="least_completed">Least recently completed</option>
              <option value="fixed">Fixed assignee</option>
            </select>

            <div style={{ display: "flex", gap: 8, marginTop: 24 }}>
              <Button variant="primary" icon={chore ? Check : Plus} onClick={onSave}>{chore ? "Save" : "Create"}</Button>
              <Button variant="secondary" onClick={onClose}>Cancel</Button>
            </div>

            {chore && (
              <button style={btnDanger} onClick={onDelete}>Delete chore</button>
            )}
          </>
        )}

        {tab === "history" && chore && (
          <div>
            {history.length === 0 ? (
              <p style={{ color: "var(--ink-faint)", fontSize: 13 }}>No completions yet.</p>
            ) : (
              <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
                {history.map((h) => (
                  <li key={h.id} style={{ padding: "10px 0", borderBottom: "1px solid var(--hairline)", fontSize: 13 }}>
                    <div style={{ color: "var(--ink)" }}>
                      {new Date(h.completed_at).toLocaleString()}
                    </div>
                    <div style={{ color: "var(--ink-faint)", fontSize: 12 }}>
                      {h.completed_by ? `by person #${h.completed_by}` : "completed"}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </aside>
    </div>
  );
}
