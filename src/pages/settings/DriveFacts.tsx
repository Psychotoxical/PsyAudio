import { useTranslation } from "react-i18next";
import type { DriveProfile } from "../../ipc/bindings/DriveProfile";
import { signed } from "../cd/measure";

type T = (key: string, values?: Record<string, string>) => string;

/** YYYY-MM-DD as the language writes dates. */
export function dateText(iso: string, lang: string): string {
  const [y, m, d] = iso.split("-");
  return lang === "de" && y && m && d ? `${d}.${m}.${y}` : iso;
}

/** How and when a drive's offset was found: "mit CTDB am 14.11.2026". */
export function methodText(p: DriveProfile, t: T, lang: string): string {
  const date = dateText(p.measured, lang);
  if (p.method === "Ctdb") return t("mit CTDB am {0}", { 0: date });
  if (p.method === "Manual") return t("von Hand am {0}", { 0: date });
  return t("übernommen am {0}", { 0: date });
}

/** Offset, how it was found, and the drive's buffer, as a value list. */
export function DriveFacts({ profile }: { profile: DriveProfile }) {
  const { t, i18n } = useTranslation();
  const how = methodText(profile, t, i18n.language);
  return (
    <dl className="kv">
      <div>
        <dt>{t("Versatz")}</dt>
        <dd>
          <strong>{`${signed(profile.offset)} Samples`}</strong>
        </dd>
      </div>
      <div>
        <dt>{t("Gemessen")}</dt>
        <dd>{how}</dd>
      </div>
      <div>
        <dt>{t("Puffer")}</dt>
        <dd>{profile.buffer_kb ? `${profile.buffer_kb} KB` : t("unbekannt")}</dd>
      </div>
    </dl>
  );
}
