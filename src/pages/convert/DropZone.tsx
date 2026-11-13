import { FileDown } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useUi } from "../../store/ui";

/** The empty converter list: drop files here or pick them (as in the egui version). */
export function DropZone({ onPick }: { onPick: () => void }) {
  const { t } = useTranslation();
  const dragging = useUi((s) => s.dragging);
  return (
    <div className="drop-wrap">
      <div className={dragging ? "drop over" : "drop"}>
        <FileDown size={44} strokeWidth={1.6} />
        <h2>{t("Musikdateien oder Ordner hierher ziehen")}</h2>
        <p>
          {t("MP3, FLAC, WAV, AAC/M4A, ALAC, Opus, Ogg, WMA, AIFF, APE, WavPack, DSD und mehr")}
        </p>
        <button type="button" className="btn primary" onClick={onPick}>
          {t("Dateien auswählen")}
        </button>
      </div>
    </div>
  );
}
