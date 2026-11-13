import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusChip } from "./TrackTable";

describe("track list", () => {
  it("status_chip_from_code", () => {
    render(
      <StatusChip
        status={{ kind: "Secure", passes: 1, rereads: 0, verified_first_pass: true }}
        ar={{ confirmed: true, confidence: 200, version: 2, in_db: true, unavailable: null }}
      />,
    );
    const chip = screen.getByText("Sicher · AR 200");
    expect(chip.closest(".chip")).toHaveClass("ok");
    expect(chip.closest(".chip")).toHaveAttribute(
      "title",
      "Sicher (1 Durchgang, von AccurateRip bestätigt) · AccurateRip: Genau (Vertrauen 200, v2)",
    );
  });

  it("insecure_track_warns", () => {
    render(<StatusChip status={{ kind: "Insecure", sectors: 3 }} ar={null} />);
    expect(screen.getByText("Unsicher (3 Sektoren)").closest(".chip")).toHaveClass("warn");
  });
});
