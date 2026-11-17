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

/** Format of the rip: its own chips (with "Several formats"), settings shared with the converter. */
export function RipFormatPanel() {
  const { t } = useTranslation();
  const effects = useSettings((s) => s.settings?.effects.length ?? 0);
  return (
    <>
      <FormatPanel field="rip_codec" />
      <Hint>{t("Bit rate etc. apply to the converter too.")}</Hint>
      {effects > 0 && (
        <Hint>{t("The converter's effects run along ({0} active).", { 0: String(effects) })}</Hint>
      )}
    </>
  );
}

/** Where rips go: the folder, "Change" and a button that opens it. */
export function RipFolderField() {
  const { t } = useTranslation();
  const folder = useSettings((x) => x.settings?.rip_folder ?? "");
  const update = useSettings((x) => x.update);
  const choose = async () => {
    const picked = await open({ directory: true, defaultPath: folder || undefined });
    if (typeof picked === "string")
      update((x) => {
        x.rip_folder = picked;
      });
  };
  return (
    <div className="field">
      <label className="lbl" htmlFor="rip-folder">
        {t("Target folder")}
      </label>
      <div className="row">
        <input
          id="rip-folder"
          className="text-field path-field"
          type="text"
          readOnly
          value={folder}
          placeholder={t("Choose folder …")}
          title={folder}
          onClick={() => void choose()}
        />
        <button type="button" className="btn" onClick={() => void choose()}>
          {t("Change")}
        </button>
        <button
          type="button"
          className="icon-btn"
          aria-label={t("Open folder in Explorer")}
          title={t("Open folder in Explorer")}
          disabled={!folder}
          onClick={() =>
            openFolder(folder).catch((e: unknown) => useUi.getState().notify(String(e)))
          }
        >
          <FolderOpen size={16} />
        </button>
      </div>
    </div>
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
  const checks: [keyof RipSettings, string, string][] = [
    [
      "write_log",
      t("Write log (.log)"),
      t("Puts a log with checksums and the CTDB result into the album folder."),
    ],
    [
      "write_cue",
      t("Write CUE file"),
      t("Puts a CUE file with the track order into the album folder."),
    ],
    ["embed_art", t("Embed cover in files"), t("Embeds the cover from MusicBrainz in every file.")],
    [
      "folder_jpg",
      t("Save folder.jpg"),
      t("Also saves the cover as folder.jpg in the album folder."),
    ],
  ];
  return (
    <>
      <RipFolderField />
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
      artist: meta.artist || t("Artist"),
      album: meta.album || "Album",
      year: meta.year || "2024",
      tracks: meta.tracks.length ? meta.tracks : [{ title: t("Title"), artist: "", isrc: "" }],
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
        label={t("Building blocks")}
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
          <p className="lbl">{t("Saved as")}</p>
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
