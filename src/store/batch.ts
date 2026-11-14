import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

interface BatchState {
  /** The source folder as chosen (not saved, as in the egui version). */
  folder: string;
  /** The folder the found files belong to (the one searched last). */
  root: string;
  found: string[];
  scanning: boolean;
  /** A search ran at least once (an empty result is then a result). */
  searched: boolean;
}

export const useBatch = create<BatchState>(() => ({
  folder: "",
  root: "",
  found: [],
  scanning: false,
  searched: false,
}));

/** File types the batch page offers (as in the egui version). */
export const BATCH_TYPES = [
  "mp3",
  "flac",
  "wav",
  "m4a",
  "aac",
  "ogg",
  "opus",
  "wma",
  "aiff",
  "ape",
  "wv",
  "mpc",
  "dsf",
  "dff",
  "spx",
  "caf",
];

/** Walks the source folder in the background; one answer at the end. */
export async function searchBatch(subfolders: boolean, extensions: string[]): Promise<number> {
  const folder = useBatch.getState().folder.trim();
  useBatch.setState({ scanning: true });
  try {
    const found = await invoke<string[]>("scan_folder", { folder, subfolders, extensions });
    useBatch.setState({ root: folder, found, searched: true });
    return found.length;
  } finally {
    useBatch.setState({ scanning: false });
  }
}

/** The first dropped path that is a folder, if any. */
export function firstFolder(paths: string[]): Promise<string | null> {
  return invoke<string | null>("first_folder", { paths });
}
