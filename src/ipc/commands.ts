import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "./bindings/Settings";

export interface LoadedSettings {
  settings: Settings;
  /** Set when parts of settings.json could not be read (the old file is kept as .bak). */
  warning: string | null;
}

export function loadSettings(): Promise<LoadedSettings> {
  return invoke<LoadedSettings>("load_settings");
}

export function saveSettings(settings: Settings): Promise<void> {
  return invoke<void>("save_settings", { settings });
}

/** `--demo=<page>` for screenshots: konverter, stapel, cd or einstellungen. */
export interface Demo {
  page: string;
  light: boolean;
}

export function demoState(): Promise<Demo | null> {
  return invoke<Demo | null>("demo_state");
}
