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

export function setLanguage(lang: "de" | "en"): Promise<unknown> {
  return i18n.changeLanguage(lang);
}
