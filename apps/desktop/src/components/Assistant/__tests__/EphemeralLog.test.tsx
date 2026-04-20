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

  it("renders last two exchanges with user + Manor labels", () => {
    render(
      <EphemeralLog
        exchanges={[
          { userText: "latest question", assistantText: "latest answer", key: 2 },
          { userText: "older question", assistantText: "older answer", key: 1 },
        ]}
        onExpand={vi.fn()}
      />,
    );
    expect(screen.getByText("latest question")).toBeInTheDocument();
    expect(screen.getByText("latest answer")).toBeInTheDocument();
    expect(screen.getByText("older question")).toBeInTheDocument();
  });

  it("caps at 2 exchanges even if more are passed", () => {
    render(
      <EphemeralLog
        exchanges={[
          { userText: "q3", assistantText: "a3", key: 3 },
          { userText: "q2", assistantText: "a2", key: 2 },
          { userText: "q1", assistantText: "a1", key: 1 },
        ]}
        onExpand={vi.fn()}
      />,
    );
    expect(screen.queryByText("q1")).toBeNull();
  });

  it("fades out after the configured delay", () => {
    render(
      <EphemeralLog
        exchanges={[{ userText: "hi", assistantText: "hello", key: 1 }]}
        onExpand={vi.fn()}
        fadeDelayMs={5000}
      />,
    );
    expect(screen.getByText("hi")).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(5001);
    });
    expect(screen.queryByText("hi")).toBeNull();
  });

  it("resets the fade timer when a new exchange arrives", () => {
    const { rerender } = render(
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
    expect(screen.getByText("second")).toBeInTheDocument();
  });

  it("calls onExpand when the log is clicked", () => {
    const onExpand = vi.fn();
    render(
      <EphemeralLog
        exchanges={[{ userText: "hi", assistantText: "hello", key: 1 }]}
        onExpand={onExpand}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Expand conversation history/ }));
    expect(onExpand).toHaveBeenCalledTimes(1);
  });
});
