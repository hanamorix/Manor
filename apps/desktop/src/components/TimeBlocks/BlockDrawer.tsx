import { useState } from "react";
import type { TimeBlock, BlockKind } from "../../lib/timeblocks/ipc";
import { createTimeBlock, updateTimeBlock, deleteTimeBlock } from "../../lib/timeblocks/ipc";
import { useTimeBlocksStore } from "../../lib/timeblocks/state";

const overlayStyle: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "rgba(20,20,30,0.2)",
  zIndex: 100,
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
  fontWeight: 700,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)",
  marginBottom: 6,
  marginTop: 14,
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "8px 12px",
  borderRadius: 8,
  border: "1px solid var(--hairline)",
  fontSize: 14,
  fontFamily: "inherit",
};

const btnPrimary: React.CSSProperties = {
  background: "var(--imessage-blue)",
  color: "white",
  border: "none",
  borderRadius: 999,
  padding: "8px 18px",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
};

const btnGhost: React.CSSProperties = {
  background: "transparent",
  color: "rgba(20,20,30,0.55)",
  border: "1px solid var(--hairline)",
  borderRadius: 999,
  padding: "8px 18px",
  fontSize: 13,
  cursor: "pointer",
};

const btnDanger: React.CSSProperties = {
  background: "transparent",
  color: "var(--imessage-red)",
  border: "none",
  fontSize: 12,
  cursor: "pointer",
  padding: "6px 0",
  marginTop: 12,
};

function toISODate(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10);
}

function fromISODate(iso: string): number {
  const d = new Date(iso + "T00:00:00Z");
  return d.getTime();
}

interface Props {
  block: TimeBlock | null;
  onClose: () => void;
}

export default function BlockDrawer({ block, onClose }: Props) {
  const upsertBlock = useTimeBlocksStore((s) => s.upsertBlock);
  const removeBlock = useTimeBlocksStore((s) => s.removeBlock);
  const setPatternSuggestion = useTimeBlocksStore((s) => s.setPatternSuggestion);

  const [title, setTitle] = useState(block?.title ?? "");
  const [kind, setKind] = useState<BlockKind>(block?.kind ?? "focus");
  const [dateStr, setDateStr] = useState(toISODate(block?.date ?? Date.now()));
  const [startTime, setStartTime] = useState(block?.start_time ?? "09:00");
  const [endTime, setEndTime] = useState(block?.end_time ?? "10:00");

  async function onSave() {
    if (!title.trim()) return;
    try {
      if (block) {
        const updated = await updateTimeBlock({
          id: block.id,
          title: title.trim(),
          kind,
          dateMs: fromISODate(dateStr),
          startTime,
          endTime,
        });
        upsertBlock(updated);
      } else {
        const result = await createTimeBlock({
          title: title.trim(),
          kind,
          dateMs: fromISODate(dateStr),
          startTime,
          endTime,
        });
        upsertBlock(result.block);
        if (result.suggestion) setPatternSuggestion(result.suggestion);
      }
      onClose();
    } catch (err) {
      console.error("Failed to save block:", err);
    }
  }

  async function onDelete() {
    if (!block) return;
    try {
      await deleteTimeBlock(block.id);
      removeBlock(block.id);
      onClose();
    } catch (err) {
      console.error("Failed to delete block:", err);
    }
  }

  return (
    <div style={overlayStyle} onClick={onClose}>
      <aside style={drawerStyle} onClick={(e) => e.stopPropagation()}>
        <h2 style={{ margin: "0 0 16px", fontSize: 20, fontWeight: 700 }}>
          {block ? "Edit block" : "New block"}
        </h2>

        <label style={labelStyle}>Title</label>
        <input style={inputStyle} value={title} onChange={(e) => setTitle(e.target.value)} placeholder="e.g. Deep work" aria-label="Title" />

        <label style={labelStyle}>Kind</label>
        <select style={inputStyle} value={kind} onChange={(e) => setKind(e.target.value as BlockKind)} aria-label="Kind">
          <option value="focus">Focus</option>
          <option value="errands">Errands</option>
          <option value="admin">Admin</option>
          <option value="dnd">Do Not Disturb</option>
        </select>

        <label style={labelStyle}>Date</label>
        <input style={inputStyle} type="date" value={dateStr} onChange={(e) => setDateStr(e.target.value)} aria-label="Date" />

        <div style={{ display: "flex", gap: 10 }}>
          <div style={{ flex: 1 }}>
            <label style={labelStyle}>Start</label>
            <input style={inputStyle} type="time" value={startTime} onChange={(e) => setStartTime(e.target.value)} aria-label="Start time" />
          </div>
          <div style={{ flex: 1 }}>
            <label style={labelStyle}>End</label>
            <input style={inputStyle} type="time" value={endTime} onChange={(e) => setEndTime(e.target.value)} aria-label="End time" />
          </div>
        </div>

        <div style={{ display: "flex", gap: 8, marginTop: 24 }}>
          <button style={btnPrimary} onClick={onSave}>{block ? "Save" : "Create"}</button>
          <button style={btnGhost} onClick={onClose}>Cancel</button>
        </div>

        {block && <button style={btnDanger} onClick={onDelete}>Delete block</button>}
      </aside>
    </div>
  );
}
