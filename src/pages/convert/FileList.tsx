import { useVirtualizer } from "@tanstack/react-virtual";
import { Check, X } from "lucide-react";
import { type KeyboardEvent, type MouseEvent, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { fmtFormat, fmtLen, fmtSize } from "../../format";
import { plural } from "../../i18n";
import { revealFolder } from "../../ipc/files";
import { useFiles } from "../../store/files";
import { groupRuns } from "./groupRuns";
import { select } from "./selection";

const GROUP_HEIGHT = 40;
const ROW_HEIGHT = 34;

/** The converter's file list: album groups (display only), selection as in egui, virtualised. */
export function FileList() {
  const { t, i18n } = useTranslation();
  const lang = i18n.language;
  const files = useFiles((s) => s.files);
  const selected = useFiles((s) => s.selected);
  const setSelected = useFiles((s) => s.setSelected);
  const removeSelected = useFiles((s) => s.removeSelected);
  const rows = useMemo(() => groupRuns(files), [files]);
  const scroller = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; path: string } | null>(null);

  const list = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroller.current,
    estimateSize: (i) => (rows[i]?.kind === "group" ? GROUP_HEIGHT : ROW_HEIGHT),
    overscan: 8,
    initialRect: { width: 800, height: 600 },
  });

  const onKey = (e: KeyboardEvent) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") {
      e.preventDefault();
      setSelected(new Set(files.map((_, i) => i)));
    } else if (e.key === "Delete" && selected.size > 0) {
      removeSelected();
    }
  };

  const onRowClick = (e: MouseEvent, index: number) =>
    setSelected(select(selected, index, { ctrl: e.ctrlKey || e.metaKey, shift: e.shiftKey }));

  return (
    // a list box with multiple selection: Ctrl+A and Delete work as in the egui list
    <div
      className="files"
      ref={scroller}
      role="listbox"
      aria-multiselectable="true"
      aria-label={t("Konvertieren")}
      tabIndex={0}
      onKeyDown={onKey}
      onClick={() => setMenu(null)}
    >
      <div className="files-head" aria-hidden="true">
        <span>#</span>
        <span>{t("Titel")}</span>
        <span>{t("Interpret")}</span>
        <span>Format</span>
        <span className="r">{t("Länge")}</span>
        <span className="r">{t("Größe")}</span>
      </div>
      <div style={{ height: list.getTotalSize(), position: "relative" }}>
        {list.getVirtualItems().map((v) => {
          const row = rows[v.index];
          if (!row) return null;
          const style = {
            position: "absolute" as const,
            top: 0,
            left: 0,
            right: 0,
            height: v.size,
            transform: `translateY(${v.start}px)`,
          };
          if (row.kind === "group") {
            return (
              <div
                key={v.key}
                data-row="group"
                className="files-group"
                style={style}
                role="presentation"
              >
                <span className="files-group-title">{row.title}</span>
                <small>
                  {[
                    row.year,
                    t(plural(row.count, "1 Titel", "{0} Titel"), { 0: String(row.count) }),
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </small>
                <span className={row.tagged ? "chip ok" : "chip bad"}>
                  {row.tagged ? <Check size={12} /> : <X size={12} />}Tags
                </span>
                <span className={row.cover ? "chip ok" : "chip bad"}>
                  {row.cover ? <Check size={12} /> : <X size={12} />}Cover
                </span>
              </div>
            );
          }
          const f = row.file;
          const name = f.path.split(/[\\/]/).pop() ?? f.path;
          const track = f.tags?.track ? f.tags.track.padStart(2, "0") : "";
          return (
            <div
              key={v.key}
              data-row="file"
              role="option"
              tabIndex={-1}
              aria-selected={selected.has(row.index)}
              className={selected.has(row.index) ? "files-row on" : "files-row"}
              style={style}
              onClick={(e) => {
                e.stopPropagation();
                onRowClick(e, row.index);
                setMenu(null);
              }}
              onContextMenu={(e) => {
                e.preventDefault();
                setMenu({ x: e.clientX, y: e.clientY, path: f.path });
              }}
              onKeyDown={onKey}
            >
              <span className="muted">{track}</span>
              <span className="t" title={f.path}>
                {f.tags?.title || name}
              </span>
              <span>{f.tags?.artist ?? ""}</span>
              <span className="muted">{fmtFormat(f.path, f.info, lang)}</span>
              <span className="r">{f.info ? fmtLen(f.info.seconds) : ""}</span>
              <span className="r muted">{fmtSize(f.size, lang)}</span>
            </div>
          );
        })}
      </div>
      {menu && (
        <div className="menu" style={{ left: menu.x, top: menu.y }} role="menu">
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              void revealFolder(menu.path);
              setMenu(null);
            }}
          >
            {t("Ordner öffnen")}
          </button>
        </div>
      )}
    </div>
  );
}
