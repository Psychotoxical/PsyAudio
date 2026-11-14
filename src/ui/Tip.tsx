import { type ReactNode, useId, useLayoutEffect, useRef, useState } from "react";

type Rect = { left: number; right: number; top: number; bottom: number };
type Size = { width: number; height: number };

/** Room kept to the window edge and between the element and its text. */
const EDGE = 8;
const GAP = 6;

/** Where the text goes: below the element, its right edge at the element's right edge, always
 * inside the window; above the element when there is no room below. */
export function placeTip(anchor: Rect, tip: Size, win: Size): { left: number; top: number } {
  const left = Math.max(EDGE, Math.min(anchor.right - tip.width, win.width - EDGE - tip.width));
  const below = anchor.bottom + GAP;
  const top =
    below + tip.height <= win.height - EDGE ? below : Math.max(EDGE, anchor.top - GAP - tip.height);
  return { left, top };
}

/** A hover text that stays inside the window; the native `title` text can stick out of it.
 * The text is also the element's description for screen readers. */
export function Tip({
  text,
  className,
  children,
}: {
  text: string;
  className?: string;
  children: ReactNode;
}) {
  const id = useId();
  const anchor = useRef<HTMLSpanElement>(null);
  const tip = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    if (!open || !anchor.current || !tip.current) return;
    const a = anchor.current.getBoundingClientRect();
    const t = tip.current.getBoundingClientRect();
    setPos(
      placeTip(
        a,
        { width: t.width, height: t.height },
        { width: window.innerWidth, height: window.innerHeight },
      ),
    );
  }, [open]);

  const hide = () => {
    setOpen(false);
    setPos(null);
  };

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: hovering only shows the description, which screen readers get through aria-describedby
    <span
      ref={anchor}
      className={className}
      aria-describedby={id}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={hide}
    >
      {children}
      <span
        ref={tip}
        id={id}
        role="tooltip"
        className="tip"
        hidden={!open}
        style={pos ? { left: pos.left, top: pos.top } : { visibility: "hidden" }}
      >
        {text}
      </span>
    </span>
  );
}
