import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { setLanguage } from "./i18n";
import { demoState, loadSettings } from "./ipc/commands";
import { frontendReady, onFilesAdded } from "./ipc/events";
import { Placeholder } from "./pages/Placeholder";
import { Sidebar } from "./shell/Sidebar";
import { Toast } from "./shell/Toast";
import "./shell/shell.css";
import { useSettings } from "./store/settings";
import { type Page, useUi } from "./store/ui";
import { applyTheme } from "./theme";

/** `--demo=<page>` names (German, as on the command line) to pages. */
const DEMO_PAGES: Record<string, Page> = {
  konverter: "convert",
  stapel: "batch",
  cd: "cd",
  einstellungen: "settings",
};

export function App() {
  const { t } = useTranslation();
  const settings = useSettings((s) => s.settings);
  const page = useUi((s) => s.page);

  useEffect(() => {
    // listen first, then load the settings, then show the window with its theme and language
    const unlisten = onFilesAdded(() => useUi.getState().setPage("convert"));
    demoState()
      .then((demo) => {
        const target = demo ? DEMO_PAGES[demo.page] : undefined;
        if (target) useUi.getState().setPage(target);
      })
      .then(loadSettings)
      .then(({ settings, warning }) => {
        useSettings.getState().load(settings);
        if (warning) useUi.getState().notify(warning);
      })
      .catch((e: unknown) => useUi.getState().notify(String(e)))
      .finally(() => {
        void frontendReady();
      });
    return () => {
      void unlisten.then((un) => un());
    };
  }, []);

  useEffect(() => {
    if (!settings) return;
    applyTheme(settings.dark);
    void setLanguage(settings.lang === "de" ? "de" : "en");
  }, [settings]);

  if (!settings) return null;

  const pages = {
    convert: (
      <Placeholder
        page="convert"
        title={t("Konvertieren")}
        tiles={[
          { id: "format", title: t("Format"), summary: settings.codec },
          { id: "save", title: t("Speichern in"), summary: settings.output_folder },
          { id: "naming", title: t("Benennung"), summary: settings.name_scheme.join(" ") },
          { id: "effects", title: t("Effekte"), summary: String(settings.effects.length) },
        ]}
      />
    ),
    batch: (
      <Placeholder
        page="batch"
        title={t("Stapel-Konverter")}
        tiles={[
          { id: "format", title: t("Format"), summary: settings.codec },
          { id: "target", title: t("Zielordner"), summary: settings.output_folder },
          { id: "effects", title: t("Effekte"), summary: String(settings.effects.length) },
        ]}
      />
    ),
    cd: (
      <Placeholder
        page="cd"
        title="CD-Ripper"
        tiles={[
          { id: "format", title: t("Format"), summary: settings.rip_codec },
          { id: "save", title: t("Speichern in"), summary: settings.rip_folder },
          { id: "naming", title: t("Benennung"), summary: settings.rip.name_scheme.join(" ") },
          { id: "readmode", title: t("Lesemodus"), summary: settings.rip.mode },
        ]}
      />
    ),
    settings: <Placeholder page="settings" title={t("Einstellungen")} tiles={[]} />,
  };

  return (
    <div className="app">
      <Sidebar />
      <main>{pages[page]}</main>
      <Toast />
    </div>
  );
}
