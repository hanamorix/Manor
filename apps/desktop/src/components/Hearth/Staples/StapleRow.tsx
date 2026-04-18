import { useEffect, useState } from "react";
import { X, MoreHorizontal } from "lucide-react";
import type { StapleItem } from "../../../lib/meal_plan/staples-ipc";

interface Props {
  staple: StapleItem;
  onUpdate: (name: string, aliases: string[]) => Promise<void>;
  onRemove: () => void;
}

export function StapleRow({ staple, onUpdate, onRemove }: Props) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(staple.name);
  const [aliases, setAliases] = useState<string[]>(staple.aliases);
  const [aliasInput, setAliasInput] = useState("");

  // Reset draft whenever the underlying staple changes (different row mounted)
  // or when edit mode is toggled — discards stale buffers on re-open.
  useEffect(() => {
    setName(staple.name);
    setAliases(staple.aliases);
    setAliasInput("");
  }, [staple.id, editing]);

  const save = async () => {
    await onUpdate(name.trim() || staple.name, aliases);
    setEditing(false);
  };

  const addAlias = () => {
    const v = aliasInput.trim();
    if (!v) return;
    if (aliases.includes(v)) { setAliasInput(""); return; }
    setAliases([...aliases, v]);
    setAliasInput("");
  };

  return (
    <div style={{
      padding: "10px 12px",
      borderBottom: "1px solid var(--hairline, #e5e5e5)",
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        {editing ? (
          <input
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void save();
              if (e.key === "Escape") { setName(staple.name); setAliases(staple.aliases); setEditing(false); }
            }}
            style={{ flex: 1, fontSize: 14 }}
          />
        ) : (
          <div style={{ flex: 1, fontSize: 14 }}>{staple.name}</div>
        )}
        <button type="button" aria-label="Edit aliases" onClick={() => setEditing(true)}>
          <MoreHorizontal size={14} strokeWidth={1.8} />
        </button>
        <button type="button" aria-label="Delete" onClick={onRemove}>
          <X size={14} strokeWidth={1.8} />
        </button>
      </div>

      {editing ? (
        <div style={{ marginTop: 8, display: "flex", flexWrap: "wrap", gap: 4, alignItems: "center" }}>
          {aliases.map((a, i) => (
            <span key={i} style={{
              background: "var(--paper-muted, #f5f5f5)",
              padding: "2px 8px", borderRadius: 4, fontSize: 12,
              display: "inline-flex", alignItems: "center", gap: 4,
            }}>
              {a}
              <button type="button" aria-label={`Remove alias ${a}`}
                onClick={() => setAliases(aliases.filter((x) => x !== a))}
                style={{ background: "transparent", border: "none", cursor: "pointer", padding: 0 }}>
                <X size={10} strokeWidth={1.8} />
              </button>
            </span>
          ))}
          <input
            value={aliasInput}
            onChange={(e) => setAliasInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addAlias(); } }}
            placeholder="+ alias"
            style={{ border: "none", outline: "none", fontSize: 12, minWidth: 80 }}
          />
          <button type="button" onClick={save}>Save</button>
        </div>
      ) : (
        staple.aliases.length > 0 && (
          <div style={{ marginTop: 4, fontSize: 12, color: "var(--ink-soft, #999)" }}>
            also: {staple.aliases.join(", ")}
          </div>
        )
      )}
    </div>
  );
}
