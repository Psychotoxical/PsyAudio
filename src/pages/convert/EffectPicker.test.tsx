import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { EffectEntry } from "../../store/effects";
import { EffectPicker } from "./EffectPicker";

const entries: EffectEntry[] = [
  {
    category: "Bearbeiten",
    label: "Ein-/Ausblenden (Fade)",
    description: "Blendet am Anfang ein und am Ende aus.",
    effect: { Fade: { in_ms: 0, out_ms: 3000 } },
  },
  {
    category: "Filter",
    label: "Tiefpass",
    description: "Lässt nur Frequenzen unterhalb der Grenze durch.",
    effect: { Lowpass: { hz: 16000 } },
  },
  {
    category: "Filter",
    label: "Hochpass",
    description: "Lässt nur Frequenzen oberhalb der Grenze durch.",
    effect: { Highpass: { hz: 40 } },
  },
];

describe("effect picker", () => {
  it("enter_adds_selected_effect", () => {
    const onAdd = vi.fn();
    render(<EffectPicker entries={entries} onAdd={onAdd} onClose={() => {}} />);
    const search = screen.getByRole("searchbox", { name: "Effekt suchen" });
    expect(search).toHaveFocus();
    fireEvent.change(search, { target: { value: "pass" } });
    fireEvent.keyDown(search, { key: "ArrowDown" });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onAdd).toHaveBeenCalledWith({ Highpass: { hz: 40 } });
  });

  it("category_chip_narrows_the_list", () => {
    render(<EffectPicker entries={entries} onAdd={() => {}} onClose={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Filter" }));
    expect(screen.queryByText("Ein-/Ausblenden (Fade)")).not.toBeInTheDocument();
    expect(screen.getByText("Tiefpass")).toBeInTheDocument();
  });
});
