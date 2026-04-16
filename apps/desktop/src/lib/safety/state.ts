import { create } from "zustand";
import type { BackupEntry, TrashEntry } from "./ipc";

interface SafetyStore {
  trashEntries: TrashEntry[];
  backups: BackupEntry[];
  setTrashEntries: (t: TrashEntry[]) => void;
  setBackups: (b: BackupEntry[]) => void;
}

export const useSafetyStore = create<SafetyStore>((set) => ({
  trashEntries: [],
  backups: [],
  setTrashEntries: (t) => set({ trashEntries: t }),
  setBackups: (b) => set({ backups: b }),
}));
