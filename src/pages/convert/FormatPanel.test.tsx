import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "../../ipc/bindings/Settings";

vi.mock("../../ipc/commands", () => ({ saveSettings: vi.fn(() => Promise.resolve()) }));

import { type EncodeOptions, useEncode } from "../../store/encode";
import { useSettings } from "../../store/settings";
import { CodecChips } from "./CodecChips";
import { CodecSettings } from "./CodecSettings";
import { ProfileRow } from "./ProfileRow";

const options: EncodeOptions = {
  codecs: [
    { codec: "Flac", name: "FLAC", short: "FLAC", lossless: true, tool: false },
    { codec: "MonkeysAudio", name: "Monkey's Audio", short: "APE", lossless: true, tool: false },
    { codec: "Mp3", name: "MP3", short: "MP3", lossless: false, tool: false },
    { codec: "Opus", name: "Opus", short: "Opus", lossless: false, tool: false },
  ],
  lossless: ["Flac", "MonkeysAudio"],
  compressed: ["Mp3", "Opus"],
  tools: [],
  audio_codecs: ["Flac", "Mp3", "Opus"],
  mp3_rates: [128, 192, 320],
  aac_rates: [128, 256],
  aac_vbr_kbps: [64, 80, 96, 144, 224],
  vorbis_kbps: [128, 160],
  stereo_modes: [
    "Stereo (automatisch)",
    "Joint Stereo",
    "Erzwungenes Joint Stereo",
    "Stereo",
    "Mono",
  ],
  wavpack_modes: ["Schnell", "Normal", "Hohe Qualität", "Sehr hohe Qualität"],
  ape_levels: ["Schnell", "Normal", "Hoch", "Extra hoch", "Wahnsinn"],
  dsd_modes: ["DSD64"],
  mpc_profiles: [{ name: "Standard", kbps: 180 }],
  pcm_formats: [{ format: "S16", label: "16 Bit" }],
};

function settings(): Settings {
  return {
    codec: "Mp3",
    enc: { opus_kbps: 192, opus_mode: 0 },
    profiles: [],
    ui: { panel_open: {}, tile: {}, sidebar_collapsed: false, boxes_open: {} },
  } as unknown as Settings;
}

describe("format area", () => {
  beforeEach(() => {
    useSettings.getState().load(settings());
    useEncode.setState({ options, problems: { MonkeysAudio: "Monkey's Audio: mac.exe fehlt" } });
  });

  it("missing_program_is_dashed_and_explained", () => {
    render(<CodecChips field="codec" />);
    const ape = screen.getByRole("button", { name: "APE" });
    expect(ape).toHaveClass("missing");
    expect(ape).toHaveAttribute("title", "Monkey's Audio: mac.exe fehlt");
    expect(screen.getByText(/Gestrichelt/)).toBeInTheDocument();
  });

  it("choosing_a_chip_sets_codec", () => {
    render(<CodecChips field="codec" />);
    fireEvent.click(screen.getByRole("button", { name: "Opus" }));
    expect(useSettings.getState().settings?.codec).toBe("Opus");
    expect(screen.getByRole("button", { name: "Opus" })).toHaveAttribute("aria-pressed", "true");
  });

  it("saving_a_profile_keeps_codec_and_settings", () => {
    render(<ProfileRow />);
    fireEvent.change(screen.getByRole("textbox", { name: "Profilname" }), {
      target: { value: "Unterwegs" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Profil speichern" }));
    const saved = useSettings.getState().settings?.profiles;
    expect(saved).toHaveLength(1);
    expect(saved?.[0]?.name).toBe("Unterwegs");
    expect(saved?.[0]?.codec).toBe("Mp3");
  });

  it("choosing_a_profile_loads_it", () => {
    useSettings.getState().update((s) => {
      s.profiles = [
        { name: "Archiv", codec: "Flac", enc: { flac_level: 8 } as Settings["enc"], effects: [] },
      ];
    });
    render(<ProfileRow />);
    fireEvent.change(screen.getByRole("combobox", { name: "Profil" }), {
      target: { value: "Archiv" },
    });
    expect(useSettings.getState().settings?.codec).toBe("Flac");
    expect(useSettings.getState().settings?.enc.flac_level).toBe(8);
  });

  it("opus_shows_bitrate_and_mode", () => {
    render(<CodecSettings codec="Opus" />);
    expect(screen.getByRole("textbox", { name: "Bitrate" })).toHaveValue("192 kbit/s");
    expect(screen.getByRole("radio", { name: "VBR" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Eingeschränktes VBR" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "CBR" })).toBeInTheDocument();
  });
});
