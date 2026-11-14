import { open } from "@tauri-apps/plugin-dialog";
import { CircleCheck, CircleX, RefreshCw } from "lucide-react";
import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { plural, setLanguage } from "../../i18n";
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
import { Check, Hint, Segmented } from "../../ui/controls";
import { SliderField } from "../../ui/SliderField";
import { DrivesBox } from "./DrivesBox";
import "./settings.css";

const PROGRAM_LABELS: Record<Program["key"], string> = {
  ffmpeg: "FFmpeg",
  mac: "Monkey's Audio (mac.exe)",
  mpcenc: "Musepack (mpcenc.exe)",
  sox: "SoX für DSD (sox.exe)",
};

/** One program: its path (typed or picked), where it was found, and whether it was. */
function ProgramRow({ program }: { program: Program }) {
  const { t } = useTranslation();
  const id = useId();
  const configured = useSettings((s) => s.settings?.tools[program.key] ?? "");
  const update = useSettings((s) => s.update);
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
          placeholder={program.found ?? t("nicht gefunden")}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={() => draft !== null && commit(draft)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && draft !== null) commit(draft);
          }}
        />
        <button
          type="button"
          className="icon-btn"
          aria-label={t("Programm auswählen")}
          title={t("Programm auswählen")}
          onClick={() => void pick()}
        >
          …
        </button>
        <span
          className={program.found ? "prog-state ok" : "prog-state bad"}
          title={program.found ?? t("nicht gefunden")}
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
      title={t("Encoder-Programme")}
      summary={t("{0} von {1} gefunden", { 0: String(found), 1: String(progs.list.length) })}
    >
      <ProgramsFields {...progs} />
    </CollapsibleBox>
  );
}

/** What each program is for, one row per program, and "Erneut suchen". */
export function ProgramsFields({ list, again }: ReturnType<typeof usePrograms>) {
  const { t } = useTranslation();
  const ffmpeg = useEncode((s) => s.ffmpeg);
  const encoders = useEncode((s) => s.encoders);
  return (
    <>
      <Hint>
        {t(
          "WAV und AIFF schreibt PsyAudio selbst. MP3, FLAC, AAC, ALAC, Opus, Vorbis, WavPack, WMA und Speex laufen über FFmpeg; Monkey's Audio, Musepack und DSD über ihre eigenen Programme.",
        )}
      </Hint>
      {list.map((p) => (
        <ProgramRow key={p.key} program={p} />
      ))}
      {ffmpeg && (
        <p className="muted small">
          {t("FFmpeg: {0} ({1} Encoder)", { 0: ffmpeg, 1: String(encoders) })}
        </p>
      )}
      <button type="button" className="btn" onClick={() => void again()}>
        <RefreshCw size={15} />
        {t("Erneut suchen")}
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
  const kerne = (n: number) => t(plural(n, "{0} Kern", "{0} Kerne"), { 0: String(n) });
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
      title={t("Leistung")}
      summary={t("{0} gleichzeitig", { 0: kerne(s.threads) })}
    >
      <div className="field">
        <SliderField
          label={t("Gleichzeitige Umwandlungen")}
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
          {t("Dieser Rechner hat {0}; mehr gleichzeitige Umwandlungen bringen selten etwas.", {
            0: kerne(cores),
          })}
        </Hint>
      </div>
      <div className="field">
        <Check
          label={t("Mit niedriger Priorität arbeiten (PC bleibt flüssig)")}
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
          {t("Temporärer Ordner")}
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
            {t("Ändern")}
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
              {t("Standard")}
            </button>
          )}
        </div>
        <Hint>
          {t("Hier entstehen Zwischendateien, etwa für Monkey's Audio, Musepack und DSD.")}
        </Hint>
      </div>
    </CollapsibleBox>
  );
}

/** "Deutsch · Mocha": language and theme in one line. */
export function lookSummary(lang: string, dark: boolean): string {
  return `${lang === "de" ? "Deutsch" : "English"} · ${dark ? "Mocha" : "Latte"}`;
}

function LookBox() {
  const { t } = useTranslation();
  const s = useSettings((x) => x.settings);
  if (!s) return null;
  return (
    <CollapsibleBox id="look" title={t("Aussehen")} summary={lookSummary(s.lang, s.dark)}>
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
  const lang = s.lang === "de" ? "de" : "en";
  return (
    <>
      <Segmented<"en" | "de">
        label={t("Sprache")}
        value={lang}
        options={[
          ["en", "English"],
          ["de", "Deutsch"],
        ]}
        onChange={(v) => {
          update((x) => {
            x.lang = v;
          });
          // the window switches at once; Rust follows with the next format lists (App)
          void setLanguage(v);
        }}
      />
      <Segmented<boolean>
        label={t("Thema")}
        value={s.dark}
        options={[
          [true, t("Mocha, dunkel")],
          [false, t("Latte, hell")],
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
  if (!info || info.os === "macos") return null;
  return (
    <CollapsibleBox
      id="integration"
      title={info.os === "linux" ? t("Dolphin-Integration") : t("Windows-Integration")}
      summary={t("Kontextmenü")}
    >
      <IntegrationFields info={info} />
    </CollapsibleBox>
  );
}

/** Adds or removes "Mit PsyAudio konvertieren" in the file manager; `onDone` hears which worked. */
export function IntegrationFields({
  info,
  onDone,
}: {
  info: SystemInfo;
  onDone?: (added: boolean) => void;
}) {
  const { t } = useTranslation();
  const linux = info.os === "linux";
  const menu = (add: boolean) =>
    fileManagerMenu(add)
      .then((msg) => {
        useUi.getState().notify(msg);
        onDone?.(add);
      })
      .catch((e: unknown) => useUi.getState().notify(String(e)));
  return (
    <>
      <Hint>
        {linux
          ? t(
              "Fügt im Dolphin-Rechtsklickmenü von Audiodateien „Mit PsyAudio konvertieren“ hinzu (nur für Ihr Benutzerkonto).",
            )
          : t(
              "Fügt im Explorer-Rechtsklickmenü von Audiodateien „Mit PsyAudio konvertieren“ hinzu (nur für Ihr Benutzerkonto).",
            )}
      </Hint>
      <div className="row">
        <button type="button" className="btn" onClick={() => void menu(true)}>
          {t("Kontextmenü einrichten")}
        </button>
        <button type="button" className="btn" onClick={() => void menu(false)}>
          {t("Kontextmenü entfernen")}
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
      void setLanguage(fresh.lang === "de" ? "de" : "en");
      useUi.getState().notify(t("Einstellungen zurückgesetzt"));
    } catch (e) {
      useUi.getState().notify(String(e));
    } finally {
      setAsking(false);
    }
  };
  return (
    <CollapsibleBox id="reset" title={t("Zurücksetzen")} defaultOpen={false}>
      {asking ? (
        <div className="confirm">
          <p>
            {t(
              "Alle Einstellungen außer den Profilen gehen auf Standard zurück, auch Sprache und Thema.",
            )}
          </p>
          <div className="row">
            <button type="button" className="btn danger" onClick={() => void reset()}>
              {t("Jetzt zurücksetzen")}
            </button>
            <button type="button" className="btn" onClick={() => setAsking(false)}>
              {t("Abbrechen")}
            </button>
          </div>
        </div>
      ) : (
        <div className="row">
          <button type="button" className="btn" onClick={() => setAsking(true)}>
            {t("Alle Einstellungen auf Standard")}
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
        <PageHeader title={t("Einstellungen")} />
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
