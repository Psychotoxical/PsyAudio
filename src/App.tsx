import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { demoFiles } from "./demo";
import { setLanguage } from "./i18n";
import { demoState, loadSettings } from "./ipc/commands";
import { refreshEncode } from "./ipc/encode";
import { frontendReady, onDragState, onFilesAdded } from "./ipc/events";
import { loadNamingOptions } from "./ipc/naming";
import { addFiles } from "./pages/convert/addFiles";
import { ConvertPage } from "./pages/convert/ConvertPage";
import { Placeholder } from "./pages/Placeholder";
import { Sidebar } from "./shell/Sidebar";
import { Toast } from "./shell/Toast";
import { loadEffectCatalogue } from "./store/effects";
import { useEncode } from "./store/encode";
import { useFiles } from "./store/files";
import { useRun } from "./store/run";
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
  const fileCount = useFiles((s) => s.files.length);
  const ffmpeg = useEncode((s) => s.ffmpeg);
  // the running conversion in the sidebar, as a line with a bar
  const runTitle = useRun((s) =>
    s.run && s.run.elapsed === null ? s.run.title || t("Umwandeln …") : null,
  );
  const runProgress = useRun((s) => s.run?.maxOverall ?? 0);
  const runJob = runTitle ? { label: runTitle, progress: runProgress } : null;
  const lang = settings?.lang;
  const tools = settings ? JSON.stringify(settings.tools) : undefined;

  useEffect(() => {
    // listen first, then load the settings, then show the window with its theme and language
    const listeners = [
      onFilesAdded((e) => {
        // the batch page takes a dropped folder in task 14; everything else goes to the converter
        useUi.getState().setPage("convert");
        void addFiles(e.paths);
      }),
      onDragState((e) => useUi.getState().setDragging(e.active)),
    ];
    loadNamingOptions().catch((e: unknown) => useUi.getState().notify(String(e)));
    demoState()
      .then((demo) => {
        const target = demo ? DEMO_PAGES[demo.page] : undefined;
        if (target) useUi.getState().setPage(target);
        if (target === "convert") useFiles.setState({ files: demoFiles(), selected: new Set([2]) });
      })
      .then(loadSettings)
      .then(({ settings, warning }) => {
        useSettings.getState().load(settings);
        if (warning) useUi.getState().notify(warning);
      })
      .catch((e: unknown) => useUi.getState().notify(String(e)))
      .finally(() => {
        void frontendReady().then((paths) => addFiles(paths));
      });
    return () => {
      for (const l of listeners) void l.then((un) => un());
    };
  }, []);

  useEffect(() => {
    if (!settings) return;
    applyTheme(settings.dark);
    void setLanguage(settings.lang === "de" ? "de" : "en");
  }, [settings]);

  // format lists in the current language, FFmpeg's state and missing programs; again when the
  // language or a program path changes
  // biome-ignore lint/correctness/useExhaustiveDependencies: lang and tools stand for the settings parts that matter
  useEffect(() => {
    const s = useSettings.getState().settings;
    if (!s) return;
    // Rust's language first: format and effect names come from there
    refreshEncode(s)
      .then(loadEffectCatalogue)
      .catch((e: unknown) => useUi.getState().notify(String(e)));
  }, [lang, tools]);

  if (!settings) return null;

  const pages = {
    convert: <ConvertPage />,
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
      <Sidebar
        counts={{ convert: fileCount }}
        jobs={runJob ? [runJob] : []}
        ffmpeg={ffmpeg === undefined ? undefined : ffmpeg !== null}
      />
      <main>{pages[page]}</main>
      <Toast />
    </div>
  );
}
