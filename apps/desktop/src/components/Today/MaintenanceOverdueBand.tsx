import { useEffect, useState } from "react";
import { Wrench } from "lucide-react";
import { useMaintenanceStore } from "../../lib/maintenance/state";
import { useBonesViewStore } from "../../lib/bones/view-state";
import { useNavStore } from "../../lib/nav";
import { settingGet } from "../../lib/foundation/ipc";

export function MaintenanceOverdueBand() {
  const { overdueCount, loadOverdueCount } = useMaintenanceStore();
  const { setSubview } = useBonesViewStore();
  const { setView } = useNavStore();
  const [visible, setVisible] = useState<boolean>(true);

  useEffect(() => { void loadOverdueCount(); }, [loadOverdueCount]);
  useEffect(() => {
    void settingGet("bones.show_maintenance_band").then((v) => setVisible(v !== "false")).catch(() => {});
  }, []);

  if (!visible) return null;
  if (overdueCount === 0) return null;

  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 12,
      height: 56, padding: "0 16px",
      background: "var(--paper, #fff)",
      border: "1px solid var(--hairline, #e5e5e5)",
      borderRadius: 6,
    }}>
      <Wrench size={18} strokeWidth={1.6} color="var(--ink-soft, #999)" />
      <span style={{ flex: 1 }}>
        {overdueCount} maintenance item{overdueCount === 1 ? "" : "s"} overdue
      </span>
      <button type="button" onClick={() => {
        setSubview("due_soon");
        setView("bones");
      }}>View →</button>
    </div>
  );
}
