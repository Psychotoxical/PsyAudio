import { usePanel } from "../store/layout";
import type { Page } from "../store/ui";

export interface Tile {
  id: string;
  title: string;
  /** The current state in at most two lines, e.g. "Opus · 192 kbit/s · VBR". */
  summary: string;
}

/** The numbered tiles above a page's list, as in PsyTags' step bar. */
export function StepTiles({ page, tiles }: { page: Page; tiles: Tile[] }) {
  const { open, tile, choose } = usePanel(page, tiles[0]?.id ?? "");
  return (
    <nav className="steps" aria-label="Einstellungen">
      {tiles.map((t, i) => {
        const active = open && tile === t.id;
        return (
          <button
            key={t.id}
            type="button"
            className={active ? "step on" : "step"}
            aria-pressed={active}
            onClick={() => choose(t.id)}
          >
            <span className="step-n">{i + 1}</span>
            <span className="step-text">
              <strong>{t.title}</strong>
              <small>{t.summary}</small>
            </span>
          </button>
        );
      })}
    </nav>
  );
}
