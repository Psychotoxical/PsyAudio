import { afterEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "../ipc/bindings/Settings";

vi.mock("../ipc/commands", () => ({ saveSettings: vi.fn(() => Promise.resolve()) }));

import { saveSettings } from "../ipc/commands";
import { SAVE_DELAY_MS, useSettings } from "./settings";

const base = { threads: 4, lang: "de" } as unknown as Settings;

describe("settings store", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.mocked(saveSettings).mockClear();
  });

  it("saves_at_most_twice_a_second", () => {
    vi.useFakeTimers();
    const { load, update } = useSettings.getState();
    load(base);
    update((s) => {
      s.threads = 5;
    });
    vi.advanceTimersByTime(50);
    update((s) => {
      s.threads = 6;
    });
    vi.advanceTimersByTime(50);
    update((s) => {
      s.threads = 7;
    });
    expect(saveSettings).not.toHaveBeenCalled();
    vi.advanceTimersByTime(SAVE_DELAY_MS);
    expect(saveSettings).toHaveBeenCalledTimes(1);
    expect(vi.mocked(saveSettings).mock.calls[0]![0].threads).toBe(7);
  });

  it("does_not_save_on_load", () => {
    vi.useFakeTimers();
    useSettings.getState().load(base);
    vi.advanceTimersByTime(SAVE_DELAY_MS * 4);
    expect(saveSettings).not.toHaveBeenCalled();
  });
});
