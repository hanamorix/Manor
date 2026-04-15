import { useEffect, useRef } from "react";
import { useSettingsStore } from "../../lib/settings/state";
import Tabs from "./Tabs";
import CalendarsTab from "./CalendarsTab";

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
          position: "fixed", inset: 0, background: "rgba(0,0,0,0.25)",
          backdropFilter: "blur(2px)", zIndex: 1200,
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
          borderRadius: 14,
          boxShadow: "var(--shadow-lg)",
          zIndex: 1201,
          display: "flex", flexDirection: "column",
          animation: "settingsIn 200ms ease-out",
          outline: "none",
        }}
      >
        <header
          style={{
            padding: "12px 16px",
            borderBottom: "1px solid var(--hairline)",
            display: "flex", alignItems: "center", justifyContent: "space-between",
            fontWeight: 700, fontSize: 14,
          }}
        >
          <span>Settings</span>
          <button
            onClick={() => setModalOpen(false)}
            aria-label="Close"
            style={{
              width: 22, height: 22, borderRadius: "50%",
              background: "rgba(0,0,0,0.06)",
              border: "none",
              fontSize: 14, lineHeight: 1, cursor: "pointer",
              color: "rgba(0,0,0,0.55)",
              display: "flex", alignItems: "center", justifyContent: "center",
            }}
          >
            ×
          </button>
        </header>
        <Tabs />
        <div style={{ flex: 1, overflowY: "auto" }}>
          {activeTab === "calendars" && <CalendarsTab />}
        </div>
      </div>
    </>
  );
}
