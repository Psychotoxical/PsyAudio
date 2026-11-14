import { describe, expect, it } from "vitest";
import { itemsToConvert, select } from "./selection";

const none = { ctrl: false, shift: false };

describe("selection", () => {
  it("click_selects_one", () => {
    expect([...select(new Set([1, 2]), 4, none)]).toEqual([4]);
  });

  it("ctrl_toggles", () => {
    const s = select(new Set([1]), 3, { ctrl: true, shift: false });
    expect([...s].sort()).toEqual([1, 3]);
    expect([...select(s, 1, { ctrl: true, shift: false })]).toEqual([3]);
  });

  it("shift_ranges_from_smallest_selected", () => {
    expect([...select(new Set([5, 2]), 4, { ctrl: false, shift: true })]).toEqual([2, 3, 4]);
    expect([...select(new Set([5]), 3, { ctrl: false, shift: true })]).toEqual([3, 4, 5]);
  });

  it("two_or_more_selected_convert_only_selection", () => {
    const files = ["a", "b", "c", "d"];
    expect(itemsToConvert(files, new Set([2]))).toEqual(files);
    expect(itemsToConvert(files, new Set([3, 1]))).toEqual(["b", "d"]);
  });
});
