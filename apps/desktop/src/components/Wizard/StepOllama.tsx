import { useEffect, useState } from "react";
import { ollamaStatus, type OllamaStatus } from "../../lib/settings/ipc";
import { useWizardStore } from "../../lib/wizard/state";
import {
  wizardCodeBlock,
  wizardPrimaryButton,
  wizardSecondaryButton,
  wizardStatusCardGood,
  wizardStatusCardWarn,
} from "./styles";

export default function StepOllama() {
  const advance = useWizardStore((s) => s.advance);
  const [status, setStatus] = useState<OllamaStatus | null>(null);
  const [checking, setChecking] = useState(true);

  const probe = async () => {
    setChecking(true);
    const s = await ollamaStatus().catch(
      () => ({ reachable: false, models: [] }) as OllamaStatus,
    );
    setStatus(s);
    setChecking(false);
  };

  useEffect(() => {
    void probe();
  }, []);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 16, color: "var(--ink)" }}>
          Meet your brain
        </h2>
        <p style={{ fontSize: 13, color: "rgba(0,0,0,0.65)", lineHeight: 1.5, margin: 0 }}>
          Manor runs a language model locally via <strong>Ollama</strong> — for chat,
          calendar summaries, ledger narratives, and semantic search. Nothing leaves
          your Mac unless you opt in to a remote provider later.
        </p>
      </div>

      {checking && (
        <div style={{ fontSize: 13, color: "rgba(0,0,0,0.55)" }}>Checking for Ollama…</div>
      )}

      {!checking &&
        status &&
        (status.reachable ? (
          <div style={wizardStatusCardGood}>
            <div style={{ color: "var(--imessage-green)", fontWeight: 600, fontSize: 13 }}>
              ● Ollama is reachable
            </div>
            <div style={{ fontSize: 12, color: "rgba(0,0,0,0.65)", marginTop: 4 }}>
              {status.models.length === 0
                ? "No models installed — pull one via "
                : `${status.models.length} model(s) ready: ${status.models.slice(0, 3).join(", ")}${status.models.length > 3 ? "…" : ""}`}
              {status.models.length === 0 && (
                <code
                  style={{
                    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
                    fontSize: 11,
                    background: "var(--paper-muted)",
                    padding: "1px 4px",
                    borderRadius: 3,
                  }}
                >
                  ollama pull qwen2.5:7b-instruct
                </code>
              )}
              {status.models.length === 0 && " after setup."}
            </div>
          </div>
        ) : (
          <div style={wizardStatusCardWarn}>
            <div style={{ color: "#b36b00", fontWeight: 600, fontSize: 13 }}>
              ● Ollama not running
            </div>
            <div
              style={{
                fontSize: 12,
                color: "rgba(0,0,0,0.65)",
                marginTop: 6,
                lineHeight: 1.5,
              }}
            >
              Most features work without it. You can install and start Ollama any time at{" "}
              <a
                href="https://ollama.com/download"
                target="_blank"
                rel="noreferrer"
                style={{ color: "var(--imessage-blue)" }}
              >
                ollama.com/download
              </a>
              . Then pull a model:
              <code style={wizardCodeBlock}>ollama pull qwen2.5:7b-instruct</code>
            </div>
          </div>
        ))}

      <div style={{ display: "flex", justifyContent: "space-between" }}>
        <button onClick={probe} disabled={checking} style={wizardSecondaryButton}>
          Re-check
        </button>
        <button onClick={advance} style={wizardPrimaryButton}>
          Next
        </button>
      </div>
    </div>
  );
}
