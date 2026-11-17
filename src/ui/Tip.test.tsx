import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { placeTip, Tip } from "./Tip";

const win = { width: 1280, height: 640 };

describe("hover text", () => {
  it("place_keeps_the_tip_inside_the_window", () => {
    // a chip at the right edge: the text ends at the chip's right edge, not past the window
    const anchor = { left: 1180, right: 1270, top: 100, bottom: 120 };
    const p = placeTip(anchor, { width: 600, height: 30 }, win);
    expect(p.left + 600).toBeLessThanOrEqual(win.width - 8);
    expect(p.left).toBeGreaterThanOrEqual(8);
    expect(p.top).toBe(126);
  });

  it("place_flips_above_near_the_bottom", () => {
    const anchor = { left: 1000, right: 1090, top: 610, bottom: 630 };
    const p = placeTip(anchor, { width: 300, height: 30 }, win);
    expect(p.top).toBe(610 - 6 - 30);
  });

  it("hover_shows_the_text", () => {
    render(
      <Tip text="Secure (2 passes identical)">
        <span>Secure</span>
      </Tip>,
    );
    const anchor = screen.getByText("Secure").parentElement as HTMLElement;
    expect(anchor).toHaveAccessibleDescription("Secure (2 passes identical)");
    expect(screen.queryByRole("tooltip")).toBeNull();
    fireEvent.mouseEnter(anchor);
    expect(screen.getByRole("tooltip")).toHaveTextContent("Secure (2 passes identical)");
    fireEvent.mouseLeave(anchor);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });
});
