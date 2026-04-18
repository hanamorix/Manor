import { useEffect } from "react";
import { useRecipeStore } from "../../lib/recipe/recipe-state";

export function HearthTab() {
  const { recipes, loadStatus, load } = useRecipeStore();
  useEffect(() => { void load(); }, [load]);

  return (
    <div style={{ padding: 32 }}>
      <h1 style={{ fontSize: "var(--text-2xl)", fontWeight: 600, margin: "0 0 20px" }}>
        Recipes
      </h1>
      {loadStatus.kind === "loading" && (
        <p style={{ color: "var(--ink-soft)" }}>Loading…</p>
      )}
      {loadStatus.kind === "error" && (
        <p style={{ color: "var(--ink-danger)" }}>{loadStatus.message}</p>
      )}
      {loadStatus.kind === "idle" && recipes.length === 0 && (
        <p style={{ color: "var(--ink-soft)" }}>Your recipe collection is empty.</p>
      )}
      {loadStatus.kind === "idle" && recipes.length > 0 && (
        <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
          {recipes.map((r) => (
            <li key={r.id} style={{ padding: "8px 0", borderBottom: "1px solid var(--hairline)" }}>
              {r.title}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
