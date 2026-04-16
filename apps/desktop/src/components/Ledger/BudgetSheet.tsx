import { useState } from "react";
import { deleteBudget, upsertBudget } from "../../lib/ledger/ipc";
import type { Budget, Category } from "../../lib/ledger/ipc";

interface Props {
  categories: Category[];
  budgets: Budget[];
  onClose: () => void;
  onChanged: () => Promise<void>;
}

function toPounds(pence: number): string {
  return (pence / 100).toFixed(0);
}

function parsePence(raw: string): number | null {
  const n = parseFloat(raw.replace(/[£,\s]/g, ""));
  if (isNaN(n) || n < 0) return null;
  return Math.round(n * 100);
}

export default function BudgetSheet({ categories, budgets, onClose, onChanged }: Props) {
  const budgetMap = new Map(budgets.map((b) => [b.category_id, b]));
  const [drafts, setDrafts] = useState<Record<number, string>>(() => {
    const init: Record<number, string> = {};
    budgets.forEach((b) => {
      init[b.category_id] = toPounds(b.amount_pence);
    });
    return init;
  });
  const [saving, setSaving] = useState(false);

  const expenseCategories = categories.filter((c) => !c.is_income);

  async function handleSave() {
    setSaving(true);
    try {
      for (const cat of expenseCategories) {
        const raw = drafts[cat.id] ?? "";
        const pence = raw.trim() === "" ? null : parsePence(raw);
        const existing = budgetMap.get(cat.id);

        if (pence === null || pence === 0) {
          // Clear budget if one existed
          if (existing) await deleteBudget(existing.id);
        } else {
          await upsertBudget({ categoryId: cat.id, amountPence: pence });
        }
      }
      await onChanged();
      onClose();
    } catch (e) {
      console.error("BudgetSheet save error:", e);
      setSaving(false);
    }
  }

  const inputStyle: React.CSSProperties = {
    width: 100,
    padding: "7px 10px",
    fontSize: 14,
    border: "1px solid var(--hairline)",
    borderRadius: 8,
    background: "#fafafa",
    fontFamily: "inherit",
    textAlign: "right",
  };

  return (
    <>
      <div
        onClick={onClose}
        style={{
          position: "fixed",
          inset: 0,
          background: "rgba(0,0,0,0.25)",
          zIndex: 700,
        }}
      />
      <div
        style={{
          position: "fixed",
          right: 0,
          top: 0,
          bottom: 0,
          width: 420,
          background: "var(--paper)",
          boxShadow: "-4px 0 24px rgba(0,0,0,0.12)",
          zIndex: 800,
          display: "flex",
          flexDirection: "column",
          animation: "drawerIn 200ms ease-out",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            padding: "18px 20px 14px",
            borderBottom: "1px solid var(--hairline)",
          }}
        >
          <div>
            <div style={{ fontSize: 16, fontWeight: 700 }}>Monthly Budgets</div>
            <div style={{ fontSize: 12, color: "rgba(0,0,0,0.4)", marginTop: 2 }}>
              Leave blank to skip tracking a category
            </div>
          </div>
          <button
            onClick={onClose}
            style={{
              background: "none",
              border: "none",
              fontSize: 20,
              cursor: "pointer",
              color: "rgba(0,0,0,0.4)",
              lineHeight: 1,
              padding: 0,
            }}
          >
            ✕
          </button>
        </div>

        {/* Category list */}
        <div style={{ flex: 1, overflow: "auto", padding: "12px 20px" }}>
          {expenseCategories.map((cat) => (
            <div
              key={cat.id}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "10px 0",
                borderBottom: "1px solid var(--hairline)",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <span style={{ fontSize: 18 }}>{cat.emoji}</span>
                <span style={{ fontSize: 14, fontWeight: 500 }}>{cat.name}</span>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span style={{ fontSize: 13, color: "rgba(0,0,0,0.4)" }}>£</span>
                <input
                  style={inputStyle}
                  type="number"
                  min="0"
                  placeholder="—"
                  value={drafts[cat.id] ?? ""}
                  onChange={(e) =>
                    setDrafts((d) => ({ ...d, [cat.id]: e.target.value }))
                  }
                />
                <span style={{ fontSize: 12, color: "rgba(0,0,0,0.35)" }}>/mo</span>
              </div>
            </div>
          ))}
        </div>

        {/* Footer */}
        <div
          style={{
            padding: "14px 20px",
            borderTop: "1px solid var(--hairline)",
          }}
        >
          <button
            onClick={handleSave}
            disabled={saving}
            style={{
              width: "100%",
              padding: "12px 0",
              background: "var(--imessage-blue)",
              color: "white",
              border: "none",
              borderRadius: 12,
              fontSize: 15,
              fontWeight: 700,
              cursor: saving ? "default" : "pointer",
              opacity: saving ? 0.6 : 1,
              fontFamily: "inherit",
            }}
          >
            {saving ? "Saving…" : "Save Budgets"}
          </button>
        </div>
      </div>
    </>
  );
}
