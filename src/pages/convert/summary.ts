import type { Codec } from "../../ipc/bindings/Codec";
import type { Effect } from "../../ipc/bindings/Effect";
import type { EncSettings } from "../../ipc/bindings/EncSettings";
import type { Settings } from "../../ipc/bindings/Settings";
import type { NamingOptions } from "../../ipc/naming";
import { type EffectEntry, effectKind } from "../../store/effects";
import { codecInfo, type EncodeOptions } from "../../store/encode";

type T = (key: string, values?: Record<string, string>) => string;

const OPUS_MODES = ["VBR", "CVBR", "CBR"];

/** The format tile's line: "Opus · 192 kbit/s · VBR". */
export function codecSummary(
  codec: Codec,
  e: EncSettings,
  options: EncodeOptions | null,
  t: T,
): string {
  const info = codecInfo(options, codec);
  const name = info?.short ?? codec;
  const kbps = (k: number) => `${k} kbit/s`;
  const parts: string[] = [name];
  const at = <V>(list: V[] | undefined, i: number) =>
    list && list.length > 0 ? list[i % list.length] : undefined;
  switch (codec) {
    case "Mp3":
      if (e.mp3_mode === "Vbr") parts.push(`-V ${e.mp3_vbr}`, "VBR");
      else parts.push(kbps(e.mp3_kbps), e.mp3_mode.toUpperCase());
      break;
    case "Flac":
      parts.push(t("Level {0}", { 0: String(e.flac_level) }));
      break;
    case "AacM4a":
    case "AudioBook":
      if (e.aac_vbr) parts.push(t("Quality {0}", { 0: String(e.aac_quality) }), "VBR");
      else parts.push(kbps(e.aac_kbps), "CBR");
      break;
    case "Opus":
      parts.push(kbps(e.opus_kbps), OPUS_MODES[e.opus_mode] ?? "VBR");
      break;
    case "Vorbis":
      parts.push(
        e.vorbis_mode === 0 ? t("Quality {0}", { 0: String(e.vorbis_q) }) : kbps(e.vorbis_kbps),
      );
      break;
    case "Wave": {
      const label =
        options?.pcm_formats.find((p) => p.format === e.wav_format)?.label ?? e.wav_format;
      parts.push(e.wav_match_source ? t("as the source") : label);
      break;
    }
    case "Aiff":
      parts.push(e.aiff_match_source ? t("as the source") : `${e.aiff_bits} Bit`);
      break;
    case "WavPack":
      parts.push(t(at(options?.wavpack_modes, e.wavpack_mode) ?? ""));
      break;
    case "MonkeysAudio":
      parts.push(t(at(options?.ape_levels, e.ape_level) ?? ""));
      break;
    case "Wma":
      parts.push(kbps(e.wma_kbps));
      break;
    case "Musepack":
      parts.push(t(at(options?.mpc_profiles, e.mpc_profile)?.name ?? ""));
      break;
    case "Speex":
      parts.push(t("Quality {0}", { 0: String(e.speex_q) }));
      break;
    case "Dsd":
      parts.push(at(options?.dsd_modes, e.dsd_mode) ?? "");
      break;
    case "Multi":
      parts.push(e.multi.map((c) => codecInfo(options, c)?.short ?? c).join(", "));
      break;
    case "LengthSplit":
      parts.push(`${Math.max(1, Math.round(e.split_seconds / 60))} min`);
      break;
    default:
      return info?.name ?? codec;
  }
  return parts.filter(Boolean).join(" · ");
}

/** The "Save to" tile's line: where the files go. */
export function saveSummary(s: Settings, t: T): string {
  const folder = s.output_folder || t("No folder chosen");
  switch (s.output_mode) {
    case "SourceFolder":
      return t("Next to the source");
    case "PreservePath":
      return `${t("Keep path")} · ${folder}`;
    default:
      return folder;
  }
}

/** The "Naming" tile's line: the preset's name, or how many chips. */
export function namingSummary(s: Settings, naming: NamingOptions | null, t: T): string {
  if (s.output_mode === "PreservePath") return t("As the source");
  const same = (a: string[]) =>
    a.length === s.name_scheme.length && a.every((x, i) => x === s.name_scheme[i]);
  const preset = naming?.presets.find((p) => same(p.scheme));
  return preset ? t(preset.name) : t("{0} building blocks", { 0: String(s.name_scheme.length) });
}

/** The "Effects" tile's line: the effects' names in order (the count first when several), or "None". */
export function effectsSummary(effects: Effect[], entries: EffectEntry[], t: T): string {
  if (effects.length === 0) return t("None");
  const names = effects
    .map((e) => entries.find((x) => effectKind(x.effect) === effectKind(e))?.label ?? effectKind(e))
    .join(", ");
  return effects.length === 1
    ? names
    : `${t("{0} effects", { 0: String(effects.length) })} · ${names}`;
}
