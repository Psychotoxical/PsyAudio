import type { FileRow } from "../../store/files";

export type Row =
  | {
      kind: "group";
      key: string;
      title: string;
      year: string;
      count: number;
      /** Every file of the run has a title tag. */
      tagged: boolean;
      /** Every file of the run has a cover. */
      cover: boolean;
    }
  | { kind: "file"; index: number; file: FileRow };

function parentParts(path: string): { dir: string; name: string } {
  const parts = path.split(/[\\/]/);
  parts.pop();
  return { dir: parts.join("/"), name: parts[parts.length - 1] ?? "" };
}

function album(f: FileRow): { key: string; title: string } {
  const t = f.tags ?? {};
  const artist = t["album artist"] || t.artist || "";
  const name = t.album || "";
  if (name)
    return { key: `a\u0000${artist}\u0000${name}`, title: artist ? `${artist} – ${name}` : name };
  const { dir, name: folder } = parentParts(f.path);
  return { key: `d\u0000${dir}`, title: folder };
}

/**
 * The list as shown: a group row wherever a file belongs to another album than the one before.
 * Display only; the order of the files never changes. Album = album artist (else artist) plus
 * album from the tags; without them the folder.
 */
export function groupRuns(files: FileRow[]): Row[] {
  const rows: Row[] = [];
  let group: Extract<Row, { kind: "group" }> | null = null;
  files.forEach((file, index) => {
    const { key, title } = album(file);
    if (!group || group.key !== key) {
      group = {
        kind: "group",
        key,
        title,
        year: file.tags?.year ?? "",
        count: 0,
        tagged: true,
        cover: true,
      };
      rows.push(group);
    }
    group.count += 1;
    group.tagged &&= Boolean(file.tags?.title);
    group.cover &&= file.cover;
    rows.push({ kind: "file", index, file });
  });
  return rows;
}
