import { describe, expect, it } from "vitest";
import { i18n, LANGUAGES, languageFile, plural, resolveLanguage, setLanguage } from "./i18n";

describe("i18n", () => {
  it("english_is_the_key", async () => {
    await setLanguage("en");
    expect(i18n.t("Convert")).toBe("Convert");
    expect(i18n.t("{0} files · {1}", { 0: "3", 1: "4:00" })).toBe("3 files · 4:00");
  });

  it("german_comes_from_its_file", async () => {
    await setLanguage("de");
    expect(i18n.t("Convert")).toBe("Konvertieren");
    await setLanguage("en");
  });

  it("a_missing_text_stays_english", async () => {
    await setLanguage("de");
    expect(i18n.t("A text no language has")).toBe("A text no language has");
    await setLanguage("en");
  });

  it("counts_follow_the_language", async () => {
    await setLanguage("en");
    expect(plural(1, "1 track", "{0} tracks")).toBe("1 track");
    expect(plural(2, "1 track", "{0} tracks")).toBe("{0} tracks");
    await setLanguage("de");
    expect(
      plural(
        1,
        "Done: {0} file, {accurate} confirmed by CTDB",
        "Done: {0} files, {accurate} confirmed by CTDB",
      ),
    ).toBe("Fertig: {0} Datei, {accurate} von CTDB bestätigt");
    await setLanguage("en");
  });

  it("system_language_is_used_when_known", () => {
    expect(resolveLanguage("", ["pt-BR", "de-AT", "en-US"])).toBe("de");
    expect(resolveLanguage("", ["pt-BR"])).toBe("en");
    expect(resolveLanguage("fr", ["de-DE"])).toBe("fr");
    expect(resolveLanguage("xx", ["ja-JP"])).toBe("ja");
  });

  it("every_language_has_a_file", () => {
    for (const [code] of LANGUAGES) {
      expect(Object.keys(languageFile(code)).length >= 0, code).toBe(true);
    }
    expect(Object.keys(languageFile("en")).length).toBeGreaterThan(500);
  });
});
