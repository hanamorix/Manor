import type { Category, Transaction } from "../../lib/ledger/ipc";

// Category → pastel background colour for the emoji icon
const CATEGORY_COLORS: Record<number, string> = {
  1: "#E8F4FD",  // Groceries — light blue
  2: "#FFF0F0",  // Eating Out — light red
  3: "#F0F0FF",  // Transport — light purple
  4: "#F5F0FF",  // Utilities — light violet
  5: "#FFF8E6",  // Subscriptions — light amber
  6: "#F0FFF4",  // Health — light green
  7: "#FFF0F8",  // Shopping — light pink
  8: "#F0FAFF",  // Entertainment — light cyan
  9: "#F5F5F5",  // Other — neutral
  10: "#E8FDF0", // Income — green
};

function iconBg(categoryId: number | null): string {
  if (categoryId === null) return "#F5F5F5";
  return CATEGORY_COLORS[categoryId] ?? "#F5F5F5";
}

function formatAmount(pence: number, currency: string): string {
  const symbol = currency === "GBP" ? "£" : currency === "USD" ? "$" : "€";
  const abs = Math.abs(pence) / 100;
  const formatted = abs % 1 === 0 ? abs.toFixed(0) : abs.toFixed(2);
  return `${pence < 0 ? "-" : "+"}${symbol}${formatted}`;
}

interface Props {
  tx: Transaction;
  category: Category | undefined;
  onClick: () => void;
}

export default function TransactionRow({ tx, category, onClick }: Props) {
  const isIncome = tx.amount_pence > 0;

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "10px 12px",
        background: "#fafafa",
        borderRadius: 12,
        cursor: "pointer",
        gap: 10,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 10, minWidth: 0 }}>
        <div
          style={{
            width: 32,
            height: 32,
            borderRadius: 9,
            background: iconBg(tx.category_id),
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 16,
            flexShrink: 0,
          }}
        >
          {category?.emoji ?? "💳"}
        </div>
        <div style={{ minWidth: 0 }}>
          <div
            style={{
              fontSize: 13,
              fontWeight: 600,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {tx.merchant ?? tx.description}
          </div>
          <div style={{ fontSize: 11, color: "#bbb", marginTop: 1 }}>
            {category?.name ?? "Uncategorised"}
            {tx.source === "sync" && " · Synced"}
          </div>
        </div>
      </div>

      <div
        style={{
          fontSize: 13,
          fontWeight: 600,
          color: isIncome ? "#2BB94A" : "inherit",
          flexShrink: 0,
        }}
      >
        {formatAmount(tx.amount_pence, tx.currency)}
      </div>
    </div>
  );
}
