import { describe, expect, it } from "vitest";
import type { Info } from "../ipc/bindings/Info";
import { type FileRow, useFiles } from "./files";

const row = (id: number): FileRow => ({
  id,
  path: `C:\\Musik\\${id}.flac`,
  root: null,
  size: 1,
  tags: undefined,
  info: undefined,
  cover: false,
});
const info = { seconds: 60 } as Info;

describe("file list", () => {
  it("infos_land_in_one_update", () => {
    useFiles.setState({ files: [row(1), row(2), row(3)], selected: new Set() });
    let updates = 0;
    const off = useFiles.subscribe(() => {
      updates += 1;
    });
    useFiles.getState().setInfos([
      { id: 1, tags: { title: "Eins" }, info, cover: false },
      { id: 3, tags: { title: "Drei" }, info, cover: true },
    ]);
    off();
    // 2 000 files reading in must not mean 2 000 renders of the whole list
    expect(updates).toBe(1);
    const files = useFiles.getState().files;
    expect(files.map((f) => f.tags?.title)).toEqual(["Eins", undefined, "Drei"]);
    expect(files[2]?.cover).toBe(true);
  });
});
