import { Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { RipMode } from "../../ipc/bindings/RipMode";
import type { RipSettings } from "../../ipc/bindings/RipSettings";
import { useCd } from "../../store/cd";
import { useSettings } from "../../store/settings";
import { Check, Disclosure, Hint, Segmented, Select } from "../../ui/controls";
import { SliderField } from "../../ui/SliderField";
import { detectOffset } from "./cdFlow";

/** How the disc is read: mode, re-reads, drive offset; the fine points under "Erweitert". */
export function ReadModePanel() {
  const { t } = useTranslation();
  const r = useSettings((s) => s.settings?.rip);
  const update = useSettings((s) => s.update);
  const disc = useCd((s) => s.disc);
  const ar = useCd((s) => s.ar.state);
  const offset = useCd((s) => s.offset);
  const ripping = useCd((s) => s.rip?.running ?? false);
  if (!r) return null;
  const set = (patch: Partial<RipSettings>) =>
    update((s) => {
      Object.assign(s.rip, patch);
    });
  const secure = r.mode === "Secure";
  const canFind = !!disc && ar === "done" && !offset.busy && !ripping;

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
                "Liest jeden Titel in einem Zug. Bestätigt AccurateRip das Ergebnis, ist der Titel fertig; sonst folgt ein zweiter Durchgang, und nur abweichende Stellen werden wiederholt.",
              )
            : t(
                "Schnellstes Rippen ohne Fehlerkorrektur – AccurateRip zeigt trotzdem, ob alles stimmt.",
              )}
        </Hint>
      </div>
      <div className="field">
        <SliderField
          label={t("Neu-Lesungen bei Fehlern")}
          min={1}
          max={100}
          value={r.reread_limit}
          format={(v) => t("{0} Versuche", { 0: String(v) })}
          disabled={!secure}
          onChange={(v) => set({ reread_limit: v })}
        />
        <Hint>
          {t("So oft liest PsyAudio eine fehlerhafte Stelle neu, bevor sie als unsicher gilt.")}
        </Hint>
      </div>
      <div className="field">
        <div className="row offset-row">
          <SliderField
            bare
            label={t("Laufwerks-Versatz")}
            min={-3000}
            max={3000}
            value={r.drive_offset}
            format={(v) => (v > 0 ? `+${v}` : String(v))}
            unit="Samples"
            onChange={(v) => set({ drive_offset: v })}
          />
          <button
            type="button"
            className="btn"
            disabled={!canFind}
            title={
              canFind ? undefined : t("Zuerst eine CD lesen, die in der AccurateRip-Datenbank ist.")
            }
            onClick={() => void detectOffset()}
          >
            <Search size={15} />
            {offset.busy
              ? t("Suche … {0} %", { 0: String(Math.round(offset.fraction * 100)) })
              : t("Ermitteln")}
          </button>
        </div>
        <Hint>
          {t(
            "Jedes Laufwerk liest leicht versetzt. Einmal pro Laufwerk ermitteln, dann stimmen die AccurateRip-Prüfsummen.",
          )}
        </Hint>
      </div>
      <Disclosure title={t("Erweitert")} summary={t("Kontroll-Durchgänge, Cache, Abbruch")}>
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
            label={t("Laufwerks-Cache")}
            min={0}
            max={16384}
            value={r.cache_kb}
            unit="KB"
            disabled={!secure}
            onChange={(v) => set({ cache_kb: v })}
          />
          <Hint>
            {t(
              "So viel liest PsyAudio vor einer Neu-Lesung weg, damit das Laufwerk nicht aus seinem Zwischenspeicher antwortet.",
            )}
          </Hint>
        </div>
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
              [2, t("nicht von AccurateRip bestätigt")],
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
            label={t("Mit AccurateRip prüfen")}
            checked={r.accuraterip}
            onChange={(v) => set({ accuraterip: v })}
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
