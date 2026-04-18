import type { Recipe } from "../../../lib/recipe/recipe-ipc";
import type { MealPlanEntryWithRecipe } from "../../../lib/meal_plan/meal-plan-ipc";

interface Props {
  recipe: Recipe;
  entries: MealPlanEntryWithRecipe[];   // 7 entries for the current week
  onPick: (date: string) => Promise<void>;
  onClose: () => void;
}

const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

export function AssignDayPopover({ recipe, entries, onPick, onClose }: Props) {
  const todayIso = (() => {
    const d = new Date();
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${dd}`;
  })();

  const handleClick = async (entry: MealPlanEntryWithRecipe) => {
    if (entry.recipe != null) {
      const current = entry.recipe.title;
      if (!window.confirm(`Replace "${current}" with "${recipe.title}"?`)) return;
    }
    await onPick(entry.entry_date);
  };

  return (
    <div
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.2)",
        zIndex: 60,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div style={{
        background: "var(--paper, #fff)",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 6,
        padding: 20,
        minWidth: 520,
        maxWidth: 720,
        boxShadow: "0 4px 16px rgba(0,0,0,0.15)",
      }}>
        <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 12 }}>
          <h3 style={{ margin: 0, fontSize: 15 }}>
            Plan "{recipe.title}" on…
          </h3>
          <button type="button" onClick={onClose} aria-label="Close">✕</button>
        </div>
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(7, 1fr)",
          gap: 6,
        }}>
          {entries.map((e, i) => {
            const day = new Date(e.entry_date + "T00:00:00");
            const isToday = e.entry_date === todayIso;
            const filled = e.recipe != null;
            return (
              <button
                key={e.entry_date}
                type="button"
                onClick={() => void handleClick(e)}
                style={{
                  background: isToday ? "var(--paper-muted, #f5f5f5)" : "transparent",
                  border: "1px solid var(--hairline, #e5e5e5)",
                  borderRadius: 4,
                  padding: 8,
                  cursor: "pointer",
                  display: "flex",
                  flexDirection: "column",
                  gap: 2,
                  minHeight: 64,
                }}
              >
                <div style={{ fontSize: 11, fontWeight: isToday ? 600 : 500,
                              color: isToday ? "var(--ink-strong, #111)" : "var(--ink-soft, #999)" }}>
                  {DAY_LABELS[i]} {day.getDate()}
                </div>
                <div style={{ fontSize: 11,
                              color: filled ? "var(--ink-strong, #111)" : "var(--ink-soft, #999)",
                              whiteSpace: "nowrap",
                              overflow: "hidden",
                              textOverflow: "ellipsis" }}>
                  {filled ? e.recipe!.title : "—"}
                </div>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
