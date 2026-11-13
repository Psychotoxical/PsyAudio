import { Disc3, Eject, FolderOpen, Globe, RefreshCw, ScrollText, Square } from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { fmtLen } from "../../format";
import { plural } from "../../i18n";
import { openFolder, useNaming } from "../../ipc/naming";
import { FooterBar } from "../../shell/FooterBar";
import { PageHeader } from "../../shell/PageHeader";
import { SidePanel } from "../../shell/SidePanel";
import { StepTiles, type Tile } from "../../shell/StepTiles";
import { ripProgress, useCd } from "../../store/cd";
import { useEncode } from "../../store/encode";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { codecSummary, namingSummary } from "../convert/summary";
import { cancel, ejectCd, readCd, rip, scanDrives, searchInfo } from "./cdFlow";
import { DiscHeader } from "./DiscHeader";
import { ReadModePanel } from "./ReadModePanel";
import { RipFormatPanel, RipNamingPanel, RipSavePanel } from "./RipPanels";
import { summaryText } from "./status";
import { TrackTable } from "./TrackTable";
import "./cd.css";

/** Seconds since the rip started, once a second while it runs. */
function useElapsed(startedAt: number | undefined, running: boolean): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [running]);
  return startedAt ? (now - startedAt) / 1000 : 0;
}

function RipFooter() {
  const { t } = useTranslation();
  const disc = useCd((s) => s.disc);
  const selected = useCd((s) => s.selected);
  const rows = useCd((s) => s.rows);
  const ripState = useCd((s) => s.rip);
  const [showLog, setShowLog] = useState(false);
  const running = ripState?.running ?? false;
  const elapsed = useElapsed(ripState?.startedAt, running);
  const count = selected.filter(Boolean).length;
  const seconds = disc
    ? disc.tracks.reduce((sum, tr, i) => sum + (selected[i] ? tr.seconds : 0), 0)
    : 0;
  const ripButton = (
    <button
      type="button"
      className="btn primary"
      disabled={!disc || count === 0 || running}
      onClick={() => void rip()}
    >
      <Disc3 size={16} />
      {t(plural(count, "1 Titel rippen", "{0} Titel rippen"), { 0: String(count) })}
    </button>
  );

  if (running) {
    const progress = ripProgress({ rows, selected });
    const done = rows.filter((r, i) => selected[i] && r.status).length;
    const current = Math.min(count, done + 1);
    const left = elapsed > 4 && progress > 0.02 ? (elapsed / progress) * (1 - progress) : null;
    return (
      <FooterBar
        summary={
          <>
            <b>{t("Titel {0} von {1}", { 0: String(current), 1: String(count) })}</b>
            {left !== null &&
              ` · ${t("etwa {0} min übrig", { 0: String(Math.max(1, Math.ceil(left / 60))) })}`}
          </>
        }
      >
        <span
          className="prog wide"
          role="progressbar"
          aria-label={t("Fortschritt")}
          aria-valuenow={Math.round(progress * 100)}
        >
          <i style={{ width: `${progress * 100}%` }} />
        </span>
        <button type="button" className="btn" onClick={cancel}>
          <Square size={14} />
          {t("Abbrechen")}
        </button>
        {ripButton}
      </FooterBar>
    );
  }

  const summary = ripState?.summary;
  if (summary) {
    const text = summaryText(summary, t);
    return (
      <>
        <FooterBar summary={<b>{text}</b>}>
          {summary.kind === "Done" && summary.folder && (
            <button
              type="button"
              className="btn"
              onClick={() =>
                openFolder(summary.folder ?? "").catch((e: unknown) =>
                  useUi.getState().notify(String(e)),
                )
              }
            >
              <FolderOpen size={16} />
              {t("Ordner öffnen")}
            </button>
          )}
          <button
            type="button"
            className="btn"
            disabled={!ripState?.log.length}
            onClick={() => setShowLog(true)}
          >
            <ScrollText size={16} />
            {t("Protokoll anzeigen")}
          </button>
          {ripButton}
        </FooterBar>
        {showLog && ripState && (
          <div
            className="overlay"
            onPointerDown={(e) => e.target === e.currentTarget && setShowLog(false)}
          >
            <div
              className="picker"
              role="dialog"
              aria-modal="true"
              aria-label={t("Protokoll")}
              onKeyDown={(e) => e.key === "Escape" && setShowLog(false)}
            >
              <header className="picker-head">
                <h2>{t("Protokoll")}</h2>
                <button type="button" className="btn" onClick={() => setShowLog(false)}>
                  {t("Schließen")}
                </button>
              </header>
              <pre className="rip-log">{ripState.log.join("\n")}</pre>
            </div>
          </div>
        )}
      </>
    );
  }

  return (
    <FooterBar
      summary={
        <b>
          {disc
            ? t(plural(count, "1 Titel · {1}", "{0} Titel · {1}"), {
                0: String(count),
                1: fmtLen(seconds),
              })
            : ""}
        </b>
      }
    >
      {ripButton}
    </FooterBar>
  );
}

/** Insert a disc, read it, fill in the titles, rip. */
export function CdPage() {
  const { t } = useTranslation();
  const settings = useSettings((s) => s.settings);
  const options = useEncode((s) => s.options);
  const naming = useNaming((s) => s.options);
  const drives = useCd((s) => s.drives);
  const drive = useCd((s) => s.drive);
  const scanned = useCd((s) => s.scanned);
  const disc = useCd((s) => s.disc);
  const reading = useCd((s) => s.reading);
  const error = useCd((s) => s.error);
  const lookup = useCd((s) => s.lookup);
  const busy = useCd((s) => (s.rip?.running ?? false) || s.offset.busy);

  useEffect(() => {
    if (!useCd.getState().scanned) void scanDrives();
  }, []);
  if (!settings) return null;

  const r = settings.rip;
  const tiles: Tile[] = [
    {
      id: "format",
      title: t("Format"),
      summary: codecSummary(settings.rip_codec, settings.enc, options, t),
    },
    {
      id: "save",
      title: t("Speichern in"),
      summary: settings.rip_folder || t("Kein Ordner gewählt"),
    },
    {
      id: "naming",
      title: t("Benennung"),
      summary: namingSummary(
        { ...settings, name_scheme: r.name_scheme, output_mode: "SingleFolder" },
        naming,
        t,
      ),
    },
    {
      id: "readmode",
      title: t("Lesemodus"),
      summary:
        r.mode === "Secure"
          ? t("Sicher · bis {0} Neu-Lesungen", { 0: String(r.reread_limit) })
          : t("Schnell (Burst)"),
    },
  ];
  const titles = Object.fromEntries(tiles.map((x) => [x.id, x.title]));

  let body: ReactNode;
  if (disc) {
    body = (
      <>
        <DiscHeader />
        <TrackTable />
      </>
    );
  } else {
    let title = t("Audio-CD einlegen und „CD lesen“ klicken");
    let sub = t("{0} Laufwerk(e) gefunden", { 0: String(drives.length) });
    if (reading) {
      title = t("CD wird gelesen …");
      sub = "";
    } else if (scanned && drives.length === 0) {
      sub = t("Es wurde kein CD-Laufwerk gefunden.");
    }
    body = (
      <div className="cd-empty">
        <Disc3 size={48} strokeWidth={1.5} />
        <h2>{title}</h2>
        {sub && <p>{sub}</p>}
        {error && <p className="cd-error">{error}</p>}
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page-main">
        <PageHeader title="CD-Ripper" page="cd" firstTile="format">
          <select
            className="dd sm"
            aria-label={t("Laufwerk")}
            value={drive ?? ""}
            disabled={drives.length === 0 || busy}
            onChange={(e) => useCd.setState({ drive: e.target.value })}
          >
            {drives.length === 0 && <option value="">{t("Kein Laufwerk")}</option>}
            {drives.map((d) => (
              <option key={d.id} value={d.id}>
                {t("Laufwerk {d}", { d: d.name })}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="btn"
            disabled={busy || reading}
            onClick={() => void readCd()}
          >
            <RefreshCw size={16} />
            {t("CD lesen")}
          </button>
          <button
            type="button"
            className="btn"
            disabled={!disc || lookup === "busy" || busy}
            onClick={() => void searchInfo(false)}
          >
            <Globe size={16} />
            {t("Titel-Infos suchen")}
          </button>
          <button
            type="button"
            className="btn"
            disabled={!drive || busy}
            onClick={() => void ejectCd()}
          >
            <Eject size={16} />
            {t("Auswerfen")}
          </button>
        </PageHeader>
        <StepTiles page="cd" tiles={tiles} />
        <div className="content cd-content">{body}</div>
        <RipFooter />
      </div>
      <SidePanel page="cd" titles={titles}>
        {(tile) => {
          if (tile === "format") return <RipFormatPanel />;
          if (tile === "save") return <RipSavePanel />;
          if (tile === "naming") return <RipNamingPanel />;
          return <ReadModePanel />;
        }}
      </SidePanel>
    </div>
  );
}
