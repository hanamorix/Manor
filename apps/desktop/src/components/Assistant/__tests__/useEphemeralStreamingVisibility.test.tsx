import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useEphemeralStreamingVisibility } from "../useEphemeralStreamingVisibility";

describe("useEphemeralStreamingVisibility", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("becomes visible on Started, hides after fadeMs from Done", () => {
    const { result } = renderHook(() => useEphemeralStreamingVisibility(10_000));
    expect(result.current.visible).toBe(false);

    let req: ReturnType<typeof result.current.startRequest>;
    act(() => { req = result.current.startRequest(); req.onStarted(); });
    expect(result.current.visible).toBe(true);

    act(() => { req.onDone(); });
    expect(result.current.visible).toBe(true);  // still visible during fade

    act(() => { vi.advanceTimersByTime(10_000); });
    expect(result.current.visible).toBe(false);
  });

  it("late Done for a superseded request does not hide current stream", () => {
    const { result } = renderHook(() => useEphemeralStreamingVisibility(10_000));

    let req1: ReturnType<typeof result.current.startRequest>;
    let req2: ReturnType<typeof result.current.startRequest>;
    act(() => {
      req1 = result.current.startRequest();
      req1.onStarted();
      req1.onDone(); // schedules 10s fade
    });

    act(() => { vi.advanceTimersByTime(100); });

    act(() => {
      req2 = result.current.startRequest();
      req2.onStarted();
    });
    expect(result.current.visible).toBe(true);

    // req1's fade timer fires — must be a no-op because req2 is now current.
    act(() => { vi.advanceTimersByTime(11_000); });
    expect(result.current.visible).toBe(true);
  });

  it("hide() clears the visible state and cancels the pending timer", () => {
    const { result } = renderHook(() => useEphemeralStreamingVisibility(10_000));

    let req: ReturnType<typeof result.current.startRequest>;
    act(() => { req = result.current.startRequest(); req.onStarted(); });
    expect(result.current.visible).toBe(true);

    act(() => { result.current.hide(); });
    expect(result.current.visible).toBe(false);

    act(() => { req.onDone(); });
    act(() => { vi.advanceTimersByTime(10_000); });
    // onDone() on a stale request should not re-show.
    expect(result.current.visible).toBe(false);
  });
});
