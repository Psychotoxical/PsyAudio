export interface Mods {
  ctrl: boolean;
  shift: boolean;
}

/** A click on a row, as in the egui list: Ctrl toggles, Shift selects from the smallest selected. */
export function select(current: Set<number>, index: number, mods: Mods): Set<number> {
  if (mods.ctrl) {
    const next = new Set(current);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    return next;
  }
  if (mods.shift) {
    if (current.size === 0) return current;
    const first = Math.min(...current);
    const [lo, hi] = first < index ? [first, index] : [index, first];
    return new Set(Array.from({ length: hi - lo + 1 }, (_, i) => lo + i));
  }
  return new Set([index]);
}

/** Two or more selected rows convert only those; otherwise the whole list (one click = preview). */
export function itemsToConvert<T>(files: T[], selected: Set<number>): T[] {
  if (selected.size < 2) return files;
  return [...selected]
    .sort((a, b) => a - b)
    .map((i) => files[i])
    .filter((f): f is T => f !== undefined);
}
