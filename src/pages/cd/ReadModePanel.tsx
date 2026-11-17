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

/** How the disc is read: mode, re-reads, the drive's offset; the fine points under "Advanced". */
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
          label={t("Reading mode")}
          value={r.mode}
          options={[
            ["Secure", t("Secure")],
            ["Burst", t("Fast (burst)")],
          ]}
          onChange={(v) => set({ mode: v })}
        />
        <Hint>
          {secure
            ? t(
                "Reads each track in one go. If the CUETools database confirms the result, the track is done; otherwise a second pass follows and only the parts that differ are read again.",
              )
            : t(
                "Fastest ripping without error correction – the CUETools database still shows whether everything is right.",
              )}
        </Hint>
      </div>
      <div className="field">
        <SliderField
          label={t("Re-reads on errors")}
          min={1}
          max={100}
          value={r.reread_limit}
          format={(v) => t(plural(v, "{0} try", "{0} tries"), { 0: String(v) })}
          disabled={!secure}
          onChange={(v) => set({ reread_limit: v })}
        />
        <Hint>{t("How often PsyAudio re-reads a faulty spot before it counts as insecure.")}</Hint>
      </div>
      <div className="field">
        <div className="panel-offset">
          <span className="lbl">{t("Drive offset")}</span>
          <button type="button" className="link-btn" onClick={openDriveSettings}>
            {t("Change in Settings")}
          </button>
        </div>
        {offset.busy ? (
          <div className="measure-row">
            <span
              className="prog wide"
              role="progressbar"
              aria-label={t("Progress")}
              aria-valuenow={Math.round(offset.fraction * 100)}
            >
              <i style={{ width: `${offset.fraction * 100}%` }} />
            </span>
            <span className="muted">
              {t("Measuring … {0} %", { 0: String(Math.round(offset.fraction * 100)) })}
            </span>
          </div>
        ) : profile ? (
          <p className="offset-value">
            <strong>{`${signed(profile.offset)} Samples`}</strong>{" "}
            <span className="muted">{methodText(profile, t, i18n.language)}</span>
          </p>
        ) : (
          <p className="muted">{t("not measured yet")}</p>
        )}
      </div>
      <Disclosure title={t("Advanced")} summary={t("Check passes, abort")}>
        <div className="field">
          <Check
            label={t("Extra check passes")}
            checked={r.extra_passes}
            disabled={!secure}
            onChange={(v) => set({ extra_passes: v })}
          />
          <Hint>{t("Reads secure tracks more times until enough passes agree.")}</Hint>
        </div>
        {r.extra_passes && secure && (
          <>
            <SliderField
              label={t("Minimum passes")}
              min={1}
              max={20}
              value={r.extra_min}
              onChange={(v) => set({ extra_min: v })}
            />
            <SliderField
              label={t("Maximum passes")}
              min={1}
              max={20}
              value={r.extra_max}
              onChange={(v) => set({ extra_max: v })}
            />
            <SliderField
              label={t("Stop after clean passes")}
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
            label={t("Abort after unreadable sectors")}
            min={0}
            max={100000}
            value={r.abort_unrecoverable}
            format={(v) => (v === 0 ? t("never") : String(v))}
            disabled={!secure}
            onChange={(v) => set({ abort_unrecoverable: v })}
          />
          <Hint>
            {t(
              "0 means never; otherwise a track ends after this many sectors that could not be read.",
            )}
          </Hint>
        </div>
        <div className="field">
          <Check
            label={t("Smooth unreadable parts (interpolate)")}
            checked={r.interpolate}
            disabled={!secure}
            onChange={(v) => set({ interpolate: v })}
          />
          <Hint>
            {t(
              "Fills spots that could never be read from their neighbours instead of leaving them silent.",
            )}
          </Hint>
        </div>
        <div className="field">
          <Select<number>
            label={t("Mark track as error if")}
            value={r.mark_error}
            options={[
              [0, t("(off)")],
              [1, t("insecure")],
              [2, t("not confirmed by CTDB")],
            ]}
            onChange={(v) => set({ mark_error: v })}
          />
          <Hint>{t("A marked track counts as an error in the list and the log.")}</Hint>
        </div>
        <div className="field">
          <Check
            label={t("Stop after an insecure track")}
            checked={r.cancel_on_insecure}
            onChange={(v) => set({ cancel_on_insecure: v })}
          />
          <Hint>{t("Stops the whole CD after the first insecure track.")}</Hint>
        </div>
        <div className="field">
          <Check
            label={t("Verify with the CUETools database")}
            checked={r.ctdb}
            onChange={(v) => set({ ctdb: v })}
          />
          <Hint>{t("Compares each track with the checksums of other rips of the same CD.")}</Hint>
        </div>
        <Check
          label={t("Eject the CD afterwards")}
          checked={r.eject_after}
          onChange={(v) => set({ eject_after: v })}
        />
      </Disclosure>
    </>
  );
}
