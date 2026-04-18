import { useEffect } from "react";
import { RefreshCw } from "lucide-react";
import { useIdeasStore } from "../../../lib/meal_plan/ideas-state";
import { useHearthViewStore } from "../../../lib/hearth/view-state";
import { RecipeCard } from "../RecipeCard";

export function MealIdeasRow() {
  const { mode, library, loadStatus, loadLibrary } = useIdeasStore();
  const { setSubview } = useHearthViewStore();

  useEffect(() => { void loadLibrary(); }, [loadLibrary]);

  const emptyLibrary = loadStatus.kind === "idle" && library.length === 0;

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
        </div>
        <button
          type="button"
          onClick={() => void loadLibrary()}
          style={{ display: "flex", alignItems: "center", gap: 4 }}
          aria-label="Reshuffle"
        >
          <RefreshCw size={14} strokeWidth={1.8} /> Reshuffle
        </button>
      </div>

      {loadStatus.kind === "loading" && (
        <div style={{ color: "var(--ink-soft, #999)", fontSize: 13, padding: 12 }}>
          Loading…
        </div>
      )}

      {loadStatus.kind === "error" && (
        <div style={{ color: "var(--ink-danger, #b00020)", fontSize: 13, padding: 12 }}>
          {loadStatus.message} <button type="button" onClick={() => void loadLibrary()}>Retry</button>
        </div>
      )}

      {emptyLibrary && (
        <div style={{ color: "var(--ink-soft, #999)", fontSize: 13, padding: 12 }}>
          Add some recipes to your library and suggestions will appear here.{" "}
          <button type="button" onClick={() => setSubview("recipes")}>→ Go to Recipes</button>
        </div>
      )}

      {loadStatus.kind === "idle" && library.length > 0 && (
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(3, 1fr)",
          gap: 16,
        }}>
          {library.map((r) => (
            <RecipeCard
              key={r.id}
              recipe={r}
              onClick={() => console.log("Library card tap — wired in Task 5", r.id)}
            />
          ))}
        </div>
      )}

      <div style={{ marginTop: 8, fontSize: 12, color: "var(--ink-soft, #999)" }}>
        <span>Not feeling it? </span>
        <button type="button"
          onClick={() => console.log("LLM mode — wired in Task 6")}
          style={{ background: "transparent", border: "none", cursor: "pointer",
                   color: "var(--ink-soft, #999)", textDecoration: "underline" }}>
          Try something new →
        </button>
      </div>
    </div>
  );
}
