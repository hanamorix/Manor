import { useEffect, useState } from "react";
import { ImageOff } from "lucide-react";
import * as recipeIpc from "../../../lib/recipe/recipe-ipc";
import type { Recipe } from "../../../lib/recipe/recipe-ipc";

interface Props {
  date: string;
  onClose: () => void;
  onPick: (recipeId: string) => void;
}

function formatDate(d: string): string {
  const date = new Date(d + "T00:00:00");
  return date.toLocaleDateString(undefined, { weekday: "long", month: "short", day: "numeric" });
}

function RecipeRow({ recipe, onClick }: { recipe: Recipe; onClick: () => void }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (recipe.hero_attachment_uuid) {
      void recipeIpc.attachmentSrc(recipe.hero_attachment_uuid).then(setSrc).catch(() => {});
    }
  }, [recipe.hero_attachment_uuid]);

  const meta = [
    recipe.cook_time_mins != null || recipe.prep_time_mins != null
      ? `${(recipe.prep_time_mins ?? 0) + (recipe.cook_time_mins ?? 0)}m`
      : null,
    recipe.servings != null ? `serves ${recipe.servings}` : null,
  ].filter(Boolean).join(" · ");

  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        width: "100%",
        padding: 8,
        background: "transparent",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 4,
        cursor: "pointer",
        textAlign: "left",
        marginBottom: 6,
      }}
    >
      <div style={{ width: 48, height: 48, background: "var(--paper-muted, #f5f5f5)",
                    display: "flex", alignItems: "center", justifyContent: "center",
                    borderRadius: 4, overflow: "hidden", flexShrink: 0 }}>
        {src ? <img src={src} alt="" style={{ width: "100%", height: "100%", objectFit: "cover" }} />
             : <ImageOff size={18} strokeWidth={1.4} color="var(--ink-soft, #999)" />}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontWeight: 600, fontSize: 14, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          {recipe.title}
        </div>
        {meta && <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>{meta}</div>}
      </div>
    </button>
  );
}

export function RecipePickerDrawer({ date, onClose, onPick }: Props) {
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [results, setResults] = useState<Recipe[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const h = setTimeout(() => setDebounced(query), 200);
    return () => clearTimeout(h);
  }, [query]);

  useEffect(() => {
    setError(null);
    void recipeIpc.list(debounced || undefined, []).then(setResults).catch((e: unknown) => {
      setError(e instanceof Error ? e.message : String(e));
    });
  }, [debounced]);

  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, width: 480,
      background: "var(--paper, #fff)",
      borderLeft: "1px solid var(--hairline, #e5e5e5)",
      padding: 24, overflow: "auto", zIndex: 50,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 18 }}>Plan {formatDate(date)}</h2>
        <button type="button" onClick={onClose} aria-label="Close">✕</button>
      </div>
      <input
        autoFocus
        placeholder="Search recipes"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        style={{ width: "100%", marginBottom: 16, padding: 8, fontSize: 14 }}
      />
      {error && <p style={{ color: "var(--ink-danger, #b00020)" }}>{error}</p>}
      {results == null && <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>}
      {results != null && results.length === 0 && (
        <p style={{ color: "var(--ink-soft, #999)" }}>No recipes yet. Add one in the Recipes tab first.</p>
      )}
      {results != null && results.map((r) => (
        <RecipeRow key={r.id} recipe={r} onClick={() => onPick(r.id)} />
      ))}
    </div>
  );
}
