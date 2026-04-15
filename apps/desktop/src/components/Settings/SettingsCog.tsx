import { useSettingsStore } from "../../lib/settings/state";

export default function SettingsCog() {
  const setModalOpen = useSettingsStore((s) => s.setModalOpen);

  return (
    <button
      onClick={() => setModalOpen(true)}
      aria-label="Settings"
      title="Settings (⌘,)"
      style={{
        width: 18,
        height: 18,
        padding: 0,
        background: "transparent",
        border: "none",
        cursor: "pointer",
        opacity: 0.6,
        transition: "opacity 100ms ease",
        fontSize: 15,
        lineHeight: 1,
      }}
      onMouseEnter={(e) => ((e.currentTarget as HTMLButtonElement).style.opacity = "1")}
      onMouseLeave={(e) => ((e.currentTarget as HTMLButtonElement).style.opacity = "0.6")}
    >
      ⚙
    </button>
  );
}
