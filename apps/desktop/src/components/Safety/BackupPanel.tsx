import { useEffect, useState } from "react";
import { Save, HardDrive, CalendarOff, Calendar, Undo2 } from "lucide-react";
import {
  backupCreateNow, backupHasPassphrase, backupList, backupRestore,
  backupScheduleInstall, backupScheduleIsInstalled, backupScheduleUninstall,
  backupSetPassphrase, type BackupEntry,
} from "../../lib/safety/ipc";
import {
  COLOR_DANGER,
  COLOR_SUCCESS,
  TEXT_MUTED,
  settingsListRow,
  settingsStatusWarn,
} from "../Settings/styles";
import { Button } from "../../lib/ui";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

interface Props {
  defaultOutDir: string; // resolved by caller (e.g. ~/Documents/Manor Backups)
}

export default function BackupPanel({ defaultOutDir }: Props) {
  const [outDir, setOutDir] = useState(defaultOutDir);
  const [hasPass, setHasPass] = useState(false);
  const [newPass, setNewPass] = useState("");
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [scheduled, setScheduled] = useState(false);
  const [weekday, setWeekday] = useState(0); // Sunday
  const [hour, setHour] = useState(2);
  const [creating, setCreating] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = async () => {
    setHasPass(await backupHasPassphrase());
    setScheduled(await backupScheduleIsInstalled());
    try { setBackups(await backupList(outDir)); } catch { setBackups([]); }
  };

  useEffect(() => { void refresh(); }, [outDir]);

  const savePass = async () => {
    await backupSetPassphrase(newPass);
    setNewPass("");
    await refresh();
    setMessage("Passphrase saved to Keychain.");
  };

  const runBackup = async () => {
    setCreating(true);
    setMessage(null);
    try {
      const out = await backupCreateNow(outDir);
      setMessage(`Backup written to ${out}`);
      await refresh();
    } catch (e) {
      setMessage(`Backup failed: ${e}`);
    } finally {
      setCreating(false);
    }
  };

  const toggleSchedule = async () => {
    if (scheduled) {
      await backupScheduleUninstall();
    } else {
      // programPath is the app binary — caller passes it in; here we punt to a placeholder
      // that Phase E's Settings tab will replace with the actual resolved binary path.
      await backupScheduleInstall({
        programPath: "/Applications/Manor.app/Contents/MacOS/manor-desktop",
        outDir,
        weekday,
        hour,
        minute: 0,
      });
    }
    await refresh();
  };

  return (
    <section style={{ padding: 16, display: "flex", flexDirection: "column", gap: 12 }}>
      <h2 style={{ margin: 0, fontSize: 15, color: "var(--ink)" }}>Backups</h2>
      <label style={{ color: "var(--ink)", fontSize: 13 }}>
        Backup folder
        <input
          value={outDir}
          onChange={(e) => setOutDir(e.target.value)}
          style={{ width: "100%" }}
        />
      </label>

      {!hasPass && (
        <div style={settingsStatusWarn}>
          <div style={{ fontSize: "var(--text-sm)", marginBottom: 6, color: "var(--ink)" }}>
            Set a passphrase to encrypt your backups.
          </div>
          <input
            type="password"
            value={newPass}
            onChange={(e) => setNewPass(e.target.value)}
            placeholder="Passphrase"
            style={{ width: "60%" }}
          />
          <Button
            variant="primary"
            icon={Save}
            onClick={savePass}
            disabled={newPass.length < 8}
            style={{ marginLeft: 8 }}
          >
            Save
          </Button>
        </div>
      )}

      <div style={{ display: "flex", gap: 8 }}>
        <Button variant="primary" icon={HardDrive} onClick={runBackup} disabled={!hasPass || creating}>
          {creating ? "Backing up…" : "Back up now"}
        </Button>
        <Button variant="secondary" icon={scheduled ? CalendarOff : Calendar} onClick={toggleSchedule} disabled={!hasPass}>
          {scheduled ? "Disable weekly schedule" : "Enable weekly schedule"}
        </Button>
      </div>

      {!scheduled && hasPass && (
        <div style={{ display: "flex", gap: 8, fontSize: 13 }}>
          <label>Day
            <select value={weekday} onChange={(e) => setWeekday(parseInt(e.target.value))}>
              {["Sun","Mon","Tue","Wed","Thu","Fri","Sat"].map((d,i) => <option key={i} value={i}>{d}</option>)}
            </select>
          </label>
          <label>Hour
            <input type="number" min={0} max={23} value={hour}
                   onChange={(e) => setHour(parseInt(e.target.value) || 0)} style={{ width: 60 }} />
          </label>
        </div>
      )}

      {message && (
        <div
          style={{
            fontSize: "var(--text-xs)",
            color: message.includes("failed") ? COLOR_DANGER : COLOR_SUCCESS,
          }}
        >
          {message}
        </div>
      )}

      <h3
        style={{
          fontSize: "var(--text-xs)",
          marginTop: 8,
          marginBottom: 4,
          color: TEXT_MUTED,
          textTransform: "uppercase",
          letterSpacing: 0.5,
          fontWeight: 600,
        }}
      >
        Existing backups
      </h3>
      {backups.length === 0 && (
        <div style={{ color: TEXT_MUTED, fontSize: 13 }}>None yet.</div>
      )}
      {backups.map((b) => (
        <div
          key={b.path}
          style={{
            ...settingsListRow,
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          <div>
            <div style={{ fontSize: "var(--text-sm)", color: "var(--ink)" }}>
              {b.path.split("/").pop()}
            </div>
            <div style={{ fontSize: 11, color: TEXT_MUTED }}>
              {new Date(b.mtime * 1000).toLocaleString()} · {formatSize(b.size_bytes)}
            </div>
          </div>
          <RestoreButton backupPath={b.path} />
        </div>
      ))}
    </section>
  );
}

function RestoreButton({ backupPath }: { backupPath: string }) {
  const [showing, setShowing] = useState(false);
  const [pass, setPass] = useState("");
  const [working, setWorking] = useState(false);
  const [result, setResult] = useState<string | null>(null);

  const run = async () => {
    setWorking(true);
    try {
      const staging = await backupRestore(backupPath, pass);
      setResult(`Restored to ${staging}. Quit Manor and replace manor.db / attachments in the data dir.`);
    } catch (e) {
      setResult(`Restore failed: ${e}`);
    } finally {
      setWorking(false);
    }
  };

  if (!showing) return <Button variant="secondary" icon={Undo2} onClick={() => setShowing(true)}>Restore</Button>;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <input type="password" value={pass} onChange={(e) => setPass(e.target.value)}
             placeholder="Passphrase" style={{ width: 180 }} />
      <div style={{ display: "flex", gap: 4 }}>
        <Button variant="secondary" onClick={() => { setShowing(false); setResult(null); }}>Cancel</Button>
        <Button variant="primary" onClick={run} disabled={working || pass.length === 0}>
          {working ? "…" : "Decrypt"}
        </Button>
      </div>
      {result && (
        <div
          style={{
            fontSize: 11,
            color: result.includes("failed") ? COLOR_DANGER : COLOR_SUCCESS,
          }}
        >
          {result}
        </div>
      )}
    </div>
  );
}
