import { Copy, ExternalLink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { openFfmpegDownload, type SystemInfo } from "../../ipc/system";
import { useUi } from "../../store/ui";
import { Hint } from "../../ui/controls";
import { useSystemInfo } from "./SettingsPage";

/** The usual way to install FFmpeg on each system; Linux names the distribution. */
const COMMANDS: Record<SystemInfo["os"], { distro?: string; command: string }[]> = {
  windows: [{ command: "winget install Gyan.FFmpeg" }],
  macos: [{ command: "brew install ffmpeg" }],
  linux: [
    { distro: "Arch, CachyOS", command: "sudo pacman -S ffmpeg" },
    { distro: "Debian, Ubuntu", command: "sudo apt install ffmpeg" },
    { distro: "Fedora", command: "sudo dnf install ffmpeg-free" },
  ],
};

/** Shown when no FFmpeg is found at all (a build without the shipped one): how to get it. */
export function FfmpegInstallHint() {
  const { t } = useTranslation();
  const info = useSystemInfo();
  if (!info) return null;
  const copy = async (command: string) => {
    try {
      await navigator.clipboard.writeText(command);
      useUi.getState().notify(t("Command copied"));
    } catch (e) {
      useUi.getState().notify(String(e));
    }
  };
  return (
    <div className="field ffmpeg-install">
      <Hint warn>
        {t("FFmpeg is missing. Without it PsyAudio writes only WAV and AIFF. To install it:")}
      </Hint>
      {COMMANDS[info.os].map(({ distro, command }) => (
        <div className="row" key={command}>
          {distro && <span className="distro">{distro}</span>}
          <code className="command">{command}</code>
          <button
            type="button"
            className="icon-btn"
            aria-label={t("Copy command")}
            title={t("Copy command")}
            onClick={() => void copy(command)}
          >
            <Copy size={15} />
          </button>
        </div>
      ))}
      <div className="row">
        <button
          type="button"
          className="btn"
          onClick={() => void openFfmpegDownload().catch((e) => useUi.getState().notify(String(e)))}
        >
          <ExternalLink size={15} />
          {t("Open ffmpeg.org")}
        </button>
      </div>
    </div>
  );
}
