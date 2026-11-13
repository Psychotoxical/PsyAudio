import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { demoBatch, demoCd, demoFiles } from "./demo";
import { setLanguage } from "./i18n";
import { demoState, loadSettings } from "./ipc/commands";
import { refreshEncode } from "./ipc/encode";
import { frontendReady, onDragState, onFilesAdded } from "./ipc/events";
import { loadNamingOptions } from "./ipc/naming";
import { BatchPage } from "./pages/batch/BatchPage";
import { CdPage } from "./pages/cd/CdPage";
import { addFiles } from "./pages/convert/addFiles";
import { ConvertPage } from "./pages/convert/ConvertPage";
import { OverwriteQuestion } from "./pages/convert/OverwriteQuestion";
import { SettingsPage } from "./pages/settings/SettingsPage";
import { Sidebar } from "./shell/Sidebar";
import { Toast } from "./shell/Toast";
import { firstFolder, useBatch } from "./store/batch";
import { ripProgress, useCd } from "./store/cd";
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
  // the rip in the sidebar too (it may run during a conversion)
  const ripRunning = useCd((s) => s.rip?.running ?? false);
  const ripDone = useCd((s) => (s.rip?.running ? ripProgress(s) : 0));
  const jobs = [
    ...(runJob ? [runJob] : []),
    ...(ripRunning ? [{ label: t("CD rippen"), progress: ripDone }] : []),
  ];
  const lang = settings?.lang;
  const tools = settings ? JSON.stringify(settings.tools) : undefined;

  useEffect(() => {
    // listen first, then load the settings, then show the window with its theme and language
    const listeners = [
      onFilesAdded((e) => {
        // a folder dropped on the batch page becomes its source; everything else goes to the converter
        void (async () => {
          if (e.source === "drop" && useUi.getState().page === "batch") {
            const folder = await firstFolder(e.paths);
            if (folder) {
              useBatch.setState({ folder });
              return;
            }
          }
          useUi.getState().setPage("convert");
          await addFiles(e.paths);
        })();
      }),
      onDragState((e) => useUi.getState().setDragging(e.active)),
    ];
    loadNamingOptions().catch((e: unknown) => useUi.getState().notify(String(e)));
    demoState()
      .then((demo) => {
        const target = demo ? DEMO_PAGES[demo.page] : undefined;
        if (target) useUi.getState().setPage(target);
        if (target === "convert") useFiles.setState({ files: demoFiles(), selected: new Set([2]) });
        if (target === "batch") useBatch.setState(demoBatch());
        if (target === "cd") useCd.setState(demoCd());
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
    batch: <BatchPage />,
    cd: <CdPage />,
    settings: <SettingsPage />,
  };

  return (
    <div className="app">
      <Sidebar
        counts={{ convert: fileCount }}
        jobs={jobs}
        ffmpeg={ffmpeg === undefined ? undefined : ffmpeg !== null}
      />
      <main>{pages[page]}</main>
      <OverwriteQuestion />
      <Toast />
    </div>
  );
}
