import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import en from "./locales/en.json";

/** Every `t("…")` text in the interface (also with the text on the next line) has an English
 * entry in tools/i18n_dict.tsv. */
function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) return sources(p);
    return /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name) ? [p] : [];
  });
}

describe("translation coverage", () => {
  it("all_texts_have_english", () => {
    const dict = en as Record<string, string>;
    const missing = new Set<string>();
    for (const file of sources(resolve(process.cwd(), "src"))) {
      const text = readFileSync(file, "utf8");
      for (const m of text.matchAll(/\bt\(\s*"((?:[^"\\]|\\.)+)"/g)) {
        const key = m[1]!.replace(/\\"/g, '"');
        if (!(key in dict)) missing.add(key);
      }
    }
    expect([...missing]).toEqual([]);
  });

  it("every_plural_has_an_english_singular", () => {
    const dict = en as Record<string, string>;
    const problems: string[] = [];
    let found = 0;
    const lit = '"((?:[^"\\\\]|\\\\.)+)"';
    const call = new RegExp(`\\bplural\\(\\s*[^,]+,\\s*${lit},\\s*${lit}`, "g");
    for (const file of sources(resolve(process.cwd(), "src"))) {
      for (const m of readFileSync(file, "utf8").matchAll(call)) {
        found++;
        const [one, many] = [m[1]!, m[2]!].map((s) => s.replace(/\\"/g, '"'));
        if (!(one in dict) || !(many in dict)) problems.push(`no English: ${one} / ${many}`);
        else if (dict[one] === dict[many]) problems.push(`English singular equals plural: ${one}`);
      }
    }
    expect(found).toBeGreaterThan(0);
    expect(problems).toEqual([]);
  });
});
