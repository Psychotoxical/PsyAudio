import { describe, expect, it } from "vitest";
import { i18n, plural, setLanguage } from "./i18n";
import en from "./locales/en.json";

const placeholders = (s: string) =>
  [...s.matchAll(/\{([^}]*)\}/g)].map((m) => m[1]).sort((a, b) => (a ?? "").localeCompare(b ?? ""));

describe("i18n", () => {
  it("english_from_dictionary", async () => {
    await setLanguage("en");
    expect(i18n.t("Konvertieren")).toBe("Convert");
  });

  it("placeholders_survive", async () => {
    await setLanguage("en");
    expect(i18n.t("{0} Dateien · {1}", { 0: "3", 1: "4:00" })).toBe("3 files · 4:00");
  });

  it("plural_picks_one_only_for_exactly_one", () => {
    expect(plural(1, "{0} Datei", "{0} Dateien")).toBe("{0} Datei");
    expect(plural(0, "{0} Datei", "{0} Dateien")).toBe("{0} Dateien");
    expect(plural(2, "{0} Datei", "{0} Dateien")).toBe("{0} Dateien");
  });

  it("unknown_key_falls_back_to_german", async () => {
    await setLanguage("en");
    expect(i18n.t("Diesen Text gibt es nicht")).toBe("Diesen Text gibt es nicht");
    await setLanguage("de");
    expect(i18n.t("Konvertieren")).toBe("Konvertieren");
  });

  it("json_keeps_placeholders", () => {
    for (const [de, english] of Object.entries(en as Record<string, string>)) {
      expect(placeholders(english), de).toEqual(placeholders(de));
    }
  });
});
