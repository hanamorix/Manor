import { useState } from "react";
import { updateEvent, deleteEvent } from "../../lib/today/ipc";
import type { Event } from "../../lib/today/ipc";
import { useOverlay } from "../../lib/overlay/state";

interface Props {
  event: Event;
  onClose: () => void;
  onSaved: () => Promise<void>;
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

export default function EditEventDrawer({ event, onClose, onSaved }: Props) {
  useOverlay();
  const [title, setTitle] = useState(event.title);
  const [date, setDate] = useState(toDateInputValue(event.start_at));
  const [startTime, setStartTime] = useState(toTimeInputValue(event.start_at));
  const [endTime, setEndTime] = useState(toTimeInputValue(event.end_at));
  const [allDay, setAllDay] = useState(event.all_day);
  const [description, setDescription] = useState(event.description ?? "");
  const [location, setLocation] = useState(event.location ?? "");
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isManual = !event.event_url;

  const inputStyle: React.CSSProperties = {
    width: "100%",
    padding: "9px 12px",
    fontSize: 14,
    border: "1px solid var(--hairline)",
    borderRadius: 10,
    background: "#fafafa",
    fontFamily: "inherit",
    boxSizing: "border-box",
  };

  const labelStyle: React.CSSProperties = {
    fontSize: 11,
    fontWeight: 700,
    textTransform: "uppercase",
    letterSpacing: 0.5,
    color: "rgba(0,0,0,0.5)",
    marginBottom: 5,
    display: "block",
  };

  async function handleSave() {
    if (!title.trim()) { setError("Enter a title"); return; }
    const start_at = allDay
      ? Math.floor(new Date(date + "T00:00:00").getTime() / 1000)
      : combineDateTime(date, startTime);
    const end_at = allDay ? start_at + 86400 : combineDateTime(date, endTime);
    if (end_at <= start_at) { setError("End must be after start"); return; }

    setSaving(true);
    setError(null);
    try {
      await updateEvent({
        event_id: event.id,
        title: title.trim(),
        start_at,
        end_at,
        description: description.trim() || undefined,
        location: location.trim() || undefined,
        all_day: allDay,
        edit_occurrence_only: event.is_recurring_occurrence,
      });
      await onSaved();
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  }

  async function handleDelete(occurrenceOnly: boolean) {
    setDeleting(true);
    setError(null);
    try {
      await deleteEvent({ event_id: event.id, delete_occurrence_only: occurrenceOnly });
      await onSaved();
    } catch (e) {
      setError(String(e));
      setDeleting(false);
    }
  }

  return (
    <>
      <div onClick={onClose} style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.25)", zIndex: 1050 }} />
      <div
        style={{
          position: "fixed",
          right: 0,
          top: 0,
          bottom: 0,
          width: 420,
          background: "var(--paper)",
          boxShadow: "-4px 0 24px rgba(0,0,0,0.12)",
          zIndex: 1100,
          display: "flex",
          flexDirection: "column",
          animation: "drawerIn 200ms ease-out",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "18px 20px 14px", borderBottom: "1px solid var(--hairline)" }}>
          <div style={{ fontSize: 16, fontWeight: 700 }}>Edit Event</div>
          <button onClick={onClose} style={{ background: "none", border: "none", fontSize: 20, cursor: "pointer", color: "rgba(0,0,0,0.4)", lineHeight: 1, padding: 0 }}>✕</button>
        </div>

        <div style={{ flex: 1, overflow: "auto", padding: "20px" }}>
          {isManual && (
            <div style={{ marginBottom: 16, padding: "8px 12px", background: "rgba(0,0,0,0.04)", borderRadius: 8, fontSize: 12, color: "rgba(0,0,0,0.5)" }}>
              Manual event — changes are local only
            </div>
          )}

          {event.is_recurring_occurrence && (
            <div style={{ marginBottom: 16, padding: "8px 12px", background: "rgba(0,122,255,0.08)", borderRadius: 8, fontSize: 12, color: "var(--ink)" }}>
              Recurring event — editing this occurrence only
            </div>
          )}

          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div>
              <label style={labelStyle}>Title</label>
              <input style={inputStyle} type="text" value={title} onChange={(e) => setTitle(e.target.value)} />
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <label style={{ ...labelStyle, marginBottom: 0 }}>All Day</label>
              <input type="checkbox" checked={allDay} onChange={(e) => setAllDay(e.target.checked)} />
            </div>

            <div>
              <label style={labelStyle}>Date</label>
              <input style={inputStyle} type="date" value={date} onChange={(e) => setDate(e.target.value)} />
            </div>

            {!allDay && (
              <div style={{ display: "flex", gap: 12 }}>
                <div style={{ flex: 1 }}>
                  <label style={labelStyle}>Start</label>
                  <input style={inputStyle} type="time" value={startTime} onChange={(e) => setStartTime(e.target.value)} />
                </div>
                <div style={{ flex: 1 }}>
                  <label style={labelStyle}>End</label>
                  <input style={inputStyle} type="time" value={endTime} onChange={(e) => setEndTime(e.target.value)} />
                </div>
              </div>
            )}

            <div>
              <label style={labelStyle}>Description (optional)</label>
              <input style={inputStyle} type="text" value={description} onChange={(e) => setDescription(e.target.value)} />
            </div>

            <div>
              <label style={labelStyle}>Location (optional)</label>
              <input style={inputStyle} type="text" value={location} onChange={(e) => setLocation(e.target.value)} />
            </div>

            {error && (
              <div style={{ padding: "10px 12px", background: "rgba(255,59,48,0.08)", border: "1px solid rgba(255,59,48,0.3)", borderRadius: 10, fontSize: 13, color: "var(--ink)" }}>
                {error}
              </div>
            )}

            {!confirmDelete ? (
              <button
                onClick={() => setConfirmDelete(true)}
                style={{ marginTop: 8, background: "none", border: "1px solid rgba(255,59,48,0.4)", borderRadius: 10, padding: "10px 0", color: "var(--ink)", fontSize: 13, fontWeight: 600, cursor: "pointer", fontFamily: "inherit" }}
              >
                Delete Event
              </button>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {event.is_recurring_occurrence && (
                  <button
                    onClick={() => handleDelete(true)}
                    disabled={deleting}
                    style={{ background: "rgba(255,59,48,0.08)", border: "1px solid rgba(255,59,48,0.4)", borderRadius: 10, padding: "10px 0", color: "var(--ink)", fontSize: 13, fontWeight: 600, cursor: "pointer", fontFamily: "inherit" }}
                  >
                    Delete this occurrence only
                  </button>
                )}
                <button
                  onClick={() => handleDelete(false)}
                  disabled={deleting}
                  style={{ background: "rgba(255,59,48,0.15)", border: "1px solid rgba(255,59,48,0.6)", borderRadius: 10, padding: "10px 0", color: "var(--ink)", fontSize: 14, fontWeight: 700, cursor: "pointer", fontFamily: "inherit" }}
                >
                  {event.is_recurring_occurrence ? "Delete all occurrences" : "Confirm Delete"}
                </button>
                <button onClick={() => setConfirmDelete(false)} style={{ background: "none", border: "none", fontSize: 13, color: "rgba(0,0,0,0.4)", cursor: "pointer", fontFamily: "inherit" }}>
                  Cancel
                </button>
              </div>
            )}
          </div>
        </div>

        <div style={{ padding: "14px 20px", borderTop: "1px solid var(--hairline)" }}>
          <button
            onClick={handleSave}
            disabled={saving}
            style={{ width: "100%", padding: "12px 0", background: "var(--ink)", color: "var(--action-fg)", border: "none", borderRadius: 12, fontSize: 15, fontWeight: 700, cursor: saving ? "default" : "pointer", opacity: saving ? 0.6 : 1, fontFamily: "inherit" }}
          >
            {saving ? "Saving…" : "Save Changes"}
          </button>
        </div>
      </div>
    </>
  );
}
