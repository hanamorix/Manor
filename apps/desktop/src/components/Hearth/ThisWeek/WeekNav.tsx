import { useState } from "react";
import { ChevronLeft, ChevronRight, Calendar } from "lucide-react";

interface Props {
  weekStart: string;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
  onJumpToDate: (date: string) => void;
}

export function WeekNav({ weekStart, onPrev, onNext, onToday, onJumpToDate }: Props) {
  const [open, setOpen] = useState(false);
  const start = new Date(weekStart + "T00:00:00");
  const end = new Date(start);
  end.setDate(start.getDate() + 6);

  const fmt = (d: Date, opts: Intl.DateTimeFormatOptions) =>
    d.toLocaleDateString(undefined, opts);

  const sameMonth = start.getMonth() === end.getMonth();
  const label = sameMonth
    ? `${fmt(start, { month: "short", day: "numeric" })}–${fmt(end, { day: "numeric" })}, ${fmt(start, { year: "numeric" })}`
    : `${fmt(start, { month: "short", day: "numeric" })} – ${fmt(end, { month: "short", day: "numeric" })}, ${fmt(start, { year: "numeric" })}`;

  return (
    <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 16, position: "relative" }}>
      <button type="button" onClick={onPrev} aria-label="Previous week">
        <ChevronLeft size={16} strokeWidth={1.8} />
      </button>
      <div style={{ fontSize: 14, fontWeight: 600, flex: 1 }}>{label}</div>
      <button type="button" onClick={onNext} aria-label="Next week">
        <ChevronRight size={16} strokeWidth={1.8} />
      </button>
      <button type="button" onClick={() => setOpen((v) => !v)} aria-label="Jump to date">
        <Calendar size={16} strokeWidth={1.8} />
      </button>
      <button type="button" onClick={onToday}>Today</button>
      {open && (
        <div style={{
          position: "absolute", top: "100%", right: 60, marginTop: 6, zIndex: 20,
          background: "var(--paper, #fff)", border: "1px solid var(--hairline, #e5e5e5)",
          padding: 8, borderRadius: 4, boxShadow: "0 2px 8px rgba(0,0,0,0.1)",
        }}>
          <input
            type="date"
            defaultValue={weekStart}
            onChange={(e) => { onJumpToDate(e.target.value); setOpen(false); }}
          />
        </div>
      )}
    </div>
  );
}
