import "./ui.css";
import { ChevronRight, Info, TriangleAlert } from "lucide-react";
import { type ReactNode, useId, useState } from "react";

/** Options side by side, one chosen (a radio group drawn as segments). */
export function Segmented<T extends string | number | boolean>({
  label,
  value,
  options,
  onChange,
  disabled,
}: {
  label?: string;
  value: T;
  options: [T, string][];
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  const name = useId();
  return (
    <div className="field">
      {label && <span className="lbl">{label}</span>}
      <div className="seg" role="radiogroup" aria-label={label}>
        {options.map(([v, text]) => (
          <label key={String(v)} className={v === value ? "on" : undefined}>
            <input
              type="radio"
              name={name}
              checked={v === value}
              disabled={disabled}
              onChange={() => onChange(v)}
            />
            {text}
          </label>
        ))}
      </div>
    </div>
  );
}

/** One line that explains an option; `warn` for something that blocks a run. */
export function Hint({ children, warn }: { children: ReactNode; warn?: boolean }) {
  return (
    <p className={warn ? "hint warn" : "hint"}>
      {warn ? <TriangleAlert size={14} /> : <Info size={14} />}
      <span>{children}</span>
    </p>
  );
}

/** Pro options folded away under "Erweitert" with a short list of what is inside. */
export function Disclosure({
  title,
  summary,
  defaultOpen = false,
  children,
}: {
  title: string;
  summary?: string;
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className={open ? "more open" : "more"}>
      <button type="button" aria-expanded={open} onClick={() => setOpen(!open)}>
        <ChevronRight size={16} />
        <span>{title}</span>
        {summary && !open && <small>{summary}</small>}
      </button>
      {open && <div className="more-body">{children}</div>}
    </div>
  );
}

export function Check({
  label,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label className="check">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      {label}
    </label>
  );
}

export function Select<T extends string | number>({
  label,
  value,
  options,
  onChange,
  disabled,
}: {
  label: string;
  value: T;
  options: [T, string][];
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  const id = useId();
  return (
    <div className="field">
      <label className="lbl" htmlFor={id}>
        {label}
      </label>
      <select
        id={id}
        className="dd"
        value={String(value)}
        disabled={disabled}
        onChange={(e) => {
          const hit = options.find(([v]) => String(v) === e.target.value);
          if (hit) onChange(hit[0]);
        }}
      >
        {options.map(([v, text]) => (
          <option key={String(v)} value={String(v)}>
            {text}
          </option>
        ))}
      </select>
    </div>
  );
}
