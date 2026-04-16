import { useEffect, useState } from "react";
import { ollamaStatus, type OllamaStatus } from "../../lib/settings/ipc";
import { useWizardStore } from "../../lib/wizard/state";

export default function StepOllama() {
  const advance = useWizardStore((s) => s.advance);
  const [status, setStatus] = useState<OllamaStatus | null>(null);
  const [checking, setChecking] = useState(true);

  const probe = async () => {
    setChecking(true);
    const s = await ollamaStatus().catch(() =>
      ({ reachable: false, models: [] } as OllamaStatus));
    setStatus(s);
    setChecking(false);
  };

  useEffect(() => { void probe(); }, []);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <h2 style={{ margin: "0 0 8px 0" }}>Meet your brain</h2>
        <p style={{ fontSize: 13, color: "#aaa", lineHeight: 1.5 }}>
          Manor runs a language model locally via <strong>Ollama</strong> — for chat,
          calendar summaries, ledger narratives, and semantic search. Nothing leaves
          your Mac unless you opt in to a remote provider later.
        </p>
      </div>

      {checking && <div style={{ fontSize: 13, color: "#888" }}>Checking for Ollama…</div>}

      {!checking && status && (
        status.reachable ? (
          <div style={{
            padding: 12, border: "1px solid #244",
            background: "#0f1f1f", borderRadius: 6,
          }}>
            <div style={{ color: "#6f6" }}>● Ollama is reachable</div>
            <div style={{ fontSize: 12, color: "#888", marginTop: 4 }}>
              {status.models.length === 0
                ? "No models installed — pull one via `ollama pull qwen2.5:7b-instruct` after setup."
                : `${status.models.length} model(s) ready: ${status.models.slice(0, 3).join(", ")}${status.models.length > 3 ? "…" : ""}`}
            </div>
          </div>
        ) : (
          <div style={{
            padding: 12, border: "1px solid #442",
            background: "#1a1410", borderRadius: 6,
          }}>
            <div style={{ color: "#d90" }}>● Ollama not running</div>
            <div style={{ fontSize: 12, color: "#888", marginTop: 4, lineHeight: 1.5 }}>
              Most features work without it. You can install and start Ollama any time:{" "}
              <a href="https://ollama.com/download" target="_blank" rel="noreferrer">
                ollama.com/download
              </a>
              . Then pull a model:
              <code style={{ display: "block", padding: 6, background: "#111", borderRadius: 4, marginTop: 6, fontSize: 11 }}>
                ollama pull qwen2.5:7b-instruct
              </code>
            </div>
          </div>
        )
      )}

      <div style={{ display: "flex", justifyContent: "space-between" }}>
        <button onClick={probe} disabled={checking} style={{ padding: "8px 12px" }}>
          Re-check
        </button>
        <button onClick={advance} style={{ padding: "8px 16px" }}>Next</button>
      </div>
    </div>
  );
}
