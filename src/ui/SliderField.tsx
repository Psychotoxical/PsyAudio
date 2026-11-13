import { useId, useState } from "react";
import { useTranslation } from "react-i18next";

interface Props {
  label: string;
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  /** A fixed list of values (bit rates); the slider moves over the list. */
  values?: number[];
  /** Shown inside the value field: "192 kbit/s". */
  unit?: string;
  /** Text for a value instead of the number ("-V 2", "Standard (ca. 180 kbit/s)"). */
  format?: (value: number) => string;
  /** Logarithmic slider between min and max (frequencies). */
  log?: boolean;
  /** The value field only shows the value. */
  readOnly?: boolean;
  disabled?: boolean;
}

function nearest(list: number[], v: number): number {
  return list.reduce((best, x) => (Math.abs(x - v) < Math.abs(best - v) ? x : best), list[0] ?? v);
}

/** Positions of a logarithmic slider. */
const LOG_STEPS = 1000;

/** A slider with its value field; the unit sits inside the field, a typed value is kept in range. */
export function SliderField(p: Props) {
  const id = useId();
  const { i18n } = useTranslation();
  const [draft, setDraft] = useState<string | null>(null);
  const list = p.values;
  const step = p.step ?? 1;
  const decimals = (String(step).split(".")[1] ?? "").length;
  const round = (v: number) => Number((Math.round(v / step) * step).toFixed(decimals));
  const lo = p.min ?? 0;
  const hi = p.max ?? 100;
  const span = Math.log(hi / Math.max(lo, 1e-9));
  const toPos = (v: number) => Math.round((LOG_STEPS * Math.log(Math.max(lo, v) / lo)) / span);
  const fromPos = (x: number) => round(lo * Math.exp((x / LOG_STEPS) * span));
  const min = list || p.log ? 0 : lo;
  const max = list ? list.length - 1 : p.log ? LOG_STEPS : hi;
  const pos = list
    ? Math.max(0, list.indexOf(nearest(list, p.value)))
    : p.log
      ? toPos(p.value)
      : p.value;
  const number = p.value.toLocaleString(i18n.language === "de" ? "de-DE" : "en-US", {
    maximumFractionDigits: decimals,
    useGrouping: false,
  });
  const shown = `${p.format ? p.format(p.value) : number}${p.unit ? ` ${p.unit}` : ""}`;

  const commit = () => {
    if (draft === null) return;
    const n = Number.parseFloat(draft.replace(",", ".").replace(/[^\d.+-]/g, ""));
    setDraft(null);
    if (Number.isNaN(n)) return;
    if (list) {
      p.onChange(nearest(list, n));
    } else {
      const clamped = Math.min(p.max ?? n, Math.max(p.min ?? n, n));
      p.onChange(round(clamped));
    }
  };

  return (
    <div className="field">
      <label className="lbl" htmlFor={id}>
        {p.label}
      </label>
      <div className="slider">
        <input
          type="range"
          aria-label={p.label}
          min={min}
          max={max}
          step={list || p.log ? 1 : step}
          value={pos}
          disabled={p.disabled}
          onChange={(e) => {
            const v = Number(e.target.value);
            p.onChange(list ? (list[v] ?? p.value) : p.log ? fromPos(v) : round(v));
          }}
          style={
            { "--fill": `${((pos - min) / Math.max(1, max - min)) * 100}%` } as React.CSSProperties
          }
        />
        <input
          id={id}
          className="val"
          type="text"
          style={{ width: `max(112px, ${shown.length + 3}ch)` }}
          aria-label={p.label}
          value={draft ?? shown}
          readOnly={p.readOnly}
          disabled={p.disabled}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            if (e.key === "Escape") setDraft(null);
          }}
        />
      </div>
    </div>
  );
}
