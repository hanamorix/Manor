import type { CategorySpendTotal } from "../../../lib/maintenance/event-ipc";

interface Props {
  totals: CategorySpendTotal[];
}

const CATEGORIES = [
  { key: "appliance", label: "Appliance", emoji: "🏠" },
  { key: "vehicle",   label: "Vehicle",   emoji: "🚗" },
  { key: "fixture",   label: "Fixture",   emoji: "🔧" },
  { key: "other",     label: "Other",     emoji: "📦" },
] as const;

function gbp(pence: number): string {
  return `£${(pence / 100).toFixed(0)}`;
}

export function SpendCategoryStrip({ totals }: Props) {
  const byKey: Record<string, CategorySpendTotal | undefined> = Object.fromEntries(
    totals.map((t) => [t.category, t]),
  );

  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(4, 1fr)",
        gap: 12,
        marginBottom: 24,
      }}
    >
      {CATEGORIES.map((c) => {
        const t = byKey[c.key];
        const twelveM = t ? t.total_last_12m_pence : 0;
        return (
          <div
            key={c.key}
            style={{
              padding: 12,
              border: "1px solid var(--border, #eee)",
              borderRadius: 6,
            }}
          >
            <div style={{ fontSize: 13, color: "var(--ink-soft, #777)" }}>
              {c.emoji} {c.label}
            </div>
            <div style={{ fontSize: 20, fontWeight: 500 }}>{gbp(twelveM)}</div>
          </div>
        );
      })}
    </div>
  );
}
