import { useState } from "react";
import { useTranslation } from "react-i18next";

/**
 * "Dateien sind schon vorhanden": every existing target ticked; unticked ones are kept.
 * `onStart` gets the targets to overwrite ([] = skip all existing ones).
 */
export function OverwriteDialog({
  paths,
  onStart,
  onCancel,
}: {
  paths: string[];
  onStart: (overwrite: string[]) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [ticked, setTicked] = useState<Set<string>>(() => new Set(paths));
  const toggle = (p: string) => {
    const next = new Set(ticked);
    if (next.has(p)) next.delete(p);
    else next.add(p);
    setTicked(next);
  };
  const chosen = paths.filter((p) => ticked.has(p));

  return (
    <div className="overlay">
      <div
        className="picker"
        role="dialog"
        aria-modal="true"
        aria-label={t("Dateien sind schon vorhanden")}
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
        }}
      >
        <header className="picker-head">
          <h2>{t("Dateien sind schon vorhanden")}</h2>
        </header>
        <p className="dialog-text">
          {t("{count} Zieldatei(en) gibt es bereits. Überschreiben?", {
            count: String(paths.length),
          })}
          <br />
          <span className="muted">{t("Häkchen entfernen, um eine Datei zu überspringen.")}</span>
        </p>
        <div className="row">
          <button type="button" className="btn" onClick={() => setTicked(new Set(paths))}>
            {t("Alle")}
          </button>
          <button type="button" className="btn" onClick={() => setTicked(new Set())}>
            {t("Keine")}
          </button>
        </div>
        <div className="overwrite-list">
          {paths.map((p) => (
            <label key={p} className="check">
              <input type="checkbox" checked={ticked.has(p)} onChange={() => toggle(p)} />
              <span className="mono">{p}</span>
            </label>
          ))}
        </div>
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onCancel}>
            {t("Abbrechen")}
          </button>
          <button
            type="button"
            className="btn"
            disabled={chosen.length === 0}
            onClick={() => onStart([])}
          >
            {t("Vorhandene überspringen")}
          </button>
          <button
            type="button"
            className="btn primary"
            // biome-ignore lint/a11y/noAutofocus: the dialog's main answer, as in the egui window
            autoFocus
            onClick={() => onStart(chosen)}
          >
            {chosen.length > 0
              ? t("Überschreiben ({count})", { count: String(chosen.length) })
              : t("Konvertieren")}
          </button>
        </div>
      </div>
    </div>
  );
}
