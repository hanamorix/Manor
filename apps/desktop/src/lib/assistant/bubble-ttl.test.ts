import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createTtlTimer } from "./bubble-ttl";

describe("createTtlTimer", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("fires onExpire after ttlMs", () => {
    const onExpire = vi.fn();
    createTtlTimer(3000, onExpire).start();

    vi.advanceTimersByTime(2999);
    expect(onExpire).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(onExpire).toHaveBeenCalledOnce();
  });

  it("pauses and resumes with remaining 3s after mouse-out", () => {
    const onExpire = vi.fn();
    const timer = createTtlTimer(7000, onExpire);
    timer.start();

    vi.advanceTimersByTime(2000);
    timer.pause();

    vi.advanceTimersByTime(10_000); // no firing while paused
    expect(onExpire).not.toHaveBeenCalled();

    timer.resumeWith(3000); // spec §6.2: resumes with 3s remaining
    vi.advanceTimersByTime(2999);
    expect(onExpire).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(onExpire).toHaveBeenCalledOnce();
  });

  it("cancel prevents future firing", () => {
    const onExpire = vi.fn();
    const timer = createTtlTimer(1000, onExpire);
    timer.start();
    timer.cancel();
    vi.advanceTimersByTime(5000);
    expect(onExpire).not.toHaveBeenCalled();
  });
});
