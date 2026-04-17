import type { Category, Transaction } from "../../lib/ledger/ipc";

// CATEGORY_COLORS placeholder — Task 13 replaces with Lucide icons + deletes this
function iconBg(_categoryId: number | null): string {
  return "var(--hairline)";
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
        background: "var(--hairline)",
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
          <div style={{ fontSize: 11, color: "var(--ink-soft)", marginTop: 1 }}>
            {category?.name ?? "Uncategorised"}
            {tx.source === "sync" && " · Synced"}
          </div>
        </div>
      </div>

      <div
        style={{
          fontSize: 13,
          fontWeight: 600,
          color: "var(--ink)",
          flexShrink: 0,
        }}
      >
        {formatAmount(tx.amount_pence, tx.currency)}
      </div>
    </div>
  );
}
