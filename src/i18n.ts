import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.json";

/**
 * The German text is the key (as in the Rust core); English comes from tools/i18n_dict.tsv via
 * tools/i18n_gen.py. A missing key shows the German text. Placeholders are `{name}` as in Rust.
 */
export const i18n = i18next.createInstance();

void i18n.use(initReactI18next).init({
  resources: { de: { translation: {} }, en: { translation: en } },
  lng: "de",
  fallbackLng: false,
  keySeparator: false,
  nsSeparator: false,
  returnEmptyString: false,
  interpolation: { prefix: "{", suffix: "}", escapeValue: false },
  initAsync: false,
});

/** The German text for a count: `one` for exactly 1, else `many` (German and English share the
 * rule). Where German has one form for both ("Titel"), `one` spells out the 1 ("1 Titel"). */
export function plural(n: number, one: string, many: string): string {
  return n === 1 ? one : many;
}

export function setLanguage(lang: "de" | "en"): Promise<unknown> {
  return i18n.changeLanguage(lang);
}
