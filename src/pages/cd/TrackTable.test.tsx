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
    const chip = screen.getByText("Sicher · CTDB 1085");
    expect(chip.closest(".chip")).toHaveClass("ok");
    // the full wording in a hover text that stays inside the window, not in `title`
    expect(chip.closest(".chip")).not.toHaveAttribute("title");
    expect(chip.closest(".chip")).toHaveAccessibleDescription(
      "Sicher (1 Durchgang, von CTDB bestätigt) · CTDB: bestätigt (1085 von 1107)",
    );
  });

  it("unconfirmed_track_warns", () => {
    render(
      <StatusChip
        status={{ kind: "Secure", passes: 2, rereads: 0, verified_first_pass: false }}
        ctdb={{ confirmed: false, confidence: 0, total: 1107, in_db: true, unavailable: null }}
      />,
    );
    const chip = screen.getByText("Sicher · CTDB nicht bestätigt").closest(".chip");
    expect(chip).toHaveClass("warn");
    expect(chip).toHaveAccessibleDescription(
      "Sicher (2 Durchgänge identisch) · CTDB: nicht bestätigt",
    );
  });

  it("insecure_track_warns", () => {
    render(<StatusChip status={{ kind: "Insecure", sectors: 3 }} ctdb={null} />);
    // the hover text says the same here; it is hidden until hovered
    const chip = screen.getByText("Unsicher (3 Sektoren)", { ignore: "[role=tooltip]" });
    expect(chip.closest(".chip")).toHaveClass("warn");
  });
});
