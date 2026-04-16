import type { MonthlySummary } from "../../lib/ledger/ipc";

const MONTH_NAMES = [
  "January","February","March","April","May","June",
  "July","August","September","October","November","December",
];

function gradientForSpend(totalOut: number, totalBudget: number | null): string {
  if (totalBudget === null || totalBudget === 0) return "linear-gradient(135deg, #1a1a2e 0%, #16213e 100%)";
  const pct = totalOut / totalBudget;
  if (pct >= 1) return "linear-gradient(135deg, #2d0000 0%, #3d0a0a 100%)";
  if (pct >= 0.75) return "linear-gradient(135deg, #2d1f00 0%, #3d2900 100%)";
  return "linear-gradient(135deg, #1a1a2e 0%, #16213e 100%)";
}

function progressColor(pct: number): string {
  if (pct >= 1) return "#FF3B30";
  if (pct >= 0.75) return "#FFB347";
  return "white";
}

interface Props {
  summary: MonthlySummary;
  year: number;
  month: number;
  totalBudget: number | null;
  onBudgetPress: () => void;
}

export default function SummaryCard({ summary, year, month, totalBudget, onBudgetPress }: Props) {
  const pct = totalBudget ? Math.min(summary.total_out_pence / totalBudget, 1.1) : 0;
  const remaining = totalBudget ? totalBudget - summary.total_out_pence : null;

  const alertCategories = summary.by_category.filter(
    (c) => c.budget_pence !== null && c.budget_pence > 0 && c.spent_pence / c.budget_pence >= 0.75
  ).slice(0, 3);

  function formatPounds(pence: number): string {
    return `£${(Math.abs(pence) / 100).toFixed(0)}`;
  }

  return (
    <div
      onClick={onBudgetPress}
      role="button"
      tabIndex={0}
      style={{
        background: gradientForSpend(summary.total_out_pence, totalBudget),
        borderRadius: 14,
        padding: "18px 20px",
        color: "white",
        cursor: "pointer",
        marginBottom: 8,
      }}
    >
      <div style={{ fontSize: 11, opacity: 0.5, letterSpacing: 0.6, marginBottom: 8 }}>
        {MONTH_NAMES[month - 1].toUpperCase()} {year}
      </div>

      <div style={{ fontSize: 28, fontWeight: 700, marginBottom: 4 }}>
        {formatPounds(summary.total_out_pence)}
      </div>

      {totalBudget !== null && (
        <div style={{ fontSize: 12, opacity: 0.5, marginBottom: 12 }}>
          of {formatPounds(totalBudget)} budget
          {remaining !== null && remaining >= 0
            ? ` · ${formatPounds(remaining)} remaining`
            : ` · ${formatPounds(Math.abs(remaining ?? 0))} over`}
        </div>
      )}

      {totalBudget !== null && (
        <div
          style={{
            background: "rgba(255,255,255,0.12)",
            borderRadius: 6,
            height: 6,
            marginBottom: alertCategories.length > 0 ? 14 : 0,
          }}
        >
          <div
            style={{
              background: progressColor(pct),
              width: `${Math.min(pct * 100, 100)}%`,
              height: 6,
              borderRadius: 6,
              transition: "width 0.3s",
            }}
          />
        </div>
      )}

      {alertCategories.length > 0 && (
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {alertCategories.map((c) => {
            const catPct = c.budget_pence! > 0 ? c.spent_pence / c.budget_pence! : 0;
            const over = catPct >= 1;
            return (
              <div
                key={c.category_id}
                style={{
                  background: over
                    ? "rgba(255,59,48,0.25)"
                    : "rgba(255,179,71,0.25)",
                  border: `1px solid ${over ? "rgba(255,59,48,0.5)" : "rgba(255,179,71,0.5)"}`,
                  borderRadius: 20,
                  padding: "4px 10px",
                  fontSize: 11,
                }}
              >
                {over ? "🔴" : "⚠️"} {c.category_name} {Math.round(catPct * 100)}%
              </div>
            );
          })}
        </div>
      )}

      {totalBudget === null && (
        <div style={{ fontSize: 12, opacity: 0.4 }}>
          Tap to set budgets →
        </div>
      )}

      {summary.total_in_pence > 0 && (
        <div style={{ fontSize: 12, opacity: 0.5, marginTop: 8 }}>
          +{formatPounds(summary.total_in_pence)} income
        </div>
      )}
    </div>
  );
}
