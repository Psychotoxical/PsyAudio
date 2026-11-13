import type { CdState } from "./store/cd";
import type { FileRow } from "./store/files";

// Sample content for `--demo=konverter` screenshots (made-up names, as in draft tiles.html).

function album(
  first: number,
  dir: string,
  artist: string,
  name: string,
  year: string,
  songs: [string, number, number][],
  format: { bits: number | null; rate: number; bitrate: number | null },
  cover: boolean,
): FileRow[] {
  return songs.map(([title, seconds, size], i) => ({
    id: first + i,
    path: `C:\\Musik\\${dir}\\${String(i + 1).padStart(2, "0")} - ${title}.${format.bits ? "flac" : "mp3"}`,
    root: null,
    size,
    tags: { title, artist, "album artist": artist, album: name, year, track: String(i + 1) },
    info: { seconds, bitrate: format.bitrate, rate: format.rate, bits: format.bits, channels: 2 },
    cover,
  }));
}

export function demoFiles(): FileRow[] {
  return [
    ...album(
      1,
      "Beispiel-Band - Erstes Album",
      "Beispiel-Band",
      "Erstes Album",
      "2021",
      [
        ["Erstes Lied", 221, 29_780_000],
        ["Zweites Lied", 242, 32_610_000],
        ["Drittes Lied", 198, 26_840_000],
        ["Viertes Lied", 307, 40_890_000],
        ["Fünftes Lied", 175, 23_380_000],
        ["Sechstes Lied", 266, 35_440_000],
        ["Siebtes Lied", 372, 49_810_000],
      ],
      { bits: 16, rate: 44100, bitrate: null },
      true,
    ),
    ...album(
      20,
      "Testensemble - Ruhige Räume",
      "Testensemble",
      "Ruhige Räume",
      "2024",
      [
        ["Raum Eins", 185, 7_440_000],
        ["Raum Zwei", 251, 10_070_000],
        ["Treppenhaus", 227, 9_120_000],
        ["Dachboden", 320, 12_900_000],
        ["Flur", 169, 6_820_000],
      ],
      { bits: null, rate: 44100, bitrate: 320 },
      false,
    ),
  ];
}

/** Sample result of a batch search (made-up folders and names, nothing on disk). */
export function demoBatch(): { folder: string; root: string; found: string[]; searched: boolean } {
  const root = "D:\\Musik\\Sammlung";
  const album = (dir: string, n: number, ext: string) =>
    Array.from(
      { length: n },
      (_, i) => `${root}\\${dir}\\${String(i + 1).padStart(2, "0")} Lied ${i + 1}.${ext}`,
    );
  return {
    folder: root,
    root,
    searched: true,
    found: [
      `${root}\\Einzelstück.flac`,
      ...album("Beispiel-Band\\Erstes Album", 9, "flac"),
      ...album("Beispiel-Band\\Live-Aufnahme", 6, "wav"),
      ...album("Testensemble\\Ruhige Räume", 5, "flac"),
    ],
  };
}

/** A sample disc halfway through a rip (made-up titles, no drive involved). */
export function demoCd(): Partial<CdState> {
  const lengths = [186, 252, 243, 242, 226, 220, 269, 233, 203, 196, 215, 258];
  const names = [
    "Erstes Lied",
    "Zweites Lied",
    "Drittes Lied",
    "Viertes Lied (Live)",
    "Fünftes Lied",
    "Sechstes Lied",
    "Siebtes Lied",
    "Achtes Lied",
    "Neuntes Lied",
    "Zehntes Lied",
    "Elftes Lied",
    "Zwölftes Lied",
  ];
  const meta = {
    album: "Erstes Album",
    artist: "Beispiel-Band",
    year: "2021",
    genre: "Rock",
    disc: 1,
    disc_total: 1,
    label: "",
    release_id: "release-1",
    tracks: names.map((title, i) => ({
      title,
      artist: i === 6 ? "Beispiel-Band feat. Gast" : "",
      isrc: "",
    })),
  };
  const secure = (ar: number) => ({
    fraction: 1,
    phase: null,
    status: { kind: "Secure" as const, passes: 1, rereads: 0, verified_first_pass: true },
    ar: { confirmed: true, confidence: ar, version: 2, in_db: true, unavailable: null },
  });
  return {
    drives: [{ id: "G", name: "G:" }],
    drive: "G",
    scanned: true,
    disc: {
      tracks: lengths.map((seconds, i) => ({ number: i + 1, seconds })),
      total_seconds: lengths.reduce((a, b) => a + b, 0),
      musicbrainz_id: "Beispiel.ID_0123456789abcdefgh-",
      freedb_id: "a10b4e0c",
    },
    meta,
    candidates: [
      meta,
      { ...meta, album: "Erstes Album (Neuauflage)", year: "2023", release_id: "release-2" },
    ],
    lookup: "done",
    ar: { state: "done", pressings: 7, reason: null },
    selected: lengths.map(() => true),
    rows: lengths.map((_, i) =>
      i === 0
        ? secure(200)
        : i === 1
          ? secure(198)
          : i === 2
            ? {
                fraction: 0.5,
                phase: { kind: "Reading" as const, done: 0.63 },
                status: null,
                ar: null,
              }
            : { fraction: 0, phase: null, status: null, ar: null },
    ),
    rip: { running: true, startedAt: Date.now() - 95_000, summary: null, log: [] },
  };
}
