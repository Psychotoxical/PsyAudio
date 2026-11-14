import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../ipc/commands", () => ({ saveSettings: vi.fn(() => Promise.resolve()) }));
vi.mock("../../ipc/cd", async (original) => ({
  ...(await original<typeof import("../../ipc/cd")>()),
  measureDrive: vi.fn(() => Promise.resolve()),
}));

import type { Settings } from "../../ipc/bindings/Settings";
import { measureDrive } from "../../ipc/cd";
import { useCd } from "../../store/cd";
import { useSettings } from "../../store/settings";
import { lookupHandler, nextDisc } from "./cdFlow";

const url = "https://db.cue.tools/ui/?tocid=Beispiel";
const drive = { id: "G", name: "G:", model: "Band Drive X" };

function settings(drives: unknown[]): Settings {
  return {
    rip: { ctdb: true, drives, drive_offset: 0 },
    ui: { boxes_open: {} },
  } as unknown as Settings;
}

describe("cd lookups", () => {
  beforeEach(() => {
    vi.mocked(measureDrive).mockClear();
    useSettings.getState().load(
      settings([
        {
          model: "Band Drive X",
          offset: 667,
          method: "Ctdb",
          measured: "2026-11-14",
          buffer_kb: null,
        },
      ]),
    );
    useCd.setState({
      cover: null,
      candidates: [],
      drives: [drive],
      drive: "G",
      rip: null,
      offset: { busy: false, fraction: 0 },
      ctdb: { state: "busy", total: 0, url: null, reason: null },
    });
  });

  it("stale_lookup_is_ignored", () => {
    const old = lookupHandler(nextDisc());
    nextDisc(); // the next disc was read meanwhile
    old({ kind: "Cover", mime: "image/jpeg", data_base64: "AAAA" });
    old({ kind: "Ctdb", total: 7, url });
    expect(useCd.getState().cover).toBeNull();
    expect(useCd.getState().ctdb.state).toBe("busy");
  });

  it("current_lookup_applies", () => {
    const now = lookupHandler(nextDisc());
    now({ kind: "Ctdb", total: 7, url });
    expect(useCd.getState().ctdb).toEqual({ state: "done", total: 7, url, reason: null });
  });

  it("auto_measure_runs_only_without_profile", () => {
    lookupHandler(nextDisc())({ kind: "Ctdb", total: 7, url });
    expect(measureDrive).not.toHaveBeenCalled();
    useSettings.getState().load(settings([]));
    lookupHandler(nextDisc())({ kind: "Ctdb", total: 7, url });
    expect(measureDrive).toHaveBeenCalledWith("G", expect.any(Function));
  });

  it("no_auto_measure_with_a_disc_ctdb_does_not_know", () => {
    useSettings.getState().load(settings([]));
    lookupHandler(nextDisc())({ kind: "Ctdb", total: 0, url });
    expect(measureDrive).not.toHaveBeenCalled();
  });
});
