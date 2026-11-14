import { i18n, plural } from "../../i18n";
import type { DriveProfile } from "../../ipc/bindings/DriveProfile";
import { measureDrive } from "../../ipc/cd";
import { useCd } from "../../store/cd";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { setDriveProfile } from "./driveProfile";

const t = (key: string, values?: Record<string, string>) => i18n.t(key, values);
const notify = (text: string) => useUi.getState().notify(text);

/** "+667" / "-12" / "0". */
export function signed(n: number): string {
  return n > 0 ? `+${n}` : String(n);
}

function store(p: DriveProfile, agreeing: number): void {
  useSettings.getState().update((x) => {
    setDriveProfile(x.rip, p);
  });
  notify(
    t(
      plural(
        agreeing,
        "Laufwerks-Versatz {off} Samples gefunden und eingetragen (1 Titel stimmt überein)",
        "Laufwerks-Versatz {off} Samples gefunden und eingetragen ({n} Titel stimmen überein)",
      ),
      { off: signed(p.offset), n: String(agreeing) },
    ),
  );
}

/** Keep a measured profile; an offset entered by hand is only replaced after asking. */
export function applyMeasurement(p: DriveProfile, agreeing: number): void {
  const old = useSettings.getState().settings?.rip.drives.find((d) => d.model === p.model);
  if (old && old.method === "Manual" && old.offset !== p.offset) {
    useCd.setState({ pendingProfile: { profile: p, agreeing } });
    return;
  }
  store(p, agreeing);
}

/** The answer to "Ersetzen" (true) or "Behalten" (false) for a waiting measurement. */
export function settlePending(replace: boolean): void {
  const pending = useCd.getState().pendingProfile;
  useCd.setState({ pendingProfile: null });
  if (pending && replace) store(pending.profile, pending.agreeing);
}

/** Measures a drive (offset with the disc in it, buffer) and keeps the result. `auto`: started by
 * PsyAudio itself after reading a disc, so the notice says so. */
export async function measure(drive: string, auto = false): Promise<void> {
  if (useCd.getState().offset.busy) return;
  useCd.setState({ offset: { busy: true, fraction: 0 } });
  if (auto) notify(t("Laufwerk noch nicht vermessen – PsyAudio misst den Versatz mit dieser CD"));
  try {
    await measureDrive(drive, (e) => {
      if (e.kind === "Progress") useCd.setState({ offset: { busy: true, fraction: e.fraction } });
      else {
        useCd.setState({ offset: { busy: false, fraction: 0 } });
        if (e.kind === "Done") applyMeasurement(e.profile, e.agreeing_tracks);
        else notify(e.reason);
      }
    });
  } catch (e) {
    useCd.setState({ offset: { busy: false, fraction: 0 } });
    notify(String(e));
  }
}
