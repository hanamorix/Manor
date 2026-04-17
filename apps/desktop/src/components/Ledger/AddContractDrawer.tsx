import { useState } from "react";
import { Check } from "lucide-react";
import { addContract, updateContract } from "../../lib/ledger/ipc";
import { useOverlay } from "../../lib/overlay/state";
import type { Contract } from "../../lib/ledger/ipc";
import { Button } from "../../lib/ui";

interface Props {
  existing?: Contract;
  onClose: () => void;
  onSaved: () => void;
}

function toUnix(yyyymmdd: string): number {
  return Math.floor(new Date(`${yyyymmdd}T00:00:00Z`).getTime() / 1000);
}

function toDateInput(unix: number): string {
  return new Date(unix * 1000).toISOString().slice(0, 10);
}

export default function AddContractDrawer({ existing, onClose, onSaved }: Props) {
  useOverlay();

  const [provider, setProvider] = useState(existing?.provider ?? "");
  const [kind, setKind] = useState<Contract["kind"]>(existing?.kind ?? "other");
  const [description, setDescription] = useState(existing?.description ?? "");
  const [monthlyCostRaw, setMonthlyCostRaw] = useState(
    existing ? (existing.monthly_cost_pence / 100).toFixed(2) : ""
  );
  const [termStart, setTermStart] = useState(
    existing ? toDateInput(existing.term_start) : ""
  );
  const [termEnd, setTermEnd] = useState(
    existing ? toDateInput(existing.term_end) : ""
  );
  const [exitFeeRaw, setExitFeeRaw] = useState(
    existing?.exit_fee_pence != null
      ? (existing.exit_fee_pence / 100).toFixed(2)
      : ""
  );
  const [alertDays, setAlertDays] = useState<number | "">(
    existing?.renewal_alert_days ?? 30
  );
  const [note, setNote] = useState(existing?.note ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSave =
    provider.trim().length > 0 &&
    monthlyCostRaw.trim().length > 0 &&
    !isNaN(parseFloat(monthlyCostRaw)) &&
    termStart.length > 0 &&
    termEnd.length > 0;

  async function handleSave() {
    if (!canSave) return;

    const pence = Math.round(parseFloat(monthlyCostRaw) * 100);
    if (isNaN(pence) || pence <= 0) {
      setError("Enter a valid monthly cost greater than £0");
      return;
    }

    const startUnix = toUnix(termStart);
    const endUnix = toUnix(termEnd);
    if (endUnix <= startUnix) {
      setError("Term end must be after term start");
      return;
    }

    const exitPence =
      exitFeeRaw.trim().length > 0 && !isNaN(parseFloat(exitFeeRaw))
        ? Math.round(parseFloat(exitFeeRaw) * 100)
        : undefined;

    const days = alertDays !== "" ? Number(alertDays) : 30;

    setSaving(true);
    setError(null);
    try {
      if (existing) {
        await updateContract({
          id: existing.id,
          provider: provider.trim(),
          kind,
          description: description.trim() || undefined,
          monthlyCostPence: pence,
          termStart: startUnix,
          termEnd: endUnix,
          exitFeePence: exitPence,
          renewalAlertDays: days,
          note: note.trim() || undefined,
        });
      } else {
        await addContract({
          provider: provider.trim(),
          kind,
          description: description.trim() || undefined,
          monthlyCostPence: pence,
          termStart: startUnix,
          termEnd: endUnix,
          exitFeePence: exitPence,
          renewalAlertDays: days,
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
    background: "var(--hairline)",
    fontFamily: "inherit",
    boxSizing: "border-box",
    color: "var(--ink)",
  };

  const labelStyle: React.CSSProperties = {
    fontSize: 11,
    fontWeight: 600,
    textTransform: "uppercase",
    letterSpacing: 0.5,
    color: "var(--ink-soft)",
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
          background: "var(--scrim)",
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
          <div style={{ fontSize: 16, fontWeight: 600, color: "var(--ink)" }}>
            {existing ? "Edit contract" : "Add contract"}
          </div>
          <button
            onClick={onClose}
            style={{
              background: "none",
              border: "none",
              fontSize: 20,
              cursor: "pointer",
              color: "var(--ink-faint)",
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
            {/* Provider */}
            <div>
              <label style={labelStyle}>Provider</label>
              <input
                style={inputStyle}
                type="text"
                placeholder="e.g. EE, Aviva, Octopus Energy"
                value={provider}
                onChange={(e) => setProvider(e.target.value)}
              />
            </div>

            {/* Kind */}
            <div>
              <label style={labelStyle}>Kind</label>
              <select
                style={{ ...inputStyle, appearance: "none" }}
                value={kind}
                onChange={(e) => setKind(e.target.value as Contract["kind"])}
              >
                <option value="phone">Phone</option>
                <option value="broadband">Broadband</option>
                <option value="insurance">Insurance</option>
                <option value="energy">Energy</option>
                <option value="other">Other</option>
              </select>
            </div>

            {/* Description */}
            <div>
              <label style={labelStyle}>Description (optional)</label>
              <input
                style={inputStyle}
                type="text"
                placeholder="e.g. iPhone 15 Pro, Home contents"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>

            {/* Monthly cost */}
            <div>
              <label style={labelStyle}>Monthly cost (£)</label>
              <input
                style={inputStyle}
                type="number"
                inputMode="decimal"
                step={0.01}
                min={0}
                placeholder="0.00"
                value={monthlyCostRaw}
                onChange={(e) => setMonthlyCostRaw(e.target.value)}
              />
            </div>

            {/* Term dates — two columns */}
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
              <div>
                <label style={labelStyle}>Term start</label>
                <input
                  style={inputStyle}
                  type="date"
                  value={termStart}
                  onChange={(e) => setTermStart(e.target.value)}
                />
              </div>
              <div>
                <label style={labelStyle}>Term end</label>
                <input
                  style={inputStyle}
                  type="date"
                  value={termEnd}
                  onChange={(e) => setTermEnd(e.target.value)}
                />
              </div>
            </div>

            {/* Exit fee */}
            <div>
              <label style={labelStyle}>Exit fee (£, optional)</label>
              <input
                style={inputStyle}
                type="number"
                inputMode="decimal"
                step={0.01}
                min={0}
                placeholder="0.00"
                value={exitFeeRaw}
                onChange={(e) => setExitFeeRaw(e.target.value)}
              />
            </div>

            {/* Alert days */}
            <div>
              <label style={labelStyle}>Alert days before renewal</label>
              <input
                style={inputStyle}
                type="number"
                inputMode="numeric"
                min={1}
                placeholder="30"
                value={alertDays}
                onChange={(e) =>
                  setAlertDays(e.target.value === "" ? "" : Number(e.target.value))
                }
              />
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
            display: "flex",
            gap: 10,
          }}
        >
          <Button variant="secondary" onClick={onClose} style={{ flex: 1 }}>
            Cancel
          </Button>
          <Button
            variant="primary"
            icon={Check}
            onClick={handleSave}
            disabled={!canSave || saving}
            style={{ flex: 2, opacity: !canSave || saving ? 0.5 : 1 }}
          >
            {saving ? "Saving…" : existing ? "Save changes" : "Add contract"}
          </Button>
        </div>
      </div>
    </>
  );
}
