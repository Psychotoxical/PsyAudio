import { create } from "zustand";

export type Page = "convert" | "batch" | "cd" | "settings";

interface Toast {
  id: number;
  text: string;
}

interface UiState {
  page: Page;
  toast: Toast | null;
  setPage: (page: Page) => void;
  /** A short message at the bottom of the window (4 s), as in the egui version. */
  notify: (text: string) => void;
}

let next = 1;

export const useUi = create<UiState>((set) => ({
  page: "convert",
  toast: null,
  setPage: (page) => set({ page }),
  notify: (text) => {
    next += 1;
    set({ toast: { id: next, text } });
  },
}));
