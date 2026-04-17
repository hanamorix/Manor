import { useEffect, useState } from "react";
import { ollamaStatus, type OllamaStatus, embeddingsStatus, embeddingsRebuild, type EmbeddingsStatus } from "../../lib/settings/ipc";
import { settingGet, settingSet } from "../../lib/foundation/ipc";

const DEFAULT_MODEL_KEY = "ai.default_model";

export default function AiTab() {
  const [status, setStatus] = useState<OllamaStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<string>("");

  useEffect(() => {
    void (async () => {
      setLoading(true);
      const s = await ollamaStatus().catch(() => ({ reachable: false, models: [] } as OllamaStatus));
      setStatus(s);
      const stored = await settingGet(DEFAULT_MODEL_KEY);
      setSelected(stored ?? "qwen2.5:7b-instruct");
      setLoading(false);
    })();
  }, []);

  const onModelChange = async (v: string) => {
    setSelected(v);
    await settingSet(DEFAULT_MODEL_KEY, v);
  };

  return (
    <div style={{ padding: 16, display: "flex", flexDirection: "column", gap: 16 }}>
      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Local brain (Ollama)</h2>
        {loading && <div style={{ fontSize: 13 }}>Checking…</div>}
        {!loading && status && (
          status.reachable ? (
            <>
              <div style={{ fontSize: 13 }}>
                <span style={{ color: "#6f6" }}>● Ready</span>{" "}
                <span style={{ color: "#888" }}>— {status.models.length} model(s) installed</span>
              </div>
              <label style={{ display: "block", marginTop: 8, fontSize: 13 }}>
                Default model:
                <select
                  value={selected}
                  onChange={(e) => void onModelChange(e.target.value)}
                  style={{ marginLeft: 8 }}
                >
                  {status.models.length === 0 && <option value="">(no models)</option>}
                  {status.models.map((m) => <option key={m} value={m}>{m}</option>)}
                </select>
              </label>
            </>
          ) : (
            <div>
              <div style={{ fontSize: 13, color: "#f66" }}>● Unreachable</div>
              <div style={{ fontSize: 12, color: "#888", marginTop: 4 }}>
                Start Ollama to enable chat, CalDAV AI review, and embeddings.{" "}
                <a href="https://ollama.com/download" target="_blank" rel="noreferrer">Install Ollama →</a>
              </div>
            </div>
          )
        )}
      </section>

      <EmbeddingsSection />

      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Remote providers</h2>
        <div style={{ fontSize: 12, color: "#888" }}>
          Bring-your-own-key providers (Claude, OpenAI, Gemini) will appear here in a future release.
        </div>
      </section>
    </div>
  );
}

function EmbeddingsSection() {
  const [status, setStatus] = useState<EmbeddingsStatus | null>(null);
  const [rebuilding, setRebuilding] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setStatus(await embeddingsStatus());
    } catch {
      setStatus(null);
    }
  };

  useEffect(() => { void refresh(); }, []);

  const rebuild = async () => {
    if (!confirm("Rebuild will clear the embedding index and re-embed everything. Continue?")) return;
    setRebuilding(true);
    setMessage(null);
    try {
      const cleared = await embeddingsRebuild();
      setMessage(`Cleared ${cleared} vector(s). Re-indexing started — leave Manor open until it finishes.`);
      await refresh();
    } catch (e) {
      setMessage(`Rebuild failed: ${e}`);
    } finally {
      setRebuilding(false);
    }
  };

  return (
    <section>
      <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Embeddings</h2>
      {!status && <div style={{ fontSize: 12, color: "#888" }}>Loading…</div>}
      {status && (
        <>
          <div style={{ fontSize: 13 }}>
            <span style={{ color: "#888" }}>Model:</span> {status.model}
          </div>
          <div style={{ fontSize: 13, marginTop: 4 }}>
            <span style={{ color: "#888" }}>Indexed:</span> {status.total} vector(s)
            {status.by_entity_type.length > 0 && " ("}
            {status.by_entity_type.map(([t, n], i) => (
              <span key={t}>
                {i > 0 && ", "}{t}: {n}
              </span>
            ))}
            {status.by_entity_type.length > 0 && ")"}
          </div>
          <button
            onClick={rebuild}
            disabled={rebuilding}
            style={{ marginTop: 8, fontSize: 12 }}
          >
            {rebuilding ? "Rebuilding…" : "Rebuild embeddings"}
          </button>
          {message && (
            <div style={{ fontSize: 11, color: message.includes("failed") ? "#f66" : "#6f6", marginTop: 6 }}>
              {message}
            </div>
          )}
        </>
      )}
    </section>
  );
}
