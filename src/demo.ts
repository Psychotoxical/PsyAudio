import type { CdState } from "./store/cd";
import type { FileRow } from "./store/files";

// Sample content for `--demo=konverter` screenshots (made-up names, as in draft tiles.html);
// `en` gives the English names of `--demo=… --en`.

/** The German or the English sample name. */
const pick = (en: boolean) => (de: string, english: string) => (en ? english : de);

function album(
  first: number,
  dir: string,
  artist: string,
  name: string,
  year: string,
  songs: [string, number, number][],
  format: { bits: number | null; rate: number; bitrate: number | null },
  cover: boolean,
  music: string,
): FileRow[] {
  return songs.map(([title, seconds, size], i) => ({
    id: first + i,
    path: `C:\\${music}\\${dir}\\${String(i + 1).padStart(2, "0")} - ${title}.${format.bits ? "flac" : "mp3"}`,
    root: null,
    size,
    tags: { title, artist, "album artist": artist, album: name, year, track: String(i + 1) },
    info: { seconds, bitrate: format.bitrate, rate: format.rate, bits: format.bits, channels: 2 },
    cover,
  }));
}

export function demoFiles(en = false): FileRow[] {
  const w = pick(en);
  const music = w("Musik", "Music");
  return [
    ...album(
      1,
      w("Beispiel-Band - Erstes Album", "Sample Band - First Album"),
      w("Beispiel-Band", "Sample Band"),
      w("Erstes Album", "First Album"),
      "2021",
      [
        [w("Erstes Lied", "First Song"), 221, 29_780_000],
        [w("Zweites Lied", "Second Song"), 242, 32_610_000],
        [w("Drittes Lied", "Third Song"), 198, 26_840_000],
        [w("Viertes Lied", "Fourth Song"), 307, 40_890_000],
        [w("Fünftes Lied", "Fifth Song"), 175, 23_380_000],
        [w("Sechstes Lied", "Sixth Song"), 266, 35_440_000],
        [w("Siebtes Lied", "Seventh Song"), 372, 49_810_000],
      ],
      { bits: 16, rate: 44100, bitrate: null },
      true,
      music,
    ),
    ...album(
      20,
      w("Testensemble - Ruhige Räume", "Test Ensemble - Quiet Rooms"),
      w("Testensemble", "Test Ensemble"),
      w("Ruhige Räume", "Quiet Rooms"),
      "2024",
      [
        [w("Raum Eins", "Room One"), 185, 7_440_000],
        [w("Raum Zwei", "Room Two"), 251, 10_070_000],
        [w("Treppenhaus", "Stairwell"), 227, 9_120_000],
        [w("Dachboden", "Attic"), 320, 12_900_000],
        [w("Flur", "Hallway"), 169, 6_820_000],
      ],
      { bits: null, rate: 44100, bitrate: 320 },
      false,
      music,
    ),
  ];
}

/** Sample result of a batch search (made-up folders and names, nothing on disk). */
export function demoBatch(en = false): {
  folder: string;
  root: string;
  found: string[];
  searched: boolean;
} {
  const w = pick(en);
  const root = w("D:\\Musik\\Sammlung", "D:\\Music\\Collection");
  const song = w("Lied", "Song");
  const album = (dir: string, n: number, ext: string) =>
    Array.from(
      { length: n },
      (_, i) => `${root}\\${dir}\\${String(i + 1).padStart(2, "0")} ${song} ${i + 1}.${ext}`,
    );
  return {
    folder: root,
    root,
    searched: true,
    found: [
      `${root}\\${w("Einzelstück", "Single")}.flac`,
      ...album(w("Beispiel-Band\\Erstes Album", "Sample Band\\First Album"), 9, "flac"),
      ...album(w("Beispiel-Band\\Live-Aufnahme", "Sample Band\\Live Recording"), 6, "wav"),
      ...album(w("Testensemble\\Ruhige Räume", "Test Ensemble\\Quiet Rooms"), 5, "flac"),
    ],
  };
}

/** A sample disc halfway through a rip (made-up titles, no drive involved). */
export function demoCd(en = false): Partial<CdState> {
  const w = pick(en);
  const lengths = [186, 252, 243, 242, 226, 220, 269, 233, 203, 196, 215, 258];
  const names = [
    w("Erstes Lied", "First Song"),
    w("Zweites Lied", "Second Song"),
    w("Drittes Lied", "Third Song"),
    w("Viertes Lied (Live)", "Fourth Song (Live)"),
    w("Fünftes Lied", "Fifth Song"),
    w("Sechstes Lied", "Sixth Song"),
    w("Siebtes Lied", "Seventh Song"),
    w("Achtes Lied", "Eighth Song"),
    w("Neuntes Lied", "Ninth Song"),
    w("Zehntes Lied", "Tenth Song"),
    w("Elftes Lied", "Eleventh Song"),
    w("Zwölftes Lied", "Twelfth Song"),
  ];
  const meta = {
    album: w("Erstes Album", "First Album"),
    artist: w("Beispiel-Band", "Sample Band"),
    year: "2021",
    genre: "Rock",
    disc: 1,
    disc_total: 1,
    label: "",
    release_id: "release-1",
    tracks: names.map((title, i) => ({
      title,
      artist: i === 6 ? w("Beispiel-Band feat. Gast", "Sample Band feat. Guest") : "",
      isrc: "",
    })),
  };
  const secure = (confidence: number) => ({
    fraction: 1,
    phase: null,
    status: { kind: "Secure" as const, passes: 1, rereads: 0, verified_first_pass: true },
    ctdb: { confirmed: true, confidence, total: 214, in_db: true, unavailable: null },
  });
  return {
    drives: [{ id: "G", name: "G:", model: w("Beispiel-Laufwerk BD-RE", "Sample Drive BD-RE") }],
    drive: "G",
    scanned: true,
    disc: {
      tracks: lengths.map((seconds, i) => ({ number: i + 1, seconds })),
      total_seconds: lengths.reduce((a, b) => a + b, 0),
      musicbrainz_id: w("Beispiel.ID_0123456789abcdefgh-", "Sample.ID_0123456789abcdefghij-"),
      freedb_id: "a10b4e0c",
    },
    meta,
    candidates: [
      meta,
      {
        ...meta,
        album: w("Erstes Album (Neuauflage)", "First Album (Reissue)"),
        year: "2023",
        release_id: "release-2",
      },
    ],
    lookup: "done",
    ctdb: { state: "done", total: 214, url: null, reason: null },
    selected: lengths.map(() => true),
    rows: lengths.map((_, i) =>
      i === 0
        ? secure(212)
        : i === 1
          ? secure(209)
          : i === 2
            ? {
                fraction: 0.5,
                phase: { kind: "Reading" as const, done: 0.63 },
                status: null,
                ctdb: null,
              }
            : { fraction: 0, phase: null, status: null, ctdb: null },
    ),
    rip: { running: true, startedAt: Date.now() - 95_000, summary: null, log: [] },
  };
}
