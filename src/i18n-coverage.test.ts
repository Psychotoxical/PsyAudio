import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { languageFile } from "./i18n";

const en = languageFile("en");

/** Every `t("…")` text in the interface (also with the text on the next line) is a key of
 * locales/en.json, and every `plural(n, one, other)` has its count entry there. */
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
    const problems: string[] = [];
    let found = 0;
    const lit = '"((?:[^"\\\\]|\\\\.)+)"';
    const call = new RegExp(`\\bplural\\(\\s*[^,]+,\\s*${lit},\\s*${lit}`, "g");
    for (const file of sources(resolve(process.cwd(), "src"))) {
      for (const m of readFileSync(file, "utf8").matchAll(call)) {
        found++;
        const [one, many] = [m[1]!, m[2]!].map((s) => s.replace(/\\"/g, '"'));
        const entry = (en as Record<string, unknown>)[many];
        if (typeof entry !== "object" || entry === null) problems.push(`no count entry: ${many}`);
        else if ((entry as Record<string, string>).one !== one)
          problems.push(`English singular differs: ${one}`);
      }
    }
    expect(found).toBeGreaterThan(0);
    expect(problems).toEqual([]);
  });
});
