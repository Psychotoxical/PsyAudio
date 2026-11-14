import { useVirtualizer } from "@tanstack/react-virtual";
import { open } from "@tauri-apps/plugin-dialog";
import { Folder, ListPlus, Play, Search } from "lucide-react";
import { useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { plural } from "../../i18n";
import { FooterBar } from "../../shell/FooterBar";
import { PageHeader } from "../../shell/PageHeader";
import { SidePanel } from "../../shell/SidePanel";
import { StepTiles, type Tile } from "../../shell/StepTiles";
import { BATCH_TYPES, searchBatch, useBatch } from "../../store/batch";
import { type EffectEntry, useEffects } from "../../store/effects";
import { useEncode } from "../../store/encode";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { Check } from "../../ui/controls";
import { addFiles } from "../convert/addFiles";
import { EffectsPanel } from "../convert/EffectsPanel";
import { FormatPanel } from "../convert/FormatPanel";
import { convert } from "../convert/runFlow";
import { SavePanel } from "../convert/SavePanel";
import { codecSummary, effectsSummary } from "../convert/summary";
import { groupBySubfolder } from "./groupBySubfolder";
import "./batch.css";

const NO_ENTRIES: EffectEntry[] = [];
const GROUP = 36;
const ROW = 30;

type Row = { kind: "group"; dir: string; count: number } | { kind: "file"; name: string };

/** The found files, one heading per subfolder; long lists stay smooth. */
function FoundList({ root, found }: { root: string; found: string[] }) {
  const { t } = useTranslation();
  const rows = useMemo<Row[]>(
    () =>
      groupBySubfolder(root, found).flatMap((g) => [
        { kind: "group" as const, dir: g.dir, count: g.files.length },
        ...g.files.map((f) => ({ kind: "file" as const, name: f.name })),
      ]),
    [root, found],
  );
  const scroller = useRef<HTMLDivElement>(null);
  const list = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroller.current,
    estimateSize: (i) => (rows[i]?.kind === "group" ? GROUP : ROW),
    overscan: 12,
  });
  return (
    <div className="found" ref={scroller}>
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
          return row.kind === "group" ? (
            <div key={v.key} className="found-group" style={style}>
              <Folder size={15} />
              <strong>{row.dir || t("Quellordner")}</strong>
              <small>
                {t(plural(row.count, "1 Titel", "{0} Titel"), { 0: String(row.count) })}
              </small>
            </div>
          ) : (
            <div key={v.key} className="found-file" style={style}>
              {row.name}
            </div>
          );
        })}
      </div>
    </div>
  );
}

/** Whole folder trees at once; the folder structure stays. Settings as on the converter page. */
export function BatchPage() {
  const { t } = useTranslation();
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);
  const options = useEncode((s) => s.options);
  const effectEntries = useEffects((s) => s.catalogue?.entries ?? NO_ENTRIES);
  const { folder, root, found, scanning, searched } = useBatch();
  if (!settings) return null;

  const tiles: Tile[] = [
    {
      id: "format",
      title: t("Format"),
      summary: codecSummary(settings.codec, settings.enc, options, t),
    },
    {
      id: "target",
      title: t("Zielordner"),
      summary: settings.output_folder || t("Kein Ordner gewählt"),
    },
    {
      id: "effects",
      title: t("Effekte"),
      summary: effectsSummary(settings.effects, effectEntries, t),
    },
  ];
  const titles = Object.fromEntries(tiles.map((x) => [x.id, x.title]));
  const exts = settings.batch_extensions;

  const choose = async () => {
    const picked = await open({ directory: true, defaultPath: folder || undefined });
    if (typeof picked === "string") useBatch.setState({ folder: picked });
  };
  const search = () =>
    searchBatch(settings.batch_subfolders, exts)
      .then((n) =>
        useUi
          .getState()
          .notify(t(plural(n, "{n} Datei gefunden", "{n} Dateien gefunden"), { n: String(n) })),
      )
      .catch((e: unknown) => useUi.getState().notify(String(e)));
  const items = () => found.map((p) => [p, root] as [string, string | null]);

  return (
    <div className="page">
      <div className="page-main">
        <PageHeader title={t("Stapel-Konverter")} page="batch" firstTile="format" />
        <StepTiles page="batch" tiles={tiles} />
        <section className="batch-source" aria-label={t("Quellordner")}>
          <div className="field">
            <label className="lbl" htmlFor="batch-folder">
              {t("Quellordner")}
            </label>
            <div className="row">
              <input
                id="batch-folder"
                className="text-field path-field"
                type="text"
                readOnly
                value={folder}
                placeholder={t("Klicken, um einen Ordner zu wählen (oder hierher ziehen)")}
                title={folder}
                onClick={() => void choose()}
              />
              <button type="button" className="btn" onClick={() => void choose()}>
                {t("Ändern")}
              </button>
              <button
                type="button"
                className="btn"
                disabled={!folder.trim() || scanning || exts.length === 0}
                onClick={() => void search()}
              >
                <Search size={16} />
                {scanning ? t("Suche läuft…") : t("Dateien suchen")}
              </button>
            </div>
          </div>
          <div className="batch-opts">
            <Check
              label={t("Unterordner einbeziehen")}
              checked={settings.batch_subfolders}
              onChange={(v) =>
                update((s) => {
                  s.batch_subfolders = v;
                })
              }
            />
            <p className="lbl">{t("Dateitypen")}</p>
            <fieldset className="chips type-chips" aria-label={t("Dateitypen")}>
              {BATCH_TYPES.map((ext) => {
                const on = exts.includes(ext);
                return (
                  <button
                    key={ext}
                    type="button"
                    className={on ? "chip-btn on" : "chip-btn"}
                    aria-pressed={on}
                    onClick={() =>
                      update((s) => {
                        s.batch_extensions = on
                          ? s.batch_extensions.filter((e) => e !== ext)
                          : [...s.batch_extensions, ext];
                      })
                    }
                  >
                    {ext}
                  </button>
                );
              })}
            </fieldset>
          </div>
        </section>
        <div className="content">
          {found.length > 0 ? (
            <FoundList root={root} found={found} />
          ) : (
            <p className="empty">
              {searched
                ? t("Keine passenden Dateien in diesem Ordner.")
                : t(
                    "Ganze Ordner mit Unterordnern auf einmal umwandeln – die Ordnerstruktur bleibt erhalten.",
                  )}
            </p>
          )}
        </div>
        <FooterBar
          summary={
            <b>
              {t(plural(found.length, "{n} Datei gefunden", "{n} Dateien gefunden"), {
                n: String(found.length),
              })}
            </b>
          }
        >
          <button
            type="button"
            className="btn"
            disabled={found.length === 0}
            onClick={() => {
              void addFiles(found, root);
              useUi.getState().setPage("convert");
            }}
          >
            <ListPlus size={16} />
            {t("Zur Konverter-Liste")}
          </button>
          <button
            type="button"
            className="btn primary"
            disabled={found.length === 0}
            onClick={() => void convert(items(), settings.codec, true)}
          >
            <Play size={16} />
            {t("Mit Ordnerstruktur umwandeln")}
          </button>
        </FooterBar>
      </div>
      <SidePanel page="batch" titles={titles}>
        {(tile) => {
          if (tile === "format") return <FormatPanel field="codec" />;
          if (tile === "target") return <SavePanel batch />;
          return <EffectsPanel />;
        }}
      </SidePanel>
    </div>
  );
}
