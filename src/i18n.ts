import i18next from "i18next";
import { initReactI18next } from "react-i18next";

/**
 * The English text is the key, as in the Rust core. Every language is one file in `locales/`:
 * `{"English": "translation"}`; a count is `{"one": …, "few": …, "many": …, "other": …}` with
 * the plural categories of its language. A text a language lacks stays English. Placeholders
 * are `{name}` as in Rust.
 */
type LanguageFile = Record<string, string | Record<string, string>>;

const files = import.meta.glob<LanguageFile>("../locales/*.json", {
  eager: true,
  import: "default",
});

/** Code and name in its own language, in the order of the settings list (as in the core). */
export const LANGUAGES: [string, string][] = [
  ["en", "English"],
  ["de", "Deutsch"],
  ["es", "Español"],
  ["fr", "Français"],
  ["nb", "Norsk bokmål"],
  ["nl", "Nederlands"],
  ["ro", "Română"],
  ["ru", "Русский"],
  ["zh", "中文"],
  ["ja", "日本語"],
  ["hu", "Magyar"],
  ["pl", "Polski"],
  ["bg", "Български"],
  ["it", "Italiano"],
  ["uk", "Українська"],
  ["el", "Ελληνικά"],
];

/** The file of a language ({} when there is none). */
export function languageFile(code: string): LanguageFile {
  return files[`../locales/${code}.json`] ?? {};
}

const texts = (file: LanguageFile) =>
  Object.fromEntries(Object.entries(file).filter(([, v]) => typeof v === "string" && v !== ""));

export const i18n = i18next.createInstance();

void i18n.use(initReactI18next).init({
  resources: Object.fromEntries(
    LANGUAGES.map(([code]) => [code, { translation: texts(languageFile(code)) }]),
  ),
  lng: "en",
  fallbackLng: false,
  keySeparator: false,
  nsSeparator: false,
  returnEmptyString: false,
  interpolation: { prefix: "{", suffix: "}", escapeValue: false },
  initAsync: false,
});

/** The text for a count in the current language: `one` and `other` are the English forms,
 * `other` is the key; the language file gives the forms its grammar needs. */
export function plural(n: number, one: string, other: string): string {
  const lang = i18n.language;
  const forms = languageFile(lang)[other];
  if (forms && typeof forms === "object") {
    const form = forms[new Intl.PluralRules(lang).select(n)] || forms.other;
    if (form) return form;
  }
  return n === 1 ? one : other;
}

/** The language a setting stands for: its code, or for "" (system) the first of the system's
 * languages PsyAudio has, else English. */
export function resolveLanguage(
  setting: string,
  system: readonly string[] = navigator.languages,
): string {
  const known = (code: string) => LANGUAGES.some(([c]) => c === code);
  if (setting && known(setting)) return setting;
  for (const tag of system) {
    const base = tag.split(/[-_]/)[0]?.toLowerCase() ?? "";
    if (known(base)) return base;
  }
  return "en";
}

export function setLanguage(lang: string): Promise<unknown> {
  return i18n.changeLanguage(lang);
}
