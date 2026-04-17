import { ReceiptText, Plus } from "lucide-react";
import TransactionRow from "./TransactionRow";
import { SectionLabel, Button } from "../../lib/ui";
import type { Category, Transaction } from "../../lib/ledger/ipc";

const MONTH_SHORT = ["Jan","Feb","Mar","Apr","May","Jun",
                     "Jul","Aug","Sep","Oct","Nov","Dec"];

function dayLabel(dateTs: number): string {
  const d = new Date(dateTs * 1000);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);

  if (d.toDateString() === today.toDateString()) return "Today";
  if (d.toDateString() === yesterday.toDateString()) return "Yesterday";
  return `${d.getDate()} ${MONTH_SHORT[d.getMonth()]}`;
}

function groupByDay(txns: Transaction[]): [string, Transaction[]][] {
  const groups = new Map<string, Transaction[]>();
  for (const tx of txns) {
    const label = dayLabel(tx.date);
    if (!groups.has(label)) groups.set(label, []);
    groups.get(label)!.push(tx);
  }
  return Array.from(groups.entries());
}

interface Props {
  transactions: Transaction[];
  categories: Category[];
  onAdd: () => void;
}

export default function TransactionFeed({ transactions, categories, onAdd }: Props) {
  const catMap = new Map(categories.map((c) => [c.id, c]));
  const groups = groupByDay(transactions);

  return (
    <div>
      <SectionLabel
        icon={ReceiptText}
        action={<Button variant="secondary" icon={Plus} onClick={onAdd}>Add</Button>}
      >
        Transactions
      </SectionLabel>

      {groups.length === 0 && (
        <div
          style={{
            textAlign: "center",
            padding: "24px 0",
            fontSize: "var(--text-sm)",
            color: "var(--ink-faint)",
          }}
        >
          No transactions this month.
          <br />
          <span style={{ fontSize: 11 }}>Add one manually or connect a bank in v0.3b.</span>
        </div>
      )}

      {groups.map(([label, txns]) => (
        <div key={label} style={{ marginBottom: 16 }}>
          <div
            style={{
              fontSize: "var(--text-xs)",
              color: "var(--ink-soft)",
              fontWeight: 500,
              padding: "0 4px",
              marginBottom: 6,
            }}
          >
            {label}
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 3 }}>
            {txns.map((tx) => (
              <TransactionRow
                key={tx.id}
                tx={tx}
                category={catMap.get(tx.category_id ?? -1)}
                onClick={() => {/* edit drawer — future task */}}
              />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
