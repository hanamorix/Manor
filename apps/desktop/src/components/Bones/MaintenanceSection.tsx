import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { useMaintenanceStore } from "../../lib/maintenance/state";
import type { MaintenanceSchedule } from "../../lib/maintenance/ipc";
import { ScheduleRow } from "./DueSoon/ScheduleRow";
import { ScheduleDrawer } from "./DueSoon/ScheduleDrawer";

interface Props { assetId: string }

export function MaintenanceSection({ assetId }: Props) {
  const { schedulesByAsset, loadForAsset, markDone, deleteSchedule } = useMaintenanceStore();
  const [editing, setEditing] = useState<MaintenanceSchedule | null>(null);
  const [adding, setAdding] = useState(false);

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
    </div>
  );
}
