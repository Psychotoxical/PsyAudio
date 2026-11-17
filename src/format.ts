// Lengths and sizes worded as in the Rust core (audio::fmt_len, audio::fmt_size), with the
// decimal mark of the interface language.

export function fmtLen(secs: number): string {
  const s = Math.round(Math.max(0, secs));
  const mm = String(Math.floor(s / 60) % 60).padStart(2, "0");
  const ss = String(s % 60).padStart(2, "0");
  return s >= 3600 ? `${Math.floor(s / 3600)}:${mm}:${ss}` : `${Math.floor(s / 60)}:${ss}`;
}

export function fmtSize(bytes: number, lang: string): string {
  const num = (v: number, digits: number) =>
    v.toLocaleString(lang === "de" ? "de-DE" : "en-US", {
      minimumFractionDigits: digits,
      maximumFractionDigits: digits,
    });
  if (bytes >= 2 ** 30) return `${num(bytes / 2 ** 30, 2)} GB`;
  if (bytes >= 2 ** 20) return `${num(bytes / 2 ** 20, 1)} MB`;
  return `${num(bytes / 1024, 0)} KB`;
}

/** "FLAC 44,1 kHz/16" or "MP3 320 kbit/s" for the format column. */
export function fmtFormat(
  path: string,
  info: { rate: number | null; bits: number | null; bitrate: number | null } | undefined,
  lang: string,
): string {
  const ext = (path.split(".").pop() ?? "").toUpperCase();
  if (!info) return ext;
  if (info.bits && info.rate) {
    const khz = (info.rate / 1000).toLocaleString(lang === "de" ? "de-DE" : "en-US", {
      maximumFractionDigits: 1,
    });
    return `${ext} ${khz} kHz/${info.bits}`;
  }
  if (info.bitrate) return `${ext} ${info.bitrate} kbit/s`;
  return ext;
}

type T = (key: string, values?: Record<string, string>) => string;

/**
 * Seconds as words, as timefmt::long_duration: the largest unit that isn't zero, and the next
 * one too when `verbose` or the first is below 4. Singular for exactly 1.
 */
export function longDuration(total: number, verbose: boolean, t: T): string {
  const s = Math.max(0, Math.floor(total));
  const hoursAll = Math.floor(s / 3600);
  const rest = s % 3600;
  const parts: [number, string, string][] = [
    [Math.floor(hoursAll / 24), "{0} day", "{0} days"],
    [hoursAll % 24, "{0} hour", "{0} hours"],
    [Math.floor(rest / 60), "{0} minute", "{0} minutes"],
    [rest % 60, "{0} second", "{0} seconds"],
  ];
  const word = ([n, one, many]: [number, string, string]) =>
    t(n === 1 ? one : many, { 0: String(n) });
  const i = parts.findIndex(([n]) => n > 0);
  if (i === -1) return t("{0} seconds", { 0: "0" });
  const first = parts[i] as [number, string, string];
  const next = parts[i + 1];
  return next && (verbose || first[0] < 4) ? `${word(first)} ${word(next)}` : word(first);
}

/** Audio length / time taken, as timefmt::encoding_speed (one decimal up to 4×). */
export function encodingSpeed(audioSecs: number, elapsed: number, t: T, lang: string): string {
  const x = audioSecs / (elapsed === 0 ? 1 : elapsed);
  const n =
    x <= 4
      ? x.toLocaleString(lang === "de" ? "de-DE" : "en-US", {
          minimumFractionDigits: 1,
          maximumFractionDigits: 1,
        })
      : String(Math.floor(x + 0.5));
  return t("{0}× encoding speed", { 0: n });
}
