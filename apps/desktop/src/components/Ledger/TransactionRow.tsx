import {
  ShoppingBag, UtensilsCrossed, Bus, Zap, CreditCard,
  Pill, Shirt, Music, CircleDashed, TrendingUp,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { Category, Transaction } from "../../lib/ledger/ipc";

const CATEGORY_ICON: Record<string, LucideIcon> = {
  groceries:     ShoppingBag,
  "eating out":  UtensilsCrossed,
  transport:     Bus,
  utilities:     Zap,
  subscriptions: CreditCard,
  health:        Pill,
  shopping:      Shirt,
  entertainment: Music,
  income:        TrendingUp,
};

function categoryIcon(category: Category | undefined): LucideIcon {
  if (!category) return CircleDashed;
  if (category.is_income) return TrendingUp;
  return CATEGORY_ICON[category.name.toLowerCase()] ?? CircleDashed;
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
  const Icon = categoryIcon(category);
  const isIncome = category?.is_income ?? tx.amount_pence > 0;

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      style={{
        display: "grid",
        gridTemplateColumns: "14px 1fr auto",
        alignItems: "center",
        gap: 10,
        padding: "8px 0",
        borderBottom: "1px solid var(--hairline)",
        cursor: "pointer",
      }}
    >
      <Icon size={14} strokeWidth={1.8} color="var(--ink-soft)" />
      <div style={{ minWidth: 0 }}>
        <div
          style={{
            fontSize: "var(--text-md, 13px)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            color: "var(--ink)",
          }}
        >
          {tx.merchant ?? tx.description}
        </div>
        <div style={{ fontSize: "var(--text-xs, 11px)", color: "var(--ink-soft)", marginTop: 1 }}>
          {category?.name ?? "Uncategorised"}
          {tx.source === "sync" && " · Synced"}
        </div>
      </div>
      <span
        className="num"
        style={{
          fontSize: "var(--text-md, 13px)",
          fontWeight: isIncome ? 600 : 400,
          color: "var(--ink)",
          flexShrink: 0,
        }}
      >
        {formatAmount(tx.amount_pence, tx.currency)}
      </span>
    </div>
  );
}
