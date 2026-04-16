import { useEffect, useState } from "react";
import Assistant from "./components/Assistant/Assistant";
import Today from "./components/Today/Today";
import SettingsModal from "./components/Settings/SettingsModal";
import Sidebar from "./components/Nav/Sidebar";
import ChoresView from "./components/Chores/ChoresView";
import TimeBlocksView from "./components/TimeBlocks/TimeBlocksView";
import LedgerView from "./components/Ledger/LedgerView";
import Wizard from "./components/Wizard/Wizard";
import { useSettingsStore } from "./lib/settings/state";
import { useNavStore } from "./lib/nav";
import { useWizardStore } from "./lib/wizard/state";
import { settingGet } from "./lib/foundation/ipc";

const ONBOARDING_KEY = "onboarding_completed";

const shellStyle: React.CSSProperties = {
  display: "flex",
  height: "100vh",
  width: "100vw",
};

const mainStyle: React.CSSProperties = {
  flex: 1,
  overflow: "auto",
  position: "relative",
  zIndex: 0,
};

export default function App() {
  const setModalOpen = useSettingsStore((s) => s.setModalOpen);
  const modalOpen = useSettingsStore((s) => s.modalOpen);
  const view = useNavStore((s) => s.view);
  const showWizard = useWizardStore((s) => s.show);
  const setShowWizard = useWizardStore((s) => s.setShow);
  const [checking, setChecking] = useState(true);

  useEffect(() => {
    void (async () => {
      const val = await settingGet(ONBOARDING_KEY);
      setShowWizard(val !== "1");
      setChecking(false);
    })();
  }, [setShowWizard]);

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        setModalOpen(!modalOpen);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [modalOpen, setModalOpen]);

  if (checking) {
    return <div style={{ padding: 40, color: "#888" }}>Loading…</div>;
  }

  if (showWizard) {
    return <Wizard />;
  }

  return (
    <>
      <div style={shellStyle}>
        <Sidebar />
        <div style={mainStyle}>
          {view === "today" && <Today />}
          {view === "chores" && <ChoresView />}
          {view === "timeblocks" && <TimeBlocksView />}
          {view === "ledger" && <LedgerView />}
        </div>
      </div>
      <Assistant />
      <SettingsModal />
    </>
  );
}
