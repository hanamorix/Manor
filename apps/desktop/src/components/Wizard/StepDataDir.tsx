import { useEffect, useState } from "react";
import { dataDirPath } from "../../lib/settings/ipc";
import { useWizardStore } from "../../lib/wizard/state";
import { wizardPrimaryButton } from "./styles";

export default function StepDataDir() {
  const advance = useWizardStore((s) => s.advance);
  const [dir, setDir] = useState<string>("…");

  useEffect(() => {
    void dataDirPath().then(setDir).catch(() => setDir("(unavailable)"));
  }, []);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 16, color: "var(--ink)" }}>
          Where your life lives
        </h2>
        <p style={{ fontSize: 13, color: "var(--ink-soft)", lineHeight: 1.5, margin: 0 }}>
          Manor stores everything — tasks, events, money, chores, attachments — in one
          folder on this Mac. You can change it later, back it up to iCloud Drive, Dropbox,
          or anywhere you like.
        </p>
      </div>
      <div
        style={{
          fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
          fontSize: 12,
          color: "var(--ink)",
          background: "var(--paper-muted)",
          border: "1px solid var(--hairline)",
          padding: 10,
          borderRadius: "var(--radius-sm)",
          wordBreak: "break-all",
        }}
      >
        {dir}
      </div>
      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button onClick={advance} style={wizardPrimaryButton}>
          Next
        </button>
      </div>
    </div>
  );
}
