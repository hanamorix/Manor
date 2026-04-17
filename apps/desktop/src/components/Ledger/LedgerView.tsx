import { useEffect, useState } from "react";
import { useLedgerStore } from "../../lib/ledger/state";
import {
  listCategories,
  listTransactions,
  listBudgets,
  getMonthlySummary,
} from "../../lib/ledger/ipc";
import { AVATAR_FOOTPRINT_PX } from "../../lib/layout";
import MonthReviewPanel from "./MonthReviewPanel";
import RecurringSection from "./RecurringSection";
import ContractsSection from "./ContractsSection";
import TransactionFeed from "./TransactionFeed";
import AddTransactionForm from "./AddTransactionForm";
import BudgetSheet from "./BudgetSheet";
import CsvImportDrawer from "./CsvImportDrawer";

export default function LedgerView() {
  const {
    categories, transactions, budgets, summary, currentYear, currentMonth,
    setCategories, setTransactions, setBudgets, setSummary,
  } = useLedgerStore();
  const [showAdd, setShowAdd] = useState(false);
  const [showBudgets, setShowBudgets] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [importToast, setImportToast] = useState<string | null>(null);

  useEffect(() => {
    void listCategories().then(setCategories);
    void listBudgets().then(setBudgets);
    void listTransactions(currentYear, currentMonth).then(setTransactions);
    void getMonthlySummary(currentYear, currentMonth).then(setSummary);
  }, [currentYear, currentMonth, setCategories, setBudgets, setTransactions, setSummary]);

  const refreshAfterChange = async () => {
    const [txns, s, bs] = await Promise.all([
      listTransactions(currentYear, currentMonth),
      getMonthlySummary(currentYear, currentMonth),
      listBudgets(),
    ]);
    setTransactions(txns);
    setSummary(s);
    setBudgets(bs);
  };

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
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h2 style={{ margin: 0, fontSize: 22, fontWeight: 700, color: "var(--ink)" }}>Ledger</h2>
          <div style={{ display: "flex", gap: 8 }}>
            <button onClick={() => setShowBudgets(true)}>Budgets</button>
            <button onClick={() => setShowImport(true)}>Import CSV</button>
          </div>
        </div>
        {summary && (
          <MonthReviewPanel year={currentYear} month={currentMonth} summary={summary} />
        )}
        <RecurringSection categories={categories} />
        <ContractsSection />
        <TransactionFeed
          transactions={transactions}
          categories={categories}
          onAdd={() => setShowAdd(true)}
        />
        {importToast && (
          <div style={{ fontSize: 12, color: "var(--imessage-green)" }}>{importToast}</div>
        )}
      </main>

      {showAdd && (
        <AddTransactionForm
          categories={categories}
          onClose={() => setShowAdd(false)}
          onSaved={async () => {
            setShowAdd(false);
            await refreshAfterChange();
          }}
        />
      )}
      {showBudgets && (
        <BudgetSheet
          categories={categories}
          budgets={budgets}
          onClose={() => setShowBudgets(false)}
          onChanged={async () => {
            await refreshAfterChange();
          }}
        />
      )}
      {showImport && (
        <CsvImportDrawer
          onClose={() => setShowImport(false)}
          onImported={async (r) => {
            setShowImport(false);
            setImportToast(`Imported ${r.inserted} · skipped ${r.skipped_duplicates} duplicate(s)`);
            await refreshAfterChange();
            setTimeout(() => setImportToast(null), 4000);
          }}
        />
      )}
    </>
  );
}
