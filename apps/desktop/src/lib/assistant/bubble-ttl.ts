export interface TtlTimer {
  start: () => void;
  pause: () => void;
  resumeWith: (remainingMs: number) => void;
  cancel: () => void;
}

export function createTtlTimer(ttlMs: number, onExpire: () => void): TtlTimer {
  let handle: ReturnType<typeof setTimeout> | null = null;

  const clear = () => {
    if (handle !== null) {
      clearTimeout(handle);
      handle = null;
    }
  };

  return {
    start() {
      clear();
      handle = setTimeout(onExpire, ttlMs);
    },
    pause() {
      clear();
    },
    resumeWith(remainingMs) {
      clear();
      handle = setTimeout(onExpire, remainingMs);
    },
    cancel() {
      clear();
    },
  };
}
