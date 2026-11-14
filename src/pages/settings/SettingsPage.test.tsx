import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "../../ipc/bindings/Settings";

vi.mock("../../ipc/commands", () => ({ saveSettings: vi.fn(() => Promise.resolve()) }));
vi.mock("../../ipc/system", () => ({
  programs: vi.fn(() => Promise.resolve([])),
  systemInfo: vi.fn(() => Promise.resolve({ cores: 12, temp_default: "C:\\Temp", os: "windows" })),
  resetSettings: vi.fn((s: Settings) => Promise.resolve(s)),
}));
vi.mock("../../ipc/cd", async (original) => ({
  ...(await original<typeof import("../../ipc/cd")>()),
  listDrives: vi.fn(() => Promise.resolve([])),
}));

import { i18n } from "../../i18n";
import { resetSettings, systemInfo } from "../../ipc/system";
import { newRun } from "../../store/run";
import { useSettings } from "../../store/settings";
import { RunView } from "../convert/RunView";
import { SettingsPage } from "./SettingsPage";

function settings(): Settings {
  return {
    lang: "de",
    dark: true,
    threads: 4,
    low_priority: false,
    temp_folder: "",
    tools: { ffmpeg: "", mac: "", mpcenc: "", sox: "" },
    profiles: [],
    rip: { drives: [], drive_offset: 0, cache_kb: 0, ctdb: true },
    ui: {
      panel_open: {},
      tile: {},
      sidebar_collapsed: false,
      boxes_open: { programs: true, performance: true, look: true, integration: true, reset: true },
    },
  } as unknown as Settings;
}

describe("settings page", () => {
  beforeEach(() => useSettings.getState().load(settings()));
  afterEach(async () => {
    vi.restoreAllMocks();
    await act(() => i18n.changeLanguage("de"));
  });

  it("finder_integration_on_macos", async () => {
    vi.mocked(systemInfo).mockResolvedValueOnce({ cores: 8, temp_default: "/tmp", os: "macos" });
    render(<SettingsPage />);
    const box = await screen.findByRole("region", { name: "Finder-Integration" });
    expect(box.textContent).toContain("Finder");
    expect(box.textContent).toContain("Kontextmenü einrichten");
  });

  it("boxes_in_two_columns_for_wide_windows", async () => {
    render(<SettingsPage />);
    await screen.findByRole("region", { name: "Windows-Integration" });
    // one under the other in narrow windows, side by side in wide ones (settings.css)
    const cols = [...document.querySelectorAll(".settings-grid > .settings-col")];
    const names = cols.map((c) =>
      [...c.querySelectorAll(":scope > section")].map((s) => s.getAttribute("aria-label")),
    );
    expect(names).toEqual([
      ["Encoder-Programme", "Leistung"],
      ["CD-Laufwerke", "Aussehen", "Windows-Integration", "Zurücksetzen"],
    ]);
  });

  it("reset_needs_confirmation", async () => {
    render(<SettingsPage />);
    fireEvent.click(screen.getByRole("button", { name: "Alle Einstellungen auf Standard" }));
    expect(resetSettings).not.toHaveBeenCalled();
    expect(screen.getByText(/außer den Profilen/)).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Jetzt zurücksetzen" }));
    });
    expect(resetSettings).toHaveBeenCalledTimes(1);
  });

  it("language_switch_keeps_run_status", async () => {
    // the job list is virtual: give it a size, as jsdom has no layout
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
      DOMRect.fromRect({ width: 800, height: 600 }),
    );
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(600);
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(800);
    const run = newRun(
      { title: "Umwandeln nach Opus", target: "Opus", threads: 1 },
      [["C:\\Musik\\a.flac", null]],
      0,
    );
    render(
      <>
        <SettingsPage />
        <RunView run={run} />
      </>,
    );
    expect(screen.getByText("wartet")).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByRole("radio", { name: "English" }));
    });
    expect(useSettings.getState().settings?.lang).toBe("en");
    expect(screen.getByText("waiting")).toBeInTheDocument();
  });
});
