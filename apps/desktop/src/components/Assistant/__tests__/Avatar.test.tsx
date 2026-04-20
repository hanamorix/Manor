import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import Avatar from "../Avatar";

describe("Avatar", () => {
  afterEach(() => cleanup());

  it("renders an img with the Manor alt text", () => {
    render(<Avatar />);
    expect(screen.getByAltText("Manor")).toBeInTheDocument();
  });

  it("respects the height prop and computes width from the aspect ratio", () => {
    render(<Avatar height={100} />);
    const img = screen.getByAltText("Manor") as HTMLImageElement;
    expect(img.getAttribute("height")).toBe("100");
    // NATURAL_RATIO = 274/400 = 0.685 → width ≈ 69 (Math.round)
    expect(img.getAttribute("width")).toBe("69");
  });

  it("defaults to height 72 when no prop is given", () => {
    render(<Avatar />);
    const img = screen.getByAltText("Manor") as HTMLImageElement;
    expect(img.getAttribute("height")).toBe("72");
  });

  it("wraps in a button when onClick is provided and fires on click", () => {
    const onClick = vi.fn();
    render(<Avatar onClick={onClick} />);
    fireEvent.click(screen.getByRole("button", { name: /Open conversation with Manor/ }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("renders img-only (no button) when onClick is absent", () => {
    render(<Avatar />);
    expect(screen.queryByRole("button")).toBeNull();
  });
});
