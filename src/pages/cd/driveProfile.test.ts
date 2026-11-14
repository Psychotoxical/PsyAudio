import { describe, expect, it } from "vitest";
import type { RipSettings } from "../../ipc/bindings/RipSettings";
import { driveKey, driveOffset, setDriveProfile, withDriveOffset } from "./driveProfile";

const rip = (patch: Partial<RipSettings>) =>
  ({ drive_offset: 0, drives: [], ...patch }) as unknown as RipSettings;

describe("drive profiles", () => {
  it("keys_a_drive_by_its_model_else_its_name", () => {
    expect(driveKey({ id: "G", name: "G:", model: "Band Drive X" })).toBe("Band Drive X");
    expect(driveKey({ id: "G", name: "G:", model: null })).toBe("G:");
  });

  it("reads_the_profile_then_the_old_single_offset", () => {
    const profile = {
      model: "Band Drive X",
      offset: 667,
      method: "Ctdb" as const,
      measured: "2026-11-14",
      buffer_kb: null,
    };
    expect(driveOffset(rip({ drives: [profile] }), "Band Drive X")).toBe(667);
    expect(driveOffset(rip({ drive_offset: 12 }), "Band Drive X")).toBe(12);
    expect(driveOffset(rip({}), null)).toBe(0);
  });

  it("a_measured_profile_replaces_the_old_one", () => {
    const r = rip({});
    withDriveOffset(r, "Band Drive X", 6, "Manual", "2026-11-14");
    const measured = {
      model: "Band Drive X",
      offset: 667,
      method: "Ctdb" as const,
      measured: "2026-11-15",
      buffer_kb: 4096,
    };
    setDriveProfile(r, measured);
    expect(r.drives).toEqual([measured]);
  });

  it("stores_one_profile_per_model", () => {
    const r = rip({});
    withDriveOffset(r, "Band Drive X", 6, "Manual", "2026-11-14");
    withDriveOffset(r, "Band Drive X", 667, "Ctdb", "2026-11-15");
    expect(r.drives).toEqual([
      {
        model: "Band Drive X",
        offset: 667,
        method: "Ctdb",
        measured: "2026-11-15",
        buffer_kb: null,
      },
    ]);
  });
});
