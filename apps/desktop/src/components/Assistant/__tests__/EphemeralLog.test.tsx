import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { EphemeralLog } from "../EphemeralLog";

describe("EphemeralLog", () => {
  afterEach(() => cleanup());

  it("renders nothing when not visible", () => {
    const { container } = render(
      <EphemeralLog
        exchanges={[{ userText: "q", assistantText: "a", key: 1 }]}
        onExpand={vi.fn()}
        visible={false}
      />,
    );
    expect(container.textContent).toBe("");
  });

  it("renders nothing when exchanges are empty, even if visible", () => {
    const { container } = render(
      <EphemeralLog exchanges={[]} onExpand={vi.fn()} visible />,
    );
    expect(container.textContent).toBe("");
  });

  it("renders only the latest assistant reply when visible", () => {
    render(
      <EphemeralLog
        exchanges={[
          { userText: "latest question", assistantText: "latest answer", key: 2 },
          { userText: "older q", assistantText: "older a", key: 1 },
        ]}
        onExpand={vi.fn()}
        visible
      />,
    );
    expect(screen.getByText("latest answer")).toBeInTheDocument();
    expect(screen.queryByText("latest question")).toBeNull();
    expect(screen.queryByText("older a")).toBeNull();
    expect(screen.queryByText("older q")).toBeNull();
  });

  it("calls onExpand when the log is clicked", () => {
    const onExpand = vi.fn();
    render(
      <EphemeralLog
        exchanges={[{ userText: "hi", assistantText: "hello", key: 1 }]}
        onExpand={onExpand}
        visible
      />,
    );
    fireEvent.click(
      screen.getByRole("button", { name: /Expand conversation history/ }),
    );
    expect(onExpand).toHaveBeenCalledTimes(1);
  });
});
