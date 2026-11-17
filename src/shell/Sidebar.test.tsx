import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "../ipc/bindings/Settings";

vi.mock("../ipc/commands", () => ({ saveSettings: vi.fn(() => Promise.resolve()) }));

import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";
import { Sidebar } from "./Sidebar";

describe("Sidebar", () => {
  beforeEach(() => {
    const ui = { panel_open: {}, tile: {}, sidebar_collapsed: false, boxes_open: {} };
    useSettings.getState().load({ ui } as unknown as Settings);
    useUi.setState({ page: "convert" });
  });

  it("nav_switches_page", () => {
    render(<Sidebar />);
    fireEvent.click(screen.getByRole("button", { name: /CD-Ripper/ }));
    expect(useUi.getState().page).toBe("cd");
    expect(screen.getByRole("button", { name: /CD-Ripper/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("badge_shows_file_count", () => {
    render(<Sidebar counts={{ convert: 12 }} />);
    expect(screen.getByRole("button", { name: /Convert/ })).toHaveTextContent("12");
  });
});
