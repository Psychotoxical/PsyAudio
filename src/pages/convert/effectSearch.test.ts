import { describe, expect, it } from "vitest";
import type { EffectEntry } from "../../store/effects";
import { filterEffects } from "./effectSearch";

const entries: EffectEntry[] = [
  {
    category: "Filter",
    label: "Tiefpass",
    description: "Lässt nur Frequenzen unterhalb der Grenze durch.",
    effect: { Lowpass: { hz: 16000 } },
  },
  {
    category: "Filter",
    label: "Hochpass",
    description: "Lässt nur Frequenzen oberhalb der Grenze durch, etwa gegen Rumpeln.",
    effect: { Highpass: { hz: 40 } },
  },
  {
    category: "Lautstärke",
    label: "Lautstärke normalisieren",
    description: "Bringt jede Datei auf einen einheitlichen Pegel.",
    effect: {
      VolumeNormalize: { mode: 0, target_db: 0, max_amp: 10, window_ms: 3000, fixed_db: 0 },
    },
  },
];

const labels = (list: EffectEntry[]) => list.map((e) => e.label);

describe("effect search", () => {
  it("matches_label_and_description", () => {
    expect(labels(filterEffects(entries, "TIEFPASS", null))).toEqual(["Tiefpass"]);
    expect(labels(filterEffects(entries, "rumpeln", null))).toEqual(["Hochpass"]);
    expect(labels(filterEffects(entries, "lautstärke", null))).toEqual([
      "Lautstärke normalisieren",
    ]);
    expect(filterEffects(entries, "lautstarke", null)).toEqual([]);
    expect(filterEffects(entries, "  ", null)).toHaveLength(3);
  });

  it("category_filter", () => {
    expect(labels(filterEffects(entries, "", "Filter"))).toEqual(["Tiefpass", "Hochpass"]);
    expect(labels(filterEffects(entries, "grenze", "Lautstärke"))).toEqual([]);
  });
});
