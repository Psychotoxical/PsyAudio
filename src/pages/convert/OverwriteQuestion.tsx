import { useRun } from "../../store/run";
import { OverwriteDialog } from "./OverwriteDialog";
import { launch } from "./runFlow";

/** The overwrite question of the converter or batch page, over the whole window. */
export function OverwriteQuestion() {
  const ask = useRun((s) => s.ask);
  if (!ask) return null;
  const close = () => useRun.setState({ ask: null });
  return (
    <OverwriteDialog
      paths={ask.paths}
      onCancel={close}
      onStart={(overwrite) => {
        close();
        void launch(ask.items, ask.codec, ask.keepStructure, overwrite);
      }}
    />
  );
}
