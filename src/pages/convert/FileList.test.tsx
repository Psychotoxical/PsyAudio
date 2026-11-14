import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { type FileRow, useFiles } from "../../store/files";
import { FileList } from "./FileList";

describe("FileList", () => {
  it("renders_only_visible_rows_for_2000_files", () => {
    const files: FileRow[] = Array.from({ length: 2000 }, (_, i) => ({
      id: i + 1,
      path: `/musik/album ${Math.floor(i / 10)}/lied ${i}.flac`,
      root: null,
      size: 1000,
      tags: { title: `Lied ${i}`, artist: "Band", album: `Album ${Math.floor(i / 10)}` },
      info: undefined,
      cover: false,
    }));
    useFiles.setState({ files, selected: new Set() });
    // jsdom lays nothing out: give the scroll area the size of a window
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
      DOMRect.fromRect({ width: 800, height: 600 }),
    );
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(600);
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(800);
    const { container } = render(<FileList />);
    const rows = container.querySelectorAll("[data-row]");
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThan(100);
  });
});
