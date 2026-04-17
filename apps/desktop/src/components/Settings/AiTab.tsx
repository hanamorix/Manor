import { useEffect, useState } from "react";
import { ollamaStatus, type OllamaStatus, embeddingsStatus, embeddingsRebuild, type EmbeddingsStatus } from "../../lib/settings/ipc";
import { settingGet, settingSet } from "../../lib/foundation/ipc";
import {
  remoteProviderStatus, remoteSetKey, remoteRemoveKey,
  remoteSetBudget, remoteSetEnabledForReview, remoteTest,
  type RemoteProviderStatus,
} from "../../lib/remote/ipc";

const DEFAULT_MODEL_KEY = "ai.default_model";

function RemoteProvidersSection() {
  const [status, setStatus] = useState<RemoteProviderStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [newKey, setNewKey] = useState("");
  const [budgetInput, setBudgetInput] = useState("");
  const [testing, setTesting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = async () => {
    setLoading(true);
    try {
      const s = await remoteProviderStatus();
      setStatus(s);
      setBudgetInput((s.budget_pence / 100).toFixed(2));
    } catch { setStatus(null); }
    setLoading(false);
  };

  useEffect(() => { void refresh(); }, []);

  const saveKey = async () => {
    if (!newKey.trim()) return;
    setMessage(null);
    try {
      await remoteSetKey("claude", newKey.trim());
      setNewKey("");
      setMessage("Key stored in macOS Keychain.");
      await refresh();
    } catch (e) { setMessage(`Failed: ${e}`); }
  };

  const removeKey = async () => {
    if (!confirm("Remove the Claude API key from Keychain?")) return;
    await remoteRemoveKey("claude");
    setMessage("Key removed.");
    await refresh();
  };

  const saveBudget = async () => {
    const pence = Math.round(parseFloat(budgetInput) * 100);
    if (isNaN(pence) || pence < 0) { setMessage("Budget must be a non-negative number."); return; }
    try {
      await remoteSetBudget("claude", pence);
      setMessage("Budget saved.");
      await refresh();
    } catch (e) { setMessage(`Failed: ${e}`); }
  };

  const toggleEnabled = async (next: boolean) => {
    try {
      await remoteSetEnabledForReview(next);
      await refresh();
    } catch (e) { setMessage(`Failed: ${e}`); }
  };

  const test = async () => {
    setTesting(true);
    setMessage(null);
    try {
      const text = await remoteTest();
      setMessage(`Test call succeeded: "${text}"`);
      await refresh();
    } catch (e) { setMessage(`Test failed: ${e}`); }
    setTesting(false);
  };

  if (loading) return <section><div style={{ fontSize: 13, color: "#888" }}>Loading remote providers…</div></section>;
  if (!status) return <section><div style={{ fontSize: 13, color: "#f66" }}>Failed to load remote status.</div></section>;

  const pct = status.budget_pence > 0
    ? Math.min(100, (status.spent_month_pence / status.budget_pence) * 100)
    : 0;
  const barColor = pct >= 100 ? "#c33" : pct >= 75 ? "#d90" : "#6a6";

  return (
    <section>
      <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Remote providers</h2>

      <div style={{ padding: 10, border: "1px solid #333", borderRadius: 6, marginBottom: 10 }}>
        <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 4 }}>Claude</div>
        {status.has_key ? (
          <div style={{ fontSize: 12, color: "#6f6" }}>
            ● API key set in Keychain
            <button onClick={removeKey} style={{ marginLeft: 8, fontSize: 11 }}>Remove</button>
          </div>
        ) : (
          <div style={{ display: "flex", gap: 6, marginTop: 4 }}>
            <input
              type="password"
              value={newKey}
              onChange={(e) => setNewKey(e.target.value)}
              placeholder="sk-ant-..."
              style={{ flex: 1, fontSize: 12 }}
            />
            <button onClick={saveKey} disabled={!newKey.trim()} style={{ fontSize: 12 }}>
              Set key
            </button>
          </div>
        )}
      </div>

      <div style={{ marginBottom: 10 }}>
        <div style={{ fontSize: 12, color: "#888", marginBottom: 4 }}>
          Monthly budget: £{(status.spent_month_pence / 100).toFixed(2)} spent of £{(status.budget_pence / 100).toFixed(2)}
        </div>
        <div style={{ height: 6, background: "#222", borderRadius: 3, overflow: "hidden" }}>
          <div style={{ width: `${pct}%`, height: "100%", background: barColor, transition: "width 200ms" }} />
        </div>
        <div style={{ display: "flex", gap: 6, marginTop: 4, alignItems: "center" }}>
          <span style={{ fontSize: 12 }}>£</span>
          <input
            type="number" step="0.01" min="0"
            value={budgetInput}
            onChange={(e) => setBudgetInput(e.target.value)}
            style={{ width: 80, fontSize: 12 }}
          />
          <button onClick={saveBudget} style={{ fontSize: 12 }}>Save budget</button>
        </div>
      </div>

      <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 13, marginBottom: 8 }}>
        <input
          type="checkbox"
          checked={status.enabled_for_review}
          onChange={(e) => void toggleEnabled(e.target.checked)}
          disabled={!status.has_key}
        />
        Use Claude for ledger AI month review
      </label>

      {status.has_key && (
        <button onClick={test} disabled={testing} style={{ fontSize: 12 }}>
          {testing ? "Testing…" : "Test connection"}
        </button>
      )}

      {message && (
        <div style={{ fontSize: 11, color: message.includes("ailed") ? "#f66" : "#6f6", marginTop: 6 }}>
          {message}
        </div>
      )}
    </section>
  );
}

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

      <RemoteProvidersSection />
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
