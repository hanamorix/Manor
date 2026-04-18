import { X } from "lucide-react";
import type { ShoppingListItem } from "../../../lib/shopping_list/ipc";

interface Props {
  item: ShoppingListItem;
  onToggle: () => void;
  onDelete?: () => void;  // undefined for generated items (no manual delete)
}

export function ShoppingItemRow({ item, onToggle, onDelete }: Props) {
  const label = [
    item.quantity_text ?? "",
    item.ingredient_name,
    item.note ? `, ${item.note}` : "",
  ].join(" ").trim().replace(/ +,/g, ",");

  const meta = item.source === "manual"
    ? "manual"
    : (item.recipe_title ?? "recipe");

  return (
    <div
      onClick={onToggle}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "10px 12px",
        borderBottom: "1px solid var(--hairline, #e5e5e5)",
        cursor: "pointer",
        opacity: item.ticked ? 0.5 : 1,
      }}
    >
      <input
        type="checkbox"
        checked={item.ticked}
        onChange={onToggle}
        onClick={(e) => e.stopPropagation()}
        aria-label={`Tick ${item.ingredient_name}`}
      />
      <div style={{
        flex: 1,
        fontSize: 14,
        textDecoration: item.ticked ? "line-through" : "none",
        whiteSpace: "nowrap",
        overflow: "hidden",
        textOverflow: "ellipsis",
      }}>
        {label}
      </div>
      <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
        · {meta}
      </div>
      {onDelete && (
        <button
          type="button"
          aria-label="Remove item"
          onClick={(e) => { e.stopPropagation(); onDelete(); }}
          style={{ background: "transparent", border: "none", cursor: "pointer", padding: 4 }}
        >
          <X size={14} strokeWidth={1.8} />
        </button>
      )}
    </div>
  );
}
