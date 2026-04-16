import { useEffect, useState } from "react";
import { useLedgerStore } from "../../lib/ledger/state";
import {
  listCategories,
  listTransactions,
  listBudgets,
  getMonthlySummary,
} from "../../lib/ledger/ipc";
import { AVATAR_FOOTPRINT_PX } from "../../lib/layout";
import SummaryCard from "./SummaryCard";
import TransactionFeed from "./TransactionFeed";
import AddTransactionForm from "./AddTransactionForm";
import BudgetSheet from "./BudgetSheet";

export default function LedgerView() {
  const { categories, transactions, budgets, summary, currentYear, currentMonth,
          setCategories, setTransactions, setBudgets, setSummary } = useLedgerStore();
  const [showAdd, setShowAdd] = useState(false);
  const [showBudgets, setShowBudgets] = useState(false);

  useEffect(() => {
    void listCategories().then(setCategories);
    void listBudgets().then(setBudgets);
    void listTransactions(currentYear, currentMonth).then(setTransactions);
    void getMonthlySummary(currentYear, currentMonth).then(setSummary);
  }, [currentYear, currentMonth, setCategories, setBudgets, setTransactions, setSummary]);

  const totalBudget = budgets.length > 0
    ? budgets.reduce((sum, b) => sum + b.amount_pence, 0)
    : null;

  return (
    <>
      <main
        style={{
          maxWidth: 760,
          margin: "0 auto",
          padding: `24px 24px ${AVATAR_FOOTPRINT_PX}px 24px`,
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}
      >
        {summary && (
          <SummaryCard
            summary={summary}
            year={currentYear}
            month={currentMonth}
            totalBudget={totalBudget}
            onBudgetPress={() => setShowBudgets(true)}
          />
        )}
        <TransactionFeed
          transactions={transactions}
          categories={categories}
          onAdd={() => setShowAdd(true)}
        />
      </main>

      {showAdd && (
        <AddTransactionForm
          categories={categories}
          onClose={() => setShowAdd(false)}
          onSaved={async () => {
            setShowAdd(false);
            const [txns, s] = await Promise.all([
              listTransactions(currentYear, currentMonth),
              getMonthlySummary(currentYear, currentMonth),
            ]);
            setTransactions(txns);
            setSummary(s);
          }}
        />
      )}

      {showBudgets && (
        <BudgetSheet
          categories={categories}
          budgets={budgets}
          onClose={() => setShowBudgets(false)}
          onChanged={async () => {
            const [bs, s] = await Promise.all([
              listBudgets(),
              getMonthlySummary(currentYear, currentMonth),
            ]);
            setBudgets(bs);
            setSummary(s);
          }}
        />
      )}
    </>
  );
}
