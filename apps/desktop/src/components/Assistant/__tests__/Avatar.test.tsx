import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import Avatar from "../Avatar";

describe("Avatar", () => {
  afterEach(() => cleanup());

  it("renders an img with the Manor alt text", () => {
    render(<Avatar />);
    expect(screen.getByAltText("Manor")).toBeInTheDocument();
  });

  it("applies the height via inline style; width uses auto so intrinsic ratio is preserved", () => {
    render(<Avatar height={100} />);
    const img = screen.getByAltText("Manor") as HTMLImageElement;
    expect(img.style.height).toBe("100px");
    expect(img.style.width).toBe("auto");
  });

  it("defaults to height 72 when no prop is given", () => {
    render(<Avatar />);
    const img = screen.getByAltText("Manor") as HTMLImageElement;
    expect(img.style.height).toBe("72px");
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
