import { Channel, invoke } from "@tauri-apps/api/core";
import type { DriveProfile } from "./bindings/DriveProfile";
import type { Settings } from "./bindings/Settings";

/** The CD ripper's commands (src-tauri/src/cd_cmd.rs) and what they answer. */

export interface DriveInfo {
  id: string;
  name: string;
  /** Vendor and product, when the system tells them. */
  model: string | null;
}

export interface DiscInfo {
  tracks: { number: number; seconds: number }[];
  total_seconds: number;
  musicbrainz_id: string;
  freedb_id: string;
}

export interface TrackMeta {
  title: string;
  artist: string;
  isrc: string;
}

export interface DiscMetaView {
  album: string;
  artist: string;
  year: string;
  genre: string;
  disc: number;
  disc_total: number;
  label: string;
  release_id: string | null;
  tracks: TrackMeta[];
}

export type LookupEvent =
  | { kind: "Candidates"; list: DiscMetaView[] }
  | { kind: "Cover"; mime: string; data_base64: string }
  /** How many rips of the disc the CUETools database knows, and its public page. */
  | { kind: "Ctdb"; total: number; url: string }
  | { kind: "Failed"; source: "musicbrainz" | "ctdb"; reason: string };

/** Measuring a drive: progress, then its profile to store (and how many tracks agreed). */
export type MeasureEvent =
  | { kind: "Progress"; fraction: number }
  | { kind: "Done"; profile: DriveProfile; agreeing_tracks: number }
  | { kind: "Failed"; reason: string };

export type RipPhase = { kind: "Reading"; done: number } | { kind: "Encoding" };

export type TrackStatus =
  | { kind: "Secure"; passes: number; rereads: number; verified_first_pass: boolean }
  | { kind: "Burst" }
  | { kind: "Insecure"; sectors: number }
  | { kind: "Marked"; unconfirmed: boolean; was: TrackStatus }
  | { kind: "Failed"; reason: string; encoding: boolean }
  | { kind: "Cancelled" };

/** What the CUETools database says about one ripped track (rip::CtdbResult). */
export interface CtdbResult {
  confirmed: boolean;
  /** Rips with the same checksum for this track. */
  confidence: number;
  /** Rips of the disc the database knows. */
  total: number;
  in_db: boolean;
  unavailable: string | null;
}

export type RipSummary =
  | { kind: "Done"; files: number; accurate: number; insecure: number; folder: string | null }
  | { kind: "Cancelled" }
  | { kind: "Failed"; reason: string };

export type RipEvent =
  | { kind: "Progress"; row: number; fraction: number; phase: RipPhase }
  | { kind: "TrackDone"; row: number; status: TrackStatus; ctdb: CtdbResult | null }
  | { kind: "Log"; line: string }
  | { kind: "Finished"; summary: RipSummary };

export interface RipTrack {
  number: number;
  title: string;
  artist: string;
  isrc: string;
  selected: boolean;
}

export function listDrives(): Promise<DriveInfo[]> {
  return invoke<DriveInfo[]>("list_drives");
}

export function readDisc(drive: string): Promise<DiscInfo> {
  return invoke<DiscInfo>("read_disc", { drive });
}

export function lookupDisc(
  musicbrainz: boolean,
  ctdb: boolean,
  onEvent: (e: LookupEvent) => void,
): Promise<void> {
  const on = new Channel<LookupEvent>();
  on.onmessage = onEvent;
  return invoke<void>("lookup_disc", { musicbrainz, ctdb, on });
}

/** Opens the CUETools database's page of the disc read last in the browser. */
export function openCtdbPage(): Promise<void> {
  return invoke<void>("open_ctdb_page");
}

export function ejectDisc(drive: string): Promise<void> {
  return invoke<void>("eject", { drive });
}

/** Measures the drive's read offset (disc in it, CUETools database) and its buffer. */
export function measureDrive(drive: string, onEvent: (e: MeasureEvent) => void): Promise<void> {
  const on = new Channel<MeasureEvent>();
  on.onmessage = onEvent;
  return invoke<void>("measure_drive", { drive, on });
}

export function startRip(
  settings: Settings,
  tracks: RipTrack[],
  meta: DiscMetaView,
  onEvent: (e: RipEvent) => void,
): Promise<void> {
  const on = new Channel<RipEvent>();
  on.onmessage = onEvent;
  return invoke<void>("start_rip", { settings, tracks, meta, on });
}

export function cancelRip(): Promise<void> {
  return invoke<void>("cancel_rip");
}

export function previewRipName(
  settings: Settings,
  meta: DiscMetaView,
  row: number,
  number: number,
  total: number,
): Promise<string | null> {
  return invoke<string | null>("preview_rip_name", { settings, meta, row, number, total });
}
