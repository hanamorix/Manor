import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, ImageOff, Pencil, Trash2 } from "lucide-react";
import * as ipc from "../../lib/recipe/recipe-ipc";
import type { Recipe } from "../../lib/recipe/recipe-ipc";
import { ImportMethodBadge } from "./ImportMethodBadge";
import { RecipeEditDrawer } from "./RecipeEditDrawer";

interface Props {
  id: string;
  onBack: () => void;
}

export function RecipeDetail({ id, onBack }: Props) {
  const [recipe, setRecipe] = useState<Recipe | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [heroUrl, setHeroUrl] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);

  const reload = useCallback(() => {
    setLoaded(false);
    void ipc.get(id).then((r) => {
      setRecipe(r);
      setLoaded(true);
      setHeroUrl(null); // reset while loading new hero
      if (r?.hero_attachment_uuid) {
        void ipc.attachmentSrc(r.hero_attachment_uuid).then(setHeroUrl).catch(() => {});
      }
    });
  }, [id]);

  useEffect(() => { reload(); }, [reload]);

  if (!loaded) {
    return <div style={{ padding: 32 }}>Loading…</div>;
  }

  if (!recipe) {
    return (
      <div style={{ padding: 32, maxWidth: 720, margin: "0 auto" }}>
        <button type="button" onClick={onBack}
          style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <ArrowLeft size={14} strokeWidth={1.8} /> Back
        </button>
        <h1 style={{ fontSize: 24, fontWeight: 600, marginTop: 16 }}>Recipe not found</h1>
        <p style={{ color: "var(--ink-soft, #999)" }}>
          It may have been moved to Trash. You can restore it from the Trash view.
        </p>
      </div>
    );
  }

  const meta = [
    recipe.prep_time_mins != null ? `${recipe.prep_time_mins}m prep` : null,
    recipe.cook_time_mins != null ? `${recipe.cook_time_mins}m cook` : null,
    recipe.servings != null ? `serves ${recipe.servings}` : null,
  ].filter(Boolean).join(" · ");

  const handleDelete = async () => {
    if (!window.confirm("Move this recipe to Trash?")) return;
    try {
      await ipc.deleteRecipe(id);
      onBack();
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      window.alert(`Failed to delete: ${message}`);
    }
  };

  return (
    <div style={{ padding: 32, maxWidth: 720, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <button
          type="button"
          onClick={onBack}
          style={{ display: "flex", alignItems: "center", gap: 4 }}
        >
          <ArrowLeft size={14} strokeWidth={1.8} /> Back
        </button>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            type="button"
            onClick={() => setEditing(true)}
            style={{ display: "flex", alignItems: "center", gap: 4 }}
          >
            <Pencil size={14} strokeWidth={1.8} /> Edit
          </button>
          <button
            type="button"
            onClick={handleDelete}
            style={{ display: "flex", alignItems: "center", gap: 4 }}
          >
            <Trash2 size={14} strokeWidth={1.8} /> Delete
          </button>
        </div>
      </div>

      {/* Hero image — 16:9, max 360px tall. Flat placeholder when absent. */}
      <div style={{
        width: "100%",
        aspectRatio: "16 / 9",
        maxHeight: 360,
        borderRadius: 8,
        overflow: "hidden",
        background: "var(--paper-muted, #f5f5f5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        marginBottom: 24,
      }}>
        {heroUrl ? (
          <img
            src={heroUrl}
            alt={recipe.title}
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : (
          <ImageOff size={48} strokeWidth={1.4} color="var(--ink-soft, #999)" />
        )}
      </div>

      <h1 style={{ fontSize: 28, fontWeight: 600, margin: 0 }}>{recipe.title}</h1>
      {meta && (
        <div style={{ color: "var(--ink-soft, #999)", marginTop: 4 }}>{meta}</div>
      )}

      <div style={{ marginTop: 12, display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        {recipe.source_host && (
          <span style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
            Source: {recipe.source_host}
          </span>
        )}
        <ImportMethodBadge method={recipe.import_method} />
      </div>

      <h2 style={{ marginTop: 32, fontSize: 18 }}>Ingredients</h2>
      {recipe.ingredients.length === 0 ? (
        <p style={{ color: "var(--ink-soft, #999)" }}>No ingredients listed.</p>
      ) : (
        <ul>
          {recipe.ingredients.map((ing, i) => (
            <li key={i}>
              {ing.quantity_text && <strong>{ing.quantity_text} </strong>}
              {ing.ingredient_name}
              {ing.note && <span style={{ color: "var(--ink-soft, #999)" }}>, {ing.note}</span>}
            </li>
          ))}
        </ul>
      )}

      <h2 style={{ marginTop: 32, fontSize: 18 }}>Instructions</h2>
      {recipe.instructions.trim() ? (
        <pre
          style={{
            whiteSpace: "pre-wrap",
            fontFamily: "inherit",
            background: "var(--paper-muted, #f5f5f5)",
            padding: 16,
            borderRadius: 6,
          }}
        >
          {recipe.instructions}
        </pre>
      ) : (
        <p style={{ color: "var(--ink-soft, #999)" }}>No instructions yet.</p>
      )}

      {editing && (
        <RecipeEditDrawer
          recipeId={id}
          onClose={() => { setEditing(false); reload(); }}
        />
      )}
    </div>
  );
}
