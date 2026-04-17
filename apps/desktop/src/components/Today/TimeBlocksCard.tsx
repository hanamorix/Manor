import { useState } from "react";
import { useTimeBlocksStore } from "../../lib/timeblocks/state";
import { createTimeBlock, dismissPatternNudge, promoteToPattern, type BlockKind } from "../../lib/timeblocks/ipc";

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

const addBtn: React.CSSProperties = {
  background: "transparent",
  border: "none",
  color: "var(--ink)",
  fontWeight: 700,
  fontSize: 12,
  cursor: "pointer",
  padding: 0,
};

const KIND_COLOR: Record<BlockKind, string> = {
  focus: "#007aff",
  errands: "#FFC15C",
  admin: "#9b59b6",
  dnd: "#ff3b30",
};

const KIND_LABEL: Record<BlockKind, string> = {
  focus: "Focus",
  errands: "Errands",
  admin: "Admin",
  dnd: "DND",
};

const pillStyle = (kind: BlockKind): React.CSSProperties => ({
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "8px 12px",
  borderRadius: 8,
  borderLeft: `3px solid ${KIND_COLOR[kind]}`,
  background: "rgba(20,20,30,0.03)",
  fontSize: 13,
  color: "var(--ink)",
  marginBottom: 6,
});

const emptyStyle: React.CSSProperties = {
  padding: "10px 4px",
  fontSize: 13,
  color: "rgba(20,20,30,0.5)",
};

const nudgeStyle: React.CSSProperties = {
  marginTop: 10,
  padding: "10px 12px",
  background: "rgba(0,122,255,0.06)",
  borderRadius: 8,
  fontSize: 12,
  color: "rgba(20,20,30,0.7)",
  display: "flex",
  alignItems: "center",
  gap: 10,
};

const nudgeBtn: React.CSSProperties = {
  background: "var(--ink)",
  color: "var(--action-fg)",
  border: "none",
  borderRadius: 999,
  padding: "4px 10px",
  fontSize: 11,
  fontWeight: 600,
  cursor: "pointer",
};

const nudgeBtnGhost: React.CSSProperties = {
  background: "transparent",
  color: "rgba(20,20,30,0.55)",
  border: "none",
  padding: "4px 8px",
  fontSize: 11,
  cursor: "pointer",
};

function suggestionToRrule(weekday: string): string {
  const map: Record<string, string> = {
    Monday: "MO", Tuesday: "TU", Wednesday: "WE", Thursday: "TH",
    Friday: "FR", Saturday: "SA", Sunday: "SU",
  };
  return `FREQ=WEEKLY;BYDAY=${map[weekday] || "MO"}`;
}

export default function TimeBlocksCard() {
  const blocks = useTimeBlocksStore((s) => s.todayBlocks);
  const suggestion = useTimeBlocksStore((s) => s.patternSuggestion);
  const upsertBlock = useTimeBlocksStore((s) => s.upsertBlock);
  const setPatternSuggestion = useTimeBlocksStore((s) => s.setPatternSuggestion);

  const [adding, setAdding] = useState(false);
  const [form, setForm] = useState({ title: "", kind: "focus" as BlockKind, startTime: "09:00", endTime: "10:00" });

  async function onAdd() {
    if (!form.title.trim()) {
      setAdding(false);
      return;
    }
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const { block, suggestion: sugg } = await createTimeBlock({
      title: form.title.trim(),
      kind: form.kind,
      dateMs: today.getTime(),
      startTime: form.startTime,
      endTime: form.endTime,
    });
    upsertBlock(block);
    if (sugg) setPatternSuggestion(sugg);
    setAdding(false);
    setForm({ title: "", kind: "focus", startTime: "09:00", endTime: "10:00" });
  }

  async function onPromote() {
    if (!suggestion) return;
    const rrule = suggestionToRrule(suggestion.weekday);
    const updated = await promoteToPattern(suggestion.trigger_id, rrule);
    upsertBlock(updated);
    setPatternSuggestion(null);
  }

  async function onDismiss() {
    if (!suggestion) return;
    await dismissPatternNudge(suggestion.trigger_id);
    setPatternSuggestion(null);
  }

  return (
    <section style={cardStyle} aria-label="Time Blocks">
      <header style={sectionHeader}>
        <span>Time Blocks</span>
        <button style={addBtn} onClick={() => setAdding(true)}>+ Add</button>
      </header>

      {blocks.length === 0 && !adding ? (
        <div style={emptyStyle}>No blocks today — time is yours.</div>
      ) : (
        <div>
          {blocks.map((b) => (
            <div key={b.id} style={pillStyle(b.kind as BlockKind)}>
              <strong style={{ color: KIND_COLOR[b.kind as BlockKind], fontWeight: 700, fontSize: 11, textTransform: "uppercase", letterSpacing: 0.5 }}>
                {KIND_LABEL[b.kind as BlockKind]}
              </strong>
              <span style={{ flex: 1 }}>{b.title}</span>
              <span style={{ color: "rgba(20,20,30,0.5)", fontSize: 12 }}>
                {b.start_time}–{b.end_time}
              </span>
            </div>
          ))}
        </div>
      )}

      {adding && (
        <div style={{ marginTop: 8, padding: 10, background: "rgba(20,20,30,0.03)", borderRadius: 8 }}>
          <div style={{ display: "flex", gap: 6, marginBottom: 6 }}>
            <input
              autoFocus
              value={form.title}
              onChange={(e) => setForm({ ...form, title: e.target.value })}
              placeholder="Block title…"
              style={{ flex: 1, padding: "6px 10px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: 13, fontFamily: "inherit" }}
              onKeyDown={(e) => { if (e.key === "Enter") onAdd(); }}
            />
            <select
              value={form.kind}
              onChange={(e) => setForm({ ...form, kind: e.target.value as BlockKind })}
              style={{ padding: "6px 8px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: 13, fontFamily: "inherit" }}
            >
              <option value="focus">Focus</option>
              <option value="errands">Errands</option>
              <option value="admin">Admin</option>
              <option value="dnd">DND</option>
            </select>
          </div>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <input
              type="time"
              value={form.startTime}
              onChange={(e) => setForm({ ...form, startTime: e.target.value })}
              style={{ padding: "6px 8px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: 13, fontFamily: "inherit" }}
            />
            <span style={{ color: "rgba(20,20,30,0.5)" }}>→</span>
            <input
              type="time"
              value={form.endTime}
              onChange={(e) => setForm({ ...form, endTime: e.target.value })}
              style={{ padding: "6px 8px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: 13, fontFamily: "inherit" }}
            />
            <button onClick={onAdd} style={{ marginLeft: "auto", ...nudgeBtn }}>Save</button>
            <button onClick={() => setAdding(false)} style={nudgeBtnGhost}>Cancel</button>
          </div>
        </div>
      )}

      {suggestion && (
        <div style={nudgeStyle}>
          <span style={{ flex: 1 }}>
            Looks like <b>{suggestion.weekday}s</b> {suggestion.start_time}–{suggestion.end_time} are your {suggestion.kind} time — make it recurring?
          </span>
          <button onClick={onPromote} style={nudgeBtn}>Yes</button>
          <button onClick={onDismiss} style={nudgeBtnGhost}>Not now</button>
        </div>
      )}
    </section>
  );
}
