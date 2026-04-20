import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup, act } from "@testing-library/react";
import { EphemeralLog } from "../EphemeralLog";

describe("EphemeralLog", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  it("renders nothing when exchanges are empty", () => {
    const { container } = render(
      <EphemeralLog exchanges={[]} onExpand={vi.fn()} />,
    );
    expect(container.textContent).toBe("");
  });

  it("does not render on initial mount even when exchanges already exist", () => {
    const { container } = render(
      <EphemeralLog
        exchanges={[{ userText: "historical q", assistantText: "historical a", key: 1 }]}
        onExpand={vi.fn()}
      />,
    );
    expect(container.textContent).toBe("");
  });

  it("renders only the latest Manor response when a new exchange arrives", () => {
    const { rerender } = render(
      <EphemeralLog
        exchanges={[{ userText: "historical q", assistantText: "historical a", key: 1 }]}
        onExpand={vi.fn()}
      />,
    );
    rerender(
      <EphemeralLog
        exchanges={[
          { userText: "latest question", assistantText: "latest answer", key: 2 },
          { userText: "historical q", assistantText: "historical a", key: 1 },
        ]}
        onExpand={vi.fn()}
      />,
    );
    expect(screen.getByText("latest answer")).toBeInTheDocument();
    // User prompt + older exchange should not appear.
    expect(screen.queryByText("latest question")).toBeNull();
    expect(screen.queryByText("historical q")).toBeNull();
    expect(screen.queryByText("historical a")).toBeNull();
  });

  it("fades out after the configured delay", () => {
    const { rerender } = render(
      <EphemeralLog exchanges={[]} onExpand={vi.fn()} fadeDelayMs={5000} />,
    );
    rerender(
      <EphemeralLog
        exchanges={[{ userText: "hi", assistantText: "hello", key: 1 }]}
        onExpand={vi.fn()}
        fadeDelayMs={5000}
      />,
    );
    expect(screen.getByText("hello")).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(5001);
    });
    expect(screen.queryByText("hello")).toBeNull();
  });

  it("resets the fade timer when a new exchange arrives", () => {
    const { rerender } = render(
      <EphemeralLog exchanges={[]} onExpand={vi.fn()} fadeDelayMs={5000} />,
    );
    rerender(
      <EphemeralLog
        exchanges={[{ userText: "first", assistantText: "first-reply", key: 1 }]}
        onExpand={vi.fn()}
        fadeDelayMs={5000}
      />,
    );
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    rerender(
      <EphemeralLog
        exchanges={[{ userText: "second", assistantText: "second-reply", key: 2 }]}
        onExpand={vi.fn()}
        fadeDelayMs={5000}
      />,
    );
    act(() => {
      vi.advanceTimersByTime(3000); // total 6000ms from first — timer reset at 3000
    });
    expect(screen.getByText("second-reply")).toBeInTheDocument();
  });

  it("calls onExpand when the log is clicked", () => {
    const onExpand = vi.fn();
    const { rerender } = render(
      <EphemeralLog exchanges={[]} onExpand={onExpand} />,
    );
    rerender(
      <EphemeralLog
        exchanges={[{ userText: "hi", assistantText: "hello", key: 1 }]}
        onExpand={onExpand}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Expand conversation history/ }));
    expect(onExpand).toHaveBeenCalledTimes(1);
  });
});
