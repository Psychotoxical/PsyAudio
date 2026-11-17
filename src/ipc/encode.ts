import { invoke } from "@tauri-apps/api/core";
import { type EncodeOptions, type FfmpegSource, useEncode } from "../store/encode";
import type { Codec } from "./bindings/Codec";
import type { EncSettings } from "./bindings/EncSettings";
import type { Settings } from "./bindings/Settings";

export function encodeOptions(): Promise<EncodeOptions> {
  return invoke<EncodeOptions>("encode_options");
}

export function codecProblems(settings: Settings): Promise<Partial<Record<Codec, string>>> {
  return invoke("codec_problems", { settings });
}

export interface ToolStatus {
  /** The FFmpeg that does the work. */
  ffmpeg: string | null;
  encoders: number;
  source: FfmpegSource | null;
  /** The shipped FFmpeg when it only stands by. */
  fallback: string | null;
}

export function toolStatus(settings: Settings, refresh: boolean): Promise<ToolStatus> {
  return invoke("tool_status", { settings, refresh });
}

export function estimateKbps(codec: Codec, enc: EncSettings): Promise<number> {
  return invoke<number>("estimate_kbps", { codec, enc });
}

export function planRequirement(settings: Settings, codec: Codec): Promise<string | null> {
  return invoke<string | null>("plan_requirement", { settings, codec });
}

export function setRustLanguage(lang: string): Promise<void> {
  return invoke<void>("set_language", { lang });
}

/** Loads the format lists, FFmpeg's state and what is missing (again after a language change). */
export async function refreshEncode(settings: Settings, refresh = false): Promise<void> {
  await setRustLanguage(settings.lang);
  const [options, status, problems] = await Promise.all([
    encodeOptions(),
    toolStatus(settings, refresh),
    codecProblems(settings),
  ]);
  useEncode.setState({
    options,
    problems,
    ffmpeg: status.ffmpeg,
    encoders: status.encoders,
    ffmpegSource: status.source,
    ffmpegFallback: status.fallback,
  });
}
