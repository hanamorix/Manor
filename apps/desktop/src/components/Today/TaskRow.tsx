import { useEffect, useRef, useState } from "react";
import type { Task } from "../../lib/today/ipc";
import {
  completeTask,
  undoCompleteTask,
  updateTask,
  deleteTask,
} from "../../lib/today/ipc";
import { useTodayStore } from "../../lib/today/state";

interface TaskRowProps {
  task: Task;
}

const COMPLETE_FADE_MS = 4000;
const DELETE_CONFIRM_MS = 1000;

export default function TaskRow({ task }: TaskRowProps) {
  const removeTask = useTodayStore((s) => s.removeTask);
  const upsertTask = useTodayStore((s) => s.upsertTask);

  const [hovering, setHovering] = useState(false);
  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState(task.title);
  const [completing, setCompleting] = useState(false);
  const [deleteArmed, setDeleteArmed] = useState(false);

  const completeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (completeTimerRef.current) clearTimeout(completeTimerRef.current);
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
    };
  }, []);

  const startComplete = () => {
    if (completing) {
      // Click while in completing state = undo
      void undoCompleteTask(task.id);
      setCompleting(false);
      if (completeTimerRef.current) {
        clearTimeout(completeTimerRef.current);
        completeTimerRef.current = null;
      }
      return;
    }
    setCompleting(true);
    void completeTask(task.id);
    completeTimerRef.current = setTimeout(() => {
      removeTask(task.id);
    }, COMPLETE_FADE_MS);
  };

  const startEdit = () => {
    setEditValue(task.title);
    setEditing(true);
  };

  const commitEdit = () => {
    const trimmed = editValue.trim();
    if (trimmed.length === 0 || trimmed === task.title) {
      setEditing(false);
      return;
    }
    void updateTask(task.id, trimmed);
    upsertTask({ ...task, title: trimmed });
    setEditing(false);
  };

  const armOrConfirmDelete = () => {
    if (deleteArmed) {
      void deleteTask(task.id);
      removeTask(task.id);
      return;
    }
    setDeleteArmed(true);
    deleteTimerRef.current = setTimeout(() => setDeleteArmed(false), DELETE_CONFIRM_MS);
  };

  return (
    <div
      onMouseEnter={() => setHovering(true)}
      onMouseLeave={() => setHovering(false)}
      style={{
        display: "flex",
        gap: 10,
        padding: "6px 4px",
        alignItems: "center",
        background: deleteArmed ? "var(--ink-danger)" : "transparent",
        borderRadius: 6,
        transition: "background 100ms ease",
      }}
    >
      <button
        onClick={startComplete}
        aria-label={completing ? "undo complete" : "complete"}
        style={{
          width: 18,
          height: 18,
          padding: 0,
          border: completing ? "1.5px solid var(--ink)" : "1.5px solid var(--ink-faint)",
          background: completing ? "var(--ink)" : "transparent",
          color: "var(--action-fg)",
          borderRadius: 4,
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
          fontSize: "var(--text-xs)",
          fontWeight: 600,
          lineHeight: 1,
        }}
      >
        {completing ? "✓" : ""}
      </button>

      {editing ? (
        <input
          type="text"
          value={editValue}
          autoFocus
          onChange={(e) => setEditValue(e.target.value)}
          onBlur={commitEdit}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              commitEdit();
            } else if (e.key === "Escape") {
              setEditing(false);
            }
          }}
          style={{
            flex: 1,
            padding: "2px 6px",
            border: "1px solid var(--hairline)",
            borderRadius: 4,
            fontSize: "var(--text-md)",
            fontFamily: "inherit",
          }}
        />
      ) : (
        <span
          onClick={startEdit}
          style={{
            flex: 1,
            fontSize: "var(--text-md)",
            lineHeight: 1.4,
            cursor: "text",
            textDecoration: completing ? "line-through" : "none",
            opacity: completing ? 0.5 : 1,
            transition: "opacity 200ms ease",
          }}
        >
          {task.title}
        </span>
      )}

      {hovering && !editing && !completing && (
        <div style={{ display: "flex", gap: 4, opacity: 0.6, transition: "opacity 100ms ease" }}>
          <button
            onClick={startEdit}
            aria-label="edit"
            style={{
              width: 24,
              height: 24,
              padding: 0,
              background: "transparent",
              border: "none",
              cursor: "pointer",
              fontSize: "var(--text-md)",
            }}
          >
            ✎
          </button>
          <button
            onClick={armOrConfirmDelete}
            aria-label={deleteArmed ? "confirm delete" : "delete"}
            style={{
              width: 24,
              height: 24,
              padding: 0,
              background: "transparent",
              border: "none",
              cursor: "pointer",
              fontSize: "var(--text-md)",
              color: deleteArmed ? "var(--ink)" : "inherit",
            }}
          >
            🗑
          </button>
        </div>
      )}
    </div>
  );
}
