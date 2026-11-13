import { open } from "@tauri-apps/plugin-dialog";
import { FolderPlus, Play, Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { fmtLen } from "../../format";
import { inputExtensions } from "../../ipc/files";
import { useNaming } from "../../ipc/naming";
import { FooterBar } from "../../shell/FooterBar";
import { PageHeader } from "../../shell/PageHeader";
import { SidePanel } from "../../shell/SidePanel";
import { StepTiles, type Tile } from "../../shell/StepTiles";
import { type EffectEntry, useEffects } from "../../store/effects";
import { useEncode } from "../../store/encode";
import { useFiles } from "../../store/files";
import { useRun } from "../../store/run";
import { useSettings } from "../../store/settings";
import { addFiles } from "./addFiles";
import { DropZone } from "./DropZone";
import "./convert.css";
import { EffectsPanel } from "./EffectsPanel";
import { FileList } from "./FileList";
import { FormatPanel } from "./FormatPanel";
import { NamingPanel } from "./NamingPanel";
import { RunView } from "./RunView";
import { convert } from "./runFlow";
import { SavePanel } from "./SavePanel";
import { codecSummary, effectsSummary, namingSummary, saveSummary } from "./summary";

export async function pickFiles() {
  const extensions = await inputExtensions();
  const picked = await open({ multiple: true, filters: [{ name: "Audio", extensions }] });
  if (picked) await addFiles(Array.isArray(picked) ? picked : [picked]);
}

async function pickFolder() {
  const picked = await open({ directory: true });
  if (typeof picked === "string") await addFiles([picked]);
}

/** Stable empty list while the effect catalogue loads (a new [] each render would re-render). */
const NO_ENTRIES: EffectEntry[] = [];

export function ConvertPage() {
  const { t } = useTranslation();
  const files = useFiles((s) => s.files);
  const selected = useFiles((s) => s.selected);
  const removeSelected = useFiles((s) => s.removeSelected);
  const clear = useFiles((s) => s.clear);
  const settings = useSettings((s) => s.settings);
  const run = useRun((s) => s.run);
  const options = useEncode((s) => s.options);
  const naming = useNaming((s) => s.options);
  const effectEntries = useEffects((s) => s.catalogue?.entries ?? NO_ENTRIES);
  if (!settings) return null;
  if (run) {
    return (
      <div className="page">
        <RunView run={run} />
      </div>
    );
  }

  const tiles: Tile[] = [
    {
      id: "format",
      title: t("Format"),
      summary: codecSummary(settings.codec, settings.enc, options, t),
    },
    { id: "save", title: t("Speichern in"), summary: saveSummary(settings, t) },
    { id: "naming", title: t("Benennung"), summary: namingSummary(settings, naming, t) },
    {
      id: "effects",
      title: t("Effekte"),
      summary: effectsSummary(settings.effects, effectEntries, t),
    },
  ];
  const titles = Object.fromEntries(tiles.map((x) => [x.id, x.title]));
  const seconds = files.reduce((sum, f) => sum + (f.info?.seconds ?? 0), 0);

  return (
    <div className="page">
      <div className="page-main">
        <PageHeader title={t("Konvertieren")} page="convert" firstTile="format">
          <button type="button" className="btn" onClick={() => void pickFiles()}>
            <Plus size={16} />
            {t("Dateien")}
          </button>
          <button type="button" className="btn" onClick={() => void pickFolder()}>
            <FolderPlus size={16} />
            {t("Ordner")}
          </button>
          <button
            type="button"
            className="btn"
            disabled={selected.size === 0}
            onClick={removeSelected}
          >
            <X size={16} />
            {t("Entfernen")}
          </button>
          <button type="button" className="btn" disabled={files.length === 0} onClick={clear}>
            {t("Liste leeren")}
          </button>
        </PageHeader>
        <StepTiles page="convert" tiles={tiles} />
        <div className="content">
          {files.length > 0 ? <FileList /> : <DropZone onPick={() => void pickFiles()} />}
        </div>
        <FooterBar
          summary={<b>{t("{0} Dateien · {1}", { 0: String(files.length), 1: fmtLen(seconds) })}</b>}
        >
          <button
            type="button"
            className="btn primary"
            disabled={files.length === 0}
            onClick={() => {
              const items = files.map((f) => [f.path, f.root] as [string, string | null]);
              void convert(items, settings.codec);
            }}
          >
            <Play size={16} />
            {t("Konvertieren")}
          </button>
        </FooterBar>
      </div>
      <SidePanel page="convert" titles={titles}>
        {(tile) => {
          if (tile === "format") return <FormatPanel field="codec" />;
          if (tile === "save") return <SavePanel />;
          if (tile === "naming") return <NamingPanel />;
          if (tile === "effects") return <EffectsPanel />;
          return <p className="empty">{titles[tile]}</p>;
        }}
      </SidePanel>
    </div>
  );
}
