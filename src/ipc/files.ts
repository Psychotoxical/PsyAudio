import { Channel, invoke } from "@tauri-apps/api/core";
import type { Info } from "./bindings/Info";

export interface FileEntry {
  id: number;
  path: string;
  root: string | null;
  size: number;
}

export interface FileInfo {
  id: number;
  tags: Record<string, string>;
  info: Info;
  cover: boolean;
}

/** Files and folders → audio files (folders walked as in the egui version). */
export function addPaths(paths: string[], root: string | null = null): Promise<FileEntry[]> {
  return invoke<FileEntry[]>("add_paths", { paths, root });
}

/** Reads tags and length in the background; each file reports on its own. */
export function readFileInfo(
  items: [number, string][],
  onInfo: (i: FileInfo) => void,
): Promise<void> {
  const on = new Channel<FileInfo>();
  on.onmessage = onInfo;
  return invoke<void>("read_file_info", { items, on });
}

export function revealFolder(path: string): Promise<void> {
  return invoke<void>("reveal_folder", { path });
}

export function inputExtensions(): Promise<string[]> {
  return invoke<string[]>("input_extensions");
}
