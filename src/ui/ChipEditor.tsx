import { Folder, Plus } from "lucide-react";
import { useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Hint } from "./controls";

/** Token that starts a new folder (naming::FOLDER). */
export const FOLDER = "/";

export interface Preset {
  name: string;
  scheme: string[];
}

interface Props {
  label: string;
  value: string[];
  /** Data chips on offer ("artist", "title" …). */
  data: string[];
  separators: string[];
  /** "Neuer Ordner" is on offer. */
  folders: boolean;
  /** Chip → German label (translated here). */
  labels: Record<string, string>;
  /** A "Vorlage" list above the chips; choosing one replaces them. */
  presets?: Preset[];
  onChange: (value: string[]) => void;
}

function move(list: string[], from: number, to: number): string[] {
  const next = [...list];
  const [item] = next.splice(from, 1);
  if (item !== undefined) next.splice(to, 0, item);
  return next;
}

/**
 * Chips that build a name, as in PsyTags: drag sorts, a click removes, "+ Baustein" adds data and
 * separator chips. Alt+arrow keys move the focused chip, Delete removes it.
 */
export function ChipEditor(p: Props) {
  const { t } = useTranslation();
  const presetId = useId();
  const [picking, setPicking] = useState(false);
  const [drag, setDrag] = useState<{ from: number; over: number | null } | null>(null);
  // a drag that ends on the dragged chip fires a click; that click must not remove it
  const dragged = useRef(false);
  const label = (tok: string) => t(p.labels[tok] ?? tok);
  const same = (a: string[], b: string[]) => a.length === b.length && a.every((x, i) => x === b[i]);
  const preset = p.presets?.find((x) => same(x.scheme, p.value));

  const endDrag = () => setDrag(null);

  return (
    <div className="field">
      {p.presets && (
        <div className="field" style={{ marginBottom: 14 }}>
          <label className="lbl" htmlFor={presetId}>
            {t("Vorlage")}
          </label>
          <select
            id={presetId}
            className="dd"
            value={preset?.name ?? ""}
            onChange={(e) => {
              const chosen = p.presets?.find((x) => x.name === e.target.value);
              if (chosen) p.onChange([...chosen.scheme]);
            }}
          >
            <option value="" disabled>
              {t("Eigene Bausteine")}
            </option>
            {p.presets.map((x) => (
              <option key={x.name} value={x.name}>
                {t(x.name)}
              </option>
            ))}
          </select>
        </div>
      )}
      <p className="lbl">{p.label}</p>
      <div className="chips chip-edit">
        {p.value.map((tok, i) => {
          const over = drag !== null && drag.over === i && drag.from !== i;
          const cls = [
            "chip-btn",
            tok === FOLDER && "dir",
            p.separators.includes(tok) && "sep",
            drag?.from === i && "dragging",
            over && (drag.from < i ? "drop-after" : "drop-before"),
          ].filter(Boolean);
          return (
            <button
              // the same chip may appear twice; position is its identity here
              // biome-ignore lint/suspicious/noArrayIndexKey: chips have no id of their own
              key={`${i}-${tok}`}
              type="button"
              className={cls.join(" ")}
              aria-label={t("{0} entfernen", { 0: label(tok) })}
              title={t("{0} entfernen", { 0: label(tok) })}
              onPointerDown={(e) => {
                if (e.button !== 0) return;
                // touch captures the pointer on its own; moves must reach the chip below
                if (e.currentTarget.hasPointerCapture?.(e.pointerId)) {
                  e.currentTarget.releasePointerCapture(e.pointerId);
                }
                dragged.current = false;
                setDrag({ from: i, over: null });
                // a release outside the chips ends the drag too
                window.addEventListener("pointerup", endDrag, { once: true });
              }}
              onPointerMove={() => {
                if (drag && drag.over !== i && (drag.from !== i || drag.over !== null)) {
                  dragged.current = true;
                  setDrag({ ...drag, over: i });
                }
              }}
              onPointerUp={() => {
                if (drag && dragged.current && drag.over !== null && drag.over !== drag.from) {
                  p.onChange(move(p.value, drag.from, drag.over));
                }
                endDrag();
              }}
              onClick={() => {
                if (dragged.current) {
                  dragged.current = false;
                  return;
                }
                p.onChange(p.value.filter((_, j) => j !== i));
              }}
              onKeyDown={(e) => {
                if (e.altKey && e.key === "ArrowLeft" && i > 0) {
                  e.preventDefault();
                  p.onChange(move(p.value, i, i - 1));
                } else if (e.altKey && e.key === "ArrowRight" && i < p.value.length - 1) {
                  e.preventDefault();
                  p.onChange(move(p.value, i, i + 1));
                } else if (e.key === "Delete" || e.key === "Backspace") {
                  e.preventDefault();
                  p.onChange(p.value.filter((_, j) => j !== i));
                }
              }}
            >
              {tok === FOLDER && <Folder size={14} />}
              {label(tok)}
            </button>
          );
        })}
        <button
          type="button"
          className="chip-btn add"
          aria-expanded={picking}
          onClick={() => setPicking(!picking)}
        >
          <Plus size={14} />
          {t("Baustein")}
        </button>
      </div>
      {picking && (
        <div className="chip-pick">
          <p className="lbl">{t("Daten")}</p>
          <div className="chips">
            {p.data.map((tok) => (
              <button
                key={tok}
                type="button"
                className="chip-btn"
                onClick={() => p.onChange([...p.value, tok])}
              >
                {label(tok)}
              </button>
            ))}
          </div>
          <p className="lbl">{t("Trenner")}</p>
          <div className="chips">
            {p.separators.map((tok) => (
              <button
                key={tok}
                type="button"
                className="chip-btn sep"
                title={tok === " " ? t("Abstand") : undefined}
                onClick={() => p.onChange([...p.value, tok])}
              >
                {label(tok)}
              </button>
            ))}
            {p.folders && (
              <button
                type="button"
                className="chip-btn dir"
                onClick={() => p.onChange([...p.value, FOLDER])}
              >
                <Folder size={14} />
                {label(FOLDER)}
              </button>
            )}
          </div>
        </div>
      )}
      <Hint>
        {p.folders
          ? t(
              "Ziehen sortiert, Klick entfernt. „Neuer Ordner“ trennt den Ordnernamen vom Dateinamen.",
            )
          : t("Ziehen sortiert, Klick entfernt.")}
      </Hint>
    </div>
  );
}
