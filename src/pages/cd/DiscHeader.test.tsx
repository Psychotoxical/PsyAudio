import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../../ipc/cd", async (original) => ({
  ...(await original<typeof import("../../ipc/cd")>()),
  openCtdbPage: vi.fn(() => Promise.resolve()),
}));

import { openCtdbPage } from "../../ipc/cd";
import { emptyMeta, useCd } from "../../store/cd";
import { DiscHeader } from "./DiscHeader";

describe("disc header", () => {
  it("shows_the_ctdb_count_and_opens_its_page", () => {
    useCd.setState({
      disc: {
        tracks: [{ number: 1, seconds: 180 }],
        total_seconds: 180,
        musicbrainz_id: "x",
        freedb_id: "y",
      },
      meta: emptyMeta(),
      ctdb: {
        state: "done",
        total: 214,
        url: "https://db.cue.tools/ui/?tocid=Beispiel",
        reason: null,
      },
    });
    render(<DiscHeader />);
    fireEvent.click(screen.getByRole("button", { name: /CTDB · 214 Rips/ }));
    expect(openCtdbPage).toHaveBeenCalledTimes(1);
  });

  it("an_unknown_disc_has_no_link", () => {
    useCd.setState({ ctdb: { state: "done", total: 0, url: null, reason: null } });
    render(<DiscHeader />);
    expect(screen.getByText("CTDB: CD nicht in der Datenbank")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /CTDB/ })).toBeNull();
  });
});
