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

  it("renders only the latest Manor response (not the user prompt)", () => {
    render(
      <EphemeralLog
        exchanges={[
          { userText: "latest question", assistantText: "latest answer", key: 2 },
          { userText: "older question", assistantText: "older answer", key: 1 },
        ]}
        onExpand={vi.fn()}
      />,
    );
    expect(screen.getByText("latest answer")).toBeInTheDocument();
    // User prompts + older exchanges should not appear — just the latest response.
    expect(screen.queryByText("latest question")).toBeNull();
    expect(screen.queryByText("older question")).toBeNull();
    expect(screen.queryByText("older answer")).toBeNull();
  });

  it("fades out after the configured delay", () => {
    render(
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
