import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** The close button was clicked while something runs. */
export function onCloseRequested(handler: () => void): Promise<UnlistenFn> {
  return listen("close-requested", () => handler());
}

/** The close button was clicked with nothing running: save what waits, then confirm. */
export function onCloseNow(handler: () => void): Promise<UnlistenFn> {
  return listen("close-now", () => handler());
}

/** `true`: stop conversion and rip, then close; `false`: keep working. */
export function confirmClose(cancelWork: boolean): Promise<void> {
  return invoke<void>("confirm_close", { cancelWork });
}
