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

/** Where converted files go and what happens when one exists already. */
export function SavePanel() {
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
    SingleFolder: t("Alle Dateien landen im Zielordner, Unterordner entstehen aus der Benennung."),
    SourceFolder: t("Jede neue Datei landet neben ihrer Quelle."),
    PreservePath: t(
      "Im Zielordner entsteht der Ordnerpfad der Quelle noch einmal (ohne Laufwerk).",
    ),
  };
  const overwriteHint: Record<Overwrite, string> = {
    Ask: t("Vor dem Start erscheint eine Liste der Dateien, die es schon gibt."),
    Always: t("Vorhandene Dateien werden ohne Rückfrage ersetzt."),
    Skip: t("Vorhandene Dateien bleiben; die Quelle wird übersprungen."),
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
      <div className="field">
        <Segmented<OutputMode>
          label={t("Ziel")}
          value={s.output_mode}
          options={[
            ["SingleFolder", t("Ein Ordner")],
            ["SourceFolder", t("Quellordner")],
            ["PreservePath", t("Pfad beibehalten")],
          ]}
          onChange={(v) =>
            update((x) => {
              x.output_mode = v;
            })
          }
        />
        <Hint>{modeHint[s.output_mode]}</Hint>
      </div>
      {s.output_mode !== "SourceFolder" && (
        <div className="field">
          <label className="lbl" htmlFor={folderId}>
            {t("Zielordner")}
          </label>
          <div className="row">
            <input
              id={folderId}
              ref={folderField}
              className="text-field path-field"
              type="text"
              readOnly
              value={s.output_folder}
              placeholder={t("Ordner wählen …")}
              title={s.output_folder}
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
          label={t("Wenn vorhanden")}
          value={s.overwrite}
          options={[
            ["Ask", t("Fragen")],
            ["Always", t("Überschreiben")],
            ["Skip", t("Überspringen")],
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
          label={t("Tags übernehmen")}
          checked={s.copy_tags}
          onChange={(v) =>
            update((x) => {
              x.copy_tags = v;
            })
          }
        />
        <Hint>{t("Schreibt die Tags der Quelle in die neue Datei.")}</Hint>
      </div>
      <div className="field">
        <Check
          label={t("Cover übernehmen")}
          checked={s.copy_art}
          onChange={(v) =>
            update((x) => {
              x.copy_art = v;
            })
          }
        />
        <Hint>{t("Bettet das Cover der Quelle ein, wenn das Format Cover kann.")}</Hint>
      </div>
    </>
  );
}
