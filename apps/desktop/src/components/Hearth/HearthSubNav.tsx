import { useHearthViewStore, type HearthSubview } from "../../lib/hearth/view-state";

const TABS: { key: HearthSubview; label: string }[] = [
  { key: "recipes", label: "Recipes" },
  { key: "this_week", label: "This Week" },
  { key: "shopping", label: "Shopping" },
  { key: "staples", label: "Staples" },
];

export function HearthSubNav() {
  const { subview, setSubview } = useHearthViewStore();
  return (
    <div style={{
      display: "flex",
      gap: 24,
      borderBottom: "1px solid var(--hairline, #e5e5e5)",
      marginBottom: 24,
    }}>
      {TABS.map((t) => {
        const active = subview === t.key;
        return (
          <button
            key={t.key}
            type="button"
            onClick={() => setSubview(t.key)}
            style={{
              background: "transparent",
              border: "none",
              padding: "8px 0",
              fontSize: 14,
              fontWeight: active ? 600 : 500,
              color: active ? "var(--ink-strong, #111)" : "var(--ink-soft, #999)",
              borderBottom: active ? "2px solid var(--ink-strong, #111)" : "2px solid transparent",
              cursor: "pointer",
            }}
          >
            {t.label}
          </button>
        );
      })}
    </div>
  );
}
