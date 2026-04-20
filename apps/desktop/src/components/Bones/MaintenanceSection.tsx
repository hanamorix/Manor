import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { useMaintenanceStore } from "../../lib/maintenance/state";
import type { MaintenanceSchedule } from "../../lib/maintenance/ipc";
import { ScheduleRow } from "./DueSoon/ScheduleRow";
import { ScheduleDrawer } from "./DueSoon/ScheduleDrawer";
import { LogCompletionDrawer } from "./LogCompletionDrawer";

interface Props { assetId: string }

export function MaintenanceSection({ assetId }: Props) {
  const { schedulesByAsset, loadForAsset, markDone, deleteSchedule } = useMaintenanceStore();
  const [editing, setEditing] = useState<MaintenanceSchedule | null>(null);
  const [adding, setAdding] = useState(false);
  const [logDrawerOpen, setLogDrawerOpen] = useState(false);
  const [logActiveSched, setLogActiveSched] = useState<MaintenanceSchedule | null>(null);

  useEffect(() => { void loadForAsset(assetId); }, [assetId, loadForAsset]);

  const schedules = schedulesByAsset[assetId] ?? [];

  return (
    <div>
      {schedules.length === 0 && (
        <p style={{ color: "var(--ink-soft, #999)", fontStyle: "italic" }}>
          No maintenance schedules yet.
        </p>
      )}
      {schedules.length > 0 && (
        <div style={{ border: "1px solid var(--hairline, #e5e5e5)", borderRadius: 6, overflow: "hidden" }}>
          {schedules.map((s) => (
            <ScheduleRow
              key={s.id}
              schedule={s}
              onMarkDone={() => void markDone(s.id)}
              onLogCompletion={() => { setLogActiveSched(s); setLogDrawerOpen(true); }}
              onEdit={() => setEditing(s)}
              onDelete={() => void deleteSchedule(s.id)}
            />
          ))}
        </div>
      )}
      <button type="button" onClick={() => setAdding(true)}
        style={{ marginTop: 12, display: "flex", alignItems: "center", gap: 4 }}>
        <Plus size={14} strokeWidth={1.8} /> Add schedule
      </button>

      {adding && (
        <ScheduleDrawer
          initialAssetId={assetId}
          lockAsset
          onClose={() => setAdding(false)}
          onSaved={() => { setAdding(false); void loadForAsset(assetId); }}
        />
      )}
      {editing && (
        <ScheduleDrawer
          schedule={editing}
          initialAssetId={assetId}
          lockAsset
          onClose={() => setEditing(null)}
          onSaved={() => { setEditing(null); void loadForAsset(assetId); }}
        />
      )}
      {logActiveSched && (
        <LogCompletionDrawer
          open={logDrawerOpen}
          mode={{
            kind: "schedule_completion",
            assetId: logActiveSched.asset_id,
            scheduleId: logActiveSched.id,
            taskName: logActiveSched.task,
          }}
          onClose={() => { setLogDrawerOpen(false); setLogActiveSched(null); }}
        />
      )}
    </div>
  );
}
