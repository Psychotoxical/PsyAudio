import { create } from "zustand";
import type {
  ArResult,
  DiscInfo,
  DiscMetaView,
  DriveInfo,
  RipPhase,
  RipSummary,
  TrackStatus,
} from "../ipc/cd";

/** One row of the track list during and after a rip. */
export interface TrackRow {
  /** Share of the row done (0..1). */
  fraction: number;
  phase: RipPhase | null;
  status: TrackStatus | null;
  ar: ArResult | null;
}

export interface RipState {
  running: boolean;
  /** Date.now() at the start. */
  startedAt: number;
  summary: RipSummary | null;
  log: string[];
}

export type LookupState = "idle" | "busy" | "done" | "failed";

export interface CdState {
  drives: DriveInfo[];
  drive: string | null;
  /** The drives were listed at least once. */
  scanned: boolean;
  reading: boolean;
  error: string | null;
  disc: DiscInfo | null;
  meta: DiscMetaView;
  candidates: DiscMetaView[];
  /** Cover as a data URL. */
  cover: string | null;
  lookup: LookupState;
  lookupError: string | null;
  ar: { state: LookupState; pressings: number; reason: string | null };
  selected: boolean[];
  rows: TrackRow[];
  rip: RipState | null;
  offset: { busy: boolean; fraction: number };
}

export const emptyMeta = (): DiscMetaView => ({
  album: "",
  artist: "",
  year: "",
  genre: "",
  disc: 1,
  disc_total: 1,
  label: "",
  release_id: null,
  tracks: [],
});

export const emptyRow = (): TrackRow => ({ fraction: 0, phase: null, status: null, ar: null });

export const useCd = create<CdState>(() => ({
  drives: [],
  drive: null,
  scanned: false,
  reading: false,
  error: null,
  disc: null,
  meta: emptyMeta(),
  candidates: [],
  cover: null,
  lookup: "idle",
  lookupError: null,
  ar: { state: "idle", pressings: 0, reason: null },
  selected: [],
  rows: [],
  rip: null,
  offset: { busy: false, fraction: 0 },
}));

/** Share of the ticked tracks done, for the footer and the sidebar. */
export function ripProgress(s: Pick<CdState, "rows" | "selected">): number {
  const rows = s.rows.filter((_, i) => s.selected[i]);
  return rows.length === 0 ? 0 : rows.reduce((sum, r) => sum + r.fraction, 0) / rows.length;
}
