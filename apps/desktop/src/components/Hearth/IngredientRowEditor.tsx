import { X } from "lucide-react";
import type { IngredientLine } from "../../lib/recipe/recipe-ipc";

interface Props {
  row: IngredientLine;
  onChange: (row: IngredientLine) => void;
  onRemove: () => void;
}

export function IngredientRowEditor({ row, onChange, onRemove }: Props) {
  return (
    <div style={{ display: "flex", gap: 6, alignItems: "center", marginBottom: 4 }}>
      <input
        placeholder="qty"
        value={row.quantity_text ?? ""}
        onChange={(e) => onChange({ ...row, quantity_text: e.target.value || null })}
        style={{ width: 80 }}
      />
      <input
        placeholder="ingredient"
        value={row.ingredient_name}
        onChange={(e) => onChange({ ...row, ingredient_name: e.target.value })}
        style={{ flex: 1 }}
      />
      <input
        placeholder="note"
        value={row.note ?? ""}
        onChange={(e) => onChange({ ...row, note: e.target.value || null })}
        style={{ flex: 1 }}
      />
      <button
        type="button"
        aria-label="Remove ingredient"
        onClick={onRemove}
        style={{ background: "transparent", border: "none", cursor: "pointer" }}
      >
        <X size={14} strokeWidth={1.8} />
      </button>
    </div>
  );
}
