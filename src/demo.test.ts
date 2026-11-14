import { describe, expect, it } from "vitest";
import { demoBatch, demoCd, demoFiles } from "./demo";

describe("sample mode", () => {
  it("english_samples_have_english_names", () => {
    const cd = demoCd(true);
    expect(cd.meta?.album).toBe("First Album");
    expect(cd.meta?.artist).toBe("Sample Band");
    expect(cd.drives?.[0]?.model).toBe("Sample Drive BD-RE");
    expect(demoFiles(true)[0]?.tags.title).toBe("First Song");
    expect(demoBatch(true).folder).toBe("D:\\Music\\Collection");
  });

  it("german_samples_stay_german", () => {
    expect(demoCd().meta?.album).toBe("Erstes Album");
    expect(demoFiles()[0]?.tags.title).toBe("Erstes Lied");
    expect(demoBatch().folder).toBe("D:\\Musik\\Sammlung");
  });
});
