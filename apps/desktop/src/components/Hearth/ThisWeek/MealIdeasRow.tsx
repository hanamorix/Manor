import { useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";
import { useIdeasStore } from "../../../lib/meal_plan/ideas-state";
import { useMealPlanStore } from "../../../lib/meal_plan/meal-plan-state";
import { useHearthViewStore } from "../../../lib/hearth/view-state";
import { RecipeCard } from "../RecipeCard";
import { IdeaTitleCard } from "./IdeaTitleCard";
import { AssignDayPopover } from "./AssignDayPopover";
import { RecipeEditDrawer } from "../RecipeEditDrawer";
import * as recipeIpc from "../../../lib/recipe/recipe-ipc";
import type { Recipe } from "../../../lib/recipe/recipe-ipc";
import type { IdeaTitle, ImportPreview } from "../../../lib/meal_plan/ideas-ipc";

export function MealIdeasRow() {
  const { mode, library, llm, loadStatus, loadLibrary, loadLlm, backToLibrary, expandAiTitle } = useIdeasStore();
  const { entries, setEntry } = useMealPlanStore();
  const { setSubview } = useHearthViewStore();

  const [assigningRecipe, setAssigningRecipe] = useState<Recipe | null>(null);
  const [expandingIdx, setExpandingIdx] = useState<number | null>(null);
  const [previewDrawer, setPreviewDrawer] = useState<ImportPreview | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [forceShow, setForceShow] = useState(false);

  useEffect(() => { void loadLibrary(); }, [loadLibrary]);

  const allFilled = entries.length === 7 && entries.every((e) => e.recipe !== null);
  const collapsed = allFilled && !forceShow;

  const emptyLibrary = !collapsed && mode === "library" && loadStatus.kind === "idle" && library.length === 0;

  const onReshuffle = () => {
    setForceShow(true);
    if (mode === "library") void loadLibrary();
    else void loadLlm();
  };

  const handleExpand = async (i: number, idea: IdeaTitle) => {
    setExpandingIdx(i);
    try {
      const preview = await expandAiTitle(idea);
      setPreviewDrawer(preview);
    } catch (e: unknown) {
      setToast(e instanceof Error ? e.message : String(e));
    } finally {
      setExpandingIdx(null);
    }
  };

  return (
    <div style={{ marginBottom: 24 }}>
      <div style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        marginBottom: 8,
      }}>
        <div style={{ fontSize: 14, fontWeight: 600 }}>
          Meal ideas{mode === "llm" ? " — AI" : ""}
          {allFilled && <span style={{ color: "var(--ink-soft, #999)", fontWeight: 500 }}>
            {" · Week is fully planned"}
          </span>}
        </div>
        <button
          type="button"
          onClick={onReshuffle}
          style={{ display: "flex", alignItems: "center", gap: 4 }}
          aria-label="Reshuffle"
          disabled={loadStatus.kind === "loading"}
        >
          <RefreshCw size={14} strokeWidth={1.8} /> Reshuffle
        </button>
      </div>

      {!collapsed && loadStatus.kind === "loading" && (
        <div style={{ color: "var(--ink-soft, #999)", fontSize: 13, padding: 12 }}>
          Loading…
        </div>
      )}

      {!collapsed && loadStatus.kind === "error" && (
        <div style={{ color: "var(--ink-danger, #b00020)", fontSize: 13, padding: 12 }}>
          {loadStatus.message}{" "}
          <button type="button" onClick={onReshuffle}>Retry</button>
        </div>
      )}

      {!collapsed && emptyLibrary && (
        <div style={{ color: "var(--ink-soft, #999)", fontSize: 13, padding: 12 }}>
          Add some recipes to your library and suggestions will appear here.{" "}
          <button type="button" onClick={() => setSubview("recipes")}>→ Go to Recipes</button>
        </div>
      )}

      {!collapsed && mode === "library" && loadStatus.kind === "idle" && library.length > 0 && (
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(3, 1fr)",
          gap: 16,
        }}>
          {library.map((r) => (
            <RecipeCard
              key={r.id}
              recipe={r}
              onClick={() => setAssigningRecipe(r)}
            />
          ))}
        </div>
      )}

      {!collapsed && mode === "llm" && loadStatus.kind === "idle" && llm.length > 0 && (
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(3, 1fr)",
          gap: 16,
        }}>
          {llm.map((idea, i) => (
            <IdeaTitleCard
              key={i}
              idea={idea}
              onClick={() => void handleExpand(i, idea)}
              loading={expandingIdx === i}
            />
          ))}
        </div>
      )}

      {!collapsed && mode === "library" && (
        <div style={{ marginTop: 8, fontSize: 12, color: "var(--ink-soft, #999)" }}>
          <span>Not feeling it? </span>
          <button type="button" onClick={() => void loadLlm()}
            disabled={loadStatus.kind === "loading"}
            style={{ background: "transparent", border: "none", cursor: "pointer",
                     color: "var(--ink-soft, #999)", textDecoration: "underline" }}>
            Try something new →
          </button>
        </div>
      )}

      {!collapsed && mode === "llm" && (
        <div style={{ marginTop: 8, fontSize: 12, color: "var(--ink-soft, #999)" }}>
          <button type="button" onClick={backToLibrary}
            disabled={loadStatus.kind === "loading"}
            style={{ background: "transparent", border: "none", cursor: "pointer",
                     color: "var(--ink-soft, #999)", textDecoration: "underline" }}>
            ← Back to library
          </button>
        </div>
      )}

      {assigningRecipe && (
        <AssignDayPopover
          recipe={assigningRecipe}
          entries={entries}
          onClose={() => setAssigningRecipe(null)}
          onPick={async (date) => {
            await setEntry(date, assigningRecipe.id);
            setAssigningRecipe(null);
            await loadLibrary();
          }}
        />
      )}

      {previewDrawer && (
        <RecipeEditDrawer
          initialDraft={previewDrawer.recipe_draft}
          title="Save AI recipe"
          saveLabel="Save to library"
          onClose={() => setPreviewDrawer(null)}
          onSubmit={async (draft) => {
            return await recipeIpc.importCommit(draft, previewDrawer.hero_image_url);
          }}
          onSaved={() => setPreviewDrawer(null)}
        />
      )}

      {toast && (
        <div style={{
          position: "fixed", bottom: 24, left: "50%", transform: "translateX(-50%)",
          background: "var(--paper, #fff)", border: "1px solid var(--hairline, #e5e5e5)",
          padding: "8px 16px", borderRadius: 6, fontSize: 13,
          boxShadow: "0 2px 8px rgba(0,0,0,0.1)",
          zIndex: 70,
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
