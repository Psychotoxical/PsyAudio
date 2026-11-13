import { useEffect, useState } from "react";
import {
  type FilesSource,
  fileManagerMenu,
  frontendReady,
  onDragState,
  onFilesAdded,
} from "./ipc/events";

interface Received {
  path: string;
  source: FilesSource;
}

// Platform check (stage 2): shows every path that reaches the window and where it came from.
export function App() {
  const [files, setFiles] = useState<Received[]>([]);
  const [dragging, setDragging] = useState(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    const add = (paths: string[], source: FilesSource) =>
      setFiles((f) => [...f, ...paths.map((path) => ({ path, source }))]);
    const listeners = [
      onFilesAdded((e) => add(e.paths, e.source)),
      onDragState((e) => setDragging(e.active)),
    ];
    // listen first, then ask for the start files, so nothing falls in between
    Promise.all(listeners)
      .then(() => frontendReady())
      .then((paths) => add(paths, "start"));
    return () => {
      for (const l of listeners) l.then((un) => un());
    };
  }, []);

  const menu = (add: boolean) => fileManagerMenu(add).then(setMessage);

  return (
    <main
      style={{
        minHeight: "100vh",
        padding: 24,
        background: dragging ? "#313244" : "#1e1e2e",
        color: "#cdd6f4",
        font: "14px/1.5 system-ui, sans-serif",
      }}
    >
      <h1 style={{ color: "#cba6f7", fontSize: 22, margin: "0 0 12px" }}>
        PsyAudio – Plattform-Test
      </h1>
      <p style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <button type="button" onClick={() => menu(true)}>
          Menü einrichten
        </button>
        <button type="button" onClick={() => menu(false)}>
          Menü entfernen
        </button>
        <span>{message}</span>
      </p>
      <p>{dragging ? "Loslassen zum Hinzufügen" : `${files.length} Pfade empfangen`}</p>
      <ol>
        {files.map((f, i) => (
          // the same path may arrive twice from different sources
          // biome-ignore lint/suspicious/noArrayIndexKey: entries are only ever appended
          <li key={i}>
            <code>{f.path}</code> <span style={{ color: "#a6adc8" }}>({f.source})</span>
          </li>
        ))}
      </ol>
    </main>
  );
}
