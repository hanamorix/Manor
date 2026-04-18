import { useEffect, useState } from "react";
import { Plus, Download } from "lucide-react";
import { useRecipeStore } from "../../lib/recipe/recipe-state";
import { useHearthViewStore } from "../../lib/hearth/view-state";
import { RecipeCard } from "./RecipeCard";
import { RecipeEditDrawer } from "./RecipeEditDrawer";
import { RecipeImportDrawer } from "./RecipeImportDrawer";
import { RecipeDetail } from "./RecipeDetail";

export function RecipesView() {
  const { recipes, search, setSearch, loadStatus, load } = useRecipeStore();
  const { pendingDetailId, clearPendingDetail } = useHearthViewStore();
  const [detailId, setDetailId] = useState<string | null>(null);
  const [drawer, setDrawer] = useState<null | "new" | "import">(null);
  // Local search input tracks keystrokes; store update (and IPC load) is
  // debounced at 200ms so we don't fire one round-trip per keystroke (spec §6.2).
  const [searchInput, setSearchInput] = useState(search);

  useEffect(() => { void load(); }, [load]);

  useEffect(() => {
    if (pendingDetailId) {
      setDetailId(pendingDetailId);
      clearPendingDetail();
    }
  }, [pendingDetailId, clearPendingDetail]);

  useEffect(() => {
    const handle = setTimeout(() => {
      if (searchInput !== search) setSearch(searchInput);
    }, 200);
    return () => clearTimeout(handle);
  }, [searchInput, search, setSearch]);

  if (detailId) {
    return (
      <RecipeDetail
        id={detailId}
        onBack={() => { setDetailId(null); void load(); }}
      />
    );
  }

  const onNew = () => { setDrawer("new"); };
  const onImport = () => { setDrawer("import"); };

  return (
    <div>
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
        value={searchInput}
        onChange={(e) => setSearchInput(e.target.value)}
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
              onClick={() => setDetailId(r.id)}
            />
          ))}
        </div>
      )}

      {drawer === "new" && (
        <RecipeEditDrawer
          onClose={() => setDrawer(null)}
          onSaved={() => { setDrawer(null); void load(); }}
        />
      )}
      {drawer === "import" && (
        <RecipeImportDrawer
          onClose={() => setDrawer(null)}
          onSaved={() => { setDrawer(null); void load(); }}
        />
      )}
    </div>
  );
}
