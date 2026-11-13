import {
  CircleAlert,
  CircleCheck,
  Disc3,
  Folders,
  Music,
  PanelLeftClose,
  PanelLeftOpen,
  SlidersHorizontal,
} from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import wordmark1x from "../assets/wordmark-1x.png";
import wordmark2x from "../assets/wordmark-2x.png";
import { useSettings } from "../store/settings";
import { type Page, useUi } from "../store/ui";

interface Job {
  label: string;
  /** 0..1 */
  progress: number;
}

interface Props {
  /** Count badges per page (files in the converter list). */
  counts?: Partial<Record<Page, number>>;
  /** Whether FFmpeg was found; undefined while not yet known. */
  ffmpeg?: boolean;
  /** One line per running conversion or rip. */
  jobs?: Job[];
}

/** The navigation on the left, as in Psysonic: wordmark, tools, system, status. */
export function Sidebar({ counts = {}, ffmpeg, jobs = [] }: Props) {
  const { t } = useTranslation();
  const page = useUi((s) => s.page);
  const setPage = useUi((s) => s.setPage);
  const collapsed = useSettings((s) => s.settings?.ui.sidebar_collapsed ?? false);
  const update = useSettings((s) => s.update);

  const item = (id: Page, icon: ReactNode, label: string) => {
    const count = counts[id] ?? 0;
    return (
      <button
        type="button"
        className={page === id ? "nav on" : "nav"}
        aria-current={page === id ? "page" : undefined}
        title={collapsed ? label : undefined}
        onClick={() => setPage(id)}
      >
        {icon}
        <span className="nav-label">{label}</span>
        {count > 0 && <span className="badge">{count}</span>}
      </button>
    );
  };

  return (
    <aside className={collapsed ? "side collapsed" : "side"}>
      <div
        className="wordmark"
        role="img"
        aria-label="PsyAudio"
        style={
          {
            "--wm-1x": `url(${wordmark1x})`,
            "--wm-2x": `url(${wordmark2x})`,
          } as React.CSSProperties
        }
      />
      <p className="grp">{t("Werkzeuge")}</p>
      {item("convert", <Music size={17} />, t("Konvertieren"))}
      {item("batch", <Folders size={17} />, t("Stapel"))}
      {item("cd", <Disc3 size={17} />, "CD-Ripper")}
      <div className="grow" />
      {jobs.map((j) => (
        <div key={j.label} className="side-job" title={j.label}>
          <span className="nav-label">{j.label}</span>
          <span className="bar">
            <i style={{ width: `${Math.round(j.progress * 100)}%` }} />
          </span>
        </div>
      ))}
      <p className="grp">System</p>
      {item("settings", <SlidersHorizontal size={17} />, t("Einstellungen"))}
      <div className="side-foot">
        {ffmpeg !== undefined && (
          <span className={ffmpeg ? "status ok" : "status warn"}>
            {ffmpeg ? <CircleCheck size={14} /> : <CircleAlert size={14} />}
            <span className="nav-label">
              {ffmpeg ? t("FFmpeg bereit") : t("FFmpeg fehlt (Einstellungen)")}
            </span>
          </span>
        )}
        <span className="version nav-label">{__APP_VERSION__}</span>
      </div>
      <button
        type="button"
        className="side-toggle"
        aria-label={collapsed ? t("Seitenleiste ausklappen") : t("Seitenleiste einklappen")}
        title={collapsed ? t("Seitenleiste ausklappen") : t("Seitenleiste einklappen")}
        onClick={() =>
          update((s) => {
            s.ui.sidebar_collapsed = !collapsed;
          })
        }
      >
        {collapsed ? <PanelLeftOpen size={15} /> : <PanelLeftClose size={15} />}
      </button>
    </aside>
  );
}
