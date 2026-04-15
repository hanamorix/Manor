import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";
import { listTasks } from "../../lib/today/ipc";
import { AVATAR_FOOTPRINT_PX } from "../../lib/layout";
import HeaderCard from "./HeaderCard";
import EventsCard from "./EventsCard";
import TasksCard from "./TasksCard";
import ProposalBanner from "./ProposalBanner";
import Toast from "./Toast";

export default function Today() {
  const setTasks = useTodayStore((s) => s.setTasks);

  useEffect(() => {
    void listTasks().then(setTasks);
  }, [setTasks]);

  return (
    <>
      <main
        style={{
          maxWidth: 760,
          margin: "0 auto",
          padding: `24px 24px ${AVATAR_FOOTPRINT_PX}px 24px`,
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}
      >
        <ProposalBanner />
        <HeaderCard />
        <EventsCard />
        <TasksCard />
      </main>
      <Toast />
    </>
  );
}
