import { useTranslation } from "react-i18next";
import { FooterBar } from "../shell/FooterBar";
import { PageHeader } from "../shell/PageHeader";
import { SidePanel } from "../shell/SidePanel";
import { StepTiles, type Tile } from "../shell/StepTiles";
import type { Page } from "../store/ui";

// Stage 3 only: the shell around empty pages; the real pages replace this from stage 4 on.
export function Placeholder({ page, title, tiles }: { page: Page; title: string; tiles: Tile[] }) {
  const { t } = useTranslation();
  const titles = Object.fromEntries(tiles.map((x) => [x.id, x.title]));
  return (
    <div className="page">
      <div className="page-main">
        <PageHeader title={title} page={tiles.length ? page : undefined} firstTile={tiles[0]?.id} />
        {tiles.length > 0 && <StepTiles page={page} tiles={tiles} />}
        <div className="content">
          <p className="empty">{t("Folgt in der nächsten Stufe")}</p>
        </div>
        {tiles.length > 0 && <FooterBar summary={<b>{title}</b>} />}
      </div>
      {tiles.length > 0 && (
        <SidePanel page={page} titles={titles}>
          {(tile) => <p className="empty">{titles[tile]}</p>}
        </SidePanel>
      )}
    </div>
  );
}
