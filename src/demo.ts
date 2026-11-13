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
