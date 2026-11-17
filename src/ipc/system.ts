import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "./bindings/Settings";

export interface Program {
  key: "ffmpeg" | "mac" | "mpcenc" | "sox";
  /** Where it was found; null when missing. */
  found: string | null;
}

export interface SystemInfo {
  cores: number;
  temp_default: string;
  os: "windows" | "linux" | "macos";
}

/** The four external programs with where each was found; `refresh` asks FFmpeg again. */
export function programs(settings: Settings, refresh = false): Promise<Program[]> {
  return invoke<Program[]>("programs", { settings, refresh });
}

/** FFmpeg's download page in the browser (when no FFmpeg is found). */
export function openFfmpegDownload(): Promise<void> {
  return invoke<void>("open_ffmpeg_download");
}

export function systemInfo(): Promise<SystemInfo> {
  return invoke<SystemInfo>("system_info");
}

/** The defaults with the profiles kept ("Alle Einstellungen auf Standard"). */
export function resetSettings(settings: Settings): Promise<Settings> {
  return invoke<Settings>("reset_settings", { settings });
}
