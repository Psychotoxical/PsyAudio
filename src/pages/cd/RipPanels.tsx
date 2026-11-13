import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { RipSettings } from "../../ipc/bindings/RipSettings";
import { previewRipName } from "../../ipc/cd";
import { openFolder, useNaming } from "../../ipc/naming";
import { useCd } from "../../store/cd";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { ChipEditor } from "../../ui/ChipEditor";
import { Check, Hint } from "../../ui/controls";
import { FormatPanel } from "../convert/FormatPanel";

/** Format of the rip: its own chips (with "Mehrere Formate"), settings shared with the converter. */
export function RipFormatPanel() {
  const { t } = useTranslation();
  const effects = useSettings((s) => s.settings?.effects.length ?? 0);
  return (
    <>
      <FormatPanel field="rip_codec" />
      <Hint>{t("Bitrate usw. gelten auch im Konverter.")}</Hint>
      {effects > 0 && (
        <Hint>{t("Effekte vom Konverter laufen mit ({0} aktiv).", { 0: String(effects) })}</Hint>
      )}
    </>
  );
}

/** Where rips go and what comes with them. */
export function RipSavePanel() {
  const { t } = useTranslation();
  const s = useSettings((x) => x.settings);
  const update = useSettings((x) => x.update);
  if (!s) return null;
  const set = (patch: Partial<RipSettings>) =>
    update((x) => {
      Object.assign(x.rip, patch);
    });
  const choose = async () => {
    const picked = await open({ directory: true, defaultPath: s.rip_folder || undefined });
    if (typeof picked === "string")
      update((x) => {
        x.rip_folder = picked;
      });
  };
  const checks: [keyof RipSettings, string, string][] = [
    [
      "write_log",
      t("Protokoll (.log) schreiben"),
      t("Legt ein Protokoll mit Prüfsummen und AccurateRip-Ergebnis in den Album-Ordner."),
    ],
    [
      "write_cue",
      t("CUE-Datei schreiben"),
      t("Legt eine CUE-Datei mit der Titelfolge in den Album-Ordner."),
    ],
    [
      "embed_art",
      t("Cover in Dateien einbetten"),
      t("Bettet das Cover von MusicBrainz in jede Datei ein."),
    ],
    [
      "folder_jpg",
      t("folder.jpg speichern"),
      t("Speichert das Cover zusätzlich als folder.jpg im Album-Ordner."),
    ],
  ];
  return (
    <>
      <div className="field">
        <label className="lbl" htmlFor="rip-folder">
          {t("Zielordner")}
        </label>
        <div className="row">
          <input
            id="rip-folder"
            className="text-field path-field"
            type="text"
            readOnly
            value={s.rip_folder}
            placeholder={t("Ordner wählen …")}
            title={s.rip_folder}
            onClick={() => void choose()}
          />
          <button type="button" className="btn" onClick={() => void choose()}>
            {t("Ändern")}
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label={t("Ordner im Explorer öffnen")}
            title={t("Ordner im Explorer öffnen")}
            disabled={!s.rip_folder}
            onClick={() =>
              openFolder(s.rip_folder).catch((e: unknown) => useUi.getState().notify(String(e)))
            }
          >
            <FolderOpen size={16} />
          </button>
        </div>
      </div>
      {checks.map(([key, label, hint]) => (
        <div className="field" key={key}>
          <Check label={label} checked={Boolean(s.rip[key])} onChange={(v) => set({ [key]: v })} />
          <Hint>{hint}</Hint>
        </div>
      ))}
    </>
  );
}

/** Album folders and track names from chips, with the path of the first ticked track. */
export function RipNamingPanel() {
  const { t } = useTranslation();
  const s = useSettings((x) => x.settings);
  const update = useSettings((x) => x.update);
  const naming = useNaming((x) => x.options);
  const disc = useCd((x) => x.disc);
  const meta = useCd((x) => x.meta);
  const selected = useCd((x) => x.selected);
  const [path, setPath] = useState<string | null>(null);
  useEffect(() => {
    if (!s) return;
    const row = Math.max(0, selected.indexOf(true));
    // without a disc: sample data, as the egui version showed
    const sample = {
      ...meta,
      artist: meta.artist || t("Interpret"),
      album: meta.album || "Album",
      year: meta.year || "2024",
      tracks: meta.tracks.length ? meta.tracks : [{ title: t("Titel"), artist: "", isrc: "" }],
    };
    let live = true;
    const timer = setTimeout(() => {
      previewRipName(
        s,
        sample,
        disc ? row : 0,
        disc?.tracks[row]?.number ?? 1,
        disc?.tracks.length ?? 1,
      )
        .then((p) => live && setPath(p))
        .catch(() => live && setPath(null));
    }, 150);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [s, disc, meta, selected, t]);
  if (!s || !naming) return null;
  const cut = path ? Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/")) + 1 : 0;
  return (
    <>
      <ChipEditor
        label={t("Bausteine")}
        value={s.rip.name_scheme}
        data={naming.data.filter((d) => d !== "filename")}
        separators={naming.separators}
        folders
        labels={naming.labels}
        presets={naming.presets.filter((p) => !p.scheme.includes("filename"))}
        onChange={(v) =>
          update((x) => {
            x.rip.name_scheme = v;
          })
        }
      />
      {path && (
        <div className="field">
          <p className="lbl">{t("So wird gespeichert")}</p>
          <p className="path" title={path}>
            {path.slice(0, cut)}
            <wbr />
            <em>{path.slice(cut)}</em>
          </p>
        </div>
      )}
    </>
  );
}
