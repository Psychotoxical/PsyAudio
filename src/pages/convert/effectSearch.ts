import type { EffectEntry } from "../../store/effects";

/**
 * Effects whose name or description contains the query (case ignored, umlauts as typed),
 * within one category or all.
 */
export function filterEffects(
  entries: EffectEntry[],
  query: string,
  category: string | null,
): EffectEntry[] {
  const q = query.trim().toLowerCase();
  return entries.filter(
    (e) =>
      (category === null || e.category === category) &&
      (q === "" || e.label.toLowerCase().includes(q) || e.description.toLowerCase().includes(q)),
  );
}

/** The categories in catalogue order. */
export function categories(entries: EffectEntry[]): string[] {
  return [...new Set(entries.map((e) => e.category))];
}
