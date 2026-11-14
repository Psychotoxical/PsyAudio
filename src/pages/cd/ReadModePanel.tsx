import { useTranslation } from "react-i18next";
import { plural } from "../../i18n";
import type { RipMode } from "../../ipc/bindings/RipMode";
import type { RipSettings } from "../../ipc/bindings/RipSettings";
import { useCd } from "../../store/cd";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { Check, Disclosure, Hint, Segmented, Select } from "../../ui/controls";
import { SliderField } from "../../ui/SliderField";
import { methodText } from "../settings/DriveFacts";
import { driveKey } from "./driveProfile";
import { signed } from "./measure";

/** Settings with the drives box open: where the offset is measured or entered. */
function openDriveSettings(): void {
  useSettings.getState().update((s) => {
    s.ui.boxes_open.drives = true;
  });
  useUi.getState().setPage("settings");
}

/** How the disc is read: mode, re-reads, the drive's offset; the fine points under "Erweitert". */
export function ReadModePanel() {
  const { t, i18n } = useTranslation();
  const r = useSettings((s) => s.settings?.rip);
  const update = useSettings((s) => s.update);
  const offset = useCd((s) => s.offset);
  const current = useCd((s) => s.drives.find((d) => d.id === s.drive));
  if (!r) return null;
  const key = current ? driveKey(current) : null;
  const profile = key ? r.drives.find((p) => p.model === key) : undefined;
  const set = (patch: Partial<RipSettings>) =>
    update((s) => {
      Object.assign(s.rip, patch);
    });
  const secure = r.mode === "Secure";

  return (
    <>
      <div className="field">
        <Segmented<RipMode>
          label={t("Lesemodus")}
          value={r.mode}
          options={[
            ["Secure", t("Sicher")],
            ["Burst", t("Schnell (Burst)")],
          ]}
          onChange={(v) => set({ mode: v })}
        />
        <Hint>
          {secure
            ? t(
                "Liest jeden Titel in einem Zug. Bestätigt die CUETools-Datenbank das Ergebnis, ist der Titel fertig; sonst folgt ein zweiter Durchgang, und nur abweichende Stellen werden wiederholt.",
              )
            : t(
                "Schnellstes Rippen ohne Fehlerkorrektur – die CUETools-Datenbank zeigt trotzdem, ob alles stimmt.",
              )}
        </Hint>
      </div>
      <div className="field">
        <SliderField
          label={t("Neu-Lesungen bei Fehlern")}
          min={1}
          max={100}
          value={r.reread_limit}
          format={(v) => t(plural(v, "{0} Versuch", "{0} Versuche"), { 0: String(v) })}
          disabled={!secure}
          onChange={(v) => set({ reread_limit: v })}
        />
        <Hint>
          {t("So oft liest PsyAudio eine fehlerhafte Stelle neu, bevor sie als unsicher gilt.")}
        </Hint>
      </div>
      <div className="field">
        <div className="panel-offset">
          <span className="lbl">{t("Laufwerks-Versatz")}</span>
          <button type="button" className="link-btn" onClick={openDriveSettings}>
            {t("In den Einstellungen ändern")}
          </button>
        </div>
        {offset.busy ? (
          <div className="measure-row">
            <span
              className="prog wide"
              role="progressbar"
              aria-label={t("Fortschritt")}
              aria-valuenow={Math.round(offset.fraction * 100)}
            >
              <i style={{ width: `${offset.fraction * 100}%` }} />
            </span>
            <span className="muted">
              {t("Misst … {0} %", { 0: String(Math.round(offset.fraction * 100)) })}
            </span>
          </div>
        ) : profile ? (
          <p className="offset-value">
            <strong>{`${signed(profile.offset)} Samples`}</strong>{" "}
            <span className="muted">{methodText(profile, t, i18n.language)}</span>
          </p>
        ) : (
          <p className="muted">{t("noch nicht gemessen")}</p>
        )}
      </div>
      <Disclosure title={t("Erweitert")} summary={t("Kontroll-Durchgänge, Abbruch")}>
        <div className="field">
          <Check
            label={t("Zusätzliche Kontroll-Durchgänge")}
            checked={r.extra_passes}
            disabled={!secure}
            onChange={(v) => set({ extra_passes: v })}
          />
          <Hint>
            {t("Liest sichere Titel weitere Male, bis genug Durchgänge gleich ausfallen.")}
          </Hint>
        </div>
        {r.extra_passes && secure && (
          <>
            <SliderField
              label={t("Mindestens Durchgänge")}
              min={1}
              max={20}
              value={r.extra_min}
              onChange={(v) => set({ extra_min: v })}
            />
            <SliderField
              label={t("Höchstens Durchgänge")}
              min={1}
              max={20}
              value={r.extra_max}
              onChange={(v) => set({ extra_max: v })}
            />
            <SliderField
              label={t("Ende nach sauberen Durchgängen")}
              min={1}
              max={10}
              value={r.extra_clean}
              onChange={(v) => set({ extra_clean: v })}
            />
          </>
        )}
        <div className="field">
          <SliderField
            bare
            label={t("Abbrechen nach unlesbaren Sektoren")}
            min={0}
            max={100000}
            value={r.abort_unrecoverable}
            format={(v) => (v === 0 ? t("nie") : String(v))}
            disabled={!secure}
            onChange={(v) => set({ abort_unrecoverable: v })}
          />
          <Hint>
            {t(
              "0 heißt nie; sonst endet ein Titel nach so vielen Sektoren, die sich nicht lesen ließen.",
            )}
          </Hint>
        </div>
        <div className="field">
          <Check
            label={t("Unlesbare Stellen glätten (interpolieren)")}
            checked={r.interpolate}
            disabled={!secure}
            onChange={(v) => set({ interpolate: v })}
          />
          <Hint>
            {t(
              "Füllt Stellen, die sich nie lesen ließen, aus ihren Nachbarn, statt sie stumm zu lassen.",
            )}
          </Hint>
        </div>
        <div className="field">
          <Select<number>
            label={t("Titel als Fehler markieren, wenn")}
            value={r.mark_error}
            options={[
              [0, t("(aus)")],
              [1, t("unsicher")],
              [2, t("nicht von CTDB bestätigt")],
            ]}
            onChange={(v) => set({ mark_error: v })}
          />
          <Hint>{t("Ein markierter Titel zählt in der Liste und im Protokoll als Fehler.")}</Hint>
        </div>
        <div className="field">
          <Check
            label={t("Abbrechen nach unsicherem Titel")}
            checked={r.cancel_on_insecure}
            onChange={(v) => set({ cancel_on_insecure: v })}
          />
          <Hint>{t("Hört nach dem ersten unsicheren Titel mit der ganzen CD auf.")}</Hint>
        </div>
        <div className="field">
          <Check
            label={t("Mit der CUETools-Datenbank prüfen")}
            checked={r.ctdb}
            onChange={(v) => set({ ctdb: v })}
          />
          <Hint>{t("Vergleicht jeden Titel mit den Prüfsummen anderer Rips derselben CD.")}</Hint>
        </div>
        <Check
          label={t("CD danach auswerfen")}
          checked={r.eject_after}
          onChange={(v) => set({ eject_after: v })}
        />
      </Disclosure>
    </>
  );
}
