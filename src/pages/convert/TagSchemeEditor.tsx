import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { tagsFromName, useNaming } from "../../ipc/naming";
import { useSettings } from "../../store/settings";
import { ChipEditor } from "../../ui/ChipEditor";
import { Hint } from "../../ui/controls";
import { usePreviewFile } from "./NamingPanel";

/** Chip → field name shown in the preview ("album artist" → "Album artist"). */
const FIELD_CHIP: Record<string, string> = {
  "album artist": "albumartist",
  artist: "artist",
  album: "album",
  year: "year",
  track: "track",
  disc: "disc",
  title: "title",
};

/** "Tags from file name": how file names are read, with what the first file gives. */
export function TagSchemeEditor() {
  const { t } = useTranslation();
  const s = useSettings((x) => x.settings);
  const update = useSettings((x) => x.update);
  const naming = useNaming((x) => x.options);
  const file = usePreviewFile();
  const scheme = s?.tag_scheme;
  const [found, setFound] = useState<[string, string][] | null>(null);
  useEffect(() => {
    if (!file || !scheme) return;
    let live = true;
    tagsFromName(file.path, scheme)
      .then((r) => live && setFound(r))
      .catch(() => live && setFound(null));
    return () => {
      live = false;
    };
  }, [file, scheme]);

  if (!s || !naming) return null;
  return (
    <>
      <Hint>{t("Reads tags from the file name and writes them into the source files.")}</Hint>
      <ChipEditor
        label={t("Pattern")}
        value={s.tag_scheme}
        data={naming.data.filter((d) => d !== "filename")}
        separators={naming.separators}
        folders={false}
        labels={naming.labels}
        onChange={(v) =>
          update((x) => {
            x.tag_scheme = v;
          })
        }
      />
      {file && found && (
        <div className="field">
          <p className="lbl">{t("Read from “{0}”", { 0: file.path.split(/[\\/]/).pop() ?? "" })}</p>
          {found.length === 0 ? (
            <Hint warn>{t("The file name does not match the pattern.")}</Hint>
          ) : (
            <dl className="kv">
              {found.map(([k, v]) => (
                <div key={k}>
                  <dt>{t(naming.labels[FIELD_CHIP[k] ?? k] ?? k)}</dt>
                  <dd>{v}</dd>
                </div>
              ))}
            </dl>
          )}
        </div>
      )}
    </>
  );
}
