import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import { useMaintenanceStore } from "../../../lib/maintenance/state";
import { useAssetStore } from "../../../lib/asset/state";
import type {
  MaintenanceSchedule, MaintenanceScheduleDraft,
} from "../../../lib/maintenance/ipc";

interface Props {
  schedule?: MaintenanceSchedule;         // undefined = create mode
  initialAssetId?: string;
  lockAsset?: boolean;
  onClose: () => void;
  onSaved: () => void;
  onDeleted?: () => void;
}

const EMPTY_DRAFT: MaintenanceScheduleDraft = {
  asset_id: "",
  task: "",
  interval_months: 12,
  last_done_date: null,
  notes: "",
};

export function ScheduleDrawer({
  schedule, initialAssetId, lockAsset, onClose, onSaved, onDeleted,
}: Props) {
  const { create, update, deleteSchedule } = useMaintenanceStore();
  const { assets, load: loadAssets } = useAssetStore();

  const [draft, setDraft] = useState<MaintenanceScheduleDraft>(() => {
    if (schedule) {
      return {
        asset_id: schedule.asset_id,
        task: schedule.task,
        interval_months: schedule.interval_months,
        last_done_date: schedule.last_done_date,
        notes: schedule.notes,
      };
    }
    return { ...EMPTY_DRAFT, asset_id: initialAssetId ?? "" };
  });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => { void loadAssets(); }, [loadAssets]);

  const save = async () => {
    if (!draft.asset_id) { setError("Pick an asset"); return; }
    if (!draft.task.trim()) { setError("Task required"); return; }
    if (draft.interval_months < 1) { setError("Interval must be at least 1 month"); return; }
    setSaving(true); setError(null);
    try {
      if (schedule) await update(schedule.id, draft);
      else await create(draft);
      onSaved();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const del = async () => {
    if (!schedule) return;
    if (!window.confirm("Move this schedule to Trash?")) return;
    try {
      await deleteSchedule(schedule.id);
      onDeleted?.();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, width: 480,
      background: "var(--paper, #fff)", borderLeft: "1px solid var(--hairline, #e5e5e5)",
      padding: 24, overflow: "auto", zIndex: 50,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 20 }}>
          {schedule ? "Edit schedule" : "New schedule"}
        </h2>
        <button type="button" onClick={onClose} aria-label="Close">✕</button>
      </div>

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Asset</label>
      <select
        value={draft.asset_id}
        onChange={(e) => setDraft({ ...draft, asset_id: e.target.value })}
        disabled={lockAsset}
        style={{ width: "100%", marginBottom: 12, padding: 6 }}
      >
        <option value="">— Pick one —</option>
        {assets.map((a) => (
          <option key={a.id} value={a.id}>{a.name}</option>
        ))}
      </select>

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Task</label>
      <input
        value={draft.task}
        onChange={(e) => setDraft({ ...draft, task: e.target.value })}
        placeholder="e.g. Annual boiler service"
        style={{ width: "100%", marginBottom: 12, padding: 6 }}
      />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Interval (months)</label>
      <input
        type="number" min={1}
        value={draft.interval_months}
        onChange={(e) => setDraft({ ...draft, interval_months: parseInt(e.target.value) || 1 })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }}
      />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>
        Last done (optional)
      </label>
      <input
        type="date"
        value={draft.last_done_date ?? ""}
        onChange={(e) => setDraft({ ...draft, last_done_date: e.target.value || null })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }}
      />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Notes (markdown)</label>
      <textarea
        value={draft.notes}
        onChange={(e) => setDraft({ ...draft, notes: e.target.value })}
        rows={5} style={{ width: "100%", fontFamily: "inherit", padding: 6 }}
      />

      {error && <div style={{ color: "var(--ink-danger, #b00020)", marginTop: 8 }}>{error}</div>}

      <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
        <button type="button" onClick={onClose}>Cancel</button>
        <button type="button" onClick={save} disabled={saving}>
          {saving ? "Saving…" : "Save"}
        </button>
        {schedule && (
          <button type="button" onClick={del}
            style={{ marginLeft: "auto", display: "flex", alignItems: "center", gap: 4 }}>
            <Trash2 size={14} strokeWidth={1.8} /> Delete
          </button>
        )}
      </div>
    </div>
  );
}
