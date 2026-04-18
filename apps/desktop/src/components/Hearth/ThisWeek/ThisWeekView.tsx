import { useEffect } from "react";
import { useMealPlanStore } from "../../../lib/meal_plan/meal-plan-state";
import { DaySlotCard } from "./DaySlotCard";
import { WeekNav } from "./WeekNav";

const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function stepWeek(dateStr: string, days: number): string {
  const d = new Date(dateStr + "T00:00:00");
  d.setDate(d.getDate() + days);
  return d.toISOString().slice(0, 10);
}

function mondayOfToday(): string {
  const d = new Date();
  const day = (d.getDay() + 6) % 7;
  d.setDate(d.getDate() - day);
  return d.toISOString().slice(0, 10);
}

export function ThisWeekView() {
  const { weekStart, entries, loadStatus, setWeekStart, loadWeek, clearEntry } = useMealPlanStore();

  useEffect(() => { void loadWeek(); }, [weekStart, loadWeek]);

  const todayIso = new Date().toISOString().slice(0, 10);

  return (
    <div>
      <WeekNav
        weekStart={weekStart}
        onPrev={() => setWeekStart(stepWeek(weekStart, -7))}
        onNext={() => setWeekStart(stepWeek(weekStart, +7))}
        onToday={() => setWeekStart(mondayOfToday())}
      />
      {loadStatus.kind === "loading" && <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>}
      {loadStatus.kind === "error" && <p style={{ color: "var(--ink-danger, #b00020)" }}>{loadStatus.message}</p>}
      <div style={{
        display: "grid",
        gridTemplateColumns: "repeat(7, 1fr)",
        gap: 8,
      }}>
        {entries.map((entry, i) => {
          const isToday = entry.entry_date === todayIso;
          const d = new Date(entry.entry_date + "T00:00:00");
          return (
            <div key={entry.entry_date} style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <div style={{
                fontSize: 11,
                color: isToday ? "var(--ink-strong, #111)" : "var(--ink-soft, #999)",
                fontWeight: isToday ? 600 : 500,
              }}>
                {DAY_LABELS[i]} {d.getDate()}
              </div>
              <DaySlotCard
                entry={entry}
                isToday={isToday}
                onEmptyClick={() => console.log("Picker — Task 10", entry.entry_date)}
                onFilledClick={(id) => console.log("Detail nav — Task 11", id)}
                onGhostClick={(e) => console.log("Ghost — Task 11", e)}
                onSwap={() => console.log("Swap — Task 10", entry.entry_date)}
                onRemove={() => void clearEntry(entry.entry_date)}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
