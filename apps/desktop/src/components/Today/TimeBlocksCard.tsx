import { useState } from "react";
import { LayoutGrid, Target, Inbox, ShoppingCart, BellOff } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useTimeBlocksStore } from "../../lib/timeblocks/state";
import { createTimeBlock, dismissPatternNudge, promoteToPattern, type BlockKind } from "../../lib/timeblocks/ipc";
import { SectionLabel } from "../../lib/ui";

const addBtn: React.CSSProperties = {
  background: "transparent",
  border: "none",
  color: "var(--ink)",
  fontWeight: 600,
  fontSize: "var(--text-xs)",
  cursor: "pointer",
  padding: 0,
};

const KIND_ICON: Record<BlockKind, LucideIcon> = {
  focus: Target,
  errands: ShoppingCart,
  admin: Inbox,
  dnd: BellOff,
};

const KIND_LABEL: Record<BlockKind, string> = {
  focus: "Focus",
  errands: "Errands",
  admin: "Admin",
  dnd: "DND",
};

const pillStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "6px 0",
  borderBottom: "1px solid var(--hairline)",
  fontSize: "var(--text-sm)",
  color: "var(--ink)",
};

const emptyStyle: React.CSSProperties = {
  padding: "10px 4px",
  fontSize: "var(--text-sm)",
  color: "var(--ink-faint)",
};

const nudgeStyle: React.CSSProperties = {
  marginTop: 10,
  padding: "10px 12px",
  background: "var(--paper-muted)",
  borderRadius: "var(--radius-lg)",
  fontSize: "var(--text-xs)",
  color: "var(--ink-soft)",
  display: "flex",
  alignItems: "center",
  gap: 10,
};

const nudgeBtn: React.CSSProperties = {
  background: "var(--ink)",
  color: "var(--action-fg)",
  border: "none",
  borderRadius: "var(--radius-md)",
  padding: "4px 10px",
  fontSize: 11,
  fontWeight: 600,
  cursor: "pointer",
};

const nudgeBtnGhost: React.CSSProperties = {
  background: "transparent",
  color: "var(--ink-soft)",
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
    <section style={{ marginBottom: 22 }} aria-label="Time Blocks">
      <SectionLabel
        icon={LayoutGrid}
        action={<button style={addBtn} onClick={() => setAdding(true)}>+ Add</button>}
      >
        Time blocks
      </SectionLabel>

      {blocks.length === 0 && !adding ? (
        <div style={emptyStyle}>No blocks today — time is yours.</div>
      ) : (
        <div>
          {blocks.map((b) => {
            const Icon = KIND_ICON[b.kind as BlockKind] ?? Target;
            return (
              <div key={b.id} style={pillStyle}>
                <Icon size={14} strokeWidth={1.8} color="var(--ink-soft)" aria-label={KIND_LABEL[b.kind as BlockKind]} />
                <span style={{ flex: 1, fontSize: "var(--text-sm)" }}>{b.title}</span>
                <time className="num" style={{ fontSize: "var(--text-xs)", color: "var(--ink-soft)" }}>
                  {b.start_time}–{b.end_time}
                </time>
              </div>
            );
          })}
        </div>
      )}

      {adding && (
        <div style={{ marginTop: 8, padding: 10, background: "var(--paper-muted)", borderRadius: 8 }}>
          <div style={{ display: "flex", gap: 6, marginBottom: 6 }}>
            <input
              autoFocus
              value={form.title}
              onChange={(e) => setForm({ ...form, title: e.target.value })}
              placeholder="Block title…"
              style={{ flex: 1, padding: "6px 10px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: "var(--text-sm)", fontFamily: "inherit" }}
              onKeyDown={(e) => { if (e.key === "Enter") onAdd(); }}
            />
            <select
              value={form.kind}
              onChange={(e) => setForm({ ...form, kind: e.target.value as BlockKind })}
              style={{ padding: "6px 8px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: "var(--text-sm)", fontFamily: "inherit" }}
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
              style={{ padding: "6px 8px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: "var(--text-sm)", fontFamily: "inherit" }}
            />
            <span style={{ color: "var(--ink-faint)" }}>→</span>
            <input
              type="time"
              value={form.endTime}
              onChange={(e) => setForm({ ...form, endTime: e.target.value })}
              style={{ padding: "6px 8px", borderRadius: 6, border: "1px solid var(--hairline)", fontSize: "var(--text-sm)", fontFamily: "inherit" }}
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
