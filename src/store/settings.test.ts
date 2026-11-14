import { afterEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "../ipc/bindings/Settings";

vi.mock("../ipc/commands", () => ({ saveSettings: vi.fn(() => Promise.resolve()) }));

import { saveSettings } from "../ipc/commands";
import { flushSettings, SAVE_DELAY_MS, useSettings } from "./settings";
import { useUi } from "./ui";

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

  it("flush_saves_pending_change_now", async () => {
    vi.useFakeTimers();
    const { load, update } = useSettings.getState();
    load(base);
    update((s) => {
      s.threads = 9;
    });
    // closing comes before the 500 ms are over: the change must still reach the disk
    await flushSettings();
    expect(saveSettings).toHaveBeenCalledTimes(1);
    expect(vi.mocked(saveSettings).mock.calls[0]?.[0].threads).toBe(9);
    vi.advanceTimersByTime(SAVE_DELAY_MS);
    expect(saveSettings).toHaveBeenCalledTimes(1);
  });

  it("failed_save_is_reported", async () => {
    vi.useFakeTimers();
    vi.mocked(saveSettings).mockImplementationOnce(() => Promise.reject("Zugriff verweigert"));
    const { load, update } = useSettings.getState();
    load(base);
    update((s) => {
      s.threads = 3;
    });
    await flushSettings();
    expect(useUi.getState().toast?.text).toContain("Zugriff verweigert");
  });
});
