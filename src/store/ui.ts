import { create } from "zustand";

export type Page = "convert" | "batch" | "cd" | "settings";

interface Toast {
  id: number;
  text: string;
}

interface UiState {
  page: Page;
  toast: Toast | null;
  /** Files are being dragged over the window. */
  dragging: boolean;
  setPage: (page: Page) => void;
  setDragging: (dragging: boolean) => void;
  /** A short message at the bottom of the window (4 s), as in the egui version. */
  notify: (text: string) => void;
}

let next = 1;

export const useUi = create<UiState>((set) => ({
  page: "convert",
  toast: null,
  dragging: false,
  setPage: (page) => set({ page }),
  setDragging: (dragging) => set({ dragging }),
  notify: (text) => {
    next += 1;
    set({ toast: { id: next, text } });
  },
}));
