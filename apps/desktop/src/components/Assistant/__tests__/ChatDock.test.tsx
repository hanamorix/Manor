import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import ChatDock from "../ChatDock";

describe("ChatDock", () => {
  afterEach(() => cleanup());

  it("renders the placeholder", () => {
    render(<ChatDock onSubmit={vi.fn()} onExpand={vi.fn()} />);
    expect(screen.getByPlaceholderText("Say something…")).toBeInTheDocument();
  });

  it("submits on Enter and clears the input", () => {
    const onSubmit = vi.fn();
    render(<ChatDock onSubmit={onSubmit} onExpand={vi.fn()} />);
    const input = screen.getByPlaceholderText("Say something…") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "hello" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).toHaveBeenCalledWith("hello");
    expect(input.value).toBe("");
  });

  it("does not submit when the trimmed value is empty", () => {
    const onSubmit = vi.fn();
    render(<ChatDock onSubmit={onSubmit} onExpand={vi.fn()} />);
    const input = screen.getByPlaceholderText("Say something…");
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("calls onExpand when the expand icon is clicked", () => {
    const onExpand = vi.fn();
    render(<ChatDock onSubmit={vi.fn()} onExpand={onExpand} />);
    fireEvent.click(screen.getByLabelText("Expand conversation history"));
    expect(onExpand).toHaveBeenCalledTimes(1);
  });

  it("blurs the input on Escape", () => {
    render(<ChatDock onSubmit={vi.fn()} onExpand={vi.fn()} />);
    const input = screen.getByPlaceholderText("Say something…") as HTMLInputElement;
    input.focus();
    expect(document.activeElement).toBe(input);
    fireEvent.keyDown(input, { key: "Escape" });
    expect(document.activeElement).not.toBe(input);
  });
});
