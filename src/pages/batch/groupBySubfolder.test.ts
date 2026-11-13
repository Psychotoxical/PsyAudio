import { describe, expect, it } from "vitest";
import { groupBySubfolder } from "./groupBySubfolder";

describe("batch list", () => {
  it("groups_follow_subfolders_in_order", () => {
    const groups = groupBySubfolder("C:\\Quelle", [
      "C:\\Quelle\\a.flac",
      "C:\\Quelle\\Band\\x.flac",
      "C:\\Quelle\\Band\\y.flac",
      "C:\\Quelle\\Band\\Live\\z.flac",
    ]);
    expect(groups.map((g) => [g.dir, g.files.map((f) => f.name)])).toEqual([
      ["", ["a.flac"]],
      ["Band", ["x.flac", "y.flac"]],
      ["Band\\Live", ["z.flac"]],
    ]);
  });

  it("works_with_slashes_and_a_trailing_separator", () => {
    const groups = groupBySubfolder("/musik/quelle/", ["/musik/quelle/band/x.flac"]);
    expect(groups).toEqual([
      { dir: "band", files: [{ path: "/musik/quelle/band/x.flac", name: "x.flac" }] },
    ]);
  });
});
