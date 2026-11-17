import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ChipEditor } from "./ChipEditor";

const DATA = ["artist", "album", "track", "title"];
const SEPARATORS = [" - ", "(", ")"];
const LABELS = {
  artist: "Artist",
  album: "Album",
  track: "Track",
  title: "Title",
  " - ": "–",
  "(": "(",
  ")": ")",
  "/": "New folder",
};

function setup(value: string[], presets?: { name: string; scheme: string[] }[]) {
  const onChange = vi.fn();
  render(
    <ChipEditor
      label="Building blocks"
      value={value}
      data={DATA}
      separators={SEPARATORS}
      folders
      labels={LABELS}
      presets={presets}
      onChange={onChange}
    />,
  );
  return onChange;
}

describe("chip editor", () => {
  it("click_removes_chip", () => {
    const onChange = setup(["artist", " - ", "title"]);
    fireEvent.click(screen.getByRole("button", { name: "Remove Title" }));
    expect(onChange).toHaveBeenCalledWith(["artist", " - "]);
  });

  it("preset_replaces_chips", () => {
    const onChange = setup(
      ["artist"],
      [{ name: "Track – Title", scheme: ["track", " - ", "title"] }],
    );
    fireEvent.change(screen.getByRole("combobox", { name: "Preset" }), {
      target: { value: "Track – Title" },
    });
    expect(onChange).toHaveBeenCalledWith(["track", " - ", "title"]);
  });

  it("drag_moves_chip", () => {
    const onChange = setup(["artist", "album", "track", "title"]);
    const from = screen.getByRole("button", { name: "Remove Artist" });
    const to = screen.getByRole("button", { name: "Remove Track" });
    fireEvent.pointerDown(from, { pointerId: 1, button: 0 });
    fireEvent.pointerMove(to, { pointerId: 1 });
    fireEvent.pointerUp(to, { pointerId: 1 });
    // a real drag ends without a click on the dragged chip; one that lands on it must not remove it
    fireEvent.click(from);
    expect(onChange).toHaveBeenCalledWith(["album", "track", "artist", "title"]);
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it("added_chip_goes_to_the_end", () => {
    const onChange = setup(["artist"]);
    fireEvent.click(screen.getByRole("button", { name: "Building block" }));
    fireEvent.click(screen.getByRole("button", { name: "Title" }));
    expect(onChange).toHaveBeenCalledWith(["artist", "title"]);
  });
});
