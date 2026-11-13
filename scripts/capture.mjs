// Screenshot of one page in the sample mode, through the WebView2 DevTools protocol (Windows).
//   node scripts/capture.mjs <konverter|stapel|cd|einstellungen> <file.png> [--light]
// Needs the debug build (cargo build -p psyaudio) and the Vite dev server (npm run dev).
// A window opens for a few seconds; the sample mode saves nothing.
import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [page, file, ...rest] = process.argv.slice(2);
if (!page || !file) {
  console.error("usage: node scripts/capture.mjs <page> <file.png> [--light]");
  process.exit(2);
}
const port = 9333;
const exe = resolve(import.meta.dirname, "..", "target", "debug", "PsyAudio.exe");
const args = [`--demo=${page}`, ...(rest.includes("--light") ? ["--light"] : [])];
const child = spawn(exe, args, {
  env: {
    ...process.env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
    // its own browser process: an open PsyAudio would otherwise take this window in, without the port
    WEBVIEW2_USER_DATA_FOLDER: join(tmpdir(), "psyaudio-capture-webview"),
  },
  stdio: "ignore",
});
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function target() {
  for (let i = 0; i < 60; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
      const t = list.find((x) => x.type === "page" && /localhost:1420|tauri\.localhost/.test(x.url));
      if (t) return t;
    } catch {
      // not listening yet
    }
    await sleep(500);
  }
  throw new Error("no PsyAudio page on the DevTools port");
}

try {
  const t = await target();
  const ws = new WebSocket(t.webSocketDebuggerUrl);
  await new Promise((ok, fail) => {
    ws.onopen = ok;
    ws.onerror = fail;
  });
  let id = 0;
  const waiting = new Map();
  ws.onmessage = (m) => {
    const msg = JSON.parse(m.data);
    if (msg.id && waiting.has(msg.id)) {
      waiting.get(msg.id)(msg);
      waiting.delete(msg.id);
    }
  };
  const send = (method, params = {}) =>
    new Promise((ok) => {
      id += 1;
      waiting.set(id, ok);
      ws.send(JSON.stringify({ id, method, params }));
    });
  await send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  // wait until the shell is drawn and the fonts are in
  for (let i = 0; i < 40; i++) {
    const r = await send("Runtime.evaluate", {
      expression: "document.fonts.ready.then(() => !!document.querySelector('.app'))",
      awaitPromise: true,
      returnByValue: true,
    });
    if (r.result?.result?.value) break;
    await sleep(250);
  }
  await sleep(400);
  const shot = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(file, Buffer.from(shot.result.data, "base64"));
  console.log("wrote", file);
  ws.close();
} finally {
  child.kill();
}
