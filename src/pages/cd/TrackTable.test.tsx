import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusChip } from "./TrackTable";

describe("track list", () => {
  it("status_chip_from_code", () => {
    render(
      <StatusChip
        status={{ kind: "Secure", passes: 1, rereads: 0, verified_first_pass: true }}
        ctdb={{ confirmed: true, confidence: 1085, total: 1107, in_db: true, unavailable: null }}
      />,
    );
    const chip = screen.getByText("Secure · CTDB 1085");
    expect(chip.closest(".chip")).toHaveClass("ok");
    // the full wording in a hover text that stays inside the window, not in `title`
    expect(chip.closest(".chip")).not.toHaveAttribute("title");
    expect(chip.closest(".chip")).toHaveAccessibleDescription(
      "Secure (1 pass, confirmed by CTDB) · CTDB: confirmed (1085 of 1107)",
    );
  });

  it("unconfirmed_track_warns", () => {
    render(
      <StatusChip
        status={{ kind: "Secure", passes: 2, rereads: 0, verified_first_pass: false }}
        ctdb={{ confirmed: false, confidence: 0, total: 1107, in_db: true, unavailable: null }}
      />,
    );
    const chip = screen.getByText("Secure · CTDB not confirmed").closest(".chip");
    expect(chip).toHaveClass("warn");
    expect(chip).toHaveAccessibleDescription("Secure (2 passes identical) · CTDB: not confirmed");
  });

  it("insecure_track_warns", () => {
    render(<StatusChip status={{ kind: "Insecure", sectors: 3 }} ctdb={null} />);
    // the hover text says the same here; it is hidden until hovered
    const chip = screen.getByText("Insecure (3 sectors)", { ignore: "[role=tooltip]" });
    expect(chip.closest(".chip")).toHaveClass("warn");
  });
});
