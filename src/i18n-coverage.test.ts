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
});
