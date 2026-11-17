import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { confirmClose, onCloseNow, onCloseRequested } from "../ipc/close";
import { flushSettings } from "../store/settings";

/** Saves the settings still waiting, then lets Rust stop the work and close. */
async function quit() {
  await flushSettings();
  await confirmClose(true);
}

/** "Quit PsyAudio?" when the window is closed while converting or ripping (as in egui). */
export function CloseDialog() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  useEffect(() => {
    const offs = [
      onCloseRequested(() => setOpen(true)),
      // nothing runs: a settings change still waiting for its save goes to disk first
      onCloseNow(() => void quit()),
    ];
    return () => {
      for (const off of offs) void off.then((un) => un());
    };
  }, []);
  if (!open) return null;
  const answer = (cancelWork: boolean) => {
    setOpen(false);
    if (cancelWork) void quit();
    else void confirmClose(false);
  };
  return (
    <div className="overlay">
      <div
        className="picker close-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t("Quit PsyAudio?")}
        onKeyDown={(e) => {
          if (e.key === "Escape") answer(false);
        }}
      >
        <header className="picker-head">
          <h2>{t("Quit PsyAudio?")}</h2>
        </header>
        <p className="dialog-text">
          {t("Converting or ripping is still running. Quitting cancels it; finished files stay.")}
        </p>
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={() => answer(true)}>
            {t("Cancel and quit")}
          </button>
          <button
            type="button"
            className="btn primary"
            // biome-ignore lint/a11y/noAutofocus: the safe answer, as in the egui window
            autoFocus
            onClick={() => answer(false)}
          >
            {t("Keep working")}
          </button>
        </div>
      </div>
    </div>
  );
}
