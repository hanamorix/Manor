import { useState } from "react";
import { X } from "lucide-react";
import type { Proposal } from "../../lib/today/ipc";
import { approveProposalWithOverride } from "../../lib/today/ipc";
import { useOverlay } from "../../lib/overlay/state";
import type { AddEventParsed } from "./registry";

export interface ProposalEventEditDrawerProps {
  parsed: AddEventParsed[];
  proposal: Proposal;
  onClose: () => void;
  onApplied: () => void;
}

function toDateInputValue(ts: number): string {
  return new Date(ts * 1000).toISOString().slice(0, 10);
}

function toTimeInputValue(ts: number): string {
  return new Date(ts * 1000).toISOString().slice(11, 16);
}

function combineDateTime(date: string, time: string): number {
  return Math.floor(new Date(`${date}T${time}:00`).getTime() / 1000);
}

export function ProposalEventEditDrawer({
  parsed,
  proposal,
  onClose,
  onApplied,
}: ProposalEventEditDrawerProps) {
  useOverlay();
  const event = parsed[0];
  const [title, setTitle] = useState(event?.title ?? "");
  const [date, setDate] = useState(toDateInputValue(event?.start_at ?? Date.now() / 1000));
  const [startTime, setStartTime] = useState(toTimeInputValue(event?.start_at ?? Date.now() / 1000));
  const [endTime, setEndTime] = useState(toTimeInputValue(event?.end_at ?? Date.now() / 1000 + 3600));
  const [allDay, setAllDay] = useState(event?.all_day ?? false);
  const [description, setDescription] = useState(event?.description ?? "");
  const [location, setLocation] = useState(event?.location ?? "");
  const [accountId, setAccountId] = useState(event?.account_id?.toString() ?? "");
  const [calendarUrl, setCalendarUrl] = useState(event?.calendar_url ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSave() {
    if (!event) {
      setError("Event details could not be parsed.");
      return;
    }
    if (!title.trim()) {
      setError("Enter a title");
      return;
    }
    const start_at = allDay
      ? Math.floor(new Date(`${date}T00:00:00`).getTime() / 1000)
      : combineDateTime(date, startTime);
    const end_at = allDay ? start_at + 86400 : combineDateTime(date, endTime);
    if (end_at <= start_at) {
      setError("End must be after start");
      return;
    }

    const account_id = accountId.trim() ? Number(accountId.trim()) : undefined;
    if (account_id !== undefined && !Number.isInteger(account_id)) {
      setError("Account id must be a number");
      return;
    }

    const edited: AddEventParsed = {
      ...event,
      account_id,
      calendar_url: calendarUrl.trim() || undefined,
      title: title.trim(),
      start_at,
      end_at,
      description: description.trim() || undefined,
      location: location.trim() || undefined,
      all_day: allDay,
    };
    const editedDiff = JSON.stringify(parsed.length === 1 ? edited : [edited, ...parsed.slice(1)]);

    setSaving(true);
    setError(null);
    try {
      await approveProposalWithOverride(proposal.id, editedDiff);
      onApplied();
      onClose();
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  }

  return (
    <>
      <div
        onClick={onClose}
        style={{ position: "fixed", inset: 0, background: "var(--ink-faint)", zIndex: 1050 }}
      />
      <div style={drawerStyle}>
        <div style={headerStyle}>
          <div style={{ fontSize: "var(--text-lg)", fontWeight: 600 }}>Edit Event</div>
          <button type="button" onClick={onClose} aria-label="Close" style={iconButtonStyle}>
            <X size={18} strokeWidth={1.8} />
          </button>
        </div>

        <div style={{ flex: 1, overflow: "auto", padding: 20 }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            {parsed.length > 1 && (
              <div style={noticeStyle}>
                Editing the first event in this bundle. Other events will be approved unchanged.
              </div>
            )}

            <label style={labelStyle}>
              Title
              <input style={inputStyle} type="text" value={title} onChange={(e) => setTitle(e.target.value)} />
            </label>

            <label style={checkLabelStyle}>
              <input type="checkbox" checked={allDay} onChange={(e) => setAllDay(e.target.checked)} />
              All day
            </label>

            <label style={labelStyle}>
              Date
              <input style={inputStyle} type="date" value={date} onChange={(e) => setDate(e.target.value)} />
            </label>

            {!allDay && (
              <div style={{ display: "flex", gap: 12 }}>
                <label style={{ ...labelStyle, flex: 1 }}>
                  Start
                  <input style={inputStyle} type="time" value={startTime} onChange={(e) => setStartTime(e.target.value)} />
                </label>
                <label style={{ ...labelStyle, flex: 1 }}>
                  End
                  <input style={inputStyle} type="time" value={endTime} onChange={(e) => setEndTime(e.target.value)} />
                </label>
              </div>
            )}

            <label style={labelStyle}>
              Description
              <input style={inputStyle} type="text" value={description} onChange={(e) => setDescription(e.target.value)} />
            </label>

            <label style={labelStyle}>
              Location
              <input style={inputStyle} type="text" value={location} onChange={(e) => setLocation(e.target.value)} />
            </label>

            <label style={labelStyle}>
              Account id
              <input style={inputStyle} type="text" value={accountId} onChange={(e) => setAccountId(e.target.value)} />
            </label>

            <label style={labelStyle}>
              Calendar URL
              <input style={inputStyle} type="text" value={calendarUrl} onChange={(e) => setCalendarUrl(e.target.value)} />
            </label>

            {error && <div style={errorStyle}>{error}</div>}
          </div>
        </div>

        <div style={footerStyle}>
          <button type="button" onClick={onClose} disabled={saving} style={secondaryButtonStyle}>
            Cancel
          </button>
          <button type="button" onClick={() => void handleSave()} disabled={saving} style={primaryButtonStyle}>
            {saving ? "Saving..." : "Approve"}
          </button>
        </div>
      </div>
    </>
  );
}

const drawerStyle: React.CSSProperties = {
  position: "fixed",
  right: 0,
  top: 0,
  bottom: 0,
  width: 420,
  background: "var(--paper)",
  boxShadow: "var(--shadow-lg)",
  zIndex: 1100,
  display: "flex",
  flexDirection: "column",
  animation: "drawerIn 200ms ease-out",
};

const headerStyle: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  padding: "18px 20px 14px",
  borderBottom: "1px solid var(--hairline)",
};

const iconButtonStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  cursor: "pointer",
  color: "var(--ink-soft)",
  padding: 0,
  display: "inline-flex",
  alignItems: "center",
};

const labelStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 5,
  fontSize: 11,
  fontWeight: 600,
  color: "var(--scrim)",
};

const checkLabelStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  fontSize: 11,
  fontWeight: 600,
  color: "var(--scrim)",
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "9px 12px",
  fontSize: "var(--text-md)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  background: "#fafafa",
  fontFamily: "inherit",
  boxSizing: "border-box",
  color: "var(--ink)",
  fontWeight: 400,
};

const noticeStyle: React.CSSProperties = {
  border: "1px solid var(--hairline)",
  borderRadius: 6,
  padding: "8px 10px",
  fontSize: "var(--text-xs)",
  color: "var(--ink-soft)",
};

const errorStyle: React.CSSProperties = {
  fontSize: "var(--text-xs)",
  color: "var(--danger)",
};

const footerStyle: React.CSSProperties = {
  display: "flex",
  justifyContent: "flex-end",
  gap: 10,
  padding: 20,
  borderTop: "1px solid var(--hairline)",
};

const secondaryButtonStyle: React.CSSProperties = {
  padding: "8px 12px",
  border: "1px solid var(--hairline)",
  borderRadius: 6,
  background: "transparent",
  color: "var(--ink)",
  cursor: "pointer",
};

const primaryButtonStyle: React.CSSProperties = {
  padding: "8px 12px",
  border: "1px solid var(--ink)",
  borderRadius: 6,
  background: "var(--ink)",
  color: "var(--paper)",
  cursor: "pointer",
};
