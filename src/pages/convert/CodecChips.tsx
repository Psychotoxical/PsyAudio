import { useTranslation } from "react-i18next";
import type { Codec } from "../../ipc/bindings/Codec";
import { codecInfo, useEncode } from "../../store/encode";
import { useSettings } from "../../store/settings";
import { Disclosure, Hint } from "../../ui/controls";

/** The settings field a chip row chooses: the converter's format or the CD ripper's. */
export type CodecField = "codec" | "rip_codec";

function ChipRow({
  list,
  field,
  label,
}: {
  list: Codec[];
  field: CodecField;
  /** A text instead of the format's short name. */
  label?: string;
}) {
  const options = useEncode((s) => s.options);
  const problems = useEncode((s) => s.problems);
  const current = useSettings((s) => s.settings?.[field]);
  const update = useSettings((s) => s.update);
  return (
    <div className="chips">
      {list.map((c) => {
        const info = codecInfo(options, c);
        const problem = problems[c];
        const cls = ["chip-btn", c === current && "on", problem && "missing"].filter(Boolean);
        return (
          <button
            key={c}
            type="button"
            className={cls.join(" ")}
            aria-pressed={c === current}
            title={problem ?? info?.name}
            onClick={() =>
              update((s) => {
                s[field] = c;
              })
            }
          >
            {label ?? info?.short ?? c}
          </button>
        );
      })}
    </div>
  );
}

/** Format chips: lossless and compressed, tools folded away (converter only). */
export function CodecChips({ field }: { field: CodecField }) {
  const { t } = useTranslation();
  const options = useEncode((s) => s.options);
  const problems = useEncode((s) => s.problems);
  const current = useSettings((s) => s.settings?.[field]);
  if (!options) return null;
  const shown = [...options.lossless, ...options.compressed];
  const anyMissing = shown.some((c) => problems[c]);
  return (
    <>
      <div className="field">
        <p className="lbl">{t("Lossless")}</p>
        <ChipRow list={options.lossless} field={field} />
      </div>
      <div className="field">
        <p className="lbl">{t("Compressed")}</p>
        <ChipRow list={options.compressed} field={field} />
        {anyMissing && <Hint>{t("Dashed: the program for it is still missing (Settings).")}</Hint>}
      </div>
      {field === "rip_codec" && (
        <ChipRow list={["Multi"]} field={field} label={t("Several formats")} />
      )}
      {field === "codec" && options.tools.length > 0 && (
        <Disclosure
          title={t("Tools")}
          summary={t("Check, split, tags, ReplayGain")}
          defaultOpen={options.tools.includes(current as Codec)}
        >
          <ChipRow list={options.tools} field={field} />
        </Disclosure>
      )}
    </>
  );
}
