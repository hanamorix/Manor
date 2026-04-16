import { create } from "zustand";

type StepId = 1 | 2 | 3 | 4;

interface WizardStore {
  show: boolean;
  step: StepId;
  setShow: (show: boolean) => void;
  setStep: (step: StepId) => void;
  advance: () => void;
  reset: () => void;
}

export const useWizardStore = create<WizardStore>((set) => ({
  show: false,
  step: 1,
  setShow: (show) => set({ show }),
  setStep: (step) => set({ step }),
  advance: () => set((s) => ({ step: Math.min(4, s.step + 1) as StepId })),
  reset: () => set({ step: 1 }),
}));
