import { Suspense, lazy, useEffect, useState } from "react";
import Assistant from "./components/Assistant/Assistant";
import SettingsModal from "./components/Settings/SettingsModal";
import Sidebar from "./components/Nav/Sidebar";
import { useSettingsStore } from "./lib/settings/state";
import { useNavStore } from "./lib/nav";
import { useWizardStore } from "./lib/wizard/state";
import { settingGet } from "./lib/foundation/ipc";

const Today = lazy(() => import("./components/Today/Today"));
const ChoresView = lazy(() => import("./components/Chores/ChoresView"));
const TimeBlocksView = lazy(() => import("./components/TimeBlocks/TimeBlocksView"));
const LedgerView = lazy(() => import("./components/Ledger/LedgerView"));
const HearthTab = lazy(() =>
  import("./components/Hearth/HearthTab").then((m) => ({ default: m.HearthTab })),
);
const BonesTab = lazy(() =>
  import("./components/Bones/BonesTab").then((m) => ({ default: m.BonesTab })),
);
const Wizard = lazy(() => import("./components/Wizard/Wizard"));

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

const viewFallback = (
  <div style={{ padding: 40, color: "var(--ink-soft)" }}>Loading…</div>
);

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
    return <div style={{ padding: 40, color: "var(--ink-soft)" }}>Loading…</div>;
  }

  if (showWizard) {
    return (
      <Suspense fallback={viewFallback}>
        <Wizard />
      </Suspense>
    );
  }

  return (
    <>
      <div style={shellStyle}>
        <Sidebar />
        <div style={mainStyle}>
          <Suspense fallback={viewFallback}>
            {view === "today" && <Today />}
            {view === "chores" && <ChoresView />}
            {view === "timeblocks" && <TimeBlocksView />}
            {view === "ledger" && <LedgerView />}
            {view === "bones" && <BonesTab />}
            {view === "hearth" && <HearthTab />}
          </Suspense>
        </div>
      </div>
      <Assistant />
      <SettingsModal />
    </>
  );
}
