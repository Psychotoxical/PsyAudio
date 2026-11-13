import { useEffect, useState } from "react";
import { planRequirement } from "../../ipc/encode";
import { codecInfo, useEncode } from "../../store/encode";
import { useSettings } from "../../store/settings";
import { Hint } from "../../ui/controls";
import { CodecChips, type CodecField } from "./CodecChips";
import { CodecSettings } from "./CodecSettings";
import { ProfileRow } from "./ProfileRow";

/** What stops a run with the chosen format and its settings (a missing program or encoder). */
function useRequirement(field: CodecField): string | null {
  const settings = useSettings((s) => s.settings);
  const [text, setText] = useState<string | null>(null);
  useEffect(() => {
    if (!settings) return;
    let live = true;
    planRequirement(settings, settings[field])
      .then((r) => live && setText(r))
      .catch(() => live && setText(null));
    return () => {
      live = false;
    };
  }, [settings, field]);
  return text;
}

/** The format area of the side column: chips, the format's settings, profiles. */
export function FormatPanel({ field }: { field: CodecField }) {
  const codec = useSettings((s) => s.settings?.[field]);
  const options = useEncode((s) => s.options);
  const requirement = useRequirement(field);
  if (!codec) return null;
  const info = codecInfo(options, codec);
  return (
    <>
      <CodecChips field={field} />
      <section className="group" aria-label={info?.name ?? codec}>
        <h3 className="group-title">{info?.name ?? codec}</h3>
        {requirement && <Hint warn>{requirement}</Hint>}
        <CodecSettings codec={codec} />
      </section>
      {field === "codec" && <ProfileRow />}
    </>
  );
}
