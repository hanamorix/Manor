import { useEffect } from "react";
import Assistant from "./components/Assistant/Assistant";
import Today from "./components/Today/Today";
import SettingsModal from "./components/Settings/SettingsModal";
import Sidebar from "./components/Nav/Sidebar";
import ChoresView from "./components/Chores/ChoresView";
import TimeBlocksView from "./components/TimeBlocks/TimeBlocksView";
import { useSettingsStore } from "./lib/settings/state";
import { useNavStore } from "./lib/nav";

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

  return (
    <>
      <div style={shellStyle}>
        <Sidebar />
        <div style={mainStyle}>
          {view === "today" && <Today />}
          {view === "chores" && <ChoresView />}
          {view === "timeblocks" && <TimeBlocksView />}
        </div>
      </div>
      <Assistant />
      <SettingsModal />
    </>
  );
}
