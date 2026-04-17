import { useState } from "react";
import { addTransaction } from "../../lib/ledger/ipc";
import { useOverlay } from "../../lib/overlay/state";
import type { Category } from "../../lib/ledger/ipc";

interface Props {
  categories: Category[];
  onClose: () => void;
  onSaved: () => Promise<void>;
}

function todayTs(): number {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return Math.floor(d.getTime() / 1000);
}

function toDateInputValue(ts: number): string {
  return new Date(ts * 1000).toISOString().slice(0, 10);
}

function parsePence(raw: string): number | null {
  // Accept "12.50", "12", "£12.50", "-12.50"
  const cleaned = raw.replace(/[£$€,\s]/g, "");
  const n = parseFloat(cleaned);
  if (isNaN(n)) return null;
  return Math.round(n * 100);
}

export default function AddTransactionForm({ categories, onClose, onSaved }: Props) {
  useOverlay();
  const [amountRaw, setAmountRaw] = useState("");
  const [description, setDescription] = useState("");
  const [categoryId, setCategoryId] = useState<number | "">("");
  const [date, setDate] = useState(toDateInputValue(todayTs()));
  const [note, setNote] = useState("");
  const [isIncome, setIsIncome] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const expenseCategories = categories.filter((c) => !c.is_income);
  const incomeCategories = categories.filter((c) => c.is_income);
  const visibleCategories = isIncome ? incomeCategories : expenseCategories;

  async function handleSave() {
    const pence = parsePence(amountRaw);
    if (pence === null || pence === 0) {
      setError("Enter a valid amount");
      return;
    }
    if (!description.trim()) {
      setError("Enter a description");
      return;
    }
    const dateTs = Math.floor(new Date(date + "T00:00:00").getTime() / 1000);
    const signedPence = isIncome ? Math.abs(pence) : -Math.abs(pence);

    setSaving(true);
    setError(null);
    try {
      await addTransaction({
        amountPence: signedPence,
        currency: "GBP",
        description: description.trim(),
        categoryId: categoryId !== "" ? categoryId : undefined,
        date: dateTs,
        note: note.trim() || undefined,
      });
      await onSaved();
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  }

  const inputStyle: React.CSSProperties = {
    width: "100%",
    padding: "9px 12px",
    fontSize: 14,
    border: "1px solid var(--hairline)",
    borderRadius: 10,
    background: "#fafafa",
    fontFamily: "inherit",
    boxSizing: "border-box",
  };

  const labelStyle: React.CSSProperties = {
    fontSize: 11,
    fontWeight: 700,
    textTransform: "uppercase",
    letterSpacing: 0.5,
    color: "rgba(0,0,0,0.5)",
    marginBottom: 5,
    display: "block",
  };

  return (
    <>
      <div
        onClick={onClose}
        style={{
          position: "fixed",
          inset: 0,
          background: "rgba(0,0,0,0.25)",
          zIndex: 1050,
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
          zIndex: 1100,
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
          <div style={{ fontSize: 16, fontWeight: 700 }}>Add Transaction</div>
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

        {/* Form body */}
        <div style={{ flex: 1, overflow: "auto", padding: "20px" }}>
          {/* Income / Expense toggle */}
          <div style={{ display: "flex", gap: 8, marginBottom: 20 }}>
            {(["expense", "income"] as const).map((type) => (
              <button
                key={type}
                onClick={() => {
                  setIsIncome(type === "income");
                  setCategoryId("");
                }}
                style={{
                  flex: 1,
                  padding: "8px 0",
                  borderRadius: 10,
                  border: "1px solid var(--hairline)",
                  background:
                    (type === "income") === isIncome
                      ? type === "income"
                        ? "#2BB94A"
                        : "#0866EF"
                      : "transparent",
                  color: (type === "income") === isIncome ? "white" : "rgba(0,0,0,0.5)",
                  fontWeight: 600,
                  fontSize: 13,
                  cursor: "pointer",
                  fontFamily: "inherit",
                }}
              >
                {type === "income" ? "Income" : "Expense"}
              </button>
            ))}
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div>
              <label style={labelStyle}>Amount</label>
              <input
                style={inputStyle}
                type="text"
                inputMode="decimal"
                placeholder="£0.00"
                value={amountRaw}
                onChange={(e) => setAmountRaw(e.target.value)}
              />
            </div>

            <div>
              <label style={labelStyle}>Description</label>
              <input
                style={inputStyle}
                type="text"
                placeholder="e.g. Tesco Express"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>

            <div>
              <label style={labelStyle}>Category</label>
              <select
                style={{ ...inputStyle, appearance: "none" }}
                value={categoryId}
                onChange={(e) =>
                  setCategoryId(e.target.value === "" ? "" : Number(e.target.value))
                }
              >
                <option value="">Uncategorised</option>
                {visibleCategories.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.emoji} {c.name}
                  </option>
                ))}
              </select>
            </div>

            <div>
              <label style={labelStyle}>Date</label>
              <input
                style={inputStyle}
                type="date"
                value={date}
                onChange={(e) => setDate(e.target.value)}
              />
            </div>

            <div>
              <label style={labelStyle}>Note (optional)</label>
              <input
                style={inputStyle}
                type="text"
                placeholder="Optional note"
                value={note}
                onChange={(e) => setNote(e.target.value)}
              />
            </div>
          </div>

          {error && (
            <div
              style={{
                marginTop: 16,
                padding: "10px 12px",
                background: "rgba(255,59,48,0.08)",
                border: "1px solid rgba(255,59,48,0.3)",
                borderRadius: 10,
                fontSize: 13,
                color: "var(--ink)",
              }}
            >
              {error}
            </div>
          )}
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
              background: "var(--action-bg)",
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
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </>
  );
}
