import { i18n } from "../../i18n";
import type { Codec } from "../../ipc/bindings/Codec";
import { readFileInfo } from "../../ipc/files";
import { checkExisting, clearRunTitle, type Items, startConversion } from "../../ipc/run";
import { useFiles } from "../../store/files";
import { applyEvent, failedItems, newRun, type RunEvent, useRun } from "../../store/run";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { addFiles } from "./addFiles";

let queued: RunEvent[] = [];
let scheduled = false;

/** Events arrive in bursts: they are applied once per frame, not one render each. */
function enqueue(e: RunEvent) {
  queued.push(e);
  if (scheduled) return;
  scheduled = true;
  requestAnimationFrame(() => {
    scheduled = false;
    const events = queued;
    queued = [];
    const run = useRun.getState().run;
    if (!run) return;
    const next = events.reduce(applyEvent, run);
    useRun.setState({ run: next });
    if (events.some((x) => x.kind === "Finished")) refreshInfo();
  });
}

/** After a run, tags and lengths may have changed (tag tools, ReplayGain): read them again. */
function refreshInfo() {
  const files = useFiles.getState().files;
  if (files.length === 0) return;
  void readFileInfo(
    files.map((f) => [f.id, f.path]),
    (i) => useFiles.getState().setInfo(i.id, i.tags, i.info, i.cover),
  );
}

/** Starts the run right away (the overwrite question is answered or not needed). */
export async function launch(
  items: Items,
  codec: Codec,
  keepStructure: boolean,
  overwrite: string[],
): Promise<void> {
  const settings = useSettings.getState().settings;
  if (!settings) return;
  if (useRun.getState().run?.elapsed === null) {
    useUi
      .getState()
      .notify(i18n.t("Es läuft schon eine Umwandlung – erst abwarten oder abbrechen"));
    return;
  }
  const files = useFiles.getState().files;
  const audioSecs = items.reduce(
    (sum, [p]) => sum + (files.find((f) => f.path === p)?.info?.seconds ?? 0),
    0,
  );
  // the run view (on the converter page) shows at once; title and threads follow from Rust
  useRun.setState({ run: newRun({ title: "", target: "", threads: 1 }, items, audioSecs) });
  useUi.getState().setPage("convert");
  try {
    const info = await startConversion(settings, items, codec, keepStructure, overwrite, enqueue);
    const run = useRun.getState().run;
    if (run) useRun.setState({ run: { ...run, ...info } });
  } catch (e) {
    useRun.setState({ run: null });
    useUi.getState().notify(String(e));
  }
}

/** "Konvertieren": checks first, asks about existing targets (useRun.ask), then starts. */
export async function convert(items: Items, codec: Codec, keepStructure = false): Promise<void> {
  const settings = useSettings.getState().settings;
  if (!settings) return;
  try {
    const paths = await checkExisting(settings, items, codec, keepStructure);
    if (paths.length > 0) {
      useRun.setState({ ask: { items, codec, keepStructure, paths } });
      return;
    }
  } catch (e) {
    useUi.getState().notify(String(e));
    return;
  }
  await launch(items, codec, keepStructure, []);
}

/** "Fertig": back to the list. */
export function closeRun() {
  useRun.setState({ run: null });
  void clearRunTitle();
}

/** "Fehlgeschlagene erneut umwandeln": the list holds the failed files only. */
export async function retryFailed(t: (key: string) => string) {
  const run = useRun.getState().run;
  if (!run) return;
  const failed = failedItems(run);
  closeRun();
  const keep = new Set(failed.map(([p]) => p));
  const files = useFiles.getState();
  useFiles.setState({ files: files.files.filter((f) => keep.has(f.path)), selected: new Set() });
  // files that left the list meanwhile come back, with their batch folder
  const present = new Set(useFiles.getState().files.map((f) => f.path));
  const missing = failed.filter(([p]) => !present.has(p));
  for (const root of new Set(missing.map(([, r]) => r))) {
    await addFiles(
      missing.filter(([, r]) => r === root).map(([p]) => p),
      root,
    );
  }
  useUi
    .getState()
    .notify(
      t(
        "Fehlgeschlagene Dateien sind wieder in der Liste – Einstellungen anpassen und erneut umwandeln",
      ),
    );
}
