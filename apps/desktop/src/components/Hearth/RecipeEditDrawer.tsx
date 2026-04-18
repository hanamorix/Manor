import { useEffect, useState } from "react";
import * as ipc from "../../lib/recipe/recipe-ipc";
import type { Recipe, RecipeDraft, IngredientLine } from "../../lib/recipe/recipe-ipc";
import { IngredientRowEditor } from "./IngredientRowEditor";

interface Props {
  recipeId?: string;        // undefined = create mode
  initialDraft?: RecipeDraft; // prefill when coming from import preview
  onClose: () => void;
  onSaved?: (id: string) => void;
  onSubmit?: (draft: RecipeDraft) => Promise<string>; // override save path (e.g. importCommit)
  title?: string;           // override heading
  saveLabel?: string;       // override save button label
}

const EMPTY_DRAFT: RecipeDraft = {
  title: "",
  servings: null,
  prep_time_mins: null,
  cook_time_mins: null,
  instructions: "",
  source_url: null,
  source_host: null,
  import_method: "manual",
  ingredients: [],
  hero_attachment_uuid: null,
};

export function RecipeEditDrawer({ recipeId, initialDraft, onClose, onSaved, onSubmit, title, saveLabel }: Props) {
  const [draft, setDraft] = useState<RecipeDraft>(initialDraft ?? EMPTY_DRAFT);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (recipeId && !initialDraft) {
      void ipc.get(recipeId).then((r: Recipe | null) => {
        if (r) {
          setDraft({
            title: r.title,
            servings: r.servings,
            prep_time_mins: r.prep_time_mins,
            cook_time_mins: r.cook_time_mins,
            instructions: r.instructions,
            source_url: r.source_url,
            source_host: r.source_host,
            import_method: r.import_method,
            ingredients: r.ingredients,
            hero_attachment_uuid: r.hero_attachment_uuid,
          });
        }
      });
    }
  }, [recipeId, initialDraft]);

  const addIngredient = () =>
    setDraft({
      ...draft,
      ingredients: [
        ...draft.ingredients,
        { quantity_text: null, ingredient_name: "", note: null },
      ],
    });

  const save = async () => {
    if (!draft.title.trim()) {
      setError("Title required");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      let id: string;
      if (onSubmit) {
        id = await onSubmit(draft);
      } else if (recipeId) {
        await ipc.update(recipeId, draft);
        id = recipeId;
      } else {
        id = await ipc.create(draft);
      }
      onSaved?.(id);
      onClose();
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      setError(message);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={{
      position: "fixed",
      top: 0,
      right: 0,
      bottom: 0,
      width: 480,
      background: "var(--paper, #fff)",
      borderLeft: "1px solid var(--hairline, #e5e5e5)",
      padding: 24,
      overflow: "auto",
      zIndex: 50,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 20 }}>
          {title ?? (recipeId ? "Edit recipe" : "New recipe")}
        </h2>
        <button type="button" onClick={onClose} aria-label="Close">✕</button>
      </div>

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Title</label>
      <input
        value={draft.title}
        onChange={(e) => setDraft({ ...draft, title: e.target.value })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }}
      />

      <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
        <div style={{ flex: 1 }}>
          <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Servings</label>
          <input
            type="number"
            value={draft.servings ?? ""}
            onChange={(e) => setDraft({ ...draft, servings: e.target.value ? parseInt(e.target.value, 10) : null })}
            style={{ width: "100%", padding: 6 }}
          />
        </div>
        <div style={{ flex: 1 }}>
          <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Prep (min)</label>
          <input
            type="number"
            value={draft.prep_time_mins ?? ""}
            onChange={(e) => setDraft({ ...draft, prep_time_mins: e.target.value ? parseInt(e.target.value, 10) : null })}
            style={{ width: "100%", padding: 6 }}
          />
        </div>
        <div style={{ flex: 1 }}>
          <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Cook (min)</label>
          <input
            type="number"
            value={draft.cook_time_mins ?? ""}
            onChange={(e) => setDraft({ ...draft, cook_time_mins: e.target.value ? parseInt(e.target.value, 10) : null })}
            style={{ width: "100%", padding: 6 }}
          />
        </div>
      </div>

      <h3 style={{ margin: "16px 0 8px 0", fontSize: 14 }}>Ingredients</h3>
      {draft.ingredients.map((row: IngredientLine, i: number) => (
        <IngredientRowEditor
          key={i}
          row={row}
          onChange={(r) => {
            const next = [...draft.ingredients];
            next[i] = r;
            setDraft({ ...draft, ingredients: next });
          }}
          onRemove={() =>
            setDraft({ ...draft, ingredients: draft.ingredients.filter((_, j) => j !== i) })
          }
        />
      ))}
      <button type="button" onClick={addIngredient} style={{ marginTop: 4 }}>
        + Add ingredient
      </button>

      <h3 style={{ margin: "16px 0 8px 0", fontSize: 14 }}>Instructions (markdown)</h3>
      <textarea
        value={draft.instructions}
        onChange={(e) => setDraft({ ...draft, instructions: e.target.value })}
        rows={10}
        style={{ width: "100%", fontFamily: "inherit", padding: 6 }}
      />

      <label style={{ display: "block", fontSize: 12, marginTop: 12, marginBottom: 4 }}>Source URL</label>
      <input
        value={draft.source_url ?? ""}
        onChange={(e) => setDraft({ ...draft, source_url: e.target.value || null })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }}
      />

      {error && (
        <div style={{ color: "var(--ink-danger, #b00020)", marginBottom: 8 }}>{error}</div>
      )}

      <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
        <button type="button" onClick={onClose}>Cancel</button>
        <button type="button" onClick={save} disabled={saving}>
          {saving ? "Saving…" : (saveLabel ?? "Save")}
        </button>
      </div>
    </div>
  );
}
