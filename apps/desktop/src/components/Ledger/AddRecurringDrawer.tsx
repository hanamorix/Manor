import { useState } from "react";
import { addRecurring, updateRecurring } from "../../lib/ledger/ipc";
import { useOverlay } from "../../lib/overlay/state";
import type { Category, RecurringPayment } from "../../lib/ledger/ipc";

interface Props {
  categories: Category[];
  existing?: RecurringPayment;
  onClose: () => void;
  onSaved: () => void;
}

export default function AddRecurringDrawer({ categories, existing, onClose, onSaved }: Props) {
  useOverlay();

  const [description, setDescription] = useState(existing?.description ?? "");
  const [amountRaw, setAmountRaw] = useState(
    existing ? (existing.amount_pence / 100).toFixed(2) : ""
  );
  const [dayOfMonth, setDayOfMonth] = useState<number | "">(
    existing?.day_of_month ?? ""
  );
  const [categoryId, setCategoryId] = useState<number | "">(
    existing?.category_id ?? ""
  );
  const [note, setNote] = useState(existing?.note ?? "");
  const [active, setActive] = useState(existing?.active ?? true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const expenseCategories = categories.filter((c) => !c.is_income);

  const canSave =
    description.trim().length > 0 &&
    amountRaw.trim().length > 0 &&
    dayOfMonth !== "" &&
    !isNaN(parseFloat(amountRaw));

  async function handleSave() {
    if (!canSave) return;
    const pence = Math.round(parseFloat(amountRaw) * 100);
    if (isNaN(pence) || pence <= 0) {
      setError("Enter a valid amount greater than £0");
      return;
    }
    const day = Number(dayOfMonth);
    if (day < 1 || day > 28) {
      setError("Day of month must be between 1 and 28");
      return;
    }

    setSaving(true);
    setError(null);
    try {
      if (existing) {
        await updateRecurring({
          id: existing.id,
          description: description.trim(),
          amountPence: pence,
          categoryId: categoryId !== "" ? categoryId : undefined,
          dayOfMonth: day,
          active,
          note: note.trim() || undefined,
        });
      } else {
        await addRecurring({
          description: description.trim(),
          amountPence: pence,
          currency: "GBP",
          categoryId: categoryId !== "" ? categoryId : undefined,
          dayOfMonth: day,
          note: note.trim() || undefined,
        });
      }
      onSaved();
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
    color: "var(--ink)",
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
      {/* Backdrop */}
      <div
        onClick={onClose}
        style={{
          position: "fixed",
          inset: 0,
          background: "rgba(0,0,0,0.25)",
          zIndex: 1050,
        }}
      />

      {/* Drawer */}
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
          <div style={{ fontSize: 16, fontWeight: 700, color: "var(--ink)" }}>
            {existing ? "Edit recurring payment" : "Add recurring payment"}
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

        {/* Form body */}
        <div style={{ flex: 1, overflow: "auto", padding: "20px" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            {/* Description */}
            <div>
              <label style={labelStyle}>Description</label>
              <input
                style={inputStyle}
                type="text"
                placeholder="e.g. Netflix, Rent, Gym"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>

            {/* Amount */}
            <div>
              <label style={labelStyle}>Amount (£)</label>
              <input
                style={inputStyle}
                type="number"
                inputMode="decimal"
                step={0.01}
                min={0}
                placeholder="0.00"
                value={amountRaw}
                onChange={(e) => setAmountRaw(e.target.value)}
              />
            </div>

            {/* Day of month */}
            <div>
              <label style={labelStyle}>Day of month</label>
              <input
                style={inputStyle}
                type="number"
                inputMode="numeric"
                min={1}
                max={28}
                placeholder="1–28"
                value={dayOfMonth}
                onChange={(e) =>
                  setDayOfMonth(e.target.value === "" ? "" : Number(e.target.value))
                }
              />
            </div>

            {/* Category */}
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
                {expenseCategories.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.emoji} {c.name}
                  </option>
                ))}
              </select>
            </div>

            {/* Note */}
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

            {/* Active toggle (edit mode only) */}
            {existing && (
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <input
                  id="recurring-active"
                  type="checkbox"
                  checked={active}
                  onChange={(e) => setActive(e.target.checked)}
                  style={{ width: 16, height: 16, cursor: "pointer" }}
                />
                <label
                  htmlFor="recurring-active"
                  style={{ ...labelStyle, marginBottom: 0, cursor: "pointer" }}
                >
                  Active
                </label>
              </div>
            )}
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
                color: "var(--imessage-red)",
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
            display: "flex",
            gap: 10,
          }}
        >
          <button
            onClick={onClose}
            style={{
              flex: 1,
              padding: "10px 16px",
              background: "transparent",
              color: "var(--ink)",
              border: "1px solid var(--hairline)",
              borderRadius: "var(--radius-pill)",
              fontSize: 14,
              fontWeight: 600,
              cursor: "pointer",
              fontFamily: "inherit",
            }}
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={!canSave || saving}
            style={{
              flex: 2,
              padding: "10px 16px",
              background: "var(--imessage-blue)",
              color: "#fff",
              border: "none",
              borderRadius: "var(--radius-pill)",
              fontSize: 14,
              fontWeight: 600,
              cursor: !canSave || saving ? "default" : "pointer",
              opacity: !canSave || saving ? 0.5 : 1,
              fontFamily: "inherit",
            }}
          >
            {saving ? "Saving…" : existing ? "Save changes" : "Add recurring"}
          </button>
        </div>
      </div>
    </>
  );
}
