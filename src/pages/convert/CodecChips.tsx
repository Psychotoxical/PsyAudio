import { useTranslation } from "react-i18next";
import type { Codec } from "../../ipc/bindings/Codec";
import { codecInfo, useEncode } from "../../store/encode";
import { useSettings } from "../../store/settings";
import { Disclosure, Hint } from "../../ui/controls";

/** The settings field a chip row chooses: the converter's format or the CD ripper's. */
export type CodecField = "codec" | "rip_codec";

function ChipRow({ list, field }: { list: Codec[]; field: CodecField }) {
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
            {info?.short ?? c}
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
        <p className="lbl">{t("Verlustfrei")}</p>
        <ChipRow list={options.lossless} field={field} />
      </div>
      <div className="field">
        <p className="lbl">{t("Komprimiert")}</p>
        <ChipRow list={options.compressed} field={field} />
        {anyMissing && (
          <Hint>{t("Gestrichelt: Das Programm dafür fehlt noch (Einstellungen).")}</Hint>
        )}
      </div>
      {field === "codec" && options.tools.length > 0 && (
        <Disclosure
          title={t("Werkzeuge")}
          summary={t("Prüfen, Aufteilen, Tags, ReplayGain")}
          defaultOpen={options.tools.includes(current as Codec)}
        >
          <ChipRow list={options.tools} field={field} />
        </Disclosure>
      )}
    </>
  );
}
