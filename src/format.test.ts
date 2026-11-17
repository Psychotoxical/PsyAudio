import { describe, expect, it } from "vitest";
import { encodingSpeed, longDuration } from "./format";

const t = (key: string, values?: Record<string, string>) =>
  key.replace(/\{(\w+)\}/g, (_, k: string) => values?.[k] ?? "");

describe("durations in words (as timefmt.rs)", () => {
  it("duration_and_speed_wording", () => {
    expect(longDuration(0, true, t)).toBe("0 seconds");
    expect(longDuration(1, true, t)).toBe("1 second");
    expect(longDuration(75, false, t)).toBe("1 minute 15 seconds");
    expect(longDuration(5 * 60 + 9, false, t)).toBe("5 minutes");
    expect(longDuration(5 * 60 + 9, true, t)).toBe("5 minutes 9 seconds");
    expect(longDuration(90_000, true, t)).toBe("1 day 1 hour");
    expect(encodingSpeed(300, 100, t, "en")).toBe("3.0× encoding speed");
    expect(encodingSpeed(300, 100, t, "de")).toBe("3,0× encoding speed");
    expect(encodingSpeed(300, 40, t, "de")).toBe("8× encoding speed");
  });
});
