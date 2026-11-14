import { describe, expect, it } from "vitest";
import { i18n, setLanguage } from "../../i18n";
import { ctdbText, statusText, summaryText } from "./status";

const t = (key: string, values?: Record<string, string>) => i18n.t(key, values ?? {});

describe("rip status words", () => {
  it("one_sector_or_block_is_singular_in_english", async () => {
    await setLanguage("en");
    expect(statusText({ kind: "Insecure", sectors: 1 }, t)).toBe("Insecure (1 sector)");
    expect(statusText({ kind: "Insecure", sectors: 3 }, t)).toBe("Insecure (3 sectors)");
    expect(
      statusText({ kind: "Secure", passes: 2, rereads: 1, verified_first_pass: false }, t),
    ).toBe("Secure (warning: 2 passes, 1 block re-read)");
  });

  it("one_file_is_singular_in_both_languages", async () => {
    const done = { kind: "Done", files: 1, accurate: 1, insecure: 0, folder: null } as const;
    await setLanguage("de");
    expect(summaryText(done, t)).toBe("Fertig: 1 Datei, 1 von CTDB bestätigt");
    await setLanguage("en");
    expect(summaryText(done, t)).toBe("Done: 1 file, 1 confirmed by CTDB");
  });

  it("ctdb_answer_in_words", async () => {
    await setLanguage("de");
    const base = { confidence: 0, total: 1107, in_db: true, unavailable: null };
    expect(ctdbText({ ...base, confirmed: true, confidence: 1085 }, t)).toBe(
      "CTDB: bestätigt (1085 von 1107)",
    );
    expect(ctdbText({ ...base, confirmed: false }, t)).toBe("CTDB: nicht bestätigt");
    expect(ctdbText({ ...base, confirmed: false, total: 0, in_db: false }, t)).toBe(
      "CTDB: CD nicht in der Datenbank",
    );
    expect(ctdbText({ ...base, confirmed: false, unavailable: "nicht abgefragt" }, t)).toBe(
      "CTDB: nicht abgefragt",
    );
  });
});
