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
      {t(plural(count, "Rip 1 track", "Rip {0} tracks"), { 0: String(count) })}
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
            <b>{t("Track {0} of {1}", { 0: String(current), 1: String(count) })}</b>
            {left !== null &&
              ` · ${t("about {0} min left", { 0: String(Math.max(1, Math.ceil(left / 60))) })}`}
          </>
        }
      >
        <span
          className="prog wide"
          role="progressbar"
          aria-label={t("Progress")}
          aria-valuenow={Math.round(progress * 100)}
        >
          <i style={{ width: `${progress * 100}%` }} />
        </span>
        <button type="button" className="btn" onClick={cancel}>
          <Square size={14} />
          {t("Cancel")}
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
              {t("Open folder")}
            </button>
          )}
          <button
            type="button"
            className="btn"
            disabled={!ripState?.log.length}
            onClick={() => setShowLog(true)}
          >
            <ScrollText size={16} />
            {t("Show log")}
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
              aria-label={t("Log")}
              onKeyDown={(e) => e.key === "Escape" && setShowLog(false)}
            >
              <header className="picker-head">
                <h2>{t("Log")}</h2>
                <button type="button" className="btn" onClick={() => setShowLog(false)}>
                  {t("Close")}
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
            ? t(plural(count, "1 track · {1}", "{0} tracks · {1}"), {
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
      title: t("Save to"),
      summary: settings.rip_folder || t("No folder chosen"),
    },
    {
      id: "naming",
      title: t("Naming"),
      summary: namingSummary(
        { ...settings, name_scheme: r.name_scheme, output_mode: "SingleFolder" },
        naming,
        t,
      ),
    },
    {
      id: "readmode",
      title: t("Reading mode"),
      summary:
        r.mode === "Secure"
          ? t("Secure · up to {0} re-reads", { 0: String(r.reread_limit) })
          : t("Fast (burst)"),
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
    let title = t("Insert an audio CD and click “Read CD”");
    let sub = t("{0} drive(s) found", { 0: String(drives.length) });
    if (reading) {
      title = t("Reading the CD …");
      sub = "";
    } else if (scanned && drives.length === 0) {
      sub = t("No CD drive was found.");
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
        <PageHeader title={t("CD-Ripper")} page="cd" firstTile="format">
          <select
            className="dd sm"
            aria-label={t("Drive")}
            value={drive ?? ""}
            disabled={drives.length === 0 || busy}
            onChange={(e) => useCd.setState({ drive: e.target.value })}
          >
            {drives.length === 0 && <option value="">{t("No drive")}</option>}
            {drives.map((d) => (
              <option key={d.id} value={d.id}>
                {t("Drive {d}", { d: d.name })}
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
            {t("Read CD")}
          </button>
          <button
            type="button"
            className="btn"
            disabled={!disc || lookup === "busy" || busy}
            onClick={() => void searchInfo(false)}
          >
            <Globe size={16} />
            {t("Look up track info")}
          </button>
          <button
            type="button"
            className="btn narrow-icon"
            aria-label={t("Eject")}
            title={t("Eject")}
            disabled={!drive || busy}
            onClick={() => void ejectCd()}
          >
            <Eject size={16} />
            <span className="narrow-hide">{t("Eject")}</span>
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
