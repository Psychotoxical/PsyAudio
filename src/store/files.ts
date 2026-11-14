import { create } from "zustand";
import type { Info } from "../ipc/bindings/Info";

/** One file in the converter list; tags and info arrive later from Rust. */
export interface FileRow {
  id: number;
  path: string;
  /** The folder the file was found in when a folder was added (for "keep path"). */
  root: string | null;
  size: number;
  tags: Record<string, string> | undefined;
  info: Info | undefined;
  cover: boolean;
}

interface FilesState {
  files: FileRow[];
  /** Selected row indices, as in the egui list. */
  selected: Set<number>;
  /** Adds files not in the list yet; returns the ones added. */
  add: (rows: FileRow[]) => FileRow[];
  /** Tags and info of several files in one update (they arrive one by one from Rust). */
  setInfos: (
    infos: { id: number; tags: Record<string, string>; info: Info; cover: boolean }[],
  ) => void;
  setSelected: (selected: Set<number>) => void;
  removeSelected: () => void;
  clear: () => void;
}

export const useFiles = create<FilesState>((set, get) => ({
  files: [],
  selected: new Set(),
  add: (rows) => {
    const known = new Set(get().files.map((f) => f.path));
    const fresh = rows.filter((r) => {
      if (known.has(r.path)) return false;
      known.add(r.path);
      return true;
    });
    if (fresh.length) set({ files: [...get().files, ...fresh] });
    return fresh;
  },
  setInfos: (infos) => {
    if (infos.length === 0) return;
    const byId = new Map(infos.map((i) => [i.id, i]));
    set({
      files: get().files.map((f) => {
        const i = byId.get(f.id);
        return i ? { ...f, tags: i.tags, info: i.info, cover: i.cover } : f;
      }),
    });
  },
  setSelected: (selected) => set({ selected }),
  removeSelected: () => {
    const { files, selected } = get();
    set({ files: files.filter((_, i) => !selected.has(i)), selected: new Set() });
  },
  clear: () => set({ files: [], selected: new Set() }),
}));
