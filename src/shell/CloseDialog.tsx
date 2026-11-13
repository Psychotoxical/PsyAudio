import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { confirmClose, onCloseRequested } from "../ipc/close";

/** "PsyAudio beenden?" when the window is closed while converting or ripping (as in egui). */
export function CloseDialog() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  useEffect(() => {
    const off = onCloseRequested(() => setOpen(true));
    return () => {
      void off.then((un) => un());
    };
  }, []);
  if (!open) return null;
  const answer = (cancelWork: boolean) => {
    setOpen(false);
    void confirmClose(cancelWork);
  };
  return (
    <div className="overlay">
      <div
        className="picker close-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t("PsyAudio beenden?")}
        onKeyDown={(e) => {
          if (e.key === "Escape") answer(false);
        }}
      >
        <header className="picker-head">
          <h2>{t("PsyAudio beenden?")}</h2>
        </header>
        <p className="dialog-text">
          {t(
            "Es wird noch umgewandelt oder gerippt. Beim Beenden wird die Arbeit abgebrochen; fertige Dateien bleiben.",
          )}
        </p>
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={() => answer(true)}>
            {t("Abbrechen und beenden")}
          </button>
          <button
            type="button"
            className="btn primary"
            // biome-ignore lint/a11y/noAutofocus: the safe answer, as in the egui window
            autoFocus
            onClick={() => answer(false)}
          >
            {t("Weiterarbeiten")}
          </button>
        </div>
      </div>
    </div>
  );
}
