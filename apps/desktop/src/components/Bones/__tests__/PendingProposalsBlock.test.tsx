import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup, waitFor } from "@testing-library/react";
import { PendingProposalsBlock } from "../PendingProposalsBlock";
import { usePdfExtractStore } from "../../../lib/pdf_extract/state";
import { useMaintenanceStore } from "../../../lib/maintenance/state";

vi.mock("../../../lib/pdf_extract/state", () => ({
  usePdfExtractStore: vi.fn(),
}));

vi.mock("../../../lib/maintenance/state", () => ({
  useMaintenanceStore: vi.fn(),
}));

vi.mock("../../../lib/today/ipc", async () => {
  const actual = await vi.importActual<typeof import("../../../lib/today/ipc")>(
    "../../../lib/today/ipc",
  );
  return {
    ...actual,
    approveProposal: vi.fn(),
    rejectProposal: vi.fn(),
  };
});

// Stub the proposal-edit drawer — its deps pull in Tauri APIs that don't
// work under jsdom.
vi.mock("../../Proposal/ProposalScheduleEditDrawer", () => ({
  ProposalScheduleEditDrawer: ({
    proposal,
    onApplied,
  }: {
    proposal: { id: number };
    onApplied: () => void;
  }) => (
    <div data-testid="schedule-drawer">
      <span>proposalId={proposal.id}</span>
      <button onClick={onApplied}>Drawer Save</button>
    </div>
  ),
}));

import { approveProposal, rejectProposal } from "../../../lib/today/ipc";
const mockApprove = vi.mocked(approveProposal);
const mockReject = vi.mocked(rejectProposal);

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
  const loadSchedules = vi.fn();

  function mockStores(proposals: unknown[]) {
    (usePdfExtractStore as unknown as ReturnType<typeof vi.fn>).mockImplementation(
      () => ({
        proposalsByAsset: { a1: proposals },
        loadForAsset,
      }),
    );
    (useMaintenanceStore as unknown as ReturnType<typeof vi.fn>).mockImplementation(
      () => ({
        loadForAsset: loadSchedules,
      }),
    );
  }

  beforeEach(() => {
    mockApprove.mockReset();
    mockReject.mockReset();
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

  it("approve click routes through central pipeline + refreshes both stores", async () => {
    mockApprove.mockResolvedValueOnce({
      proposal_id: 42,
      status: "applied",
      items_applied: 1,
      items_failed: 0,
      errors: [],
    });
    mockStores([makeProposal(42, "Task", 12)]);
    render(<PendingProposalsBlock assetId="a1" />);
    fireEvent.click(screen.getByLabelText("approve"));
    await waitFor(() => {
      expect(mockApprove).toHaveBeenCalledWith(42);
      expect(loadForAsset).toHaveBeenCalledWith("a1");
      expect(loadSchedules).toHaveBeenCalledWith("a1");
    });
  });

  it("reject click routes through central pipeline + refreshes both stores", async () => {
    mockReject.mockResolvedValueOnce(undefined);
    mockStores([makeProposal(7, "Task", 12)]);
    render(<PendingProposalsBlock assetId="a1" />);
    fireEvent.click(screen.getByLabelText("reject"));
    await waitFor(() => {
      expect(mockReject).toHaveBeenCalledWith(7);
      expect(loadForAsset).toHaveBeenCalledWith("a1");
      expect(loadSchedules).toHaveBeenCalledWith("a1");
    });
  });

  it("edit click opens drawer with proposalId set", () => {
    mockStores([makeProposal(99, "Task", 12)]);
    render(<PendingProposalsBlock assetId="a1" />);
    fireEvent.click(screen.getByLabelText("edit"));
    expect(screen.getByTestId("schedule-drawer")).toBeInTheDocument();
    expect(screen.getByText(/proposalId=99/)).toBeInTheDocument();
  });

  it("invalid diff JSON falls back to a parse-error card", () => {
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
    render(<PendingProposalsBlock assetId="a1" />);
    expect(
      screen.getByText(/could not parse details/),
    ).toBeInTheDocument();
  });
});
