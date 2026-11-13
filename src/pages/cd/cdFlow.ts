import { i18n } from "../../i18n";
import {
  cancelRip,
  type DiscInfo,
  type DiscMetaView,
  ejectDisc,
  findOffset,
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

const t = (key: string, values?: Record<string, string>) => i18n.t(key, values);
const notify = (e: unknown) => useUi.getState().notify(String(e));

/** The album data of a freshly read disc: "Titel 01" … until MusicBrainz knows better. */
function defaultMeta(disc: DiscInfo) {
  return {
    ...emptyMeta(),
    tracks: disc.tracks.map((tr) => ({
      title: t("Titel {0}", { 0: String(tr.number).padStart(2, "0") }),
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

function onLookup(e: LookupEvent) {
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
    case "AccurateRip":
      useCd.setState({ ar: { state: "done", pressings: e.pressings, reason: null } });
      break;
    case "Failed":
      if (e.source === "musicbrainz") useCd.setState({ lookup: "failed", lookupError: e.reason });
      else useCd.setState({ ar: { state: "failed", pressings: 0, reason: e.reason } });
      break;
  }
}

/** "Titel-Infos suchen" (MusicBrainz), and after reading AccurateRip too. */
export async function searchInfo(accuraterip = false): Promise<void> {
  useCd.setState((s) => ({
    lookup: "busy",
    ar: accuraterip ? { state: "busy", pressings: 0, reason: null } : s.ar,
  }));
  try {
    await lookupDisc(true, accuraterip, onLookup);
  } catch (e) {
    useCd.setState({ lookup: "failed", lookupError: String(e) });
  }
}

/** "CD lesen": drives again, the table of contents, then the lookups. */
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
  try {
    const disc = await readDisc(drive);
    useCd.setState({
      disc,
      meta: defaultMeta(disc),
      selected: disc.tracks.map(() => true),
      rows: disc.tracks.map(emptyRow),
      lookup: "idle",
      ar: { state: "idle", pressings: 0, reason: null },
    });
    const accuraterip = useSettings.getState().settings?.rip.accuraterip ?? true;
    void searchInfo(accuraterip);
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
      if (row) rows[e.row] = { fraction: 1, phase: null, status: e.status, ar: e.ar };
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

/** "Ermitteln": finds the drive offset with this disc and enters it. */
export async function detectOffset(): Promise<void> {
  useCd.setState({ offset: { busy: true, fraction: 0 } });
  try {
    await findOffset((e) => {
      if (e.kind === "Progress") useCd.setState({ offset: { busy: true, fraction: e.fraction } });
      else {
        useCd.setState({ offset: { busy: false, fraction: 0 } });
        if (e.kind === "Found") {
          useSettings.getState().update((x) => {
            x.rip.drive_offset = e.offset;
          });
          notify(
            t(
              "Laufwerks-Versatz {off} Samples gefunden und eingetragen ({n} Titel stimmen überein)",
              {
                off: e.offset > 0 ? `+${e.offset}` : String(e.offset),
                n: String(e.matching_tracks),
              },
            ),
          );
        } else notify(e.reason);
      }
    });
  } catch (e) {
    useCd.setState({ offset: { busy: false, fraction: 0 } });
    notify(e);
  }
}
