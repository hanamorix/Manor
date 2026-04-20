import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { TroubleshootBlock } from "../TroubleshootBlock";
import { useRepairStore } from "../../../lib/repair/state";

vi.mock("../../../lib/repair/state", () => ({
  useRepairStore: vi.fn(),
}));

// react-markdown would break jsdom tests; stub the wrapper.
vi.mock("../RepairMarkdown", () => ({
  RepairMarkdown: ({ body }: { body: string }) => <div data-testid="md">{body}</div>,
}));

// plugin-shell isn't available in jsdom.
vi.mock("@tauri-apps/plugin-shell", () => ({
  open: vi.fn(),
}));

describe("TroubleshootBlock", () => {
  const loadForAsset = vi.fn();
  const invalidateAsset = vi.fn();
  const searchOllama = vi.fn().mockResolvedValue({
    note: null,
    sources: [],
    video_sources: [],
    empty_or_failed: true,
  });
  const searchClaude = vi.fn();
  const deleteNote = vi.fn();
  const clearLastOutcome = vi.fn();

  function mockStore(overrides: Record<string, unknown> = {}) {
    (useRepairStore as unknown as ReturnType<typeof vi.fn>).mockImplementation(() => ({
      notesByAsset: {},
      lastOutcomeByAsset: {},
      lastSymptomByAsset: {},
      searchStatus: { kind: "idle" },
      loadForAsset,
      invalidateAsset,
      searchOllama,
      searchClaude,
      deleteNote,
      clearLastOutcome,
      ...overrides,
    }));
  }

  beforeEach(() => {
    mockStore();
    searchOllama.mockClear();
    searchClaude.mockClear();
    loadForAsset.mockClear();
  });

  afterEach(() => cleanup());

  it("renders the search input + header + subtitle", () => {
    render(<TroubleshootBlock assetId="a1" />);
    expect(screen.getByText("Troubleshoot")).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/What's wrong/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Search/ })).toBeInTheDocument();
  });

  it("disables Search button when symptom is empty", () => {
    render(<TroubleshootBlock assetId="a1" />);
    const btn = screen.getByRole("button", { name: /Search/ });
    expect(btn).toBeDisabled();
  });

  it("enforces 200-char maxLength on symptom input", () => {
    render(<TroubleshootBlock assetId="a1" />);
    const input = screen.getByPlaceholderText(/What's wrong/) as HTMLInputElement;
    expect(input.maxLength).toBe(200);
  });

  it("disables Search and shows 'Asking qwen2.5…' while searching", () => {
    mockStore({ searchStatus: { kind: "searching", tier: "ollama" } });
    render(<TroubleshootBlock assetId="a1" />);
    expect(screen.getByRole("button", { name: /Asking qwen2.5/ })).toBeDisabled();
  });

  it("shows an error band when searchStatus is error", () => {
    mockStore({ searchStatus: { kind: "error", message: "Local model isn't running" } });
    render(<TroubleshootBlock assetId="a1" />);
    expect(screen.getByText("Local model isn't running")).toBeInTheDocument();
  });

  it("calls searchOllama on form submit with non-empty symptom", () => {
    render(<TroubleshootBlock assetId="a1" />);
    const input = screen.getByPlaceholderText(/What's wrong/);
    fireEvent.change(input, { target: { value: "won't drain" } });
    fireEvent.submit(input.closest("form")!);
    expect(searchOllama).toHaveBeenCalledWith("a1", "won't drain");
  });

  it("trims whitespace from symptom before submitting", () => {
    render(<TroubleshootBlock assetId="a1" />);
    const input = screen.getByPlaceholderText(/What's wrong/);
    fireEvent.change(input, { target: { value: "   hello   " } });
    fireEvent.submit(input.closest("form")!);
    expect(searchOllama).toHaveBeenCalledWith("a1", "hello");
  });
});
