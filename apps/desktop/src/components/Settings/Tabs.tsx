import { useSettingsStore } from "../../lib/settings/state";

type Tab = { id: "calendars" | "ai" | "about"; label: string; disabled: boolean };

const TABS: Tab[] = [
  { id: "calendars", label: "Calendars", disabled: false },
  { id: "ai", label: "AI (soon)", disabled: true },
  { id: "about", label: "About (soon)", disabled: true },
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
            onClick={() => !t.disabled && setActiveTab(t.id)}
            disabled={t.disabled}
            style={{
              padding: "10px 14px",
              fontSize: 13,
              fontWeight: active ? 700 : 500,
              background: "transparent",
              border: "none",
              borderBottom: active ? "2px solid var(--imessage-blue)" : "2px solid transparent",
              color: t.disabled ? "rgba(0,0,0,0.3)" : "var(--ink)",
              fontStyle: t.disabled ? "italic" : "normal",
              cursor: t.disabled ? "default" : "pointer",
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
