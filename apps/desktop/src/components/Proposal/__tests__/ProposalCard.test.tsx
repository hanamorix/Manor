import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup, waitFor } from "@testing-library/react";
import { ProposalCard } from "../ProposalCard";
import type { Proposal } from "../../../lib/today/ipc";

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

// Stub the schedule edit drawer — its deps pull in Tauri APIs that don't
// work under jsdom.
vi.mock("../ProposalScheduleEditDrawer", () => ({
  ProposalScheduleEditDrawer: ({
    onApplied,
    onClose,
    proposal,
  }: {
    onApplied: () => void;
    onClose: () => void;
    proposal: Proposal;
  }) => (
    <div data-testid="edit-drawer">
      <span>drawerProposalId={proposal.id}</span>
      <button
        onClick={() => {
          onApplied();
          onClose();
        }}
      >
        Drawer Save
      </button>
    </div>
  ),
}));

import { approveProposal, rejectProposal } from "../../../lib/today/ipc";

const mockApprove = vi.mocked(approveProposal);
const mockReject = vi.mocked(rejectProposal);

const addTaskProposal: Proposal = {
  id: 1,
  kind: "add_task",
  rationale: "user said: buy milk on the way home",
  diff: JSON.stringify({ title: "Buy milk", due_date: "2026-05-15" }),
  status: "pending",
  proposed_at: 0,
  applied_at: null,
  skill: "tasks",
};

const scheduleProposal: Proposal = {
  id: 2,
  kind: "add_maintenance_schedule",
  rationale: "annual boiler service from PDF",
  diff: JSON.stringify({
    asset_id: "asset-1",
    task: "Annual service",
    interval_months: 12,
    notes: "",
    source_attachment_uuid: "att-1",
    tier: "ollama",
  }),
  status: "pending",
  proposed_at: 0,
  applied_at: null,
  skill: "pdf_extract",
};

beforeEach(() => {
  mockApprove.mockReset();
  mockReject.mockReset();
});

afterEach(() => {
  cleanup();
});

describe("ProposalCard — add_task", () => {
  it("renders summarise text and rationale", () => {
    render(<ProposalCard proposal={addTaskProposal} />);
    expect(screen.getByText("Add task: Buy milk (due 2026-05-15)")).toBeTruthy();
    expect(
      screen.getByText(/user said: buy milk on the way home/),
    ).toBeTruthy();
  });

  it("does not render an edit button (supportsEdit is false)", () => {
    render(<ProposalCard proposal={addTaskProposal} />);
    expect(screen.queryByLabelText("edit")).toBeNull();
  });

  it("calls approveProposal and onApplied when approve clicked", async () => {
    mockApprove.mockResolvedValueOnce({
      proposal_id: 1,
      status: "applied",
      items_applied: 1,
      items_failed: 0,
      errors: [],
    });
    const onApplied = vi.fn();
    render(<ProposalCard proposal={addTaskProposal} onApplied={onApplied} />);
    fireEvent.click(screen.getByLabelText("approve"));
    await waitFor(() => {
      expect(mockApprove).toHaveBeenCalledWith(1);
      expect(onApplied).toHaveBeenCalledWith(
        expect.objectContaining({ proposal_id: 1, status: "applied" }),
      );
    });
  });

  it("calls rejectProposal and onRejected when reject clicked", async () => {
    mockReject.mockResolvedValueOnce(undefined);
    const onRejected = vi.fn();
    render(<ProposalCard proposal={addTaskProposal} onRejected={onRejected} />);
    fireEvent.click(screen.getByLabelText("reject"));
    await waitFor(() => {
      expect(mockReject).toHaveBeenCalledWith(1);
      expect(onRejected).toHaveBeenCalled();
    });
  });
});

describe("ProposalCard — add_maintenance_schedule", () => {
  it("summarises with task and interval (plural)", () => {
    render(<ProposalCard proposal={scheduleProposal} />);
    expect(screen.getByText("Annual service · every 12 months")).toBeTruthy();
  });

  it("renders an edit button because supportsEdit is true", () => {
    render(<ProposalCard proposal={scheduleProposal} />);
    expect(screen.getByLabelText("edit")).toBeTruthy();
  });

  it("opens the edit drawer when edit clicked", () => {
    render(<ProposalCard proposal={scheduleProposal} />);
    fireEvent.click(screen.getByLabelText("edit"));
    expect(screen.getByTestId("edit-drawer")).toBeTruthy();
    expect(screen.getByText("drawerProposalId=2")).toBeTruthy();
  });

  it("treats drawer-save as an applied event", async () => {
    const onApplied = vi.fn();
    render(<ProposalCard proposal={scheduleProposal} onApplied={onApplied} />);
    fireEvent.click(screen.getByLabelText("edit"));
    fireEvent.click(screen.getByText("Drawer Save"));
    expect(onApplied).toHaveBeenCalledWith(
      expect.objectContaining({ proposal_id: 2, status: "applied" }),
    );
  });
});

describe("ProposalCard — typed ApplyError rendering", () => {
  it("renders StaleReference inline", async () => {
    mockApprove.mockRejectedValueOnce({
      type: "StaleReference",
      value: { entity: "asset", id: "x" },
    });
    render(<ProposalCard proposal={scheduleProposal} />);
    fireEvent.click(screen.getByLabelText("approve"));
    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toMatch(
        /asset.*no longer there/i,
      );
    });
  });

  it("renders InvalidArg with field + reason", async () => {
    mockApprove.mockRejectedValueOnce({
      type: "InvalidArg",
      value: { field: "interval_months", reason: "must be positive" },
    });
    render(<ProposalCard proposal={scheduleProposal} />);
    fireEvent.click(screen.getByLabelText("approve"));
    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toContain(
        "interval_months",
      );
      expect(screen.getByRole("alert").textContent).toContain(
        "must be positive",
      );
    });
  });

  it("renders Conflict variant", async () => {
    mockApprove.mockRejectedValueOnce({
      type: "Conflict",
      value: "proposal not pending",
    });
    render(<ProposalCard proposal={scheduleProposal} />);
    fireEvent.click(screen.getByLabelText("approve"));
    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toMatch(/Already handled/);
    });
  });

  it("renders Network variant", async () => {
    mockApprove.mockRejectedValueOnce({
      type: "Network",
      value: "503 from server",
    });
    render(<ProposalCard proposal={scheduleProposal} />);
    fireEvent.click(screen.getByLabelText("approve"));
    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toMatch(/calendar/i);
      expect(screen.getByRole("alert").textContent).toContain("503");
    });
  });

  it("renders UnknownKind variant", async () => {
    mockApprove.mockRejectedValueOnce({
      type: "UnknownKind",
      value: "bizarre_kind",
    });
    render(<ProposalCard proposal={scheduleProposal} />);
    fireEvent.click(screen.getByLabelText("approve"));
    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toContain("bizarre_kind");
    });
  });

  it("renders Internal variant", async () => {
    mockApprove.mockRejectedValueOnce({
      type: "Internal",
      value: "db connection lost",
    });
    render(<ProposalCard proposal={scheduleProposal} />);
    fireEvent.click(screen.getByLabelText("approve"));
    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toContain(
        "db connection lost",
      );
    });
  });

  it("falls back to generic message on non-typed error", async () => {
    mockApprove.mockRejectedValueOnce(new Error("network lost"));
    render(<ProposalCard proposal={scheduleProposal} />);
    fireEvent.click(screen.getByLabelText("approve"));
    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toMatch(/Try again/);
    });
  });
});

describe("ProposalCard — unknown kind", () => {
  it("renders a fallback for unregistered kinds", () => {
    const unknownProposal: Proposal = {
      ...addTaskProposal,
      kind: "futuristic_kind",
    };
    render(<ProposalCard proposal={unknownProposal} />);
    expect(screen.getByText(/Unsupported kind/)).toBeTruthy();
  });
});
