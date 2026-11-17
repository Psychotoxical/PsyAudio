import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SliderField } from "./SliderField";

describe("SliderField", () => {
  it("unit_inside_value_field", () => {
    render(
      <SliderField
        label="Bit rate"
        value={192}
        min={6}
        max={512}
        unit="kbit/s"
        onChange={() => {}}
      />,
    );
    expect(screen.getByRole("textbox", { name: "Bit rate" })).toHaveValue("192 kbit/s");
  });

  it("typed_value_is_clamped", () => {
    const onChange = vi.fn();
    render(
      <SliderField
        label="Bit rate"
        value={192}
        min={6}
        max={512}
        unit="kbit/s"
        onChange={onChange}
      />,
    );
    const box = screen.getByRole("textbox", { name: "Bit rate" });
    fireEvent.change(box, { target: { value: "900" } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith(512);
  });

  it("list_values_snap_to_the_nearest", () => {
    const onChange = vi.fn();
    render(
      <SliderField
        label="Bit rate"
        value={128}
        values={[96, 128, 160, 192]}
        unit="kbit/s"
        onChange={onChange}
      />,
    );
    const box = screen.getByRole("textbox", { name: "Bit rate" });
    fireEvent.change(box, { target: { value: "170 kbit/s" } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith(160);
  });

  it("log_scale_reaches_low_and_high_ends", () => {
    const onChange = vi.fn();
    render(
      <SliderField
        label="Grenze"
        log
        min={10}
        max={22000}
        value={1000}
        unit="Hz"
        onChange={onChange}
      />,
    );
    const range = screen.getByRole("slider", { name: "Grenze" });
    // 1000 Hz sits past the middle of a 10 Hz to 22 kHz log scale, not at 4 % as on a linear one
    const at = (n: string) => Number(range.getAttribute(n));
    expect((at("value") - at("min")) / (at("max") - at("min"))).toBeGreaterThan(0.55);
    fireEvent.change(range, { target: { value: "0" } });
    expect(onChange).toHaveBeenLastCalledWith(10);
    fireEvent.change(range, { target: { value: range.getAttribute("max") } });
    expect(onChange).toHaveBeenLastCalledWith(22000);
  });
});
