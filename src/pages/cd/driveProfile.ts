import type { DriveProfile } from "../../ipc/bindings/DriveProfile";
import type { OffsetMethod } from "../../ipc/bindings/OffsetMethod";
import type { RipSettings } from "../../ipc/bindings/RipSettings";
import type { DriveInfo } from "../../ipc/cd";

/** What a drive's offset is stored under: its model, else its name (as `cdrip::drive_key`). */
export function driveKey(d: DriveInfo): string {
  return d.model ?? d.name;
}

/** The drive's offset: its profile, else the single offset of earlier versions. */
export function driveOffset(rip: RipSettings, key: string | null): number {
  const p = key ? rip.drives.find((x) => x.model === key) : undefined;
  return p ? p.offset : rip.drive_offset;
}

/** Store an offset for a drive model, replacing its earlier profile (inside a settings update). */
export function withDriveOffset(
  rip: RipSettings,
  key: string,
  offset: number,
  method: OffsetMethod,
  today: string,
): void {
  const old = rip.drives.find((x) => x.model === key);
  const p = { model: key, offset, method, measured: today, buffer_kb: old?.buffer_kb ?? null };
  if (old) Object.assign(old, p);
  else rip.drives.push(p);
}

/** Store a measured profile, replacing the one of the same model (inside a settings update). */
export function setDriveProfile(rip: RipSettings, p: DriveProfile): void {
  const i = rip.drives.findIndex((x) => x.model === p.model);
  if (i >= 0) rip.drives[i] = p;
  else rip.drives.push(p);
}

/** Today as YYYY-MM-DD (UTC, like the Rust side). */
export function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}
