import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";
import { listTasks, listEventsToday } from "../../lib/today/ipc";
import { useChoresStore } from "../../lib/chores/state";
import { listChoresDueToday } from "../../lib/chores/ipc";
import { useTimeBlocksStore } from "../../lib/timeblocks/state";
import { listBlocksToday } from "../../lib/timeblocks/ipc";
import { AVATAR_FOOTPRINT_PX } from "../../lib/layout";
import HeaderCard from "./HeaderCard";
import EventsCard from "./EventsCard";
import TimeBlocksCard from "./TimeBlocksCard";
import ChoresCard from "./ChoresCard";
import TasksCard from "./TasksCard";
import ProposalBanner from "./ProposalBanner";
import RenewalAlertsCard from "./RenewalAlertsCard";
import SampleDataBanner from "./SampleDataBanner";
import Toast from "./Toast";

export default function Today() {
  const setTasks = useTodayStore((s) => s.setTasks);
  const setEvents = useTodayStore((s) => s.setEvents);
  const setChoresDueToday = useChoresStore((s) => s.setChoresDueToday);
  const setTodayBlocks = useTimeBlocksStore((s) => s.setTodayBlocks);

  useEffect(() => {
    void listTasks().then(setTasks);
    void listEventsToday().then(setEvents);
    void listChoresDueToday().then(setChoresDueToday);
    void listBlocksToday().then(setTodayBlocks);
  }, [setTasks, setEvents, setChoresDueToday, setTodayBlocks]);

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
        <RenewalAlertsCard />
        <SampleDataBanner />
        <HeaderCard />
        <EventsCard />
        <TimeBlocksCard />
        <ChoresCard />
        <TasksCard />
      </main>
      <Toast />
    </>
  );
}
