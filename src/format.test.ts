import { describe, expect, it } from "vitest";
import { encodingSpeed, longDuration } from "./format";

const t = (key: string, values?: Record<string, string>) =>
  key.replace(/\{(\w+)\}/g, (_, k: string) => values?.[k] ?? "");

describe("durations in words (as timefmt.rs)", () => {
  it("duration_and_speed_wording", () => {
    expect(longDuration(0, true, t)).toBe("0 Sekunden");
    expect(longDuration(1, true, t)).toBe("1 Sekunde");
    expect(longDuration(75, false, t)).toBe("1 Minute 15 Sekunden");
    expect(longDuration(5 * 60 + 9, false, t)).toBe("5 Minuten");
    expect(longDuration(5 * 60 + 9, true, t)).toBe("5 Minuten 9 Sekunden");
    expect(longDuration(90_000, true, t)).toBe("1 Tag 1 Stunde");
    expect(encodingSpeed(300, 100, t, "en")).toBe("3.0× Kodiergeschwindigkeit");
    expect(encodingSpeed(300, 100, t, "de")).toBe("3,0× Kodiergeschwindigkeit");
    expect(encodingSpeed(300, 40, t, "de")).toBe("8× Kodiergeschwindigkeit");
  });
});
