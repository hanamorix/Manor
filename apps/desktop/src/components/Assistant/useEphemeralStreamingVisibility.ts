import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Show an ephemeral banner while a streaming request is active; fade it
 * after `fadeMs` once the request finishes. Scoped per-request so a late
 * Done/fade-timer from a superseded request cannot hide a newer stream.
 */
export function useEphemeralStreamingVisibility(fadeMs: number) {
  const [visible, setVisible] = useState(false);
  const currentRequestIdRef = useRef(0);
  const timerRef = useRef<number | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  useEffect(() => () => clearTimer(), [clearTimer]);

  const startRequest = useCallback(() => {
    currentRequestIdRef.current += 1;
    const myId = currentRequestIdRef.current;
    return {
      onStarted: () => {
        if (myId !== currentRequestIdRef.current) return;
        clearTimer();
        setVisible(true);
      },
      onDone: () => {
        if (myId !== currentRequestIdRef.current) return;
        clearTimer();
        timerRef.current = window.setTimeout(() => {
          if (myId !== currentRequestIdRef.current) return;
          setVisible(false);
          timerRef.current = null;
        }, fadeMs);
      },
    };
  }, [clearTimer, fadeMs]);

  const hide = useCallback(() => {
    clearTimer();
    setVisible(false);
    // Invalidate the current request so a late onDone/onStarted from it won't
    // re-show.
    currentRequestIdRef.current += 1;
  }, [clearTimer]);

  return { visible, startRequest, hide };
}
