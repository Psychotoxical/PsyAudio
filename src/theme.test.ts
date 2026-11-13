import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { applyTheme } from "./theme";

// vitest runs from the project folder; under jsdom import.meta.url is not a file URL
const css = readFileSync(resolve(process.cwd(), "src/styles/tokens.css"), "utf8").replace(
  /\/\*[\s\S]*?\*\//g,
  "",
);

/** The custom properties of one theme block, with `var(--x)` references resolved. */
function tokens(theme: string): Record<string, string> {
  const raw: Record<string, string> = {};
  const blocks = [...css.matchAll(/([^{}]+)\{([^}]*)\}/g)];
  for (const [, selector, body] of blocks) {
    const parts = (selector ?? "").split(",").map((p) => p.trim());
    if (!parts.some((p) => p === ":root" || p === `:root[data-theme="${theme}"]`)) continue;
    for (const m of (body ?? "").matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) raw[m[1]!] = m[2]!.trim();
  }
  const resolve = (v: string, depth = 0): string => {
    const m = v.match(/^var\((--[\w-]+)\)$/);
    return m && depth < 8 ? resolve(raw[m[1]!] ?? v, depth + 1) : v;
  };
  return Object.fromEntries(Object.entries(raw).map(([k, v]) => [k, resolve(v)]));
}

function luminance(hex: string): number {
  const n = Number.parseInt(hex.replace("#", ""), 16);
  const [r, g, b] = [n >> 16, (n >> 8) & 255, n & 255].map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi! + 0.05) / (lo! + 0.05);
}

describe("theme", () => {
  it("contrast_is_readable_in_both_themes", () => {
    const pairs = [
      ["--text-primary", "--bg-app"],
      ["--text-secondary", "--bg-app"],
      ["--text-muted", "--bg-app"],
      ["--text-primary", "--bg-sidebar"],
      ["--text-muted", "--bg-sidebar"],
      ["--text-primary", "--bg-card"],
      ["--text-on-accent", "--accent"],
      ["--ok-text", "--ok-bg"],
      ["--bad-text", "--bad-bg"],
    ];
    for (const theme of ["mocha", "latte"]) {
      const t = tokens(theme);
      for (const [fg, bg] of pairs) {
        const ratio = contrast(t[fg!]!, t[bg!]!);
        expect(ratio, `${theme}: ${fg} on ${bg}`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it("applies_the_chosen_theme", () => {
    applyTheme(false);
    expect(document.documentElement.dataset.theme).toBe("latte");
    applyTheme(true);
    expect(document.documentElement.dataset.theme).toBe("mocha");
  });
});
