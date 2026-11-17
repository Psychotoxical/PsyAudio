import { plural } from "../../i18n";
import type { CtdbResult, RipPhase, RipSummary, TrackStatus } from "../../ipc/cd";

type T = (key: string, values?: Record<string, string>) => string;

/** A track's status in words, as rip::TrackStatus::text (same dictionary keys). */
export function statusText(s: TrackStatus, t: T): string {
  switch (s.kind) {
    case "Secure":
      if (s.verified_first_pass) return t("Secure (1 pass, confirmed by CTDB)");
      if (s.rereads > 0)
        return t(
          plural(
            s.rereads,
            "Secure (warning: {0} passes, {1} block re-read)",
            "Secure (warning: {0} passes, {1} blocks re-read)",
          ),
          {
            0: String(s.passes),
            1: String(s.rereads),
          },
        );
      return t("Secure ({0} passes identical)", { 0: String(s.passes) });
    case "Burst":
      return t("Ripped (burst)");
    case "Insecure":
      return t(plural(s.sectors, "Insecure ({0} sector)", "Insecure ({0} sectors)"), {
        0: String(s.sectors),
      });
    case "Marked":
      return s.unconfirmed
        ? t("Error – not confirmed by CTDB ({status})", { status: statusText(s.was, t) })
        : t("Error – {status}", { status: statusText(s.was, t) });
    case "Failed":
      return s.encoding
        ? t("Error while encoding: {e}", { e: s.reason })
        : t("Error: {e}", { e: s.reason });
    case "Cancelled":
      return t("Cancelled");
  }
}

/** The end of a rip in words, as rip::RipSummary::text (same dictionary keys). */
export function summaryText(s: RipSummary, t: T): string {
  switch (s.kind) {
    case "Done":
      return t(
        plural(
          s.files,
          "Done: {0} file, {accurate} confirmed by CTDB",
          "Done: {0} files, {accurate} confirmed by CTDB",
        ),
        { 0: String(s.files), accurate: String(s.accurate) },
      );
    case "Cancelled":
      return t("Cancelled");
    case "Failed":
      return s.reason;
  }
}

/** The CUETools database's answer in words, as rip::CtdbResult::text. */
export function ctdbText(c: CtdbResult, t: T): string {
  if (c.unavailable) return `CTDB: ${c.unavailable}`;
  if (c.confirmed)
    return t("CTDB: confirmed ({c} of {total})", {
      c: String(c.confidence),
      total: String(c.total),
    });
  if (c.in_db) return t("CTDB: not confirmed");
  return t("CTDB: disc not in the database");
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
      text = t("Secure");
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
      return { text: t("Error"), tone: "bad" };
    case "Cancelled":
      return { text: t("Cancelled"), tone: "muted" };
  }
  if (ctdb?.confirmed) return { text: `${text} · CTDB ${ctdb.confidence}`, tone };
  if (ctdb && !ctdb.unavailable && ctdb.in_db)
    return { text: `${text} · ${t("CTDB not confirmed")}`, tone: "warn" };
  return { text, tone };
}

/** "Reading 63 %" / "Encoding…" while a track runs. */
export function phaseText(p: RipPhase, t: T): string {
  if (p.kind === "Encoding") return t("Encoding…");
  return p.done <= 0 ? t("Reading…") : t("Reading {0} %", { 0: String(Math.round(p.done * 100)) });
}
