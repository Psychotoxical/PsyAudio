import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import type { Preset } from "../ui/ChipEditor";
import type { Settings } from "./bindings/Settings";

export interface NamingOptions {
  presets: Preset[];
  data: string[];
  separators: string[];
  folder: string;
  /** Chip → German label. */
  labels: Record<string, string>;
}

/** The chips on offer and the presets (loaded once at start). */
export const useNaming = create<{ options: NamingOptions | null }>(() => ({ options: null }));

export async function loadNamingOptions(): Promise<void> {
  const options = await invoke<NamingOptions>("naming_options");
  useNaming.setState({ options });
}

/** Where the converter would write `path` with these settings; null for tool formats.
 * `position` is its number and count among the list's files in its folder. */
export function previewName(
  settings: Settings,
  path: string,
  root: string | null,
  position: [number, number] | null = null,
): Promise<string | null> {
  return invoke<string | null>("preview_name", { settings, path, root, position });
}

/** The tags "Tags from file name" reads from `path`. */
export function tagsFromName(path: string, scheme: string[]): Promise<[string, string][]> {
  return invoke<[string, string][]>("tags_from_name", { path, scheme });
}

/** Creates the folder if needed and opens it in the file manager. */
export function openFolder(path: string): Promise<void> {
  return invoke<void>("open_folder", { path });
}
