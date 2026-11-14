export interface FoundFile {
  path: string;
  name: string;
}

export interface FolderGroup {
  /** The subfolder below the source folder ("" for the source folder itself). */
  dir: string;
  files: FoundFile[];
}

/**
 * The found files under one heading per subfolder, in the order Rust sorted them (by path).
 * Folders keep the separators of their path.
 */
export function groupBySubfolder(root: string, paths: string[]): FolderGroup[] {
  const base = root.replace(/[\\/]+$/, "");
  const groups: FolderGroup[] = [];
  for (const path of paths) {
    const rel = path.startsWith(base) ? path.slice(base.length).replace(/^[\\/]+/, "") : path;
    const cut = Math.max(rel.lastIndexOf("\\"), rel.lastIndexOf("/"));
    const dir = cut < 0 ? "" : rel.slice(0, cut);
    const name = rel.slice(cut + 1);
    const last = groups[groups.length - 1];
    if (last && last.dir === dir) last.files.push({ path, name });
    else groups.push({ dir, files: [{ path, name }] });
  }
  return groups;
}
