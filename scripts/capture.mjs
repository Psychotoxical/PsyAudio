// Screenshot of one page in the sample mode, through the WebView2 DevTools protocol (Windows).
//   node scripts/capture.mjs <konverter|stapel|cd|einstellungen> <file.png> [--light] [--click=<css>]…
// --click clicks the first element matching the selector before the picture (e.g. a tile).
// --eval=<js> runs an expression in the page; --wait=<css> waits up to 2 minutes for an element;
// --file=<path> passes a file on the command line; --title prints the window title at the end;
// --close clicks the window's close button (WM_CLOSE); --expect-exit waits up to 25 s for the end.
// The options run in the order given.
// Needs the debug build (cargo build -p psyaudio) and the Vite dev server (npm run dev).
// A window opens for a few seconds; the sample mode saves nothing.
import { execFileSync, spawn } from "node:child_process";
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
const files = rest.filter((a) => a.startsWith("--file=")).map((a) => a.slice(7));
const args = [`--demo=${page}`, ...(rest.includes("--light") ? ["--light"] : []), ...files];
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
  const evaluate = async (expression) =>
    (await send("Runtime.evaluate", { expression, returnByValue: true })).result?.result?.value;
  for (const opt of rest) {
    if (opt.startsWith("--click=")) {
      const sel = JSON.stringify(opt.slice(8));
      // data from Rust (format lists, chips) may arrive after the shell: wait for the element
      for (let i = 0; i < 20; i++) {
        if (await evaluate(`(() => { const e = document.querySelector(${sel}); e?.click(); return !!e; })()`)) break;
        await sleep(250);
      }
      await sleep(300);
    } else if (opt.startsWith("--eval=")) {
      console.log("eval:", await evaluate(opt.slice(7)));
      await sleep(300);
    } else if (opt === "--close") {
      // the window's close button: WM_CLOSE to the main window
      execFileSync("powershell", ["-NoProfile", "-Command", `[void](Get-Process -Id ${child.pid}).CloseMainWindow()`]);
      await sleep(600);
    } else if (opt === "--expect-exit") {
      for (let i = 0; i < 100 && child.exitCode === null; i++) await sleep(250);
      console.log("exited:", child.exitCode !== null ? `yes (code ${child.exitCode})` : "no");
      process.exit(0);
    } else if (opt.startsWith("--wait=")) {
      const sel = JSON.stringify(opt.slice(7));
      for (let i = 0; i < 480 && !(await evaluate(`!!document.querySelector(${sel})`)); i++) await sleep(250);
    }
  }
  await sleep(400);
  const shot = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(file, Buffer.from(shot.result.data, "base64"));
  console.log("wrote", file);
  if (rest.includes("--title")) {
    const title = execFileSync("powershell", ["-NoProfile", "-Command", `(Get-Process -Id ${child.pid}).MainWindowTitle`], { encoding: "utf8" });
    console.log("title:", title.trim());
  }
  ws.close();
} finally {
  child.kill();
}
