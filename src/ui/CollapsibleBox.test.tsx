import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "../ipc/bindings/Settings";

vi.mock("../ipc/commands", () => ({ saveSettings: vi.fn(() => Promise.resolve()) }));

import { useSettings } from "../store/settings";
import { CollapsibleBox } from "./CollapsibleBox";

describe("collapsible box", () => {
  beforeEach(() => {
    useSettings.getState().load({
      ui: { panel_open: {}, tile: {}, sidebar_collapsed: false, boxes_open: { programs: false } },
    } as unknown as Settings);
  });

  it("collapsed_box_shows_summary", () => {
    render(
      <CollapsibleBox id="programs" title="Programs" summary="FFmpeg ready">
        <p>Inhalt</p>
      </CollapsibleBox>,
    );
    expect(screen.getByText("FFmpeg ready")).toBeInTheDocument();
    expect(screen.queryByText("Inhalt")).not.toBeInTheDocument();
  });

  it("state_persists", () => {
    render(
      <CollapsibleBox id="programs" title="Programs" summary="FFmpeg ready">
        <p>Inhalt</p>
      </CollapsibleBox>,
    );
    fireEvent.click(screen.getByRole("button", { name: /Programs/ }));
    expect(useSettings.getState().settings?.ui.boxes_open.programs).toBe(true);
    expect(screen.getByText("Inhalt")).toBeInTheDocument();
  });
});
