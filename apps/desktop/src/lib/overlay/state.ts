// Shared overlay-visibility store.
//
// Any right-side drawer or modal that covers content should call `openOverlay`
// on mount and `closeOverlay` on unmount. The assistant (avatar, pill, bubbles)
// reads `count` to minimize itself so it doesn't hide the drawer's content.

import { useEffect } from "react";
import { create } from "zustand";

interface OverlayStore {
  count: number;
  openOverlay: () => void;
  closeOverlay: () => void;
}

export const useOverlayStore = create<OverlayStore>((set) => ({
  count: 0,
  openOverlay: () => set((s) => ({ count: s.count + 1 })),
  closeOverlay: () => set((s) => ({ count: Math.max(0, s.count - 1) })),
}));

/// Hook: call from inside a drawer/modal component body. Auto-increments on
/// mount, decrements on unmount. Safe against double-mount in StrictMode.
export function useOverlay(active: boolean = true) {
  useEffect(() => {
    if (!active) return;
    useOverlayStore.getState().openOverlay();
    return () => {
      useOverlayStore.getState().closeOverlay();
    };
  }, [active]);
}
