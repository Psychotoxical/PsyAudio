import { Check, Disc3, LoaderCircle, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import { fmtLen } from "../../format";
import { plural } from "../../i18n";
import { openCtdbPage } from "../../ipc/cd";
import { useCd } from "../../store/cd";
import { useUi } from "../../store/ui";
import { InlineText } from "../../ui/InlineText";
import { chooseCandidate, setMeta } from "./cdFlow";

function num(v: string, fallback: number): number {
  const n = Number.parseInt(v, 10);
  return Number.isFinite(n) && n > 0 ? Math.min(n, 99) : fallback;
}

/** Cover, album data (edited where it stands), CUETools database and MusicBrainz state, disc IDs. */
export function DiscHeader() {
  const { t } = useTranslation();
  const disc = useCd((s) => s.disc);
  const meta = useCd((s) => s.meta);
  const cover = useCd((s) => s.cover);
  const ctdb = useCd((s) => s.ctdb);
  const lookup = useCd((s) => s.lookup);
  const lookupError = useCd((s) => s.lookupError);
  const candidates = useCd((s) => s.candidates);
  if (!disc) return null;

  // the database's count, linked to the disc's public page so anyone can check it
  const known = ctdb.state === "done" && ctdb.total > 0;
  const ctdbChip = {
    idle: { tone: "muted", text: t("CTDB: not looked up") },
    busy: { tone: "muted", text: t("CTDB: looking up…") },
    done: known
      ? {
          tone: "ok",
          text: t(plural(ctdb.total, "CTDB · 1 rip", "CTDB · {0} rips"), { 0: String(ctdb.total) }),
        }
      : { tone: "muted", text: t("CTDB: disc not in the database") },
    failed: { tone: "warn", text: `CTDB: ${ctdb.reason ?? ""}` },
  }[ctdb.state];
  const openPage = () => openCtdbPage().catch((e: unknown) => useUi.getState().notify(String(e)));
  const current = candidates.findIndex((c) => c.release_id === meta.release_id);

  return (
    <section className="disc">
      <div className="cover">
        {cover ? <img src={cover} alt={t("Cover")} /> : <Disc3 size={40} strokeWidth={1.5} />}
      </div>
      <div className="disc-text">
        <InlineText
          id="cd-album"
          nextId="cd-artist"
          className="album"
          label={t("Album")}
          value={meta.album}
          placeholder={t("Album")}
          onCommit={(v) =>
            setMeta((m) => {
              m.album = v;
            })
          }
        />
        <p className="disc-meta">
          <InlineText
            id="cd-artist"
            nextId="cd-year"
            label={t("Artist")}
            value={meta.artist}
            placeholder={t("Artist")}
            onCommit={(v) =>
              setMeta((m) => {
                m.artist = v;
              })
            }
          />
          <span aria-hidden="true">·</span>
          <InlineText
            id="cd-year"
            nextId="cd-genre"
            label={t("Year")}
            value={meta.year}
            placeholder={t("Year")}
            onCommit={(v) =>
              setMeta((m) => {
                m.year = v;
              })
            }
          />
          <span aria-hidden="true">·</span>
          <InlineText
            id="cd-genre"
            nextId="cd-disc"
            label={t("Genre")}
            value={meta.genre}
            placeholder={t("Genre")}
            onCommit={(v) =>
              setMeta((m) => {
                m.genre = v;
              })
            }
          />
          <span aria-hidden="true">·</span>
          <span>{t("Disc")}</span>
          <InlineText
            id="cd-disc"
            nextId="cd-disc-total"
            className="short"
            label={t("Disc number")}
            value={String(meta.disc)}
            onCommit={(v) =>
              setMeta((m) => {
                m.disc = num(v, m.disc);
              })
            }
          />
          <span>{t("of")}</span>
          <InlineText
            id="cd-disc-total"
            nextId="cd-title-0"
            className="short"
            label={t("Number of discs")}
            value={String(meta.disc_total)}
            onCommit={(v) =>
              setMeta((m) => {
                m.disc_total = num(v, m.disc_total);
              })
            }
          />
        </p>
        <div className="ids">
          {known ? (
            <button
              type="button"
              className={`chip ${ctdbChip.tone} chip-link`}
              title={t("Open the disc's entry in the CUETools database")}
              onClick={() => void openPage()}
            >
              <Check size={12} />
              {ctdbChip.text}
            </button>
          ) : (
            <span className={`chip ${ctdbChip.tone}`}>
              {ctdb.state === "failed" ? <TriangleAlert size={12} /> : null}
              {ctdbChip.text}
            </span>
          )}
          {lookup === "busy" && (
            <span className="chip muted">
              <LoaderCircle size={12} className="spin" />
              {t("Searching MusicBrainz…")}
            </span>
          )}
          {lookup === "failed" && (
            <span className="chip warn" title={lookupError ?? ""}>
              <TriangleAlert size={12} />
              {t("MusicBrainz not reachable")}
            </span>
          )}
          {lookup === "done" && candidates.length === 0 && (
            <span className="chip warn">{t("MusicBrainz: no match")}</span>
          )}
          {candidates.length === 1 && (
            <span className="chip ok">
              {t(plural(1, "MusicBrainz: 1 match", "MusicBrainz: {0} matches"), { 0: "1" })}
            </span>
          )}
          {candidates.length > 1 && (
            <select
              className="dd sm"
              aria-label={t("MusicBrainz matches")}
              value={current < 0 ? "" : String(current)}
              onChange={(e) => chooseCandidate(Number(e.target.value))}
            >
              {current < 0 && (
                <option value="">
                  {t(
                    plural(candidates.length, "MusicBrainz: 1 match", "MusicBrainz: {0} matches"),
                    {
                      0: String(candidates.length),
                    },
                  )}
                </option>
              )}
              {candidates.map((c, i) => (
                <option key={`${c.release_id ?? i}`} value={String(i)}>
                  {`${c.artist} – ${c.album}${c.year ? ` (${c.year})` : ""}`}
                </option>
              ))}
            </select>
          )}
        </div>
        <p className="mono">
          {t(
            plural(
              disc.tracks.length,
              "1 track · {1} · MusicBrainz {2} · freedb {3}",
              "{0} tracks · {1} · MusicBrainz {2} · freedb {3}",
            ),
            {
              0: String(disc.tracks.length),
              1: fmtLen(disc.total_seconds),
              2: disc.musicbrainz_id,
              3: disc.freedb_id,
            },
          )}
        </p>
      </div>
    </section>
  );
}
