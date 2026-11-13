import { create } from "zustand";
import type { Settings } from "../ipc/bindings/Settings";
import { saveSettings } from "../ipc/commands";

/** Settings are written at most twice a second, as in the egui version. */
export const SAVE_DELAY_MS = 500;

interface SettingsState {
  settings: Settings | null;
  /** The first failed save, shown once. */
  error: string | null;
  load: (settings: Settings) => void;
  update: (change: (s: Settings) => void) => void;
}

let pending: ReturnType<typeof setTimeout> | undefined;
let reported = false;

function flush() {
  pending = undefined;
  const { settings } = useSettings.getState();
  if (!settings) return;
  saveSettings(settings).catch((e: unknown) => {
    if (!reported) {
      reported = true;
      useSettings.setState({ error: String(e) });
    }
  });
}

export const useSettings = create<SettingsState>((set, get) => ({
  settings: null,
  error: null,
  load: (settings) => set({ settings }),
  update: (change) => {
    const current = get().settings;
    if (!current) return;
    const next = structuredClone(current);
    change(next);
    set({ settings: next });
    if (pending === undefined) pending = setTimeout(flush, SAVE_DELAY_MS);
  },
}));
