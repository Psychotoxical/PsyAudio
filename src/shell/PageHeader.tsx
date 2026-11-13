import { PanelRightOpen } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { usePanel } from "../store/layout";
import type { Page } from "../store/ui";

/** Page title, the page's actions, and the button that brings back a folded right column. */
export function PageHeader({
  title,
  page,
  firstTile,
  children,
}: {
  title: string;
  /** Pages with a right column pass their page and first tile. */
  page?: Page;
  firstTile?: string;
  children?: ReactNode;
}) {
  const { t } = useTranslation();
  return (
    <header className="top">
      <h1>{title}</h1>
      <div className="top-actions">{children}</div>
      {page && (
        <PanelToggle
          page={page}
          firstTile={firstTile ?? ""}
          label={t("Einstellungen einblenden")}
        />
      )}
    </header>
  );
}

function PanelToggle({ page, firstTile, label }: { page: Page; firstTile: string; label: string }) {
  const { open, setOpen } = usePanel(page, firstTile);
  if (open) return null;
  return (
    <button
      type="button"
      className="icon-btn"
      title={label}
      aria-label={label}
      onClick={() => setOpen(true)}
    >
      <PanelRightOpen size={18} />
    </button>
  );
}
