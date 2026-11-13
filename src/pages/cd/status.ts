import type { ArResult, RipPhase, TrackStatus } from "../../ipc/cd";

type T = (key: string, values?: Record<string, string>) => string;

/** A track's status in words, as rip::TrackStatus::text (same dictionary keys). */
export function statusText(s: TrackStatus, t: T): string {
  switch (s.kind) {
    case "Secure":
      if (s.verified_first_pass) return t("Sicher (1 Durchgang, von AccurateRip bestätigt)");
      if (s.rereads > 0)
        return t("Sicher (Warnung: {0} Durchgänge, {1} Blöcke neu gelesen)", {
          0: String(s.passes),
          1: String(s.rereads),
        });
      return t("Sicher ({0} Durchgänge identisch)", { 0: String(s.passes) });
    case "Burst":
      return t("Gerippt (Burst)");
    case "Insecure":
      return t("Unsicher ({0} Sektoren)", { 0: String(s.sectors) });
    case "Marked":
      return s.unconfirmed
        ? t("Fehler – nicht von AccurateRip bestätigt ({status})", { status: statusText(s.was, t) })
        : t("Fehler – {status}", { status: statusText(s.was, t) });
    case "Failed":
      return s.encoding
        ? t("Fehler beim Kodieren: {e}", { e: s.reason })
        : t("Fehler: {e}", { e: s.reason });
    case "Cancelled":
      return t("Abgebrochen");
  }
}

/** AccurateRip's answer in words, as rip::ArResult::text. */
export function arText(ar: ArResult, t: T): string {
  if (ar.unavailable) return `AccurateRip: ${ar.unavailable}`;
  if (ar.confirmed)
    return t("AccurateRip: Genau (Vertrauen {c}, {v})", {
      c: String(ar.confidence),
      v: `v${ar.version}`,
    });
  if (ar.in_db)
    return t("AccurateRip: Ungenau (Vertrauen {c}) – oder andere Pressung", {
      c: String(ar.confidence),
    });
  return t("AccurateRip: Titel nicht in der Datenbank");
}

export type Tone = "ok" | "warn" | "bad" | "muted";

/** The short chip of the status column: "Sicher · AR 200" in green, "Unsicher (3 Sektoren)" in yellow. */
export function statusChip(
  s: TrackStatus,
  ar: ArResult | null,
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
  if (ar?.confirmed) return { text: `${text} · AR ${ar.confidence}`, tone };
  if (ar && !ar.unavailable && ar.in_db)
    return { text: `${text} · ${t("AR ungenau")}`, tone: "warn" };
  return { text, tone };
}

/** "Lesen 63 %" / "Kodieren…" while a track runs. */
export function phaseText(p: RipPhase, t: T): string {
  if (p.kind === "Encoding") return t("Kodieren…");
  return p.done <= 0 ? t("Lesen…") : t("Lesen {0} %", { 0: String(Math.round(p.done * 100)) });
}
