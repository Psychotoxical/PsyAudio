import { beforeEach, describe, expect, it } from "vitest";
import { useCd } from "../../store/cd";
import { lookupHandler, nextDisc } from "./cdFlow";

describe("cd lookups", () => {
  beforeEach(() =>
    useCd.setState({
      cover: null,
      candidates: [],
      ar: { state: "busy", pressings: 0, reason: null },
    }),
  );

  it("stale_lookup_is_ignored", () => {
    const old = lookupHandler(nextDisc());
    nextDisc(); // the next disc was read meanwhile
    old({ kind: "Cover", mime: "image/jpeg", data_base64: "AAAA" });
    old({ kind: "AccurateRip", pressings: 7 });
    expect(useCd.getState().cover).toBeNull();
    expect(useCd.getState().ar.state).toBe("busy");
  });

  it("current_lookup_applies", () => {
    const now = lookupHandler(nextDisc());
    now({ kind: "AccurateRip", pressings: 7 });
    expect(useCd.getState().ar).toEqual({ state: "done", pressings: 7, reason: null });
  });
});
