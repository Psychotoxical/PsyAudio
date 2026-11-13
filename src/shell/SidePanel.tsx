import { PanelRightClose } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { usePanel } from "../store/layout";
import type { Page } from "../store/ui";

/**
 * The page's right column (like the Psysonic queue): shows the open tile's settings and folds
 * away with its button or a click on the open tile.
 */
export function SidePanel({
  page,
  titles,
  children,
}: {
  page: Page;
  /** Tile id → heading, in tile order. */
  titles: Record<string, string>;
  children: (tile: string) => ReactNode;
}) {
  const { t } = useTranslation();
  const ids = Object.keys(titles);
  const { open, tile, setOpen } = usePanel(page, ids[0] ?? "");
  if (!open) return null;
  const number = ids.indexOf(tile) + 1;
  return (
    <aside className="panel" aria-label={titles[tile]}>
      <header className="panel-head">
        {number > 0 && <span className="step-n on">{number}</span>}
        <h2>{titles[tile]}</h2>
        <button
          type="button"
          className="icon-btn"
          title={t("Einstellungen ausblenden")}
          aria-label={t("Einstellungen ausblenden")}
          onClick={() => setOpen(false)}
        >
          <PanelRightClose size={18} />
        </button>
      </header>
      <div className="panel-body">{children(tile)}</div>
    </aside>
  );
}
