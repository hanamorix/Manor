import { useEffect, useState } from "react";
import { Plus, Download } from "lucide-react";
import { useRecipeStore } from "../../lib/recipe/recipe-state";
import { RecipeCard } from "./RecipeCard";

export function HearthTab() {
  const { recipes, search, setSearch, loadStatus, load } = useRecipeStore();
  // placeholder — Task 14 wires the detail view
  const [, setSelectedId] = useState<string | null>(null);

  useEffect(() => { void load(); }, [load]);

  const onNew = () => { console.log("New recipe drawer — wired in Task 12"); };
  const onImport = () => { console.log("Import drawer — wired in Task 13"); };

  return (
    <div style={{ padding: 32, maxWidth: 1200, margin: "0 auto" }}>
      {/* Header row */}
      <div style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        marginBottom: 16,
      }}>
        <h1 style={{ fontSize: "var(--text-2xl, 1.75rem)", fontWeight: 600, margin: 0 }}>
          Recipes
        </h1>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            onClick={onNew}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              padding: "6px 12px",
              background: "var(--action-bg, #1f1f1f)",
              color: "var(--action-fg, #ffffff)",
              border: "none",
              borderRadius: "var(--radius-md, 5px)",
              fontSize: "var(--text-sm, 0.8125rem)",
              cursor: "pointer",
            }}
          >
            <Plus size={14} strokeWidth={1.8} /> New
          </button>
          <button
            onClick={onImport}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              padding: "6px 12px",
              background: "transparent",
              color: "var(--ink, #1f1f1f)",
              border: "1px solid var(--action-secondary-border, #e0e0e0)",
              borderRadius: "var(--radius-md, 5px)",
              fontSize: "var(--text-sm, 0.8125rem)",
              cursor: "pointer",
            }}
          >
            <Download size={14} strokeWidth={1.8} /> Import URL
          </button>
        </div>
      </div>

      {/* Search */}
      <input
        placeholder="Search recipes"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        style={{
          width: "100%",
          marginBottom: 16,
          padding: "8px 12px",
          fontSize: "var(--text-md, 0.875rem)",
          border: "1px solid var(--hairline-strong, #e0e0e0)",
          borderRadius: "var(--radius-md, 5px)",
          background: "var(--paper, #fcfcfc)",
          color: "var(--ink, #1f1f1f)",
          boxSizing: "border-box",
        }}
      />

      {/* Loading */}
      {loadStatus.kind === "loading" && (
        <p style={{ color: "var(--ink-soft, #6b6b6b)" }}>Loading…</p>
      )}

      {/* Error */}
      {loadStatus.kind === "error" && (
        <p style={{ color: "var(--ink-danger, #7a1f1f)" }}>{loadStatus.message}</p>
      )}

      {/* Empty state */}
      {loadStatus.kind === "idle" && recipes.length === 0 && (
        <div style={{ textAlign: "center", padding: 48 }}>
          <p style={{ color: "var(--ink-soft, #6b6b6b)", marginBottom: 16 }}>
            Your recipe collection is empty.
          </p>
          <div style={{ display: "inline-flex", gap: 8 }}>
            <button
              onClick={onNew}
              style={{
                padding: "6px 12px",
                background: "var(--action-bg, #1f1f1f)",
                color: "var(--action-fg, #ffffff)",
                border: "none",
                borderRadius: "var(--radius-md, 5px)",
                fontSize: "var(--text-sm, 0.8125rem)",
                cursor: "pointer",
              }}
            >
              + New recipe
            </button>
            <button
              onClick={onImport}
              style={{
                padding: "6px 12px",
                background: "transparent",
                color: "var(--ink, #1f1f1f)",
                border: "1px solid var(--action-secondary-border, #e0e0e0)",
                borderRadius: "var(--radius-md, 5px)",
                fontSize: "var(--text-sm, 0.8125rem)",
                cursor: "pointer",
              }}
            >
              ↓ Import from URL
            </button>
          </div>
        </div>
      )}

      {/* Recipe grid */}
      {loadStatus.kind === "idle" && recipes.length > 0 && (
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))",
          gap: 16,
        }}>
          {recipes.map((r) => (
            <RecipeCard
              key={r.id}
              recipe={r}
              onClick={() => setSelectedId(r.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
