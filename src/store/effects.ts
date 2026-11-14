import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import type { Effect } from "../ipc/bindings/Effect";

/** One effect in the picker (texts in the current language, from Rust). */
export interface EffectEntry {
  category: string;
  label: string;
  description: string;
  effect: Effect;
}

export interface EffectCatalogue {
  entries: EffectEntry[];
  /** Kinds of "Lautstärke normalisieren", German. */
  normalize_modes: string[];
  /** dB to add for LUFS. */
  ebu_offset: number;
  /** Tag fields for "Tag-Bearbeitung": [key, German label]. */
  tag_fields: [string, string][];
}

export interface EffectLine {
  name: string;
  summary: string;
}

export const useEffects = create<{ catalogue: EffectCatalogue | null }>(() => ({
  catalogue: null,
}));

/** Loads the catalogue (again after a language change: labels come from Rust). */
export async function loadEffectCatalogue(): Promise<void> {
  const catalogue = await invoke<EffectCatalogue>("effect_catalogue");
  useEffects.setState({ catalogue });
}

export function effectLines(effects: Effect[]): Promise<EffectLine[]> {
  return invoke<EffectLine[]>("effect_lines", { effects });
}

/** The variant name of an effect ("Fade", "Reverse"). */
export function effectKind(e: Effect): string {
  return typeof e === "string" ? e : (Object.keys(e)[0] ?? "");
}
