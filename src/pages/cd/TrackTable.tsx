import { Check, Minus, TriangleAlert, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { fmtLen } from "../../format";
import type { CtdbResult, TrackStatus } from "../../ipc/cd";
import { useCd } from "../../store/cd";
import { InlineText } from "../../ui/InlineText";
import { Tip } from "../../ui/Tip";
import { setMeta } from "./cdFlow";
import { ctdbText, phaseText, statusChip, statusText } from "./status";

/** The status column of a ripped track: short chip, full wording on hover. */
export function StatusChip({ status, ctdb }: { status: TrackStatus; ctdb: CtdbResult | null }) {
  const { t } = useTranslation();
  const chip = statusChip(status, ctdb, t);
  const title = [statusText(status, t), ctdb ? ctdbText(ctdb, t) : ""].filter(Boolean).join(" · ");
  const icon = {
    ok: <Check size={12} />,
    warn: <TriangleAlert size={12} />,
    bad: <X size={12} />,
    muted: <Minus size={12} />,
  }[chip.tone];
  return (
    <Tip className={`chip ${chip.tone}`} text={title}>
      {icon}
      <span>{chip.text}</span>
    </Tip>
  );
}

/** Tick, number, title and artist (edited where they stand), length and the rip status. */
export function TrackTable() {
  const { t } = useTranslation();
  const disc = useCd((s) => s.disc);
  const meta = useCd((s) => s.meta);
  const selected = useCd((s) => s.selected);
  const rows = useCd((s) => s.rows);
  const rip = useCd((s) => s.rip);
  if (!disc) return null;
  const all = selected.length > 0 && selected.every(Boolean);
  const toggle = (i: number) =>
    useCd.setState({ selected: selected.map((v, j) => (j === i ? !v : v)) });

  return (
    <div className="tracks">
      <table>
        {/* fixed widths here, not on the header cells: the free room goes to title and artist */}
        <colgroup>
          <col className="c-tick" />
          <col className="c-num" />
          <col />
          <col />
          <col className="c-len" />
          <col className="c-status" />
        </colgroup>
        <thead>
          <tr>
            <th className="tick">
              <input
                type="checkbox"
                aria-label={t("Alle Titel")}
                checked={all}
                onChange={() => useCd.setState({ selected: selected.map(() => !all) })}
              />
            </th>
            <th className="num">#</th>
            <th>{t("Titel")}</th>
            <th>{t("Interpret")}</th>
            <th className="r">{t("Länge")}</th>
            <th className="st-col">Status</th>
          </tr>
        </thead>
        <tbody>
          {disc.tracks.map((tr, i) => {
            const m = meta.tracks[i];
            const row = rows[i];
            const on = selected[i] ?? false;
            const n = String(tr.number).padStart(2, "0");
            let state = null;
            if (row?.status) state = <StatusChip status={row.status} ctdb={row.ctdb} />;
            else if (row?.phase)
              state = (
                <span className="reading">
                  <span className="prog">
                    <i style={{ width: `${Math.round(row.fraction * 100)}%` }} />
                  </span>
                  {phaseText(row.phase, t)}
                </span>
              );
            else if (rip?.running && on) state = <span className="wait">{t("wartet")}</span>;
            return (
              <tr
                key={tr.number}
                className={[row?.phase && !row.status ? "on" : "", on ? "" : "off"].join(" ")}
              >
                <td className="tick">
                  <input
                    type="checkbox"
                    aria-label={t("Titel {0} rippen", { 0: n })}
                    checked={on}
                    onChange={() => toggle(i)}
                  />
                </td>
                <td className="num">{n}</td>
                <td className="t">
                  <InlineText
                    id={`cd-title-${i}`}
                    nextId={`cd-artist-${i}`}
                    label={t("Titel {0}", { 0: n })}
                    value={m?.title ?? ""}
                    onCommit={(v) =>
                      setMeta((x) => {
                        if (x.tracks[i]) x.tracks[i].title = v;
                      })
                    }
                  />
                </td>
                <td>
                  <InlineText
                    id={`cd-artist-${i}`}
                    nextId={i + 1 < disc.tracks.length ? `cd-title-${i + 1}` : undefined}
                    label={t("Interpret von Titel {0}", { 0: n })}
                    value={m?.artist ?? ""}
                    placeholder={meta.artist}
                    onCommit={(v) =>
                      setMeta((x) => {
                        if (x.tracks[i]) x.tracks[i].artist = v;
                      })
                    }
                  />
                </td>
                <td className="r">{fmtLen(tr.seconds)}</td>
                <td className="st-col">{state}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
