import { useEffect, useRef, useState } from "react";

/**
 * Text that becomes a field where it stands: click, F2 or Enter edits; Enter keeps, Esc puts the
 * old text back, Tab keeps and opens the field `nextId` (title → artist → next title).
 */
export function InlineText({
  value,
  onCommit,
  label,
  id,
  nextId,
  placeholder,
  className,
}: {
  value: string;
  onCommit: (value: string) => void;
  /** Accessible name (the column or field). */
  label: string;
  id?: string;
  nextId?: string;
  placeholder?: string;
  className?: string;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  // Chromium sends blur when the focused field is removed; the edit must count once
  const closed = useRef(false);
  const editing = draft !== null;
  useEffect(() => {
    if (editing) input.current?.focus();
  }, [editing]);

  const start = () => {
    closed.current = false;
    setDraft(value);
  };
  const keep = () => {
    if (closed.current) return;
    closed.current = true;
    if (draft !== null && draft !== value) onCommit(draft);
    setDraft(null);
  };

  if (draft === null) {
    return (
      <button
        type="button"
        id={id}
        className={["inline-text", className, !value && "empty"].filter(Boolean).join(" ")}
        aria-label={label}
        title={value || undefined}
        onClick={start}
        onKeyDown={(e) => {
          if (e.key === "F2") {
            e.preventDefault();
            start();
          }
        }}
      >
        {value || placeholder || " "}
      </button>
    );
  }
  return (
    <input
      ref={input}
      className={["inline-field", className].filter(Boolean).join(" ")}
      type="text"
      aria-label={label}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={keep}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          keep();
        } else if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          closed.current = true;
          setDraft(null);
        } else if (e.key === "Tab" && nextId && !e.shiftKey) {
          e.preventDefault();
          keep();
          // the next cell is a button until clicked; the click opens its field
          document.getElementById(nextId)?.click();
        }
      }}
    />
  );
}
