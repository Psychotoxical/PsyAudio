import { open } from "@tauri-apps/plugin-dialog";
import { CircleCheck, CircleX, RefreshCw } from "lucide-react";
import { Fragment, useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { LANGUAGES, plural, resolveLanguage, setLanguage } from "../../i18n";
import type { Tools } from "../../ipc/bindings/Tools";
import { refreshEncode } from "../../ipc/encode";
import { fileManagerMenu } from "../../ipc/events";
import {
  type Program,
  programs,
  resetSettings,
  type SystemInfo,
  systemInfo,
} from "../../ipc/system";
import { PageHeader } from "../../shell/PageHeader";
import { useEncode } from "../../store/encode";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { CollapsibleBox } from "../../ui/CollapsibleBox";
import { Check, Hint, Segmented, Select } from "../../ui/controls";
import { SliderField } from "../../ui/SliderField";
import { DrivesBox } from "./DrivesBox";
import { FfmpegInstallHint } from "./FfmpegInstallHint";
import "./settings.css";

const PROGRAM_LABELS: Record<Program["key"], string> = {
  ffmpeg: "FFmpeg",
  mac: "Monkey's Audio (mac.exe)",
  mpcenc: "Musepack (mpcenc.exe)",
  sox: "SoX for DSD (sox.exe)",
};

/** One program: its path (typed or picked), where it was found, and whether it was. */
function ProgramRow({ program }: { program: Program }) {
  const { t } = useTranslation();
  const id = useId();
  const configured = useSettings((s) => s.settings?.tools[program.key] ?? "");
  const update = useSettings((s) => s.update);
  // the shipped FFmpeg's path changes with every AppImage start: name it instead
  const shipped = useEncode((s) => program.key === "ffmpeg" && s.ffmpegSource === "bundled");
  const found = program.found && shipped ? t("bundled") : program.found;
  const [draft, setDraft] = useState<string | null>(null);
  const commit = (value: string) => {
    setDraft(null);
    if (value !== configured)
      update((s) => {
        s.tools[program.key as keyof Tools] = value.trim();
      });
  };
  const pick = async () => {
    const picked = await open({
      multiple: false,
      defaultPath: configured || undefined,
      filters: [{ name: PROGRAM_LABELS[program.key], extensions: ["exe", "*"] }],
    });
    if (typeof picked === "string") commit(picked);
  };
  return (
    <div className="field">
      <label className="lbl" htmlFor={id}>
        {t(PROGRAM_LABELS[program.key])}
      </label>
      <div className="row">
        <input
          id={id}
          className="text-field"
          type="text"
          value={draft ?? configured}
          placeholder={found ?? t("not found")}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={() => draft !== null && commit(draft)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && draft !== null) commit(draft);
          }}
        />
        <button
          type="button"
          className="icon-btn"
          aria-label={t("Choose program")}
          title={t("Choose program")}
          onClick={() => void pick()}
        >
          …
        </button>
        <span
          className={program.found ? "prog-state ok" : "prog-state bad"}
          title={found ?? t("not found")}
        >
          {program.found ? <CircleCheck size={18} /> : <CircleX size={18} />}
        </span>
      </div>
    </div>
  );
}

/** The encoder programs with where they were found; `again` searches anew. */
export function usePrograms(): { list: Program[]; again: () => Promise<void> } {
  const settings = useSettings((s) => s.settings);
  const [list, setList] = useState<Program[]>([]);
  const tools = settings ? JSON.stringify(settings.tools) : "";
  // biome-ignore lint/correctness/useExhaustiveDependencies: tools stands for settings.tools
  useEffect(() => {
    const s = useSettings.getState().settings;
    if (!s) return;
    programs(s)
      .then(setList)
      .catch(() => setList([]));
  }, [tools]);
  const again = async () => {
    const s = useSettings.getState().settings;
    if (!s) return;
    try {
      setList(await programs(s, true));
      await refreshEncode(s, true);
    } catch (e) {
      useUi.getState().notify(String(e));
    }
  };
  return { list, again };
}

function ProgramsBox() {
  const { t } = useTranslation();
  const progs = usePrograms();
  const found = progs.list.filter((p) => p.found).length;
  return (
    <CollapsibleBox
      id="programs"
      title={t("Encoder programs")}
      summary={t("{0} of {1} found", { 0: String(found), 1: String(progs.list.length) })}
    >
      <ProgramsFields {...progs} />
    </CollapsibleBox>
  );
}

/** What each program is for, one row per program, and "Search again". */
export function ProgramsFields({ list, again }: ReturnType<typeof usePrograms>) {
  const { t } = useTranslation();
  const ffmpeg = useEncode((s) => s.ffmpeg);
  const encoders = useEncode((s) => s.encoders);
  const source = useEncode((s) => s.ffmpegSource);
  const fallback = useEncode((s) => s.ffmpegFallback);
  return (
    <>
      <Hint>
        {t(
          "PsyAudio writes WAV and AIFF itself. MP3, FLAC, AAC, ALAC, Opus, Vorbis, WavPack, WMA and Speex go through FFmpeg; Monkey's Audio, Musepack and DSD through their own programs.",
        )}
      </Hint>
      {list.map((p) => (
        <Fragment key={p.key}>
          <ProgramRow program={p} />
          {/* null: looked for and not found (undefined: still looking) */}
          {p.key === "ffmpeg" && ffmpeg === null && <FfmpegInstallHint />}
        </Fragment>
      ))}
      {ffmpeg && (
        <p className="muted small">
          {source === "bundled"
            ? t("FFmpeg: bundled ({1} encoders)", { 1: String(encoders) })
            : t("FFmpeg: {0} ({1} encoders)", { 0: ffmpeg, 1: String(encoders) })}
        </p>
      )}
      {ffmpeg && fallback && (
        <p className="muted small">{t("The bundled FFmpeg stands by as a fallback.")}</p>
      )}
      <button type="button" className="btn" onClick={() => void again()}>
        <RefreshCw size={15} />
        {t("Search again")}
      </button>
    </>
  );
}

function PerformanceBox({ info }: { info: SystemInfo | null }) {
  const { t } = useTranslation();
  const s = useSettings((x) => x.settings);
  const update = useSettings((x) => x.update);
  if (!s) return null;
  const cores = info?.cores ?? 8;
  const max = Math.min(cores * 2, 64);
  const kerne = (n: number) => t(plural(n, "{0} core", "{0} cores"), { 0: String(n) });
  const chooseTemp = async () => {
    const picked = await open({
      directory: true,
      defaultPath: s.temp_folder || info?.temp_default,
    });
    if (typeof picked === "string")
      update((x) => {
        x.temp_folder = picked;
      });
  };
  return (
    <CollapsibleBox
      id="performance"
      title={t("Performance")}
      summary={t("{0} at a time", { 0: kerne(s.threads) })}
    >
      <div className="field">
        <SliderField
          label={t("Simultaneous conversions")}
          min={1}
          max={max}
          value={Math.min(s.threads, max)}
          format={kerne}
          onChange={(v) =>
            update((x) => {
              x.threads = v;
            })
          }
        />
        <Hint>
          {t("This computer has {0}; more simultaneous conversions rarely help.", {
            0: kerne(cores),
          })}
        </Hint>
      </div>
      <div className="field">
        <Check
          label={t("Work at low priority (PC stays responsive)")}
          checked={s.low_priority}
          onChange={(v) =>
            update((x) => {
              x.low_priority = v;
            })
          }
        />
      </div>
      <div className="field">
        <label className="lbl" htmlFor="temp-folder">
          {t("Temporary folder")}
        </label>
        <div className="row">
          <input
            id="temp-folder"
            className="text-field path-field"
            type="text"
            readOnly
            value={s.temp_folder}
            placeholder={info?.temp_default ?? ""}
            onClick={() => void chooseTemp()}
          />
          <button type="button" className="btn" onClick={() => void chooseTemp()}>
            {t("Change")}
          </button>
          {s.temp_folder && (
            <button
              type="button"
              className="btn"
              onClick={() =>
                update((x) => {
                  x.temp_folder = "";
                })
              }
            >
              {t("Default")}
            </button>
          )}
        </div>
        <Hint>{t("Intermediate files go here, e.g. for Monkey's Audio, Musepack and DSD.")}</Hint>
      </div>
    </CollapsibleBox>
  );
}

/** "Deutsch · Mocha": language and theme in one line. */
export function lookSummary(lang: string, dark: boolean): string {
  const code = resolveLanguage(lang);
  const name = LANGUAGES.find(([c]) => c === code)?.[1] ?? "English";
  return `${name} · ${dark ? "Mocha" : "Latte"}`;
}

function LookBox() {
  const { t } = useTranslation();
  const s = useSettings((x) => x.settings);
  if (!s) return null;
  return (
    <CollapsibleBox id="look" title={t("Appearance")} summary={lookSummary(s.lang, s.dark)}>
      <LookFields />
    </CollapsibleBox>
  );
}

/** Language and theme; both change the window at once. */
export function LookFields() {
  const { t } = useTranslation();
  const s = useSettings((x) => x.settings);
  const update = useSettings((x) => x.update);
  if (!s) return null;
  const lang = LANGUAGES.some(([c]) => c === s.lang) ? s.lang : "";
  return (
    <>
      <Select<string>
        label={t("Language")}
        value={lang}
        options={[
          [
            "",
            t("System language ({0})", {
              0: LANGUAGES.find(([c]) => c === resolveLanguage(""))?.[1] ?? "",
            }),
          ],
          ...LANGUAGES,
        ]}
        onChange={(v) => {
          update((x) => {
            x.lang = v;
          });
          // the window switches at once; Rust follows with the next format lists (App)
          void setLanguage(resolveLanguage(v));
        }}
      />
      <Segmented<boolean>
        label={t("Theme")}
        value={s.dark}
        options={[
          [true, t("Mocha, dark")],
          [false, t("Latte, light")],
        ]}
        onChange={(v) =>
          update((x) => {
            x.dark = v;
          })
        }
      />
    </>
  );
}

function IntegrationBox({ info }: { info: SystemInfo | null }) {
  const { t } = useTranslation();
  if (!info) return null;
  const title = {
    windows: t("Windows integration"),
    linux: t("Dolphin integration"),
    macos: t("Finder integration"),
  }[info.os];
  return (
    <CollapsibleBox id="integration" title={title} summary={t("Context menu")}>
      <IntegrationFields info={info} />
    </CollapsibleBox>
  );
}

/** Adds or removes "Convert with PsyAudio" in the file manager; `onDone` hears which worked. */
export function IntegrationFields({
  info,
  onDone,
}: {
  info: SystemInfo;
  onDone?: (added: boolean) => void;
}) {
  const { t } = useTranslation();
  const menu = (add: boolean) =>
    fileManagerMenu(add)
      .then((msg) => {
        useUi.getState().notify(msg);
        onDone?.(add);
      })
      .catch((e: unknown) => useUi.getState().notify(String(e)));
  const hint = {
    windows: t(
      "Adds “Convert with PsyAudio” to the Explorer right-click menu of audio files (for your user account only).",
    ),
    linux: t(
      "Adds “Convert with PsyAudio” to the Dolphin right-click menu of audio files (for your user account only).",
    ),
    macos: t(
      "Adds the quick action “Convert with PsyAudio” to the Finder right-click menu of audio files (for your user account only).",
    ),
  }[info.os];
  return (
    <>
      <Hint>{hint}</Hint>
      <div className="row">
        <button type="button" className="btn" onClick={() => void menu(true)}>
          {t("Set up context menu")}
        </button>
        <button type="button" className="btn" onClick={() => void menu(false)}>
          {t("Remove context menu")}
        </button>
      </div>
    </>
  );
}

function ResetBox() {
  const { t } = useTranslation();
  const update = useSettings((x) => x.update);
  const [asking, setAsking] = useState(false);
  const reset = async () => {
    const current = useSettings.getState().settings;
    if (!current) return;
    try {
      const fresh = await resetSettings(current);
      update((x) => {
        Object.assign(x, fresh);
      });
      void setLanguage(resolveLanguage(fresh.lang));
      useUi.getState().notify(t("Settings reset"));
    } catch (e) {
      useUi.getState().notify(String(e));
    } finally {
      setAsking(false);
    }
  };
  return (
    <CollapsibleBox id="reset" title={t("Reset")} defaultOpen={false}>
      {asking ? (
        <div className="confirm">
          <p>
            {t(
              "All settings except the profiles go back to their defaults, language and theme too.",
            )}
          </p>
          <div className="row">
            <button type="button" className="btn danger" onClick={() => void reset()}>
              {t("Reset now")}
            </button>
            <button type="button" className="btn" onClick={() => setAsking(false)}>
              {t("Cancel")}
            </button>
          </div>
        </div>
      ) : (
        <div className="row">
          <button type="button" className="btn" onClick={() => setAsking(true)}>
            {t("Reset all settings to default")}
          </button>
        </div>
      )}
    </CollapsibleBox>
  );
}

/** This computer's cores, temp folder and system, once loaded. */
export function useSystemInfo(): SystemInfo | null {
  const [info, setInfo] = useState<SystemInfo | null>(null);
  useEffect(() => {
    systemInfo()
      .then(setInfo)
      .catch(() => setInfo(null));
  }, []);
  return info;
}

/** Programs, performance, drives, look, file manager menu and reset, each in a folding box. */
export function SettingsPage() {
  const { t } = useTranslation();
  const info = useSystemInfo();
  return (
    <div className="page">
      <div className="page-main">
        <PageHeader title={t("Settings")} />
        {/* two groups: one under the other, side by side once the window is wide (settings.css) */}
        <div className="content settings">
          <div className="settings-grid">
            <div className="settings-col">
              <ProgramsBox />
              <PerformanceBox info={info} />
            </div>
            <div className="settings-col">
              <DrivesBox />
              <LookBox />
              <IntegrationBox info={info} />
              <ResetBox />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
