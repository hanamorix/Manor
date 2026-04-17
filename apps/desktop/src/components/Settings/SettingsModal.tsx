import { useEffect, useRef } from "react";
import { useSettingsStore } from "../../lib/settings/state";
import Tabs from "./Tabs";
import CalendarsTab from "./CalendarsTab";
import DataBackupTab from "./DataBackupTab";
import AiTab from "./AiTab";
import HouseholdTab from "./HouseholdTab";
import AboutTab from "./AboutTab";

export default function SettingsModal() {
  const modalOpen = useSettingsStore((s) => s.modalOpen);
  const setModalOpen = useSettingsStore((s) => s.setModalOpen);
  const activeTab = useSettingsStore((s) => s.activeTab);
  const modalRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!modalOpen) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") setModalOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [modalOpen, setModalOpen]);

  useEffect(() => {
    if (modalOpen) modalRef.current?.focus();
  }, [modalOpen]);

  if (!modalOpen) return null;

  return (
    <>
      <div
        onClick={() => setModalOpen(false)}
        style={{
          position: "fixed", inset: 0, background: "var(--scrim)",
          zIndex: 1200,
        }}
      />
      <div
        ref={modalRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        style={{
          position: "fixed", left: "50%", top: "50%",
          transform: "translate(-50%, -50%)",
          width: 540, height: 440,
          background: "var(--paper)",
          borderRadius: "var(--radius-lg)",
          boxShadow: "var(--shadow-lg)",
          zIndex: 1201,
          display: "flex", flexDirection: "column",
          animation: "settingsIn 200ms ease-out",
        }}
      >
        <header
          style={{
            padding: "12px 16px",
            borderBottom: "1px solid var(--hairline)",
            display: "flex", alignItems: "center", justifyContent: "space-between",
            fontWeight: 600, fontSize: 14,
          }}
        >
          <span>Settings</span>
          <button
            onClick={() => setModalOpen(false)}
            aria-label="Close"
            style={{
              width: 22, height: 22, borderRadius: "50%",
              background: "var(--hairline)",
              border: "none",
              fontSize: 14, lineHeight: 1, cursor: "pointer",
              color: "var(--ink-soft)",
              display: "flex", alignItems: "center", justifyContent: "center",
            }}
          >
            ×
          </button>
        </header>
        <Tabs />
        <div style={{ flex: 1, overflowY: "auto" }}>
          {activeTab === "data" && <DataBackupTab />}
          {activeTab === "ai" && <AiTab />}
          {activeTab === "calendars" && <CalendarsTab />}
          {activeTab === "household" && <HouseholdTab />}
          {activeTab === "about" && <AboutTab />}
        </div>
      </div>
    </>
  );
}
