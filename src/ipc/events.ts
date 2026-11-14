import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Where files came from: the command line, a second start, or a drop on the window. */
export type FilesSource = "start" | "instance" | "drop";

export interface FilesAdded {
  paths: string[];
  source: FilesSource;
}

export interface DragState {
  active: boolean;
}

export function onFilesAdded(handler: (e: FilesAdded) => void): Promise<UnlistenFn> {
  return listen<FilesAdded>("files-added", (ev) => handler(ev.payload));
}

export function onDragState(handler: (e: DragState) => void): Promise<UnlistenFn> {
  return listen<DragState>("drag-state", (ev) => handler(ev.payload));
}

/** Shows the window and returns the files PsyAudio was started with (once). */
export function frontendReady(): Promise<string[]> {
  return invoke<string[]>("frontend_ready");
}

/** Adds or removes "Mit PsyAudio konvertieren" in the file manager; returns the message. */
export function fileManagerMenu(add: boolean): Promise<string> {
  return invoke<string>("file_manager_menu", { add });
}
