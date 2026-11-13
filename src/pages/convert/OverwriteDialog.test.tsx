import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { OverwriteDialog } from "./OverwriteDialog";

const paths = ["C:\\Ziel\\a.opus", "C:\\Ziel\\b.opus", "C:\\Ziel\\c.opus"];

describe("overwrite question", () => {
  it("unticked_files_are_kept", () => {
    const onStart = vi.fn();
    render(<OverwriteDialog paths={paths} onStart={onStart} onCancel={() => {}} />);
    fireEvent.click(screen.getByRole("checkbox", { name: "C:\\Ziel\\b.opus" }));
    fireEvent.click(screen.getByRole("button", { name: "Überschreiben (2)" }));
    expect(onStart).toHaveBeenCalledWith(["C:\\Ziel\\a.opus", "C:\\Ziel\\c.opus"]);
  });

  it("skip_existing_overwrites_nothing", () => {
    const onStart = vi.fn();
    render(<OverwriteDialog paths={paths} onStart={onStart} onCancel={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Vorhandene überspringen" }));
    expect(onStart).toHaveBeenCalledWith([]);
  });
});
