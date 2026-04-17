import { useState, useRef, useEffect } from "react";
import { ListTodo } from "lucide-react";
import { useTodayStore } from "../../lib/today/state";
import { addTask } from "../../lib/today/ipc";
import { SectionLabel } from "../../lib/ui";
import TaskRow from "./TaskRow";

const addLink: React.CSSProperties = {
  background: "transparent",
  border: "none",
  color: "var(--ink)",
  fontWeight: 600,
  fontSize: "var(--text-xs)",
  cursor: "pointer",
  padding: 0,
};

export default function TasksCard() {
  const tasks = useTodayStore((s) => s.tasks);
  const upsertTask = useTodayStore((s) => s.upsertTask);

  const [adding, setAdding] = useState(false);
  const [addValue, setAddValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (adding) inputRef.current?.focus();
  }, [adding]);

  const commitAdd = async () => {
    const trimmed = addValue.trim();
    if (trimmed.length === 0) {
      setAdding(false);
      return;
    }
    const task = await addTask(trimmed);
    upsertTask(task);
    setAddValue("");
    // Stay in adding mode for follow-up adds
  };

  const headerCount = tasks.length;
  const headerLabel = headerCount > 0 ? `Tasks · ${headerCount} open` : `Tasks`;

  return (
    <section style={{ marginBottom: 22 }}>
      <SectionLabel
        icon={ListTodo}
        action={
          <button onClick={() => setAdding(true)} style={addLink}>
            + Add
          </button>
        }
      >
        {headerLabel}
      </SectionLabel>

      {tasks.length === 0 && !adding && (
        <p style={{ color: "var(--ink-faint)", margin: 0, fontSize: 13 }}>
          Nothing on your plate. Type <code>/task</code> or click + Add to add one.
        </p>
      )}

      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        {tasks.map((t) => (
          <TaskRow key={t.id} task={t} />
        ))}
      </div>

      {adding && (
        <input
          ref={inputRef}
          type="text"
          value={addValue}
          placeholder="New task title"
          onChange={(e) => setAddValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void commitAdd();
            } else if (e.key === "Escape") {
              setAddValue("");
              setAdding(false);
            }
          }}
          onBlur={() => {
            if (addValue.trim().length === 0) {
              setAdding(false);
            } else {
              void commitAdd();
            }
          }}
          style={{
            marginTop: 8,
            width: "100%",
            padding: "6px 10px",
            border: "1px dashed var(--hairline-strong)",
            borderRadius: 6,
            fontSize: "var(--text-md)",
            fontFamily: "inherit",
            background: "var(--paper-muted)",
          }}
        />
      )}
    </section>
  );
}
