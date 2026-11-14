import { Channel, invoke } from "@tauri-apps/api/core";
import type { RunEvent, RunInfo } from "../store/run";
import type { Codec } from "./bindings/Codec";
import type { Settings } from "./bindings/Settings";

/** [path, root folder] of each file to convert, in list order. */
export type Items = [string, string | null][];

/** Targets that exist already (only with "Wenn vorhanden: Fragen"); rejects with the reason a run can't start. */
export function checkExisting(
  settings: Settings,
  items: Items,
  codec: Codec,
  keepStructure: boolean,
): Promise<string[]> {
  return invoke<string[]>("check_existing", { settings, items, codec, keepStructure });
}

export function startConversion(
  settings: Settings,
  items: Items,
  codec: Codec,
  keepStructure: boolean,
  overwriteList: string[],
  onEvent: (e: RunEvent) => void,
): Promise<RunInfo> {
  const on = new Channel<RunEvent>();
  on.onmessage = onEvent;
  return invoke<RunInfo>("start_conversion", {
    settings,
    items,
    codec,
    keepStructure,
    overwriteList,
    on,
  });
}

export function cancelConversion(): Promise<void> {
  return invoke<void>("cancel_conversion");
}

export function clearRunTitle(): Promise<void> {
  return invoke<void>("clear_run_title");
}
