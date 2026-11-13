import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { InlineText } from "./InlineText";

describe("inline text", () => {
  it("enter_commits", () => {
    const onCommit = vi.fn();
    render(<InlineText label="Titel" value="Lied" onCommit={onCommit} />);
    fireEvent.click(screen.getByRole("button", { name: "Titel" }));
    const box = screen.getByRole("textbox", { name: "Titel" });
    fireEvent.change(box, { target: { value: "Neues Lied" } });
    fireEvent.keyDown(box, { key: "Enter" });
    expect(onCommit).toHaveBeenCalledWith("Neues Lied");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("escape_reverts", () => {
    const onCommit = vi.fn();
    render(<InlineText label="Titel" value="Lied" onCommit={onCommit} />);
    fireEvent.click(screen.getByRole("button", { name: "Titel" }));
    const box = screen.getByRole("textbox", { name: "Titel" });
    fireEvent.change(box, { target: { value: "Falsch" } });
    fireEvent.keyDown(box, { key: "Escape" });
    expect(onCommit).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Titel" })).toHaveTextContent("Lied");
  });

  it("tab_moves_to_next_cell", () => {
    const first = vi.fn();
    const second = vi.fn();
    render(
      <>
        <InlineText id="t1" nextId="t2" label="Titel 1" value="Eins" onCommit={first} />
        <InlineText id="t2" label="Titel 2" value="Zwei" onCommit={second} />
      </>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Titel 1" }));
    const box = screen.getByRole("textbox", { name: "Titel 1" });
    fireEvent.change(box, { target: { value: "Eins neu" } });
    fireEvent.keyDown(box, { key: "Tab" });
    expect(first).toHaveBeenCalledWith("Eins neu");
    expect(screen.getByRole("textbox", { name: "Titel 2" })).toHaveFocus();
  });

  it("f2_starts_editing", () => {
    render(<InlineText label="Album" value="Album" onCommit={() => {}} />);
    fireEvent.keyDown(screen.getByRole("button", { name: "Album" }), { key: "F2" });
    expect(screen.getByRole("textbox", { name: "Album" })).toHaveValue("Album");
  });
});
