import { Search, X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Effect } from "../../ipc/bindings/Effect";
import type { EffectEntry } from "../../store/effects";
import { categories, filterEffects } from "./effectSearch";

/**
 * "Effekt hinzufügen": search field (focused), category chips and a list with one line of
 * explanation per effect. Arrow keys move, Enter adds, Escape closes.
 */
export function EffectPicker({
  entries,
  onAdd,
  onClose,
}: {
  entries: EffectEntry[];
  onAdd: (effect: Effect) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<string | null>(null);
  const [active, setActive] = useState(0);
  const list = filterEffects(entries, query, category);
  const listRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  // biome-ignore lint/correctness/useExhaustiveDependencies: a new search starts at the top
  useEffect(() => setActive(0), [query, category]);
  useEffect(() => {
    const row = listRef.current?.children[active] as HTMLElement | undefined;
    row?.scrollIntoView?.({ block: "nearest" });
  }, [active]);

  const add = (i: number) => {
    const hit = list[i];
    if (hit) onAdd(structuredClone(hit.effect));
  };

  return (
    <div
      className="overlay"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="picker"
        role="dialog"
        aria-modal="true"
        aria-label={t("Effekt hinzufügen")}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            onClose();
          }
        }}
      >
        <header className="picker-head">
          <h2>{t("Effekt hinzufügen")}</h2>
          <button
            type="button"
            className="icon-btn"
            aria-label={t("Schließen")}
            title={t("Schließen")}
            onClick={onClose}
          >
            <X size={16} />
          </button>
        </header>
        <div className="picker-search">
          <Search size={16} />
          <input
            type="search"
            aria-label={t("Effekt suchen")}
            placeholder={t("Effekt suchen")}
            // the dialog exists to search: typing starts right away
            // biome-ignore lint/a11y/noAutofocus: focus moves into the dialog that just opened
            autoFocus
            value={query}
            aria-controls={listId}
            aria-activedescendant={list[active] ? `${listId}-${active}` : undefined}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setActive((a) => Math.min(a + 1, list.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setActive((a) => Math.max(a - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                add(active);
              }
            }}
          />
        </div>
        <div className="chips picker-cats">
          <button
            type="button"
            className={category === null ? "chip-btn on" : "chip-btn"}
            aria-pressed={category === null}
            onClick={() => setCategory(null)}
          >
            {t("Alle")}
          </button>
          {categories(entries).map((c) => (
            <button
              key={c}
              type="button"
              className={category === c ? "chip-btn on" : "chip-btn"}
              aria-pressed={category === c}
              onClick={() => setCategory(category === c ? null : c)}
            >
              {c}
            </button>
          ))}
        </div>
        {list.length === 0 ? (
          <p className="picker-empty">{t("Kein Effekt passt zur Suche.")}</p>
        ) : (
          <div
            className="picker-list"
            role="listbox"
            id={listId}
            aria-label={t("Effekte")}
            ref={listRef}
          >
            {list.map((e, i) => (
              // keyboard: the search field moves the selection and adds with Enter
              // biome-ignore lint/a11y/useKeyWithClickEvents: keys go through the search field
              <div
                key={e.label}
                id={`${listId}-${i}`}
                role="option"
                tabIndex={-1}
                aria-selected={i === active}
                className={i === active ? "on" : undefined}
                onPointerMove={() => setActive(i)}
                onClick={() => add(i)}
              >
                <span className="picker-name">
                  <strong>{e.label}</strong>
                  {category === null && <small>{e.category}</small>}
                </span>
                <span className="picker-desc">{e.description}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
