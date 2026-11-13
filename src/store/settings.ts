import { create } from "zustand";
import { i18n } from "../i18n";
import type { Settings } from "../ipc/bindings/Settings";
import { saveSettings } from "../ipc/commands";
import { useUi } from "./ui";

/** Settings are written at most twice a second, as in the egui version. */
export const SAVE_DELAY_MS = 500;

interface SettingsState {
  settings: Settings | null;
  /** The first failed save (also shown as a message, once). */
  error: string | null;
  load: (settings: Settings) => void;
  update: (change: (s: Settings) => void) => void;
}

let pending: ReturnType<typeof setTimeout> | undefined;
let reported = false;

async function flush(): Promise<void> {
  if (pending !== undefined) clearTimeout(pending);
  pending = undefined;
  const { settings } = useSettings.getState();
  if (!settings) return;
  try {
    await saveSettings(settings);
  } catch (e) {
    // once per session: a read-only folder or a full disk would otherwise repeat it on every change
    if (!reported) {
      reported = true;
      useSettings.setState({ error: String(e) });
      useUi
        .getState()
        .notify(i18n.t("Einstellungen konnten nicht gespeichert werden: {e}", { e: String(e) }));
    }
  }
}

/** Writes a change still waiting for its save at once (the window is about to close). */
export function flushSettings(): Promise<void> {
  return pending === undefined ? Promise.resolve() : flush();
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
    if (pending === undefined) pending = setTimeout(() => void flush(), SAVE_DELAY_MS);
  },
}));
