import { ArrowDown, ArrowUp, ChevronRight, Plus, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Effect } from "../../ipc/bindings/Effect";
import { type EffectLine, effectKind, effectLines, useEffects } from "../../store/effects";
import { useSettings } from "../../store/settings";
import { Hint } from "../../ui/controls";
import { EffectEditor } from "./EffectEditor";
import { EffectPicker } from "./EffectPicker";

/** The converter's effects in run order: add from the picker, open, move, remove. */
export function EffectsPanel() {
  const { t } = useTranslation();
  const effects = useSettings((s) => s.settings?.effects);
  const update = useSettings((s) => s.update);
  const catalogue = useEffects((s) => s.catalogue);
  const [picking, setPicking] = useState(false);
  const [openAt, setOpenAt] = useState<number | null>(null);
  const [lines, setLines] = useState<EffectLine[]>([]);

  useEffect(() => {
    if (!effects) return;
    let live = true;
    effectLines(effects)
      .then((l) => live && setLines(l))
      .catch(() => live && setLines([]));
    return () => {
      live = false;
    };
  }, [effects]);

  if (!effects || !catalogue) return null;

  const change = (fn: (list: Effect[]) => void) =>
    update((s) => {
      fn(s.effects);
    });
  const swap = (a: number, b: number) => {
    change((list) => {
      const x = list[a];
      const y = list[b];
      if (x === undefined || y === undefined) return;
      list[a] = y;
      list[b] = x;
    });
    if (openAt === a) setOpenAt(b);
    else if (openAt === b) setOpenAt(a);
  };

  return (
    <>
      <button type="button" className="btn add-effect" onClick={() => setPicking(true)}>
        <Plus size={16} />
        {t("Add effect")}
      </button>
      {effects.length === 0 && <Hint>{t("None – the audio is converted unchanged.")}</Hint>}
      <ol className="effects">
        {effects.map((e, i) => {
          const entry = catalogue.entries.find((x) => effectKind(x.effect) === effectKind(e));
          const line = lines[i];
          const isOpen = openAt === i;
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: the same effect may be in the list twice
            <li key={`${i}-${effectKind(e)}`} className={isOpen ? "effect open" : "effect"}>
              <div className="effect-head">
                <button
                  type="button"
                  className="effect-toggle"
                  aria-expanded={isOpen}
                  onClick={() => setOpenAt(isOpen ? null : i)}
                >
                  <span className="effect-n">{i + 1}</span>
                  <span className="effect-text">
                    <strong>{line?.name ?? entry?.label ?? effectKind(e)}</strong>
                    {line?.summary && <small>{line.summary}</small>}
                  </span>
                  <ChevronRight size={16} className="effect-chev" />
                </button>
                <div className="effect-actions">
                  <button
                    type="button"
                    className="icon-btn small"
                    aria-label={t("Move up")}
                    title={t("Move up")}
                    disabled={i === 0}
                    onClick={() => swap(i - 1, i)}
                  >
                    <ArrowUp size={14} />
                  </button>
                  <button
                    type="button"
                    className="icon-btn small"
                    aria-label={t("Move down")}
                    title={t("Move down")}
                    disabled={i === effects.length - 1}
                    onClick={() => swap(i, i + 1)}
                  >
                    <ArrowDown size={14} />
                  </button>
                  <button
                    type="button"
                    className="icon-btn small"
                    aria-label={t("Remove effect")}
                    title={t("Remove effect")}
                    onClick={() => {
                      change((list) => {
                        list.splice(i, 1);
                      });
                      setOpenAt(null);
                    }}
                  >
                    <X size={14} />
                  </button>
                </div>
              </div>
              {isOpen && (
                <div className="effect-body">
                  {entry && <Hint>{entry.description}</Hint>}
                  <EffectEditor
                    effect={e}
                    onChange={(next) =>
                      change((list) => {
                        list[i] = next;
                      })
                    }
                  />
                </div>
              )}
            </li>
          );
        })}
      </ol>
      {picking && (
        <EffectPicker
          entries={catalogue.entries}
          onClose={() => setPicking(false)}
          onAdd={(effect) => {
            change((list) => {
              list.push(effect);
            });
            setOpenAt(effects.length);
            setPicking(false);
          }}
        />
      )}
    </>
  );
}
