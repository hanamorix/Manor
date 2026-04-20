import { afterEach, describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, within, cleanup } from "@testing-library/react";
import { LogCompletionDrawer } from "../LogCompletionDrawer";
import { useMaintenanceEventsStore } from "../../../lib/maintenance/event-state";

vi.mock("../../../lib/maintenance/event-state", () => ({
  useMaintenanceEventsStore: vi.fn(),
}));

afterEach(cleanup);

describe("LogCompletionDrawer", () => {
  const createOneOff = vi.fn().mockResolvedValue("new-id");
  const logCompletion = vi.fn().mockResolvedValue("new-id");
  const update = vi.fn().mockResolvedValue(undefined);
  const suggestTransactions = vi.fn().mockResolvedValue([]);
  const searchTransactions = vi.fn().mockResolvedValue([]);

  beforeEach(() => {
    (useMaintenanceEventsStore as unknown as ReturnType<typeof vi.fn>).mockImplementation(
      () => ({
        createOneOff,
        logCompletion,
        update,
        suggestTransactions,
        searchTransactions,
      }),
    );
    createOneOff.mockClear();
    logCompletion.mockClear();
    update.mockClear();
  });

  it("requires title in one-off mode", async () => {
    const onClose = vi.fn();
    render(
      <LogCompletionDrawer
        open
        mode={{ kind: "one_off", assetId: "a1" }}
        onClose={onClose}
      />,
    );
    const dialog = screen.getByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: /^Save$/ }));
    expect(await screen.findByText("Title is required.")).toBeInTheDocument();
    expect(createOneOff).not.toHaveBeenCalled();
  });

  it("prefills title in schedule_completion mode", () => {
    render(
      <LogCompletionDrawer
        open
        mode={{
          kind: "schedule_completion",
          assetId: "a1",
          scheduleId: "s1",
          taskName: "Annual service",
        }}
        onClose={vi.fn()}
      />,
    );
    const dialog = screen.getByRole("dialog");
    const titleInput = within(dialog).getByLabelText("Title") as HTMLInputElement;
    expect(titleInput.value).toBe("Annual service");
  });

  it("rejects negative cost inline", async () => {
    render(
      <LogCompletionDrawer
        open
        mode={{ kind: "one_off", assetId: "a1" }}
        onClose={vi.fn()}
      />,
    );
    const dialog = screen.getByRole("dialog");
    fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "Fix" } });
    fireEvent.change(within(dialog).getByLabelText(/Cost/), { target: { value: "-5" } });
    fireEvent.click(within(dialog).getByRole("button", { name: /^Save$/ }));
    expect(
      await screen.findByText("Cost must be a positive number."),
    ).toBeInTheDocument();
  });

  it("prefills all fields in edit mode from the event", () => {
    const event = {
      id: "evt-1",
      asset_id: "a1",
      schedule_id: "s1",
      title: "Filter replaced",
      completed_date: "2026-02-15",
      cost_pence: 2200,
      currency: "GBP",
      notes: "HEPA filter from Amazon",
      transaction_id: null,
      source: "manual" as const,
      created_at: 0,
      updated_at: 0,
      deleted_at: null,
    };
    render(
      <LogCompletionDrawer
        open
        mode={{ kind: "edit", event }}
        onClose={vi.fn()}
      />,
    );
    const dialog = screen.getByRole("dialog");
    const titleInput = within(dialog).getByLabelText("Title") as HTMLInputElement;
    const dateInput = within(dialog).getByLabelText("Completed date") as HTMLInputElement;
    const costInput = within(dialog).getByLabelText(/Cost/) as HTMLInputElement;
    const notesInput = within(dialog).getByLabelText("Notes") as HTMLTextAreaElement;

    expect(titleInput.value).toBe("Filter replaced");
    expect(dateInput.value).toBe("2026-02-15");
    expect(costInput.value).toBe("22.00");
    expect(notesInput.value).toBe("HEPA filter from Amazon");
  });
});
