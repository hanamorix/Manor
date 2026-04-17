import { useState } from "react";
import { Check, SkipForward } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { settingSet } from "../../lib/foundation/ipc";
import { useWizardStore } from "../../lib/wizard/state";
import { Button } from "../../lib/ui";

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
        <h2 style={{ margin: "0 0 8px 0", fontSize: "var(--text-lg)", color: "var(--ink)" }}>
          Explore with sample data
        </h2>
        <p style={{ fontSize: "var(--text-sm)", color: "var(--ink-soft)", lineHeight: 1.5, margin: 0 }}>
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
            fontSize: "var(--text-xs)",
            color: message.includes("Failed") ? "var(--ink)" : "var(--ink)",
          }}
        >
          {message}
        </div>
      )}

      <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
        <Button variant="secondary" icon={SkipForward} onClick={() => void finish(false)} disabled={working}>
          Skip — I'll start fresh
        </Button>
        <Button variant="primary" icon={Check} onClick={() => void finish(true)} disabled={working}>
          {working ? "Setting up…" : "Add sample data"}
        </Button>
      </div>
    </div>
  );
}
