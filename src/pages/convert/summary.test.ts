import { describe, expect, it } from "vitest";
import type { EncSettings } from "../../ipc/bindings/EncSettings";
import type { Settings } from "../../ipc/bindings/Settings";
import type { NamingOptions } from "../../ipc/naming";
import type { EffectEntry } from "../../store/effects";
import type { EncodeOptions } from "../../store/encode";
import { codecSummary, effectsSummary, namingSummary, saveSummary } from "./summary";

const t = (key: string, values?: Record<string, string>) =>
  key.replace(/\{(\w+)\}/g, (_, k: string) => values?.[k] ?? "");

const options = {
  codecs: [
    { codec: "Opus", name: "Opus", short: "Opus", lossless: false, tool: false },
    { codec: "Mp3", name: "MP3", short: "MP3", lossless: false, tool: false },
    { codec: "Flac", name: "FLAC", short: "FLAC", lossless: true, tool: false },
  ],
} as unknown as EncodeOptions;

const enc = (e: Partial<EncSettings>) => e as EncSettings;

describe("format tile summary", () => {
  it("opus_shows_rate_and_mode", () => {
    expect(codecSummary("Opus", enc({ opus_kbps: 192, opus_mode: 0 }), options, t)).toBe(
      "Opus · 192 kbit/s · VBR",
    );
  });

  it("mp3_vbr_shows_quality_level", () => {
    expect(codecSummary("Mp3", enc({ mp3_mode: "Vbr", mp3_vbr: 2 }), options, t)).toBe(
      "MP3 · -V 2 · VBR",
    );
  });

  it("flac_shows_level", () => {
    expect(codecSummary("Flac", enc({ flac_level: 5 }), options, t)).toBe("FLAC · Level 5");
  });

  it("save_summary_names_the_place", () => {
    const s = { output_mode: "SourceFolder", output_folder: "C:\\Ziel" } as Settings;
    expect(saveSummary(s, t)).toBe("Next to the source");
    expect(saveSummary({ ...s, output_mode: "SingleFolder" }, t)).toBe("C:\\Ziel");
  });

  it("naming_summary_names_the_preset", () => {
    const naming = {
      presets: [{ name: "Track – Title", scheme: ["track", " - ", "title"] }],
    } as NamingOptions;
    const s = { output_mode: "SingleFolder", name_scheme: ["track", " - ", "title"] } as Settings;
    expect(namingSummary(s, naming, t)).toBe("Track – Title");
    expect(namingSummary({ ...s, name_scheme: ["artist", "title"] }, naming, t)).toBe(
      "2 building blocks",
    );
    expect(namingSummary({ ...s, output_mode: "PreservePath" }, naming, t)).toBe("As the source");
  });

  it("effects_summary_lists_names", () => {
    const entries = [
      { category: "Filter", label: "Low-pass", description: "", effect: { Lowpass: { hz: 1 } } },
      { category: "Filter", label: "De-Emphasis", description: "", effect: "DeEmphasis" },
    ] as EffectEntry[];
    expect(effectsSummary([], entries, t)).toBe("None");
    expect(effectsSummary(["DeEmphasis"], entries, t)).toBe("De-Emphasis");
    // the tile shows one line: the count comes first so a cut-off line still says it
    expect(effectsSummary(["DeEmphasis", { Lowpass: { hz: 500 } }], entries, t)).toBe(
      "2 effects · De-Emphasis, Low-pass",
    );
  });
});
