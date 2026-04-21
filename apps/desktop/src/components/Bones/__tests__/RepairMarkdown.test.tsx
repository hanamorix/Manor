import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup, waitFor } from "@testing-library/react";
import { RepairMarkdown } from "../RepairMarkdown";

vi.mock("@tauri-apps/plugin-shell");

import { open as openUrl } from "@tauri-apps/plugin-shell";
const mockOpenUrl = openUrl as ReturnType<typeof vi.fn>;

describe("RepairMarkdown link safety", () => {
  beforeEach(() => mockOpenUrl.mockClear());
  afterEach(() => cleanup());

  it("opens https links via the shell plugin", async () => {
    render(<RepairMarkdown body="[Safe](https://example.com)" />);
    fireEvent.click(screen.getByText("Safe"));
    await waitFor(() => {
      expect(mockOpenUrl).toHaveBeenCalledWith("https://example.com");
    });
  });

  it("opens http links via the shell plugin", async () => {
    render(<RepairMarkdown body="[Safe](http://example.com)" />);
    fireEvent.click(screen.getByText("Safe"));
    await waitFor(() => {
      expect(mockOpenUrl).toHaveBeenCalledWith("http://example.com");
    });
  });

  it("does NOT open file:// links", async () => {
    render(<RepairMarkdown body="[Local](file:///etc/passwd)" />);
    fireEvent.click(screen.getByText("Local"));
    await new Promise((r) => setTimeout(r, 100)); // Give it time to call if it would
    expect(mockOpenUrl).not.toHaveBeenCalled();
  });

  it("does NOT open javascript: links", async () => {
    render(<RepairMarkdown body="[Bad](javascript:alert(1))" />);
    fireEvent.click(screen.getByText("Bad"));
    await new Promise((r) => setTimeout(r, 100));
    expect(mockOpenUrl).not.toHaveBeenCalled();
  });

  it("does NOT open custom-scheme links", async () => {
    render(<RepairMarkdown body="[Vs](vscode://open?file=x)" />);
    fireEvent.click(screen.getByText("Vs"));
    await new Promise((r) => setTimeout(r, 100));
    expect(mockOpenUrl).not.toHaveBeenCalled();
  });
});
