import { useEffect } from "react";
import Assistant from "./components/Assistant/Assistant";
import Today from "./components/Today/Today";
import SettingsModal from "./components/Settings/SettingsModal";
import { useSettingsStore } from "./lib/settings/state";

export default function App() {
  const setModalOpen = useSettingsStore((s) => s.setModalOpen);
  const modalOpen = useSettingsStore((s) => s.modalOpen);

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
      <Today />
      <Assistant />
      <SettingsModal />
    </>
  );
}
