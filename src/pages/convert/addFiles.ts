import { addPaths, readFileInfo } from "../../ipc/files";
import { useFiles } from "../../store/files";

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
    (i) => useFiles.getState().setInfo(i.id, i.tags, i.info, i.cover),
  );
}
