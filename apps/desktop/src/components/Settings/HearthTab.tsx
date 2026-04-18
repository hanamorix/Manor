import { useEffect, useState } from "react";
import { useHearthViewStore } from "../../lib/hearth/view-state";
import { settingGet, settingSet } from "../../lib/foundation/ipc";

export default function HearthTab() {
  const { setSubview } = useHearthViewStore();
  const [showBand, setShowBand] = useState(true);
  const [dinnerTime, setDinnerTime] = useState("19:00");
  const [message, setMessage] = useState("");

  useEffect(() => {
    void settingGet("hearth.show_tonight_band").then((v) => setShowBand(v !== "false")).catch(() => {});
    void settingGet("hearth.dinner_time").then((v) => { if (v) setDinnerTime(v); }).catch(() => {});
  }, []);

  const toggleBand = async () => {
    const next = !showBand;
    setShowBand(next);
    await settingSet("hearth.show_tonight_band", next ? "true" : "false");
    setMessage("Saved.");
  };

  const saveDinnerTime = async (v: string) => {
    setDinnerTime(v);
    if (/^\d{2}:\d{2}$/.test(v)) {
      await settingSet("hearth.dinner_time", v);
      setMessage("Saved.");
    }
  };

  return (
    <div style={{ padding: 24 }}>
      <h2 style={{ fontSize: 18, marginTop: 0 }}>Hearth</h2>

      <div style={{ marginBottom: 20 }}>
        <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <input type="checkbox" checked={showBand} onChange={toggleBand} />
          Show tonight's meal on Today
        </label>
      </div>

      <div style={{ marginBottom: 20 }}>
        <label style={{ display: "block", fontSize: 13, marginBottom: 4 }}>Dinner time</label>
        <input
          type="time"
          value={dinnerTime}
          onChange={(e) => void saveDinnerTime(e.target.value)}
          style={{ fontSize: 14, padding: 4 }}
        />
        <div style={{ fontSize: 12, color: "var(--ink-soft, #999)", marginTop: 4 }}>
          Used when your planned meal appears on the TimeBlocks view.
        </div>
      </div>

      <div style={{ marginBottom: 20 }}>
        <button type="button" onClick={() => setSubview("staples")}>
          Manage staples →
        </button>
        <div style={{ fontSize: 12, color: "var(--ink-soft, #999)", marginTop: 4 }}>
          Items your shopping list skips by default.
        </div>
      </div>

      {message && <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>{message}</div>}
    </div>
  );
}
