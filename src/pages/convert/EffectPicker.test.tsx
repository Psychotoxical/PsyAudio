import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { EffectEntry } from "../../store/effects";
import { EffectPicker } from "./EffectPicker";

const entries: EffectEntry[] = [
  {
    category: "Edit",
    label: "Fade in/out",
    description: "Fades in at the start and out at the end.",
    effect: { Fade: { in_ms: 0, out_ms: 3000 } },
  },
  {
    category: "Filter",
    label: "Low-pass",
    description: "Lets only frequencies below the limit through.",
    effect: { Lowpass: { hz: 16000 } },
  },
  {
    category: "Filter",
    label: "High-pass",
    description: "Lässt nur Frequenzen oberhalb der Grenze durch.",
    effect: { Highpass: { hz: 40 } },
  },
];

describe("effect picker", () => {
  it("enter_adds_selected_effect", () => {
    const onAdd = vi.fn();
    render(<EffectPicker entries={entries} onAdd={onAdd} onClose={() => {}} />);
    const search = screen.getByRole("searchbox", { name: "Search effects" });
    expect(search).toHaveFocus();
    fireEvent.change(search, { target: { value: "pass" } });
    fireEvent.keyDown(search, { key: "ArrowDown" });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onAdd).toHaveBeenCalledWith({ Highpass: { hz: 40 } });
  });

  it("category_chip_narrows_the_list", () => {
    render(<EffectPicker entries={entries} onAdd={() => {}} onClose={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Filter" }));
    expect(screen.queryByText("Fade in/out")).not.toBeInTheDocument();
    expect(screen.getByText("Low-pass")).toBeInTheDocument();
  });
});
