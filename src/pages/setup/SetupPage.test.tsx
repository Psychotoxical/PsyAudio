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
  listDrives: vi.fn(() => Promise.resolve([{ id: "G", name: "G:", model: "Band Drive X" }])),
  measureDrive: vi.fn(() => Promise.resolve()),
}));

import { i18n } from "../../i18n";
import { systemInfo } from "../../ipc/system";
import { Sidebar } from "../../shell/Sidebar";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { firstPage, SetupPage } from "./SetupPage";

function settings(): Settings {
  return {
    setup_done: false,
    lang: "de",
    dark: true,
    threads: 4,
    low_priority: false,
    temp_folder: "",
    tools: { ffmpeg: "", mac: "", mpcenc: "", sox: "" },
    profiles: [],
    rip_codec: "Flac",
    rip_folder: "C:\\Musik",
    rip: { drives: [], drive_offset: 0, cache_kb: 0, ctdb: true },
    ui: { panel_open: {}, tile: {}, sidebar_collapsed: false, boxes_open: {} },
  } as unknown as Settings;
}

describe("setup", () => {
  beforeEach(() => {
    useSettings.getState().load(settings());
    useUi.setState({ page: "convert" });
  });
  afterEach(async () => {
    await act(() => i18n.changeLanguage("de"));
  });

  it("first_start_opens_setup", () => {
    expect(firstPage(false)).toBe("setup");
    expect(firstPage(true)).toBeNull();
  });

  it("skip_moves_to_the_next_step", async () => {
    render(<SetupPage />);
    expect(screen.getByRole("heading", { name: "Sprache und Aussehen" })).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Überspringen" }));
    });
    expect(screen.getByRole("heading", { name: "Encoder-Programme" })).toBeInTheDocument();
    expect(screen.getByText("Schritt 2 von 5")).toBeInTheDocument();
  });

  it("context_menu_step_on_macos", async () => {
    vi.mocked(systemInfo).mockResolvedValue({ cores: 8, temp_default: "/tmp", os: "macos" });
    render(<SetupPage />);
    for (let i = 0; i < 2; i++) {
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Überspringen" }));
      });
    }
    expect(screen.getByRole("heading", { name: "Kontextmenü" })).toBeInTheDocument();
    expect(
      await screen.findByRole("button", { name: "Kontextmenü einrichten" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/Finder/)).toBeInTheDocument();
  });

  it("finish_marks_setup_done", async () => {
    render(<SetupPage />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /CD-Laufwerk/ }));
    });
    expect(await screen.findByText("Band Drive X")).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Fertig" }));
    });
    expect(useSettings.getState().settings?.setup_done).toBe(true);
    expect(useUi.getState().page).toBe("convert");
  });

  it("sidebar_entry_opens_the_setup", () => {
    render(<Sidebar />);
    fireEvent.click(screen.getByRole("button", { name: "Einrichtung" }));
    expect(useUi.getState().page).toBe("setup");
  });
});
