import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { PendingProposalsBlock } from "../PendingProposalsBlock";
import { usePdfExtractStore } from "../../../lib/pdf_extract/state";
import { useMaintenanceStore } from "../../../lib/maintenance/state";

vi.mock("../../../lib/pdf_extract/state", () => ({
  usePdfExtractStore: vi.fn(),
}));

vi.mock("../../../lib/maintenance/state", () => ({
  useMaintenanceStore: vi.fn(),
}));

// Stub ScheduleDrawer; its deps pull in Tauri APIs that don't work in jsdom.
vi.mock("../DueSoon/ScheduleDrawer", () => ({
  ScheduleDrawer: ({
    proposalId,
    onSaved,
  }: {
    proposalId?: number;
    onSaved: () => void;
  }) => (
    <div data-testid="schedule-drawer">
      <span>proposalId={proposalId}</span>
      <button onClick={onSaved}>Drawer Save</button>
    </div>
  ),
}));

const makeProposal = (id: number, task: string, interval: number) => ({
  id,
  kind: "add_maintenance_schedule",
  rationale: `rationale-${id}`,
  diff: JSON.stringify({
    asset_id: "a1",
    task,
    interval_months: interval,
    notes: "",
    source_attachment_uuid: "att-uuid",
    tier: "ollama",
  }),
  status: "pending",
  proposed_at: 1,
  applied_at: null,
  skill: "pdf_extract",
});

describe("PendingProposalsBlock", () => {
  const loadForAsset = vi.fn();
  const approveAsIs = vi.fn().mockResolvedValue(undefined);
  const reject = vi.fn().mockResolvedValue(undefined);
  const loadSchedules = vi.fn();

  function mockStores(proposals: unknown[]) {
    (usePdfExtractStore as unknown as ReturnType<typeof vi.fn>).mockImplementation(
      () => ({
        proposalsByAsset: { a1: proposals },
        loadForAsset,
        approveAsIs,
        reject,
      }),
    );
    (useMaintenanceStore as unknown as ReturnType<typeof vi.fn>).mockImplementation(
      () => ({
        loadForAsset: loadSchedules,
      }),
    );
  }

  beforeEach(() => {
    approveAsIs.mockClear();
    reject.mockClear();
    loadForAsset.mockClear();
    loadSchedules.mockClear();
  });

  afterEach(() => cleanup());

  it("renders nothing when no proposals", () => {
    mockStores([]);
    const { container } = render(<PendingProposalsBlock assetId="a1" />);
    expect(container.textContent).toBe("");
  });

  it("renders one row per proposal with task + interval + rationale", () => {
    mockStores([
      makeProposal(1, "Annual service", 12),
      makeProposal(2, "Filter change", 6),
    ]);
    render(<PendingProposalsBlock assetId="a1" />);
    expect(screen.getByText(/Annual service · every 12 months/)).toBeInTheDocument();
    expect(screen.getByText(/Filter change · every 6 months/)).toBeInTheDocument();
    expect(screen.getByText(/rationale-1/)).toBeInTheDocument();
    expect(screen.getByText(/rationale-2/)).toBeInTheDocument();
  });

  it("approve click calls approveAsIs(id, assetId) + loadSchedules", async () => {
    mockStores([makeProposal(42, "Task", 12)]);
    render(<PendingProposalsBlock assetId="a1" />);
    fireEvent.click(screen.getByLabelText("Approve proposal"));
    await new Promise((r) => setTimeout(r, 0));
    expect(approveAsIs).toHaveBeenCalledWith(42, "a1");
    expect(loadSchedules).toHaveBeenCalledWith("a1");
  });

  it("reject click calls reject(id, assetId)", async () => {
    mockStores([makeProposal(7, "Task", 12)]);
    render(<PendingProposalsBlock assetId="a1" />);
    fireEvent.click(screen.getByLabelText("Reject proposal"));
    await new Promise((r) => setTimeout(r, 0));
    expect(reject).toHaveBeenCalledWith(7, "a1");
  });

  it("edit click opens drawer with proposalId set", () => {
    mockStores([makeProposal(99, "Task", 12)]);
    render(<PendingProposalsBlock assetId="a1" />);
    fireEvent.click(screen.getByLabelText("Edit proposal"));
    expect(screen.getByTestId("schedule-drawer")).toBeInTheDocument();
    expect(screen.getByText(/proposalId=99/)).toBeInTheDocument();
  });

  it("handles invalid diff JSON gracefully (skips row)", () => {
    const badProposal = {
      id: 1,
      kind: "add_maintenance_schedule",
      rationale: "r",
      diff: "not valid json",
      status: "pending",
      proposed_at: 1,
      applied_at: null,
      skill: "pdf_extract",
    };
    mockStores([badProposal]);
    const { container } = render(<PendingProposalsBlock assetId="a1" />);
    expect(container.querySelector("[aria-label='Approve proposal']")).toBeNull();
  });
});
