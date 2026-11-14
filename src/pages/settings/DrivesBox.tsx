import { HardDrive, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { DriveProfile } from "../../ipc/bindings/DriveProfile";
import { cancelRip, type DriveInfo, listDrives } from "../../ipc/cd";
import { useCd } from "../../store/cd";
import { useSettings } from "../../store/settings";
import { CollapsibleBox } from "../../ui/CollapsibleBox";
import { Disclosure, Hint } from "../../ui/controls";
import { SliderField } from "../../ui/SliderField";
import { driveKey, driveOffset, todayIso, withDriveOffset } from "../cd/driveProfile";
import { measure, settlePending, signed } from "../cd/measure";
import { DriveFacts } from "./DriveFacts";

/** What PsyAudio reads before a re-read when the setting is automatic (as scsi::flush_kb). */
export function autoFlushKb(bufferKb: number | null | undefined): number {
  return bufferKb ? Math.max(Math.floor((bufferKb * 3) / 2), 1024) : 8192;
}

/** The CD drives attached now, each with what was measured. */
export function useAttachedDrives(): DriveInfo[] {
  const [attached, setAttached] = useState<DriveInfo[]>([]);
  useEffect(() => {
    listDrives()
      .then(setAttached)
      .catch(() => setAttached([]));
  }, []);
  return attached;
}

/** One drive: name, what was measured, and "Messen" while it is attached. `titled={false}` leaves
 * the name to the surrounding box (setup). */
export function DriveRow({
  drive,
  profile,
  titled = true,
}: {
  drive?: DriveInfo;
  profile?: DriveProfile;
  titled?: boolean;
}) {
  const { t } = useTranslation();
  const offset = useCd((s) => s.offset);
  const ripping = useCd((s) => s.rip?.running ?? false);
  const update = useSettings((s) => s.update);
  const name = profile?.model ?? (drive ? driveKey(drive) : "");
  return (
    <div className="drive-row">
      {titled && (
        <div className="drive-title">
          <HardDrive size={16} />
          <span>{name}</span>
          {drive && <small>{drive.name}</small>}
        </div>
      )}
      {profile ? (
        <DriveFacts profile={profile} />
      ) : (
        <p className="muted">{t("Versatz noch nicht gemessen")}</p>
      )}
      {offset.busy && drive ? (
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
            {t("Liest die CD und vergleicht mit CTDB … {0} %", {
              0: String(Math.round(offset.fraction * 100)),
            })}
          </span>
          <button type="button" className="btn" onClick={() => void cancelRip()}>
            {t("Abbrechen")}
          </button>
        </div>
      ) : (
        <div className="row">
          {drive && (
            <button
              type="button"
              className="btn"
              disabled={offset.busy || ripping}
              onClick={() => void measure(drive.id)}
            >
              <RefreshCw size={15} />
              {profile ? t("Neu messen") : t("Messen")}
            </button>
          )}
          {!drive && profile && (
            <button
              type="button"
              className="btn"
              onClick={() =>
                update((s) => {
                  s.rip.drives = s.rip.drives.filter((p) => p.model !== profile.model);
                })
              }
            >
              {t("Entfernen")}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

/** A measurement waiting to replace an offset entered by hand. */
export function PendingQuestion() {
  const { t } = useTranslation();
  const pending = useCd((s) => s.pendingProfile);
  const old = useSettings((s) =>
    s.settings?.rip.drives.find((p) => p.model === pending?.profile.model),
  );
  if (!pending) return null;
  return (
    <div className="confirm ask">
      <p>
        {t("Gemessen: {0} Samples. Ihren Wert {1} ersetzen?", {
          0: signed(pending.profile.offset),
          1: signed(old?.offset ?? 0),
        })}
      </p>
      <div className="row">
        <button type="button" className="btn" onClick={() => settlePending(true)}>
          {t("Ersetzen")}
        </button>
        <button type="button" className="btn" onClick={() => settlePending(false)}>
          {t("Behalten")}
        </button>
      </div>
    </div>
  );
}

/** Settings: every drive with its offset and buffer, measuring, the offset by hand and the cache. */
export function DrivesBox() {
  const { t } = useTranslation();
  const s = useSettings((x) => x.settings);
  const update = useSettings((x) => x.update);
  const attached = useAttachedDrives();
  if (!s) return null;
  const r = s.rip;
  const keys = [...new Set([...attached.map(driveKey), ...r.drives.map((p) => p.model)])];
  const rows = keys.map((key) => ({
    key,
    drive: attached.find((d) => driveKey(d) === key),
    profile: r.drives.find((p) => p.model === key),
  }));
  const first = rows[0];
  const summary = !first
    ? t("kein Laufwerk")
    : first.profile
      ? `${first.key} · ${signed(first.profile.offset)} Samples`
      : `${first.key} · ${t("noch nicht gemessen")}`;
  const buffer = rows.find((x) => x.drive)?.profile?.buffer_kb;
  return (
    <CollapsibleBox id="drives" title={t("CD-Laufwerke")} summary={summary}>
      {rows.length === 0 && <Hint>{t("Kein CD-Laufwerk gefunden.")}</Hint>}
      {rows.map((x) => (
        <DriveRow key={x.key} drive={x.drive} profile={x.profile} />
      ))}
      <PendingQuestion />
      <Disclosure title={t("Erweitert")} summary={t("Versatz von Hand, Cache")}>
        {rows
          .filter((x) => x.drive)
          .map((x) => (
            <div className="field" key={x.key}>
              <SliderField
                label={
                  rows.filter((y) => y.drive).length > 1
                    ? t("Versatz von Hand ({0})", { 0: x.key })
                    : t("Versatz von Hand")
                }
                min={-3000}
                max={3000}
                value={driveOffset(r, x.key)}
                format={(v) => signed(v)}
                unit="Samples"
                onChange={(v) =>
                  update((st) => {
                    withDriveOffset(st.rip, x.key, v, "Manual", todayIso());
                  })
                }
              />
              <Hint>
                {t(
                  "Nur nötig, wenn keine Ihrer CDs in der CUETools-Datenbank steht. Ein späteres Messen fragt, bevor es Ihren Wert ersetzt.",
                )}
              </Hint>
            </div>
          ))}
        <div className="field">
          <SliderField
            label={t("Cache vor dem Neulesen leeren")}
            min={0}
            max={16384}
            step={512}
            value={r.cache_kb}
            format={(v) =>
              v === 0 ? t("automatisch · {0} KB", { 0: String(autoFlushKb(buffer)) }) : `${v} KB`
            }
            onChange={(v) =>
              update((st) => {
                st.rip.cache_kb = v;
              })
            }
          />
          <Hint>
            {t(
              "Bevor PsyAudio eine Stelle neu liest, liest es so viel woanders, dass das Laufwerk die Stelle wirklich von der CD holt. Automatisch: Puffer des Laufwerks plus die Hälfte.",
            )}
          </Hint>
        </div>
      </Disclosure>
    </CollapsibleBox>
  );
}
