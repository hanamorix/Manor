import { useState } from "react";
import { Link } from "lucide-react";
import { addCalendarAccount } from "../../lib/settings/ipc";
import { useSettingsStore } from "../../lib/settings/state";
import { Button } from "../../lib/ui";

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
    display: "block", fontSize: 11, fontWeight: 600,
    color: "var(--ink-soft)", marginBottom: 4, marginTop: 10,
  };

  const inputStyle: React.CSSProperties = {
    width: "100%", padding: "6px 10px",
    border: "1px solid var(--hairline)", borderRadius: 6,
    fontSize: "var(--text-sm)", fontFamily: "inherit",
  };

  return (
    <div
      style={{
        padding: 12,
        background: "var(--paper-muted)",
        border: "1px dashed var(--hairline)",
        borderRadius: "var(--radius-lg)",
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
        <Button variant="secondary" onClick={onClose}>Cancel</Button>
        <Button
          variant="primary"
          icon={Link}
          onClick={onConnect}
          disabled={!canSubmit}
          style={{ opacity: canSubmit ? 1 : 0.5 }}
        >
          {busy ? "Connecting…" : "Connect"}
        </Button>
      </div>
    </div>
  );
}
