import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "../ipc/bindings/Settings";

vi.mock("../ipc/commands", () => ({ saveSettings: vi.fn(() => Promise.resolve()) }));

import { useSettings } from "../store/settings";
import { SidePanel } from "./SidePanel";
import { StepTiles } from "./StepTiles";

const tiles = [
  { id: "format", title: "Format", summary: "Opus · 192 kbit/s" },
  { id: "save", title: "Speichern in", summary: "Neben der Quelle" },
  { id: "naming", title: "Benennung", summary: "Albuminterpret – Album" },
];

function fresh() {
  const ui = { panel_open: {}, tile: {}, sidebar_collapsed: false, boxes_open: {} };
  useSettings.getState().load({ ui } as unknown as Settings);
}

function page(name: "convert" | "cd") {
  return (
    <>
      <StepTiles page={name} tiles={tiles} />
      <SidePanel
        page={name}
        titles={{ format: "Format", save: "Speichern in", naming: "Benennung" }}
      >
        {(tile) => <p>Inhalt {tile}</p>}
      </SidePanel>
    </>
  );
}

describe("StepTiles", () => {
  beforeEach(fresh);

  it("click_opens_panel_with_that_tile", () => {
    render(page("convert"));
    fireEvent.click(screen.getByRole("button", { name: /Benennung/ }));
    expect(screen.getByText("Inhalt naming")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Benennung/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("click_on_open_tile_closes_panel", () => {
    render(page("convert"));
    fireEvent.click(screen.getByRole("button", { name: /Speichern in/ }));
    fireEvent.click(screen.getByRole("button", { name: /Speichern in/ }));
    expect(screen.queryByText(/^Inhalt/)).not.toBeInTheDocument();
  });

  it("state_is_kept_per_page", () => {
    const { unmount } = render(page("convert"));
    fireEvent.click(screen.getByRole("button", { name: /Benennung/ }));
    unmount();
    const cd = render(page("cd"));
    fireEvent.click(screen.getByRole("button", { name: /Speichern in/ }));
    cd.unmount();
    render(page("convert"));
    expect(screen.getByText("Inhalt naming")).toBeInTheDocument();
    const ui = useSettings.getState().settings?.ui;
    expect(ui?.tile).toEqual({ convert: "naming", cd: "save" });
  });
});
