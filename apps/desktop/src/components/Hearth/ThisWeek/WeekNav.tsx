import { ChevronLeft, ChevronRight } from "lucide-react";

interface Props {
  weekStart: string;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
}

export function WeekNav({ weekStart, onPrev, onNext, onToday }: Props) {
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
    <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 16 }}>
      <button type="button" onClick={onPrev} aria-label="Previous week">
        <ChevronLeft size={16} strokeWidth={1.8} />
      </button>
      <div style={{ fontSize: 14, fontWeight: 600, flex: 1 }}>{label}</div>
      <button type="button" onClick={onNext} aria-label="Next week">
        <ChevronRight size={16} strokeWidth={1.8} />
      </button>
      <button type="button" onClick={onToday}>Today</button>
    </div>
  );
}
