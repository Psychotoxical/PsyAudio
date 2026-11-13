import { describe, expect, it } from "vitest";
import type { FileRow } from "../../store/files";
import { groupRuns } from "./groupRuns";

let next = 0;
function file(path: string, tags?: Record<string, string>): FileRow {
  next += 1;
  return { id: next, path, root: null, size: 1, tags, info: undefined, cover: false };
}

const kinds = (rows: ReturnType<typeof groupRuns>) =>
  rows.map((r) => (r.kind === "group" ? `G:${r.title}` : `F:${r.index}`));

describe("groupRuns", () => {
  it("header_where_album_changes", () => {
    const rows = groupRuns([
      file("/m/a/1.flac", { "album artist": "Band", album: "Erstes" }),
      file("/m/a/2.flac", { "album artist": "Band", album: "Erstes" }),
      file("/m/b/1.mp3", { artist: "Ensemble", album: "Zweites" }),
    ]);
    expect(kinds(rows)).toEqual(["G:Band – Erstes", "F:0", "F:1", "G:Ensemble – Zweites", "F:2"]);
  });

  it("no_reordering", () => {
    const a = { "album artist": "Band", album: "A" };
    const b = { "album artist": "Band", album: "B" };
    const rows = groupRuns([file("/1", a), file("/2", b), file("/3", a)]);
    expect(kinds(rows)).toEqual(["G:Band – A", "F:0", "G:Band – B", "F:1", "G:Band – A", "F:2"]);
  });

  it("folder_when_untagged", () => {
    const rows = groupRuns([
      file("C:\\Musik\\Ordner eins\\x.wav"),
      file("C:\\Musik\\Ordner eins\\y.wav"),
    ]);
    expect(kinds(rows)).toEqual(["G:Ordner eins", "F:0", "F:1"]);
  });

  it("album_artist_falls_back_to_artist", () => {
    const rows = groupRuns([
      file("/1", { artist: "Band", album: "Platte" }),
      file("/2", { "album artist": "Band", album: "Platte" }),
    ]);
    expect(kinds(rows)).toEqual(["G:Band – Platte", "F:0", "F:1"]);
  });
});
