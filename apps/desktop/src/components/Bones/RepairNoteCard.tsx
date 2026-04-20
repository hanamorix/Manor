import { useState } from "react";
import { Calendar, Trash2, ChevronDown, ChevronRight } from "lucide-react";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import type { RepairNote } from "../../lib/repair/ipc";
import { useRepairStore } from "../../lib/repair/state";
import { RepairMarkdown } from "./RepairMarkdown";

interface Props {
  note: RepairNote;
}

function relativeDate(unixSec: number): string {
  const d = new Date(unixSec * 1000);
  const days = Math.floor((Date.now() - d.getTime()) / (1000 * 60 * 60 * 24));
  if (days === 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 14) return `${days} days ago`;
  const weeks = Math.floor(days / 7);
  if (weeks < 8) return `${weeks} weeks ago`;
  return d.toLocaleDateString("en-GB", { month: "short", day: "numeric", year: "numeric" });
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, n - 1) + "…";
}

export function RepairNoteCard({ note }: Props) {
  const [expanded, setExpanded] = useState(false);
  const { deleteNote } = useRepairStore();

  return (
    <div style={{
      border: "1px solid var(--border, #e5e5e5)",
      borderRadius: 6,
      marginBottom: 8,
      overflow: "hidden",
    }}>
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          width: "100%",
          padding: "10px 12px",
          background: "none",
          border: "none",
          cursor: "pointer",
          textAlign: "left",
        }}
      >
        {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        <Calendar size={14} color="var(--ink-soft, #888)" />
        <span style={{ color: "var(--ink-soft, #888)", fontSize: 12, minWidth: 80 }}>
          {relativeDate(note.created_at)}
        </span>
        <span style={{ flex: 1 }}>{truncate(note.symptom, 60)}</span>
        <span style={{
          fontSize: 11,
          padding: "2px 6px",
          borderRadius: 4,
          background: note.tier === "claude" ? "var(--accent-bg, #eef5ff)" : "var(--surface-subtle, #f4f4f4)",
          color: "var(--ink-soft, #666)",
        }}>
          {note.tier === "claude" ? "claude" : "local"}
        </span>
        <span
          role="button"
          aria-label="Delete repair note"
          onClick={(e) => {
            e.stopPropagation();
            void deleteNote(note.id, note.asset_id);
          }}
          style={{ cursor: "pointer", padding: 4 }}
        >
          <Trash2 size={14} color="var(--ink-soft, #888)" />
        </span>
      </button>
      {expanded && (
        <div style={{ padding: "8px 16px 16px 16px" }}>
          <RepairMarkdown body={note.body_md} />
          {note.sources.length > 0 && (
            <div style={{ marginTop: 8 }}>
              <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", marginBottom: 4 }}>Sources</div>
              <ul style={{ margin: 0, paddingLeft: 18 }}>
                {note.sources.map((s) => (
                  <li key={s.url}>
                    <a
                      href={s.url}
                      onClick={(e) => { e.preventDefault(); void openUrl(s.url); }}
                      style={{ cursor: "pointer" }}
                    >
                      {s.title}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {note.video_sources && note.video_sources.length > 0 && (
            <div style={{ marginTop: 8 }}>
              <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", marginBottom: 4 }}>Videos</div>
              <ul style={{ margin: 0, paddingLeft: 18 }}>
                {note.video_sources.map((s) => (
                  <li key={s.url}>
                    <a
                      href={s.url}
                      onClick={(e) => { e.preventDefault(); void openUrl(s.url); }}
                      style={{ cursor: "pointer" }}
                    >
                      {s.title}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
