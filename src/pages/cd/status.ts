import { plural } from "../../i18n";
import type { CtdbResult, RipPhase, RipSummary, TrackStatus } from "../../ipc/cd";

type T = (key: string, values?: Record<string, string>) => string;

/** A track's status in words, as rip::TrackStatus::text (same dictionary keys). */
export function statusText(s: TrackStatus, t: T): string {
  switch (s.kind) {
    case "Secure":
      if (s.verified_first_pass) return t("Sicher (1 Durchgang, von CTDB bestätigt)");
      if (s.rereads > 0)
        return t(
          plural(
            s.rereads,
            "Sicher (Warnung: {0} Durchgänge, {1} Block neu gelesen)",
            "Sicher (Warnung: {0} Durchgänge, {1} Blöcke neu gelesen)",
          ),
          {
            0: String(s.passes),
            1: String(s.rereads),
          },
        );
      return t("Sicher ({0} Durchgänge identisch)", { 0: String(s.passes) });
    case "Burst":
      return t("Gerippt (Burst)");
    case "Insecure":
      return t(plural(s.sectors, "Unsicher ({0} Sektor)", "Unsicher ({0} Sektoren)"), {
        0: String(s.sectors),
      });
    case "Marked":
      return s.unconfirmed
        ? t("Fehler – nicht von CTDB bestätigt ({status})", { status: statusText(s.was, t) })
        : t("Fehler – {status}", { status: statusText(s.was, t) });
    case "Failed":
      return s.encoding
        ? t("Fehler beim Kodieren: {e}", { e: s.reason })
        : t("Fehler: {e}", { e: s.reason });
    case "Cancelled":
      return t("Abgebrochen");
  }
}

/** The end of a rip in words, as rip::RipSummary::text (same dictionary keys). */
export function summaryText(s: RipSummary, t: T): string {
  switch (s.kind) {
    case "Done":
      return t(
        plural(
          s.files,
          "Fertig: {0} Datei, {accurate} von CTDB bestätigt",
          "Fertig: {0} Dateien, {accurate} von CTDB bestätigt",
        ),
        { 0: String(s.files), accurate: String(s.accurate) },
      );
    case "Cancelled":
      return t("Abgebrochen");
    case "Failed":
      return s.reason;
  }
}

/** The CUETools database's answer in words, as rip::CtdbResult::text. */
export function ctdbText(c: CtdbResult, t: T): string {
  if (c.unavailable) return `CTDB: ${c.unavailable}`;
  if (c.confirmed)
    return t("CTDB: bestätigt ({c} von {total})", {
      c: String(c.confidence),
      total: String(c.total),
    });
  if (c.in_db) return t("CTDB: nicht bestätigt");
  return t("CTDB: CD nicht in der Datenbank");
}

export type Tone = "ok" | "warn" | "bad" | "muted";

/** The short chip of the status column: "Sicher · CTDB 1085" in green, "Unsicher (3 Sektoren)" in yellow. */
export function statusChip(
  s: TrackStatus,
  ctdb: CtdbResult | null,
  t: T,
): { text: string; tone: Tone } {
  let text: string;
  let tone: Tone;
  switch (s.kind) {
    case "Secure":
      text = t("Sicher");
      tone = "ok";
      break;
    case "Burst":
      text = "Burst";
      tone = "ok";
      break;
    case "Insecure":
      return { text: statusText(s, t), tone: "warn" };
    case "Marked":
    case "Failed":
      return { text: t("Fehler"), tone: "bad" };
    case "Cancelled":
      return { text: t("Abgebrochen"), tone: "muted" };
  }
  if (ctdb?.confirmed) return { text: `${text} · CTDB ${ctdb.confidence}`, tone };
  if (ctdb && !ctdb.unavailable && ctdb.in_db)
    return { text: `${text} · ${t("CTDB nicht bestätigt")}`, tone: "warn" };
  return { text, tone };
}

/** "Lesen 63 %" / "Kodieren…" while a track runs. */
export function phaseText(p: RipPhase, t: T): string {
  if (p.kind === "Encoding") return t("Kodieren…");
  return p.done <= 0 ? t("Lesen…") : t("Lesen {0} %", { 0: String(Math.round(p.done * 100)) });
}
