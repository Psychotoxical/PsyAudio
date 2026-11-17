import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import { useEffect, useId, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { OutputMode } from "../../ipc/bindings/OutputMode";
import type { Overwrite } from "../../ipc/bindings/Overwrite";
import { openFolder } from "../../ipc/naming";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { Check, Hint, Segmented } from "../../ui/controls";

/**
 * Where converted files go and what happens when one exists already. On the batch page
 * (`batch`) the folder structure below the source folder decides, so only the folder is asked.
 */
export function SavePanel({ batch = false }: { batch?: boolean }) {
  const { t } = useTranslation();
  const folderId = useId();
  const s = useSettings((x) => x.settings);
  const update = useSettings((x) => x.update);
  const folderField = useRef<HTMLInputElement>(null);
  const folder = s?.output_folder;
  // long paths show their end: that is the part that differs
  useEffect(() => {
    const el = folderField.current;
    if (el && folder !== undefined) el.scrollLeft = el.scrollWidth;
  }, [folder]);
  if (!s) return null;

  const modeHint: Record<OutputMode, string> = {
    SingleFolder: t("All files go to the target folder; subfolders come from the naming."),
    SourceFolder: t("Each new file goes next to its source."),
    PreservePath: t(
      "The source's folder path is rebuilt inside the target folder (without the drive).",
    ),
  };
  const overwriteHint: Record<Overwrite, string> = {
    Ask: t("Before the start, a list shows the files that already exist."),
    Always: t("Existing files are replaced without asking."),
    Skip: t("Existing files stay; the source is skipped."),
  };

  const choose = async () => {
    const picked = await open({ directory: true, defaultPath: s.output_folder || undefined });
    if (typeof picked === "string")
      update((x) => {
        x.output_folder = picked;
      });
  };

  return (
    <>
      {batch ? (
        <Hint>
          {t(
            "The source folder's subfolders are rebuilt in the target folder, with the same file names.",
          )}
        </Hint>
      ) : (
        <div className="field">
          <Segmented<OutputMode>
            label={t("Destination")}
            value={s.output_mode}
            options={[
              ["SingleFolder", t("One folder")],
              ["SourceFolder", t("Source folder")],
              ["PreservePath", t("Keep path")],
            ]}
            onChange={(v) =>
              update((x) => {
                x.output_mode = v;
              })
            }
          />
          <Hint>{modeHint[s.output_mode]}</Hint>
        </div>
      )}
      {(batch || s.output_mode !== "SourceFolder") && (
        <div className="field">
          <label className="lbl" htmlFor={folderId}>
            {t("Target folder")}
          </label>
          <div className="row">
            <input
              id={folderId}
              ref={folderField}
              className="text-field path-field"
              type="text"
              readOnly
              value={s.output_folder}
              placeholder={t("Choose folder …")}
              title={s.output_folder}
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
              disabled={!s.output_folder}
              onClick={() =>
                openFolder(s.output_folder).catch((e: unknown) =>
                  useUi.getState().notify(String(e)),
                )
              }
            >
              <FolderOpen size={16} />
            </button>
          </div>
        </div>
      )}
      <div className="field">
        <Segmented<Overwrite>
          label={t("If it exists")}
          value={s.overwrite}
          options={[
            ["Ask", t("Ask")],
            ["Always", t("Overwrite")],
            ["Skip", t("Skip")],
          ]}
          onChange={(v) =>
            update((x) => {
              x.overwrite = v;
            })
          }
        />
        <Hint>{overwriteHint[s.overwrite]}</Hint>
      </div>
      <div className="field">
        <Check
          label={t("Copy tags")}
          checked={s.copy_tags}
          onChange={(v) =>
            update((x) => {
              x.copy_tags = v;
            })
          }
        />
        <Hint>{t("Writes the source's tags into the new file.")}</Hint>
      </div>
      <div className="field">
        <Check
          label={t("Copy cover")}
          checked={s.copy_art}
          onChange={(v) =>
            update((x) => {
              x.copy_art = v;
            })
          }
        />
        <Hint>{t("Embeds the source's cover when the format supports covers.")}</Hint>
      </div>
    </>
  );
}
