import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { previewName, useNaming } from "../../ipc/naming";
import { type FileRow, useFiles } from "../../store/files";
import { useSettings } from "../../store/settings";
import { ChipEditor } from "../../ui/ChipEditor";
import { Hint } from "../../ui/controls";

/** The selected file, or the first one: the file previews are shown for. */
export function usePreviewFile(): FileRow | undefined {
  return useFiles((s) => {
    const first = [...s.selected].sort((a, b) => a - b)[0];
    return (first !== undefined ? s.files[first] : undefined) ?? s.files[0];
  });
}

/** Splits a path into the part before the file name and the file name. */
function splitPath(path: string): [string, string] {
  const cut = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return [path.slice(0, cut + 1), path.slice(cut + 1)];
}

/** Where the preview file would be written with the current settings. */
function PathPreview() {
  const { t } = useTranslation();
  const settings = useSettings((s) => s.settings);
  const file = usePreviewFile();
  const [target, setTarget] = useState<string | null>(null);
  useEffect(() => {
    if (!settings || !file) return;
    let live = true;
    // wait until slider and chip changes come to rest
    const timer = setTimeout(() => {
      previewName(settings, file.path, file.root)
        .then((p) => live && setTarget(p))
        .catch(() => live && setTarget(null));
    }, 150);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [settings, file]);

  if (!file) return <Hint>{t("Die Vorschau erscheint, sobald Dateien in der Liste sind.")}</Hint>;
  if (!target) return null;
  const [dir, name] = splitPath(target);
  return (
    <div className="field">
      <p className="lbl">{t("So wird gespeichert")}</p>
      <p className="path" title={target}>
        {dir}
        <wbr />
        <em>{name}</em>
      </p>
    </div>
  );
}

/** Folder and file names of the converted files. */
export function NamingPanel() {
  const { t } = useTranslation();
  const s = useSettings((x) => x.settings);
  const update = useSettings((x) => x.update);
  const naming = useNaming((x) => x.options);
  if (!s || !naming) return null;

  if (s.output_mode === "PreservePath") {
    return (
      <>
        <Hint>{t("Ordner und Dateinamen wie bei der Quelle.")}</Hint>
        <PathPreview />
      </>
    );
  }
  return (
    <>
      <ChipEditor
        label={t("Bausteine")}
        value={s.name_scheme}
        data={naming.data}
        separators={naming.separators}
        folders
        labels={naming.labels}
        presets={naming.presets}
        onChange={(v) =>
          update((x) => {
            x.name_scheme = v;
          })
        }
      />
      <PathPreview />
    </>
  );
}
