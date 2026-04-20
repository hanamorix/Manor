import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { useMaintenanceEventsStore } from "../../lib/maintenance/event-state";
import type { MaintenanceEvent } from "../../lib/maintenance/event-ipc";
import { EventRow } from "./EventRow";
import { LogCompletionDrawer } from "./LogCompletionDrawer";

interface Props {
  assetId: string;
}

type DrawerMode =
  | { kind: "one_off"; assetId: string }
  | { kind: "edit"; event: MaintenanceEvent };

export function HistoryBlock({ assetId }: Props) {
  const { eventsByAsset, loadForAsset } = useMaintenanceEventsStore();
  const rows = eventsByAsset[assetId] ?? [];

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [drawerMode, setDrawerMode] = useState<DrawerMode | null>(null);

  useEffect(() => {
    if (!eventsByAsset[assetId]) {
      void loadForAsset(assetId);
    }
  }, [assetId, eventsByAsset, loadForAsset]);

  const openOneOff = () => {
    setDrawerMode({ kind: "one_off", assetId });
    setDrawerOpen(true);
  };
  const openEdit = (event: MaintenanceEvent) => {
    setDrawerMode({ kind: "edit", event });
    setDrawerOpen(true);
  };
  const onDrawerClose = () => {
    setDrawerOpen(false);
    // Re-fetch after mutation (store action already invalidated cache).
    void loadForAsset(assetId);
  };

  return (
    <section style={{ marginTop: 24 }}>
      <header
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          marginBottom: 12,
        }}
      >
        <h3 style={{ margin: 0, flex: 1 }}>History</h3>
        <button type="button" onClick={openOneOff}>
          <Plus size={14} strokeWidth={1.6} /> Log work
        </button>
      </header>

      {rows.length === 0 ? (
        <div style={{ color: "var(--ink-soft, #888)" }}>
          No completions logged yet.
        </div>
      ) : (
        <div>
          {rows.map((r) => (
            <EventRow
              key={r.event.id}
              row={r}
              onEdit={() => openEdit(r.event)}
            />
          ))}
        </div>
      )}

      {drawerMode && (
        <LogCompletionDrawer
          open={drawerOpen}
          mode={drawerMode}
          onClose={onDrawerClose}
        />
      )}
    </section>
  );
}
