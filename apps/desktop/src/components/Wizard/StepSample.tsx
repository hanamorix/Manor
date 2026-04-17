import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { settingSet } from "../../lib/foundation/ipc";
import { useWizardStore } from "../../lib/wizard/state";
import { wizardPrimaryButton, wizardSecondaryButton } from "./styles";

async function seedSampleData(): Promise<void> {
  // Create "sample" tag (idempotent — returns existing if already present).
  const tag = await invoke<{ id: number }>("tag_upsert", {
    name: "sample", color: "#888",
  });

  // Seed one task and tag it.
  const task = await invoke<{ id: number }>("add_task", {
    args: { title: "Try Manor — click me to mark done", dueDate: null, notes: null },
  }).catch(() => null);
  if (task) {
    await invoke<void>("tag_link", {
      tagId: tag.id, entityType: "task", entityId: task.id,
    }).catch(() => {});
  }

  // Seed one chore and tag it.
  const nowSec = Math.floor(Date.now() / 1000);
  const chore = await invoke<{ id: number }>("create_chore", {
    args: {
      title: "Sample chore — water the plants",
      emoji: "🪴",
      rrule: "FREQ=WEEKLY",
      nextDue: nowSec,
      rotation: "none",
    },
  }).catch(() => null);
  if (chore) {
    await invoke<void>("tag_link", {
      tagId: tag.id, entityType: "chore", entityId: chore.id,
    }).catch(() => {});
  }
}

export default function StepSample() {
  const setShow = useWizardStore((s) => s.setShow);
  const [working, setWorking] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const finish = async (seed: boolean) => {
    setWorking(true);
    try {
      if (seed) {
        await seedSampleData();
        setMessage("Sample task + chore added. You can delete them any time.");
      }
      await settingSet("onboarding_completed", "1");
      // Close wizard.
      setTimeout(() => setShow(false), 500);
    } catch (e) {
      setMessage(`Failed to finish setup: ${e}`);
      setWorking(false);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 16, color: "var(--ink)" }}>
          Explore with sample data
        </h2>
        <p style={{ fontSize: 13, color: "rgba(0,0,0,0.65)", lineHeight: 1.5, margin: 0 }}>
          Want a sample task and chore so you can see how Manor feels? They'll be
          tagged{" "}
          <code
            style={{
              fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
              fontSize: 11,
              background: "var(--paper-muted)",
              padding: "1px 4px",
              borderRadius: 3,
            }}
          >
            sample
          </code>{" "}
          — a banner on Today lets you delete them whenever.
        </p>
      </div>

      {message && (
        <div
          style={{
            fontSize: 12,
            color: message.includes("Failed") ? "var(--imessage-red)" : "var(--imessage-green)",
          }}
        >
          {message}
        </div>
      )}

      <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
        <button onClick={() => void finish(false)} disabled={working} style={wizardSecondaryButton}>
          Skip — I'll start fresh
        </button>
        <button onClick={() => void finish(true)} disabled={working} style={wizardPrimaryButton}>
          {working ? "Setting up…" : "Add sample data"}
        </button>
      </div>
    </div>
  );
}
