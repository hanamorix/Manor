import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import ChatHistoryPanel from "../ChatHistoryPanel";
import type { Message } from "../../../lib/assistant/ipc";

const msg = (id: number, role: "user" | "assistant", content: string): Message => ({
  id,
  conversation_id: 1,
  role,
  content,
  created_at: id,
  seen: true,
  proposal_id: null,
});

describe("ChatHistoryPanel", () => {
  afterEach(() => cleanup());

  it("renders nothing when isOpen is false", () => {
    const { container } = render(
      <ChatHistoryPanel
        isOpen={false}
        messages={[msg(1, "user", "hi")]}
        onCollapse={vi.fn()}
      />,
    );
    expect(container.textContent).toBe("");
  });

  it("renders messages when open", () => {
    render(
      <ChatHistoryPanel
        isOpen
        messages={[
          msg(1, "user", "hello manor"),
          msg(2, "assistant", "hello hana"),
        ]}
        onCollapse={vi.fn()}
      />,
    );
    expect(screen.getByText("hello manor")).toBeInTheDocument();
    expect(screen.getByText("hello hana")).toBeInTheDocument();
  });

  it("shows empty placeholder when messages is empty", () => {
    render(
      <ChatHistoryPanel isOpen messages={[]} onCollapse={vi.fn()} />,
    );
    expect(screen.getByText(/No conversation yet/)).toBeInTheDocument();
  });

  it("calls onCollapse when the ⤡ icon is clicked", () => {
    const onCollapse = vi.fn();
    render(
      <ChatHistoryPanel
        isOpen
        messages={[msg(1, "user", "hi")]}
        onCollapse={onCollapse}
      />,
    );
    fireEvent.click(screen.getByLabelText("Collapse conversation history"));
    expect(onCollapse).toHaveBeenCalledTimes(1);
  });

  it("calls onCollapse on Escape key", () => {
    const onCollapse = vi.fn();
    render(
      <ChatHistoryPanel
        isOpen
        messages={[msg(1, "user", "hi")]}
        onCollapse={onCollapse}
      />,
    );
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onCollapse).toHaveBeenCalledTimes(1);
  });

  it("calls onCollapse on outside mousedown", () => {
    const onCollapse = vi.fn();
    render(
      <>
        <div data-testid="outside">outside target</div>
        <ChatHistoryPanel
          isOpen
          messages={[msg(1, "user", "hi")]}
          onCollapse={onCollapse}
        />
      </>,
    );
    fireEvent.mouseDown(screen.getByTestId("outside"));
    expect(onCollapse).toHaveBeenCalledTimes(1);
  });

  it("does NOT call onCollapse when clicking inside the panel", () => {
    const onCollapse = vi.fn();
    render(
      <ChatHistoryPanel
        isOpen
        messages={[msg(1, "user", "hi inside")]}
        onCollapse={onCollapse}
      />,
    );
    fireEvent.mouseDown(screen.getByText("hi inside"));
    expect(onCollapse).not.toHaveBeenCalled();
  });
});
