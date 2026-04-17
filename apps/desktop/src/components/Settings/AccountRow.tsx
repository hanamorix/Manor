import { useEffect, useRef, useState } from "react";
import type { CalendarAccount } from "../../lib/settings/ipc";
import { syncAccount, removeCalendarAccount, listCalendars, setDefaultCalendar } from "../../lib/settings/ipc";
import type { CalendarInfo } from "../../lib/settings/ipc";
import { listEventsToday } from "../../lib/today/ipc";
import { useSettingsStore } from "../../lib/settings/state";
import { useTodayStore } from "../../lib/today/state";

interface AccountRowProps {
  account: CalendarAccount;
  onRefresh?: () => void;
}

function relativeTime(seconds: number): string {
  const delta = Date.now() / 1000 - seconds;
  if (delta < 60) return `${Math.floor(delta)}s ago`;
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
  return `${Math.floor(delta / 86400)}d ago`;
}

function providerBadge(url: string): string {
  if (url.includes("caldav.icloud.com")) return "iC";
  if (url.includes("fastmail")) return "FM";
  return "●";
}

export default function AccountRow({ account, onRefresh }: AccountRowProps) {
  const upsertAccount = useSettingsStore((s) => s.upsertAccount);
  const removeAccount = useSettingsStore((s) => s.removeAccount);
  const markSyncing = useSettingsStore((s) => s.markSyncing);
  const markSynced = useSettingsStore((s) => s.markSynced);
  const syncing = useSettingsStore((s) => s.syncingAccountIds.has(account.id));
  const setEvents = useTodayStore((s) => s.setEvents);

  const [removeArmed, setRemoveArmed] = useState(false);
  const removeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [calendars, setCalendarsState] = useState<CalendarInfo[]>([]);

  useEffect(() => {
    listCalendars(account.id).then(setCalendarsState).catch(() => {});
  }, [account.id]);

  useEffect(() => () => {
    if (removeTimer.current) clearTimeout(removeTimer.current);
  }, []);

  const handleSync = async () => {
    markSyncing(account.id);
    try {
      const result = await syncAccount(account.id);
      upsertAccount({
        ...account,
        last_synced_at: result.synced_at,
        last_error: result.error,
      });
      const events = await listEventsToday();
      setEvents(events);
    } finally {
      markSynced(account.id);
    }
  };

  const handleRemoveClick = () => {
    if (removeArmed) {
      removeAccount(account.id);
      void removeCalendarAccount(account.id).then(() =>
        listEventsToday().then(setEvents),
      );
      return;
    }
    setRemoveArmed(true);
    removeTimer.current = setTimeout(() => setRemoveArmed(false), 3000);
  };

  const statusLine = (() => {
    if (syncing) return "syncing…";
    if (account.last_error) return `error: ${account.last_error}`;
    if (account.last_synced_at) return `synced ${relativeTime(account.last_synced_at)}`;
    return "not synced yet";
  })();

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        padding: "8px 10px",
        background: removeArmed ? "rgba(255, 59, 48, 0.06)" : "var(--surface)",
        border: "1px solid var(--hairline)",
        borderRadius: "var(--radius-lg)",
        marginBottom: 6,
      }}
    >
      <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
        <div
          style={{
            width: 28,
            height: 28,
            borderRadius: 6,
            background: "var(--ink)",
            color: "var(--action-fg)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 11,
            fontWeight: 600,
          }}
        >
          {providerBadge(account.server_url)}
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontWeight: 600, fontSize: 13 }}>{account.display_name}</div>
          <div
            style={{
              fontSize: 11,
              color: account.last_error ? "var(--ink)" : "var(--ink-soft)",
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
            title={account.last_error ?? undefined}
          >
            {account.username} · {statusLine}
          </div>
        </div>
        <button
          onClick={handleSync}
          disabled={syncing}
          style={{
            padding: "5px 10px",
            borderRadius: 6,
            fontSize: 11,
            fontWeight: 600,
            border: "1px solid var(--hairline)",
            background: "var(--surface)",
            cursor: syncing ? "default" : "pointer",
            opacity: syncing ? 0.5 : 1,
          }}
        >
          Sync
        </button>
        <button
          onClick={handleRemoveClick}
          style={{
            padding: "5px 10px",
            borderRadius: 6,
            fontSize: 11,
            fontWeight: 600,
            border: "1px solid var(--hairline)",
            background: "var(--surface)",
            cursor: "pointer",
            color: removeArmed ? "var(--ink)" : "inherit",
          }}
        >
          {removeArmed ? "Yes?" : "Remove"}
        </button>
      </div>

      {calendars.length > 0 && (
        <div style={{ marginTop: 10, display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{ fontSize: 11, color: "var(--ink-soft)", minWidth: 100 }}>Default calendar</span>
          <select
            value={account.default_calendar_url ?? ""}
            onChange={async (e) => {
              await setDefaultCalendar(account.id, e.target.value);
              onRefresh?.();
            }}
            style={{
              flex: 1,
              padding: "5px 8px",
              fontSize: "var(--text-xs)",
              border: "1px solid var(--hairline)",
              borderRadius: "var(--radius-lg)",
              background: "var(--hairline)",
              fontFamily: "inherit",
            }}
          >
            <option value="">Auto-select</option>
            {calendars.map((c) => (
              <option key={c.id} value={c.url}>
                {c.display_name ?? c.url}
              </option>
            ))}
          </select>
        </div>
      )}
    </div>
  );
}
