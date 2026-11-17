import { create } from "zustand";
import type { Codec } from "../ipc/bindings/Codec";
import type { PcmFormat } from "../ipc/bindings/PcmFormat";

export interface CodecInfo {
  codec: Codec;
  name: string;
  short: string;
  lossless: boolean;
  tool: boolean;
  description?: string;
}

/** What the format area offers (from Rust encode_options, labels in the current language). */
export interface EncodeOptions {
  codecs: CodecInfo[];
  lossless: Codec[];
  compressed: Codec[];
  tools: Codec[];
  audio_codecs: Codec[];
  mp3_rates: number[];
  aac_rates: number[];
  aac_vbr_kbps: number[];
  vorbis_kbps: number[];
  stereo_modes: string[];
  wavpack_modes: string[];
  ape_levels: string[];
  dsd_modes: string[];
  mpc_profiles: { name: string; kbps: number }[];
  pcm_formats: { format: PcmFormat; label: string }[];
}

interface EncodeState {
  options: EncodeOptions | null;
  /** Formats that cannot run now, with the reason (missing program or encoder). */
  problems: Partial<Record<Codec, string>>;
  /** FFmpeg's path when found, null when missing, undefined while unknown. */
  ffmpeg: string | null | undefined;
  /** How many encoders that FFmpeg offers. */
  encoders: number;
  /** Where that FFmpeg comes from; null while unknown or missing. */
  ffmpegSource: FfmpegSource | null;
  /** The shipped FFmpeg when it only stands by for the one above. */
  ffmpegFallback: string | null;
}

/** The settings path, the system's FFmpeg, or the one PsyAudio ships. */
export type FfmpegSource = "configured" | "system" | "bundled";

export const useEncode = create<EncodeState>(() => ({
  options: null,
  problems: {},
  ffmpeg: undefined,
  encoders: 0,
  ffmpegSource: null,
  ffmpegFallback: null,
}));

export function codecInfo(options: EncodeOptions | null, codec: Codec): CodecInfo | undefined {
  return options?.codecs.find((c) => c.codec === codec);
}
