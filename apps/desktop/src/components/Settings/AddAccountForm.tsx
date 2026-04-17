import { useState } from "react";
import { addCalendarAccount } from "../../lib/settings/ipc";
import { useSettingsStore } from "../../lib/settings/state";

interface AddAccountFormProps { onClose: () => void; }

export default function AddAccountForm({ onClose }: AddAccountFormProps) {
  const upsertAccount = useSettingsStore((s) => s.upsertAccount);

  const [serverUrl, setServerUrl] = useState("https://caldav.icloud.com");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const canSubmit = serverUrl.trim() && username.trim() && password.trim() && !busy;

  const onConnect = async () => {
    setBusy(true);
    setError(null);
    try {
      const account = await addCalendarAccount(serverUrl.trim(), username.trim(), password);
      upsertAccount(account);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const labelStyle: React.CSSProperties = {
    display: "block", fontSize: 11, fontWeight: 700,
    textTransform: "uppercase", letterSpacing: 0.6,
    color: "var(--ink-soft)", marginBottom: 4, marginTop: 10,
  };

  const inputStyle: React.CSSProperties = {
    width: "100%", padding: "6px 10px",
    border: "1px solid var(--hairline)", borderRadius: 6,
    fontSize: 13, fontFamily: "inherit", outline: "none",
  };

  return (
    <div
      style={{
        padding: 12,
        background: "rgba(0,0,0,0.02)",
        border: "1px dashed var(--hairline)",
        borderRadius: 8,
        marginTop: 8,
      }}
    >
      <label style={labelStyle}>Server URL</label>
      <input
        type="text" value={serverUrl}
        onChange={(e) => setServerUrl(e.target.value)}
        placeholder="https://caldav.icloud.com"
        style={inputStyle}
      />
      <label style={labelStyle}>Username</label>
      <input
        type="text" value={username}
        onChange={(e) => setUsername(e.target.value)}
        placeholder="your-apple-id@icloud.com"
        style={inputStyle}
      />
      <label style={labelStyle}>App-specific password</label>
      <input
        type="password" value={password}
        onChange={(e) => setPassword(e.target.value)}
        style={inputStyle}
      />
      <p style={{ marginTop: 6, marginBottom: 0, fontSize: 11, color: "var(--ink-soft)", lineHeight: 1.4 }}>
        {serverUrl.includes("caldav.icloud.com") ? (
          <>
            iCloud needs an app-specific password — generate one at{" "}
            <a
              href="https://appleid.apple.com/account/manage"
              target="_blank"
              rel="noreferrer"
              style={{ color: "var(--ink)", textDecoration: "none", fontWeight: 600 }}
            >
              appleid.apple.com
            </a>
            {" "}→ Sign-In and Security → App-Specific Passwords.
          </>
        ) : serverUrl.includes("fastmail") ? (
          <>
            Fastmail needs an app password — generate one at{" "}
            <a
              href="https://www.fastmail.com/settings/security/devicekeys"
              target="_blank"
              rel="noreferrer"
              style={{ color: "var(--ink)", textDecoration: "none", fontWeight: 600 }}
            >
              fastmail.com → Settings → Privacy &amp; Security → App passwords
            </a>.
          </>
        ) : (
          <>Use the password your CalDAV server expects (often an app-specific password, not your main login).</>
        )}
      </p>

      {error && (
        <div style={{ marginTop: 8, color: "var(--ink)", fontSize: 12 }}>
          Connection failed: {error}
        </div>
      )}

      <div style={{ marginTop: 12, display: "flex", gap: 8, justifyContent: "flex-end" }}>
        <button onClick={onClose} style={{
          padding: "6px 12px", borderRadius: 6, fontSize: 12, fontWeight: 600,
          border: "1px solid var(--hairline)", background: "var(--surface)", cursor: "pointer",
        }}>Cancel</button>
        <button
          onClick={onConnect}
          disabled={!canSubmit}
          style={{
            padding: "6px 12px", borderRadius: 6, fontSize: 12, fontWeight: 700,
            border: "none", background: canSubmit ? "var(--ink)" : "var(--hairline)",
            color: canSubmit ? "var(--action-fg)" : "var(--ink-soft)", cursor: canSubmit ? "pointer" : "default",
          }}
        >
          {busy ? "Connecting…" : "Connect"}
        </button>
      </div>
    </div>
  );
}
