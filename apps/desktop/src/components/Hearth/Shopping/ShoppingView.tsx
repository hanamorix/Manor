import { useEffect, useState } from "react";
import { Plus, RefreshCw } from "lucide-react";
import { useShoppingListStore } from "../../../lib/shopping_list/state";
import { useMealPlanStore } from "../../../lib/meal_plan/meal-plan-state";
import { useHearthViewStore } from "../../../lib/hearth/view-state";
import { ShoppingItemRow } from "./ShoppingItemRow";

function formatWeekRange(weekStart: string): string {
  if (!weekStart) return "this week";
  const start = new Date(weekStart + "T00:00:00");
  if (Number.isNaN(start.getTime())) return "this week";
  const end = new Date(start);
  end.setDate(start.getDate() + 6);
  const fmt = (d: Date, opts: Intl.DateTimeFormatOptions) =>
    d.toLocaleDateString(undefined, opts);
  const sameMonth = start.getMonth() === end.getMonth();
  return sameMonth
    ? `${fmt(start, { month: "short", day: "numeric" })}–${fmt(end, { day: "numeric" })}`
    : `${fmt(start, { month: "short", day: "numeric" })} – ${fmt(end, { month: "short", day: "numeric" })}`;
}

export function ShoppingView() {
  const { items, loadStatus, load, toggle, addManual, deleteItem, regenerate } = useShoppingListStore();
  const { weekStart } = useMealPlanStore();
  const { setSubview } = useHearthViewStore();
  const [adding, setAdding] = useState(false);
  const [newName, setNewName] = useState("");
  const [toast, setToast] = useState<string | null>(null);

  useEffect(() => { void load(); }, [load]);

  const total = items.length;
  const ticked = items.filter((i) => i.ticked).length;
  const generatedCount = items.filter((i) => i.source === "generated").length;
  const manualCount = items.filter((i) => i.source === "manual").length;

  const submitNew = async () => {
    const v = newName.trim();
    if (!v) { setAdding(false); return; }
    try { await addManual(v); } catch (e: unknown) {
      setToast(e instanceof Error ? e.message : String(e));
    }
    setNewName("");
    setAdding(false);
  };

  const doRegenerate = async () => {
    const range = formatWeekRange(weekStart);
    const msg = `Replace ${generatedCount} generated items with a fresh list from ${range}? Your ${manualCount} manual items will be kept.`;
    if (!window.confirm(msg)) return;
    try {
      const report = await regenerate(weekStart);
      if (report.ghost_recipes_skipped > 0) {
        setToast(`Skipped ${report.ghost_recipes_skipped} deleted recipe(s) from your plan.`);
      }
    } catch (e: unknown) {
      setToast(`Couldn't regenerate: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  // Empty states
  const allEmpty = total === 0;

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
        <div>
          <div style={{ fontSize: 18, fontWeight: 600 }}>Shopping list</div>
          <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
            {total} item{total === 1 ? "" : "s"} · {ticked} ticked
          </div>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button type="button" onClick={() => setAdding(true)}
            style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <Plus size={14} strokeWidth={1.8} /> Add item
          </button>
          <button type="button" onClick={doRegenerate}
            style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <RefreshCw size={14} strokeWidth={1.8} /> Regenerate
          </button>
        </div>
      </div>

      {loadStatus.kind === "loading" && (
        <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>
      )}
      {loadStatus.kind === "error" && (
        <p style={{ color: "var(--ink-danger, #b00020)" }}>
          {loadStatus.message} — <button type="button" onClick={() => void load()}>Retry</button>
        </p>
      )}

      {adding && (
        <div style={{ padding: "10px 12px", borderBottom: "1px solid var(--hairline, #e5e5e5)" }}>
          <input
            autoFocus
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onBlur={() => void submitNew()}
            onKeyDown={(e) => {
              if (e.key === "Enter") void submitNew();
              if (e.key === "Escape") { setNewName(""); setAdding(false); }
            }}
            placeholder="e.g. bin bags"
            style={{ width: "100%", fontSize: 14, padding: 4 }}
          />
        </div>
      )}

      {loadStatus.kind === "idle" && allEmpty && !adding && (
        <div style={{ padding: 48, textAlign: "center", color: "var(--ink-soft, #999)" }}>
          <div style={{ marginBottom: 16 }}>Your shopping list is empty.</div>
          <div style={{ display: "inline-flex", gap: 8 }}>
            <button type="button" onClick={doRegenerate}
              style={{ display: "flex", alignItems: "center", gap: 4 }}>
              <RefreshCw size={14} strokeWidth={1.8} /> Generate from this week
            </button>
            <button type="button" onClick={() => setSubview("this_week")}>
              Plan meals →
            </button>
          </div>
        </div>
      )}

      {items.map((item) => (
        <ShoppingItemRow
          key={item.id}
          item={item}
          onToggle={() => { void toggle(item.id); }}
          onDelete={item.source === "manual" ? () => { void deleteItem(item.id); } : undefined}
        />
      ))}

      {toast && (
        <div style={{
          position: "fixed", bottom: 24, left: "50%", transform: "translateX(-50%)",
          background: "var(--paper, #fff)", border: "1px solid var(--hairline, #e5e5e5)",
          padding: "8px 16px", borderRadius: 6, fontSize: 13,
          boxShadow: "0 2px 8px rgba(0,0,0,0.1)",
        }}>
          {toast}
          <button type="button" onClick={() => setToast(null)}
            style={{ marginLeft: 12, background: "transparent", border: "none", cursor: "pointer" }}>
            ✕
          </button>
        </div>
      )}
    </div>
  );
}
