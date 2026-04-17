import { useEffect, useState } from "react";
import { ArrowRight } from "lucide-react";
import { dataDirPath } from "../../lib/settings/ipc";
import { useWizardStore } from "../../lib/wizard/state";
import { Button } from "../../lib/ui";

export default function StepDataDir() {
  const advance = useWizardStore((s) => s.advance);
  const [dir, setDir] = useState<string>("…");

  useEffect(() => {
    void dataDirPath().then(setDir).catch(() => setDir("(unavailable)"));
  }, []);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <h2 style={{ margin: "0 0 8px 0", fontSize: "var(--text-lg)", color: "var(--ink)" }}>
          Where your life lives
        </h2>
        <p style={{ fontSize: "var(--text-sm)", color: "var(--ink-soft)", lineHeight: 1.5, margin: 0 }}>
          Manor stores everything — tasks, events, money, chores, attachments — in one
          folder on this Mac. You can change it later, back it up to iCloud Drive, Dropbox,
          or anywhere you like.
        </p>
      </div>
      <div
        style={{
          fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
          fontSize: "var(--text-xs)",
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
        <Button variant="primary" icon={ArrowRight} onClick={advance}>
          Next
        </Button>
      </div>
    </div>
  );
}
