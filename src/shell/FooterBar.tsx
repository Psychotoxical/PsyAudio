import type { ReactNode } from "react";

/** The bar under a page's list: totals on the left, the main action on the right. */
export function FooterBar({ summary, children }: { summary: ReactNode; children?: ReactNode }) {
  return (
    <footer className="bar-foot">
      <p className="bar-sum">{summary}</p>
      {children}
    </footer>
  );
}
