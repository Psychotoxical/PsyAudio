import { useSettings } from "./settings";
import type { Page } from "./ui";

/**
 * The right column of a page and the tile it shows, kept in settings.ui per page. The column is
 * open by default and shows the first tile.
 */
export function usePanel(page: Page, firstTile: string) {
  const ui = useSettings((s) => s.settings?.ui);
  const update = useSettings((s) => s.update);
  const open = ui?.panel_open[page] ?? true;
  const tile = ui?.tile[page] ?? firstTile;

  /** A tile click: opens the column on that tile, or closes it when that tile is already open. */
  const choose = (id: string) =>
    update((s) => {
      const isOpen = s.ui.panel_open[page] ?? true;
      const current = s.ui.tile[page] ?? firstTile;
      if (isOpen && current === id) {
        s.ui.panel_open[page] = false;
      } else {
        s.ui.tile[page] = id;
        s.ui.panel_open[page] = true;
      }
    });

  const setOpen = (value: boolean) =>
    update((s) => {
      s.ui.panel_open[page] = value;
    });

  return { open, tile, choose, setOpen };
}
