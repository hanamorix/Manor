import { useEffect, useState } from "react";
import { useMaintenanceEventsStore } from "../../lib/maintenance/event-state";
import type {
  MaintenanceEvent,
  MaintenanceEventDraft,
} from "../../lib/maintenance/event-ipc";
import { TransactionSuggest } from "./TransactionSuggest";

type Mode =
  | { kind: "one_off"; assetId: string }
  | { kind: "schedule_completion"; assetId: string; scheduleId: string; taskName: string }
  | { kind: "edit"; event: MaintenanceEvent };

interface Props {
  open: boolean;
  mode: Mode;
  onClose(): void;
}

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

export function LogCompletionDrawer({ open, mode, onClose }: Props) {
  const { createOneOff, logCompletion, update } = useMaintenanceEventsStore();
  const [title, setTitle] = useState("");
  const [completedDate, setCompletedDate] = useState(todayIso());
  const [costText, setCostText] = useState("");
  const [notes, setNotes] = useState("");
  const [txId, setTxId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setError(null);
    setSaving(false);
    if (mode.kind === "one_off") {
      setTitle("");
      setCompletedDate(todayIso());
      setCostText("");
      setNotes("");
      setTxId(null);
    } else if (mode.kind === "schedule_completion") {
      setTitle(mode.taskName);
      setCompletedDate(todayIso());
      setCostText("");
      setNotes("");
      setTxId(null);
    } else {
      const e = mode.event;
      setTitle(e.title);
      setCompletedDate(e.completed_date);
      setCostText(e.cost_pence == null ? "" : (e.cost_pence / 100).toFixed(2));
      setNotes(e.notes);
      setTxId(e.transaction_id);
    }
  }, [open, mode]);

  if (!open) return null;

  // Parse costText → pence (NaN means invalid, null means empty/optional).
  const costPence: number | null | typeof NaN = (() => {
    if (costText.trim() === "") return null;
    const n = Number(costText);
    if (isNaN(n) || n < 0) return NaN;
    return Math.round(n * 100);
  })();
  const costIsNaN = Number.isNaN(costPence);
  const costNumericOrNull: number | null = costIsNaN ? null : (costPence as number | null);

  const assetId = (() => {
    switch (mode.kind) {
      case "edit":
        return mode.event.asset_id;
      case "schedule_completion":
      case "one_off":
        return mode.assetId;
    }
  })();

  const scheduleIdForDraft = (() => {
    switch (mode.kind) {
      case "edit":
        return mode.event.schedule_id;
      case "schedule_completion":
        return mode.scheduleId;
      case "one_off":
        return null;
    }
  })();

  const excludeEventId = mode.kind === "edit" ? mode.event.id : null;

  const buildDraft = (): MaintenanceEventDraft => ({
    asset_id: assetId,
    schedule_id: scheduleIdForDraft,
    title,
    completed_date: completedDate,
    cost_pence: costNumericOrNull,
    currency: "GBP",
    notes,
    transaction_id: txId,
  });

  const onSave = async () => {
    setError(null);
    if (title.trim() === "") {
      setError("Title is required.");
      return;
    }
    if (costIsNaN) {
      setError("Cost must be a positive number.");
      return;
    }
    setSaving(true);
    try {
      const draft = buildDraft();
      if (mode.kind === "one_off") {
        await createOneOff(draft);
      } else if (mode.kind === "schedule_completion") {
        await logCompletion(mode.scheduleId, draft);
      } else {
        await update(mode.event.id, draft);
      }
      onClose();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="drawer-overlay"
      style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.3)", zIndex: 100 }}
    >
      <div
        role="dialog"
        aria-label="Log completion"
        style={{
          position: "absolute",
          right: 0,
          top: 0,
          bottom: 0,
          width: 480,
          background: "var(--surface, #fff)",
          padding: 24,
          overflow: "auto",
          boxShadow: "-4px 0 12px rgba(0,0,0,0.1)",
        }}
      >
        <h2 style={{ marginTop: 0 }}>
          {mode.kind === "edit" ? "Edit completion" : "Log completion"}
        </h2>

        <label style={{ display: "block", marginBottom: 12 }}>
          Title
          <input
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            style={{ display: "block", width: "100%", padding: 6, marginTop: 4 }}
          />
        </label>

        <label style={{ display: "block", marginBottom: 12 }}>
          Completed date
          <input
            type="date"
            value={completedDate}
            onChange={(e) => setCompletedDate(e.target.value)}
            style={{ display: "block", padding: 6, marginTop: 4 }}
          />
        </label>

        <label style={{ display: "block", marginBottom: 12 }}>
          Cost (£)
          <input
            type="number"
            step="0.01"
            min="0"
            value={costText}
            onChange={(e) => setCostText(e.target.value)}
            placeholder="0.00"
            style={{ display: "block", padding: 6, marginTop: 4, width: 160 }}
          />
        </label>

        <div style={{ marginBottom: 12 }}>
          <div style={{ fontSize: 13, marginBottom: 6 }}>Link transaction</div>
          <TransactionSuggest
            completedDate={completedDate}
            costPence={costNumericOrNull}
            selectedTransactionId={txId}
            excludeEventId={excludeEventId}
            onSelect={(id) => setTxId(id)}
          />
        </div>

        <label style={{ display: "block", marginBottom: 12 }}>
          Notes
          <textarea
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            rows={4}
            style={{ display: "block", width: "100%", padding: 6, marginTop: 4 }}
          />
        </label>

        {error && (
          <div style={{ color: "var(--danger, #c43)", marginBottom: 8 }}>{error}</div>
        )}

        <div style={{ display: "flex", gap: 8 }}>
          <button type="button" onClick={onSave} disabled={saving}>
            {saving ? "Saving…" : "Save"}
          </button>
          <button type="button" onClick={onClose} disabled={saving}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
