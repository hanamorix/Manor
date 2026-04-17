import { useEffect, useState } from "react";
import TrashView from "../Safety/TrashView";
import BackupPanel from "../Safety/BackupPanel";
import PanicButton from "../Safety/PanicButton";
import { dataDirPath } from "../../lib/settings/ipc";
import { settingGet, settingSet } from "../../lib/foundation/ipc";
import { TEXT_MUTED, settingsCodeBlock } from "./styles";

const AUTO_EMPTY_OPTIONS: Array<{ value: string; label: string }> = [
  { value: "7",     label: "7 days" },
  { value: "30",    label: "30 days" },
  { value: "90",    label: "90 days" },
  { value: "never", label: "Never" },
];

export default function DataBackupTab() {
  const [dataDir, setDataDir] = useState<string>("");
  const [autoEmpty, setAutoEmpty] = useState<string>("30");

  useEffect(() => {
    void dataDirPath().then(setDataDir).catch(() => setDataDir("(unavailable)"));
    void settingGet("trash.auto_empty_days").then((v) => setAutoEmpty(v ?? "30"));
  }, []);

  const onAutoEmptyChange = async (value: string) => {
    setAutoEmpty(value);
    await settingSet("trash.auto_empty_days", value);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", padding: "8px 0" }}>
      <section style={{ padding: 16 }}>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15, color: "var(--ink)" }}>
          Data directory
        </h2>
        <div style={{ ...settingsCodeBlock, fontSize: 12 }}>{dataDir || "…"}</div>
        <div style={{ fontSize: 11, color: TEXT_MUTED, marginTop: 4 }}>
          Moving the data directory requires quitting Manor and copying the files manually.
        </div>
      </section>

      <section style={{ padding: 16, borderTop: "1px solid var(--hairline)" }}>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15, color: "var(--ink)" }}>
          Trash auto-empty
        </h2>
        <div style={{ fontSize: "var(--text-sm)", color: "var(--ink)" }}>
          Permanently delete soft-deleted items older than{" "}
          <select value={autoEmpty} onChange={(e) => void onAutoEmptyChange(e.target.value)}>
            {AUTO_EMPTY_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>{o.label}</option>
            ))}
          </select>
        </div>
      </section>

      <div style={{ borderTop: "1px solid var(--hairline)" }}>
        <TrashView />
      </div>

      <div style={{ borderTop: "1px solid var(--hairline)" }}>
        <BackupPanel defaultOutDir={`${dataDir}/backups`} />
      </div>

      <div style={{ borderTop: "1px solid var(--hairline)", padding: 8 }}>
        <PanicButton />
      </div>
    </div>
  );
}
