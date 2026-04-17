import { useSettingsStore } from "../../lib/settings/state";

type Tab = {
  id: "data" | "ai" | "calendars" | "household" | "about";
  label: string;
};

const TABS: Tab[] = [
  { id: "data",      label: "Data & Backup" },
  { id: "ai",        label: "AI" },
  { id: "calendars", label: "Calendars" },
  { id: "household", label: "Household" },
  { id: "about",     label: "About" },
];

export default function Tabs() {
  const activeTab = useSettingsStore((s) => s.activeTab);
  const setActiveTab = useSettingsStore((s) => s.setActiveTab);

  return (
    <div style={{ display: "flex", gap: 2, borderBottom: "1px solid var(--hairline)", padding: "0 14px" }}>
      {TABS.map((t) => {
        const active = activeTab === t.id;
        return (
          <button
            key={t.id}
            onClick={() => setActiveTab(t.id)}
            style={{
              padding: "10px 14px",
              fontSize: 13,
              fontWeight: active ? 700 : 500,
              background: "transparent",
              border: "none",
              borderBottom: active ? "2px solid var(--ink)" : "2px solid transparent",
              color: "var(--ink)",
              cursor: "pointer",
              fontFamily: "inherit",
            }}
          >
            {t.label}
          </button>
        );
      })}
    </div>
  );
}
