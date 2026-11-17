import { i18n } from "../../i18n";
import {
  cancelRip,
  type DiscInfo,
  type DiscMetaView,
  ejectDisc,
  type LookupEvent,
  listDrives,
  lookupDisc,
  type RipEvent,
  readDisc,
  startRip,
} from "../../ipc/cd";
import { type CdState, emptyMeta, emptyRow, useCd } from "../../store/cd";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { driveKey } from "./driveProfile";
import { measure } from "./measure";

const t = (key: string, values?: Record<string, string>) => i18n.t(key, values);
const notify = (e: unknown) => useUi.getState().notify(String(e));

/** The album data of a freshly read disc: "Titel 01" … until MusicBrainz knows better. */
function defaultMeta(disc: DiscInfo) {
  return {
    ...emptyMeta(),
    tracks: disc.tracks.map((tr) => ({
      title: t("Track {0}", { 0: String(tr.number).padStart(2, "0") }),
      artist: "",
      isrc: "",
    })),
  };
}

/** A candidate fitted to the disc: as many track rows as the disc has. */
function fitted(meta: DiscMetaView, disc: DiscInfo): DiscMetaView {
  const base = defaultMeta(disc).tracks;
  return { ...meta, tracks: base.map((b, i) => meta.tracks[i] ?? b) };
}

export async function scanDrives(): Promise<void> {
  try {
    const drives = await listDrives();
    const current = useCd.getState().drive;
    useCd.setState({
      drives,
      scanned: true,
      drive: drives.some((d) => d.id === current) ? current : (drives[0]?.id ?? null),
    });
  } catch (e) {
    useCd.setState({ scanned: true });
    notify(e);
  }
}

/** Counts the discs read (and ejects): answers of a lookup for an older disc are dropped. */
let discNumber = 0;

export function nextDisc(): number {
  discNumber += 1;
  return discNumber;
}

/** Applies the lookup answers for disc `generation`, as long as it is still the disc shown. */
export function lookupHandler(generation: number) {
  return (e: LookupEvent) => {
    if (generation === discNumber) applyLookup(e);
  };
}

function applyLookup(e: LookupEvent) {
  const s = useCd.getState();
  switch (e.kind) {
    case "Candidates": {
      const first = e.list[0];
      useCd.setState({
        candidates: e.list,
        lookup: "done",
        lookupError: null,
        meta: first && s.disc ? fitted(first, s.disc) : s.meta,
      });
      break;
    }
    case "Cover":
      useCd.setState({ cover: `data:${e.mime};base64,${e.data_base64}` });
      break;
    case "Ctdb":
      useCd.setState({ ctdb: { state: "done", total: e.total, url: e.url, reason: null } });
      measureIfUnknown(e.total);
      break;
    case "Failed":
      if (e.source === "musicbrainz") useCd.setState({ lookup: "failed", lookupError: e.reason });
      else useCd.setState({ ctdb: { state: "failed", total: 0, url: null, reason: e.reason } });
      break;
  }
}

/** "Look up track info" (MusicBrainz), and after reading the CUETools database too. */
export async function searchInfo(ctdb = false): Promise<void> {
  useCd.setState((s) => ({
    lookup: "busy",
    ctdb: ctdb ? { state: "busy", total: 0, url: null, reason: null } : s.ctdb,
  }));
  try {
    await lookupDisc(true, ctdb, lookupHandler(discNumber));
  } catch (e) {
    useCd.setState({ lookup: "failed", lookupError: String(e) });
  }
}

/** "Read CD": drives again, the table of contents, then the lookups. */
export async function readCd(): Promise<void> {
  await scanDrives();
  const drive = useCd.getState().drive;
  if (!drive) return;
  useCd.setState({
    reading: true,
    error: null,
    disc: null,
    cover: null,
    candidates: [],
    rip: null,
  });
  nextDisc();
  try {
    const disc = await readDisc(drive);
    useCd.setState({
      disc,
      meta: defaultMeta(disc),
      selected: disc.tracks.map(() => true),
      rows: disc.tracks.map(emptyRow),
      lookup: "idle",
      ctdb: { state: "idle", total: 0, url: null, reason: null },
    });
    void searchInfo(useSettings.getState().settings?.rip.ctdb ?? true);
  } catch (e) {
    useCd.setState({ error: String(e) });
  } finally {
    useCd.setState({ reading: false });
  }
}

export async function ejectCd(): Promise<void> {
  const drive = useCd.getState().drive;
  if (!drive) return;
  try {
    await ejectDisc(drive);
    nextDisc();
    useCd.setState({ disc: null, cover: null, candidates: [], rows: [], rip: null });
  } catch (e) {
    notify(e);
  }
}

export function chooseCandidate(i: number) {
  const s = useCd.getState();
  const c = s.candidates[i];
  if (c && s.disc) useCd.setState({ meta: fitted(c, s.disc) });
}

export function setMeta(change: (m: DiscMetaView) => void) {
  const meta = structuredClone(useCd.getState().meta);
  change(meta);
  useCd.setState({ meta });
}

let queued: RipEvent[] = [];
let scheduled = false;

function applyRip(s: CdState, e: RipEvent): Partial<CdState> {
  const rows = s.rows.slice();
  switch (e.kind) {
    case "Progress": {
      const row = rows[e.row];
      if (row) rows[e.row] = { ...row, fraction: e.fraction, phase: e.phase };
      return { rows };
    }
    case "TrackDone": {
      const row = rows[e.row];
      if (row) rows[e.row] = { fraction: 1, phase: null, status: e.status, ctdb: e.ctdb };
      return { rows };
    }
    case "Log":
      return s.rip ? { rip: { ...s.rip, log: [...s.rip.log, e.line] } } : {};
    case "Finished":
      return s.rip ? { rip: { ...s.rip, running: false, summary: e.summary } } : {};
  }
}

/** Events come in bursts: applied once per frame. */
function enqueue(e: RipEvent) {
  queued.push(e);
  if (scheduled) return;
  scheduled = true;
  requestAnimationFrame(() => {
    scheduled = false;
    const events = queued;
    queued = [];
    for (const ev of events) useCd.setState((s) => applyRip(s, ev));
  });
}

/** "N Titel rippen". */
export async function rip(): Promise<void> {
  const settings = useSettings.getState().settings;
  const s = useCd.getState();
  if (!settings || !s.disc) return;
  const tracks = s.disc.tracks.map((tr, i) => ({
    number: tr.number,
    title: s.meta.tracks[i]?.title ?? "",
    artist: s.meta.tracks[i]?.artist ?? "",
    isrc: s.meta.tracks[i]?.isrc ?? "",
    selected: s.selected[i] ?? false,
  }));
  useCd.setState({
    rows: s.disc.tracks.map(emptyRow),
    rip: { running: true, startedAt: Date.now(), summary: null, log: [] },
  });
  try {
    await startRip(settings, tracks, s.meta, enqueue);
  } catch (e) {
    useCd.setState({ rip: null });
    notify(e);
  }
}

export function cancel() {
  void cancelRip();
}

/** A drive without a measured offset is measured with the first disc the CUETools database knows;
 * with a disc it does not know, the next disc is tried. */
function measureIfUnknown(total: number): void {
  const settings = useSettings.getState().settings;
  const { drives, drive, offset, rip } = useCd.getState();
  const current = drives.find((d) => d.id === drive);
  if (!settings?.rip.ctdb || !drive || !current || offset.busy || rip?.running) return;
  if (settings.rip.drives.some((p) => p.model === driveKey(current))) return;
  if (total > 0) void measure(drive, true);
  else notify(t("CTDB does not know this CD – PsyAudio measures the offset with the next CD"));
}
