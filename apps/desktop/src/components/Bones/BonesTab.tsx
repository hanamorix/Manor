import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { useAssetStore } from "../../lib/asset/state";
import type { AssetCategory } from "../../lib/asset/ipc";
import { AssetCard } from "./AssetCard";
import { AssetEditDrawer } from "./AssetEditDrawer";        // Task 9
import { AssetDetail } from "./AssetDetail";                // Task 10

type View = { mode: "list" } | { mode: "detail"; id: string };

const CATEGORIES: { key: AssetCategory; label: string }[] = [
  { key: "appliance", label: "Appliance" },
  { key: "vehicle",   label: "Vehicle" },
  { key: "fixture",   label: "Fixture" },
  { key: "other",     label: "Other" },
];

export function BonesTab() {
  const { assets, search, setSearch, category, setCategory, loadStatus, load } = useAssetStore();
  const [view, setView] = useState<View>({ mode: "list" });
  const [showNew, setShowNew] = useState(false);
  const [searchInput, setSearchInput] = useState(search);

  useEffect(() => { void load(); }, [load]);

  // 200ms debounce on search input
  useEffect(() => {
    const h = setTimeout(() => {
      if (searchInput !== search) setSearch(searchInput);
    }, 200);
    return () => clearTimeout(h);
  }, [searchInput, search, setSearch]);

  if (view.mode === "detail") {
    return (
      <AssetDetail
        id={view.id}
        onBack={() => { setView({ mode: "list" }); void load(); }}
      />
    );
  }

  return (
    <div style={{ padding: 32, maxWidth: 1200, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
        <h1 style={{ fontSize: 24, fontWeight: 600, margin: 0 }}>Assets</h1>
        <button onClick={() => setShowNew(true)}
          style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <Plus size={14} strokeWidth={1.8} /> New
        </button>
      </div>

      <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
        <input
          placeholder="Search assets"
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          style={{ flex: 1, padding: 8, fontSize: 14 }}
        />
        <select
          value={category ?? ""}
          onChange={(e) => setCategory(e.target.value ? (e.target.value as AssetCategory) : null)}
          style={{ padding: 8, fontSize: 14 }}
        >
          <option value="">All categories</option>
          {CATEGORIES.map((c) => <option key={c.key} value={c.key}>{c.label}</option>)}
        </select>
      </div>

      {loadStatus.kind === "loading" && <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>}
      {loadStatus.kind === "error" && (
        <p style={{ color: "var(--ink-danger, #b00020)" }}>
          {loadStatus.message} — <button onClick={() => void load()}>Retry</button>
        </p>
      )}

      {loadStatus.kind === "idle" && assets.length === 0 && (
        <div style={{ padding: 48, textAlign: "center" }}>
          <p style={{ color: "var(--ink-soft, #999)", marginBottom: 16 }}>
            Your asset registry is empty.
          </p>
          <button onClick={() => setShowNew(true)}
            style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
            <Plus size={14} strokeWidth={1.8} /> New asset
          </button>
        </div>
      )}

      {loadStatus.kind === "idle" && assets.length > 0 && (
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))",
          gap: 16,
        }}>
          {assets.map((a) => (
            <AssetCard key={a.id} asset={a}
              onClick={() => setView({ mode: "detail", id: a.id })} />
          ))}
        </div>
      )}

      {showNew && (
        <AssetEditDrawer
          onClose={() => { setShowNew(false); void load(); }}
        />
      )}
    </div>
  );
}
