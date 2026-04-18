import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { useStaplesStore } from "../../../lib/meal_plan/staples-state";
import { StapleRow } from "./StapleRow";

export function StaplesView() {
  const { staples, load, add, updateOne, remove } = useStaplesStore();
  const [newName, setNewName] = useState("");
  const [adding, setAdding] = useState(false);

  useEffect(() => { void load(); }, [load]);

  const submitNew = async () => {
    const v = newName.trim();
    if (!v) { setAdding(false); return; }
    await add(v);
    setNewName("");
    setAdding(false);
  };

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
        <div>
          <div style={{ fontSize: 18, fontWeight: 600 }}>Staples</div>
          <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
            Items your shopping list skips by default.
          </div>
        </div>
        <button type="button" onClick={() => setAdding(true)}
          style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <Plus size={14} strokeWidth={1.8} /> Add staple
        </button>
      </div>

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
            placeholder="e.g. olive oil"
            style={{ width: "100%", fontSize: 14, padding: 4 }}
          />
        </div>
      )}

      {staples.length === 0 && !adding && (
        <div style={{ padding: 32, textAlign: "center", color: "var(--ink-soft, #999)" }}>
          No staples yet. Add "salt", "olive oil", or anything else you always have so your shopping list won't repeat them.
        </div>
      )}

      {staples.map((s) => (
        <StapleRow
          key={s.id}
          staple={s}
          onUpdate={(name, aliases) => updateOne(s.id, { name, aliases })}
          onRemove={() => void remove(s.id)}
        />
      ))}
    </div>
  );
}
