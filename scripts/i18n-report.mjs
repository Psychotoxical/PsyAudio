// How complete each language file in locales/ is, compared with English.
//   npm run i18n:report            every language
//   npm run i18n:report -- de fr   only these, with the missing texts listed
import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const dir = resolve(import.meta.dirname, "..", "locales");
const read = (code) => JSON.parse(readFileSync(join(dir, `${code}.json`), "utf8"));
const en = read("en");
const keys = Object.keys(en);
const only = process.argv.slice(2);
const codes = readdirSync(dir)
  .filter((f) => f.endsWith(".json") && f !== "en.json")
  .map((f) => f.slice(0, -5))
  .filter((c) => only.length === 0 || only.includes(c))
  .sort();

const filled = (v) =>
  typeof v === "string" ? v !== "" : v && typeof v === "object" && Object.values(v).some((x) => x);

for (const code of codes) {
  const file = read(code);
  const missing = keys.filter((k) => !filled(file[k]));
  const unknown = Object.keys(file).filter((k) => !(k in en));
  const done = keys.length - missing.length;
  const pct = Math.floor((done / keys.length) * 100);
  console.log(`${code}  ${String(pct).padStart(3)} %  ${done}/${keys.length}${unknown.length ? `  (${unknown.length} unknown keys)` : ""}`);
  if (only.length) {
    for (const k of missing) console.log(`   missing: ${k}`);
    for (const k of unknown) console.log(`   unknown: ${k}`);
  }
}
