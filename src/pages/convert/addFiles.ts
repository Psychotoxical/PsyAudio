import { addPaths, type FileInfo, readFileInfo } from "../../ipc/files";
import { useFiles } from "../../store/files";

let queued: FileInfo[] = [];
let scheduled = false;

/** Infos arrive one per file: they are applied once per frame, so a long list renders once per frame. */
export function queueInfo(i: FileInfo) {
  queued.push(i);
  if (scheduled) return;
  scheduled = true;
  requestAnimationFrame(() => {
    scheduled = false;
    const batch = queued;
    queued = [];
    useFiles.getState().setInfos(batch);
  });
}

/** Adds files and folders to the converter list and reads their tags in the background. */
export async function addFiles(paths: string[], root: string | null = null): Promise<void> {
  if (paths.length === 0) return;
  const entries = await addPaths(paths, root);
  const added = useFiles
    .getState()
    .add(entries.map((e) => ({ ...e, tags: undefined, info: undefined, cover: false })));
  if (added.length === 0) return;
  await readFileInfo(
    added.map((f) => [f.id, f.path]),
    queueInfo,
  );
}
