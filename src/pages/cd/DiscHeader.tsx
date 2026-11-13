import { Check, Disc3, LoaderCircle, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import { fmtLen } from "../../format";
import { useCd } from "../../store/cd";
import { InlineText } from "../../ui/InlineText";
import { chooseCandidate, setMeta } from "./cdFlow";

function num(v: string, fallback: number): number {
  const n = Number.parseInt(v, 10);
  return Number.isFinite(n) && n > 0 ? Math.min(n, 99) : fallback;
}

/** Cover, album data (edited where it stands), AccurateRip and MusicBrainz state, disc IDs. */
export function DiscHeader() {
  const { t } = useTranslation();
  const disc = useCd((s) => s.disc);
  const meta = useCd((s) => s.meta);
  const cover = useCd((s) => s.cover);
  const ar = useCd((s) => s.ar);
  const lookup = useCd((s) => s.lookup);
  const lookupError = useCd((s) => s.lookupError);
  const candidates = useCd((s) => s.candidates);
  if (!disc) return null;

  const arChip = {
    idle: { tone: "muted", text: t("AccurateRip: nicht abgefragt") },
    busy: { tone: "muted", text: t("AccurateRip: wird abgefragt…") },
    done: {
      tone: "ok",
      text: t("AccurateRip: CD in der Datenbank ({0} Pressungen)", { 0: String(ar.pressings) }),
    },
    failed: { tone: "warn", text: `AccurateRip: ${ar.reason ?? ""}` },
  }[ar.state];
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
            label={t("Interpret")}
            value={meta.artist}
            placeholder={t("Interpret")}
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
            label={t("Jahr")}
            value={meta.year}
            placeholder={t("Jahr")}
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
            label="Genre"
            value={meta.genre}
            placeholder="Genre"
            onCommit={(v) =>
              setMeta((m) => {
                m.genre = v;
              })
            }
          />
          <span aria-hidden="true">·</span>
          <span>Disc</span>
          <InlineText
            id="cd-disc"
            nextId="cd-disc-total"
            className="short"
            label={t("Disc-Nummer")}
            value={String(meta.disc)}
            onCommit={(v) =>
              setMeta((m) => {
                m.disc = num(v, m.disc);
              })
            }
          />
          <span>{t("von")}</span>
          <InlineText
            id="cd-disc-total"
            nextId="cd-title-0"
            className="short"
            label={t("Anzahl Discs")}
            value={String(meta.disc_total)}
            onCommit={(v) =>
              setMeta((m) => {
                m.disc_total = num(v, m.disc_total);
              })
            }
          />
        </p>
        <div className="ids">
          <span className={`chip ${arChip.tone}`}>
            {ar.state === "done" ? (
              <Check size={12} />
            ) : ar.state === "failed" ? (
              <TriangleAlert size={12} />
            ) : null}
            {arChip.text}
          </span>
          {lookup === "busy" && (
            <span className="chip muted">
              <LoaderCircle size={12} className="spin" />
              {t("Suche bei MusicBrainz…")}
            </span>
          )}
          {lookup === "failed" && (
            <span className="chip warn" title={lookupError ?? ""}>
              <TriangleAlert size={12} />
              {t("MusicBrainz nicht erreichbar")}
            </span>
          )}
          {lookup === "done" && candidates.length === 0 && (
            <span className="chip warn">{t("MusicBrainz: kein Treffer")}</span>
          )}
          {candidates.length === 1 && (
            <span className="chip ok">{t("MusicBrainz: 1 Treffer")}</span>
          )}
          {candidates.length > 1 && (
            <select
              className="dd sm"
              aria-label={t("MusicBrainz-Treffer")}
              value={current < 0 ? "" : String(current)}
              onChange={(e) => chooseCandidate(Number(e.target.value))}
            >
              {current < 0 && (
                <option value="">
                  {t("MusicBrainz: {0} Treffer", { 0: String(candidates.length) })}
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
          {t("{0} Titel · {1} · MusicBrainz {2} · freedb {3}", {
            0: String(disc.tracks.length),
            1: fmtLen(disc.total_seconds),
            2: disc.musicbrainz_id,
            3: disc.freedb_id,
          })}
        </p>
      </div>
    </section>
  );
}
