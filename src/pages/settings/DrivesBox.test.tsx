import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DriveProfile } from "../../ipc/bindings/DriveProfile";
import type { Settings } from "../../ipc/bindings/Settings";

vi.mock("../../ipc/commands", () => ({ saveSettings: vi.fn(() => Promise.resolve()) }));
vi.mock("../../ipc/cd", async (original) => ({
  ...(await original<typeof import("../../ipc/cd")>()),
  listDrives: vi.fn(() => Promise.resolve([{ id: "G", name: "G:", model: "Band Drive X" }])),
  measureDrive: vi.fn(() => Promise.resolve()),
}));

import { useCd } from "../../store/cd";
import { useSettings } from "../../store/settings";
import { applyMeasurement } from "../cd/measure";
import { DrivesBox } from "./DrivesBox";

const profile = (offset: number, method: DriveProfile["method"]): DriveProfile => ({
  model: "Band Drive X",
  offset,
  method,
  measured: "2026-11-14",
  buffer_kb: 4096,
});

function settings(drives: DriveProfile[]): Settings {
  return {
    lang: "de",
    rip: { drives, drive_offset: 0, cache_kb: 0, ctdb: true },
    ui: { panel_open: {}, tile: {}, sidebar_collapsed: false, boxes_open: { drives: true } },
  } as unknown as Settings;
}

describe("drives box", () => {
  beforeEach(() => useCd.setState({ pendingProfile: null, offset: { busy: false, fraction: 0 } }));

  it("lists_the_attached_drive_with_its_profile", async () => {
    useSettings.getState().load(settings([profile(667, "Ctdb")]));
    render(<DrivesBox />);
    expect(await screen.findByText("Band Drive X")).toBeInTheDocument();
    expect(screen.getAllByText("+667 Samples").length).toBeGreaterThan(0);
    expect(screen.getByText("4096 KB")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Measure again" })).toBeInTheDocument();
  });

  it("a_value_by_hand_is_replaced_only_after_asking", async () => {
    useSettings.getState().load(settings([profile(600, "Manual")]));
    render(<DrivesBox />);
    await screen.findByText("Band Drive X");
    await act(async () => applyMeasurement(profile(667, "Ctdb"), 2));
    expect(useSettings.getState().settings?.rip.drives[0]?.offset).toBe(600);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Replace" }));
    });
    expect(useSettings.getState().settings?.rip.drives[0]?.offset).toBe(667);
    expect(useCd.getState().pendingProfile).toBeNull();
  });
});
