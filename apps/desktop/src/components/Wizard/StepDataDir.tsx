import { useEffect, useState } from "react";
import { dataDirPath } from "../../lib/settings/ipc";
import { useWizardStore } from "../../lib/wizard/state";

export default function StepDataDir() {
  const advance = useWizardStore((s) => s.advance);
  const [dir, setDir] = useState<string>("…");

  useEffect(() => {
    void dataDirPath().then(setDir).catch(() => setDir("(unavailable)"));
  }, []);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <h2 style={{ margin: "0 0 8px 0" }}>Where your life lives</h2>
        <p style={{ fontSize: 13, color: "#aaa", lineHeight: 1.5 }}>
          Manor stores everything — tasks, events, money, chores, attachments — in one folder on this Mac.
          You can change it later, back it up to iCloud Drive, Dropbox, or anywhere you like.
        </p>
      </div>
      <div style={{
        fontFamily: "var(--mono, monospace)", fontSize: 12, color: "#888",
        background: "#141414", padding: 10, borderRadius: 4, wordBreak: "break-all",
      }}>
        {dir}
      </div>
      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button onClick={advance} style={{ padding: "8px 16px" }}>Next</button>
      </div>
    </div>
  );
}
