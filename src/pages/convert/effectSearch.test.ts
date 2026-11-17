import { describe, expect, it } from "vitest";
import type { EffectEntry } from "../../store/effects";
import { filterEffects } from "./effectSearch";

const entries: EffectEntry[] = [
  {
    category: "Filter",
    label: "Low-pass",
    description: "Lets only frequencies below the limit through.",
    effect: { Lowpass: { hz: 16000 } },
  },
  {
    category: "Filter",
    label: "High-pass",
    description: "Lets only frequencies above the limit through, e.g. against rumble.",
    effect: { Highpass: { hz: 40 } },
  },
  {
    category: "Volume",
    label: "Normalize volume",
    description: "Bringt jede Datei auf einen einheitlichen Pegel.",
    effect: {
      VolumeNormalize: { mode: 0, target_db: 0, max_amp: 10, window_ms: 3000, fixed_db: 0 },
    },
  },
];

const labels = (list: EffectEntry[]) => list.map((e) => e.label);

describe("effect search", () => {
  it("matches_label_and_description", () => {
    expect(labels(filterEffects(entries, "LOW-PASS", null))).toEqual(["Low-pass"]);
    expect(labels(filterEffects(entries, "rumble", null))).toEqual(["High-pass"]);
    expect(labels(filterEffects(entries, "normalize", null))).toEqual(["Normalize volume"]);
    expect(filterEffects(entries, "normalise", null)).toEqual([]);
    expect(filterEffects(entries, "  ", null)).toHaveLength(3);
  });

  it("category_filter", () => {
    expect(labels(filterEffects(entries, "", "Filter"))).toEqual(["Low-pass", "High-pass"]);
    expect(labels(filterEffects(entries, "grenze", "Volume"))).toEqual([]);
  });
});
