import { useEffect, useState } from "react";
import { useMealPlanStore } from "../../../lib/meal_plan/meal-plan-state";
import { useHearthViewStore } from "../../../lib/hearth/view-state";
import type { MealPlanEntryWithRecipe } from "../../../lib/meal_plan/meal-plan-ipc";
import * as recipeIpc from "../../../lib/recipe/recipe-ipc";
import { DaySlotCard } from "./DaySlotCard";
import { WeekNav } from "./WeekNav";
import { RecipePickerDrawer } from "./RecipePickerDrawer";

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
  const { weekStart, entries, loadStatus, setWeekStart, loadWeek, setEntry, clearEntry } = useMealPlanStore();
  const { openRecipeDetail } = useHearthViewStore();
  const [pickerDate, setPickerDate] = useState<string | null>(null);
  const [ghostEntry, setGhostEntry] = useState<MealPlanEntryWithRecipe | null>(null);

  useEffect(() => { void loadWeek(); }, [weekStart, loadWeek]);

  const todayIso = new Date().toISOString().slice(0, 10);

  return (
    <div>
      <WeekNav
        weekStart={weekStart}
        onPrev={() => setWeekStart(stepWeek(weekStart, -7))}
        onNext={() => setWeekStart(stepWeek(weekStart, +7))}
        onToday={() => setWeekStart(mondayOfToday())}
        onJumpToDate={(d) => {
          const date = new Date(d + "T00:00:00");
          const offset = (date.getDay() + 6) % 7;
          date.setDate(date.getDate() - offset);
          setWeekStart(date.toISOString().slice(0, 10));
        }}
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
                onEmptyClick={() => setPickerDate(entry.entry_date)}
                onFilledClick={(id) => openRecipeDetail(id)}
                onGhostClick={(e) => setGhostEntry(e)}
                onSwap={() => setPickerDate(entry.entry_date)}
                onRemove={() => void clearEntry(entry.entry_date)}
              />
            </div>
          );
        })}
      </div>
      {pickerDate && (
        <RecipePickerDrawer
          date={pickerDate}
          onClose={() => setPickerDate(null)}
          onPick={async (recipeId) => {
            await setEntry(pickerDate, recipeId);
            setPickerDate(null);
          }}
        />
      )}
      {ghostEntry && (
        <div style={{
          position: "fixed", top: 0, right: 0, bottom: 0, width: 480,
          background: "var(--paper, #fff)",
          borderLeft: "1px solid var(--hairline, #e5e5e5)",
          padding: 24, overflow: "auto", zIndex: 50,
        }}>
          <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
            <h2 style={{ margin: 0, fontSize: 18 }}>Recipe deleted</h2>
            <button type="button" onClick={() => setGhostEntry(null)} aria-label="Close">✕</button>
          </div>
          <p style={{ color: "var(--ink-soft, #999)", marginBottom: 16 }}>
            "{ghostEntry.recipe?.title ?? "—"}" was moved to Trash. Restore the recipe, or unplan this day.
          </p>
          <div style={{ display: "flex", gap: 8 }}>
            <button type="button" onClick={async () => {
              if (ghostEntry.recipe) { await recipeIpc.restore(ghostEntry.recipe.id); }
              await loadWeek();
              setGhostEntry(null);
            }}>Restore recipe</button>
            <button type="button" onClick={async () => {
              await clearEntry(ghostEntry.entry_date);
              setGhostEntry(null);
            }}>Unplan this day</button>
          </div>
        </div>
      )}
    </div>
  );
}
