import { ChevronRight } from "lucide-react";
import type { ReactNode } from "react";
import { useSettings } from "../store/settings";

/**
 * A box that folds to its heading and a one-line summary; whether it is open is kept in
 * settings.ui.boxes_open[id], so the page looks the same at the next start.
 */
export function CollapsibleBox({
  id,
  title,
  summary,
  defaultOpen = true,
  children,
}: {
  id: string;
  title: string;
  summary?: string;
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  const stored = useSettings((s) => s.settings?.ui.boxes_open[id]);
  const update = useSettings((s) => s.update);
  const open = stored ?? defaultOpen;
  return (
    <section className={open ? "box open" : "box"} aria-label={title}>
      <button
        type="button"
        className="box-head"
        aria-expanded={open}
        onClick={() =>
          update((s) => {
            s.ui.boxes_open[id] = !open;
          })
        }
      >
        <ChevronRight size={16} />
        <h2>{title}</h2>
        {summary && !open && <small>{summary}</small>}
      </button>
      {open && <div className="box-body">{children}</div>}
    </section>
  );
}
