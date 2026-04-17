import type { MonthlySummary } from "../../lib/ledger/ipc";

const MONTH_NAMES = [
  "January","February","March","April","May","June",
  "July","August","September","October","November","December",
];

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
  const over = pct >= 1;
  const nearLimit = pct >= 0.75 && !over;

  const cardBg = "var(--action-bg)";
  const cardFg = "var(--action-fg)";

  const alertCategories = summary.by_category.filter(
    (c) => c.budget_pence !== null && c.budget_pence > 0 && c.spent_pence / c.budget_pence >= 0.75
  ).slice(0, 3);

  function formatPounds(pence: number): string {
    return `£${(Math.abs(pence) / 100).toFixed(0)}`;
  }

  return (
    <div
      onClick={onBudgetPress}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onBudgetPress();
        }
      }}
      role="button"
      tabIndex={0}
      style={{
        background: cardBg,
        borderRadius: "var(--radius-lg)",
        padding: "18px 20px",
        color: cardFg,
        cursor: "pointer",
        marginBottom: 8,
      }}
    >
      <div style={{ fontSize: "var(--text-xs)", opacity: 0.5, marginBottom: 8 }}>
        {MONTH_NAMES[month - 1]} {year}
      </div>

      <div
        style={{
          fontSize: 28,
          fontWeight: over ? 600 : 600,
          marginBottom: 4,
          fontFamily: "var(--font-mono)",
          fontVariantNumeric: "tabular-nums",
          color: cardFg,
          textDecoration: over ? "underline" : "none",
        }}
      >
        {formatPounds(summary.total_out_pence)}
      </div>

      {totalBudget !== null && (
        <div style={{ fontSize: "var(--text-xs)", opacity: 0.5, marginBottom: 12, color: cardFg }}>
          of {formatPounds(totalBudget)} budget
          {remaining !== null && remaining >= 0
            ? ` · ${formatPounds(remaining)} remaining`
            : ` · ${formatPounds(Math.abs(remaining ?? 0))} over`}
        </div>
      )}

      {totalBudget !== null && (
        <div
          style={{
            background: "rgba(128,128,128,0.2)",
            borderRadius: 6,
            height: 6,
            marginBottom: alertCategories.length > 0 ? 14 : 0,
          }}
        >
          <div
            style={{
              background: cardFg,
              opacity: nearLimit ? 0.7 : 1,
              width: "100%",
              transform: `scaleX(${Math.min(pct, 1)})`,
              transformOrigin: "left",
              height: 6,
              borderRadius: 6,
              transition: "transform var(--duration-med) var(--ease-out)",
            }}
          />
        </div>
      )}

      {alertCategories.length > 0 && (
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {alertCategories.map((c) => {
            const catPct = c.budget_pence! > 0 ? c.spent_pence / c.budget_pence! : 0;
            const catOver = catPct >= 1;
            return (
              <div
                key={c.category_id}
                style={{
                  background: "rgba(128,128,128,0.15)",
                  border: `1px solid ${cardFg}`,
                  borderRadius: "var(--radius-lg)",
                  padding: "4px 10px",
                  fontSize: 11,
                  color: cardFg,
                  opacity: catOver ? 1 : 0.7,
                }}
              >
                {c.category_name} {Math.round(catPct * 100)}%
              </div>
            );
          })}
        </div>
      )}

      {totalBudget === null && (
        <div style={{ fontSize: "var(--text-xs)", opacity: 0.4, color: cardFg }}>
          Tap to set budgets →
        </div>
      )}

      {summary.total_in_pence > 0 && (
        <div style={{ fontSize: "var(--text-xs)", opacity: 0.5, marginTop: 8, color: cardFg }}>
          +{formatPounds(summary.total_in_pence)} income
        </div>
      )}
    </div>
  );
}
