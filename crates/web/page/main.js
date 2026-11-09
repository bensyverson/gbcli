// Web spike (Stage 12, leaf 0Mfl87): the demo page. Loads the .wasm handler
// through browser_wasi_shim, puts a ROM into its in-memory filesystem, and
// sends one protocol `run` of one frame with a shot per animation frame,
// drawing the PNG it reads back. Keys: arrows, Z (A), X (B), Enter (START),
// Shift (SELECT). ?bench runs bench-core.js instead and shows its JSON.
// Spike code: disposable.
import * as shim from "./node_modules/@bjorn3/browser_wasi_shim/dist/index.js";
import { instantiateWithShim } from "./gbw.js";
import { benchmark } from "./bench-core.js";

const WASM = "../../../target/wasm32-wasip1/release/gbcli_web.wasm";
const params = new URLSearchParams(location.search);
const ROM = params.get("rom") ?? "../../sameboy/tests/roms/layers.gb";
const status = document.getElementById("status");
const ctx = document.getElementById("screen").getContext("2d");
const say = (text) => (status.textContent = text);
const fetchBytes = async (url) => {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: ${res.status}`);
  return new Uint8Array(await res.arrayBuffer());
};
const instantiate = async (module) => (await instantiateWithShim(shim, module, console.log)).gbw;

async function draw(png) {
  const bitmap = await createImageBitmap(new Blob([png], { type: "image/png" }));
  ctx.drawImage(bitmap, 0, 0);
  bitmap.close();
}

async function runBench() {
  say("benchmarking...");
  const frames = Number(params.get("frames") ?? 3000);
  const reps = Number(params.get("reps") ?? 5);
  const result = {
    host: navigator.userAgent,
    rom: ROM,
    ...(await benchmark({
      wasmBytes: await fetchBytes(WASM),
      instantiate,
      rom: await fetchBytes(ROM),
      frames,
      reps,
      now: () => performance.now(),
    })),
  };
  window.__bench = result;
  say(JSON.stringify(result, null, 2));
}

// ?check: the request crates/web/node/check.mjs sends, so browser.mjs can
// compare the browser's answer with the native build's byte for byte.
async function runCheck() {
  const frames = Number(params.get("frames") ?? 600);
  const line = JSON.stringify({
    id: "1",
    op: "run",
    args: {
      frames,
      keys: ["START@10", "-START@12", "A@60", "-A@70", "RIGHT@100", "-RIGHT@160"],
      read: ["a", "pc", "[$FF40]"],
      shot: { path: "shot.png", scale: 1 },
      audio: { wav: { path: "mix.wav" } },
    },
  });
  const module = await WebAssembly.compile(await fetchBytes(WASM));
  const gbw = await instantiate(module);
  gbw.putFile("/work/rom.gb", await fetchBytes(ROM));
  gbw.open({ dir: "/work", session: "web", rom: "rom.gb" });
  const responseLine = gbw.requestLine(line);
  const hex = async (bytes) =>
    [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map((b) => b.toString(16).padStart(2, "0")).join("");
  const files = {};
  for (const name of ["shot.png", "mix.wav"]) {
    const bytes = gbw.getFile(`/work/${name}`);
    files[name] = bytes ? await hex(bytes) : null;
  }
  window.__bench = { line, response_line: responseLine, files };
  say(JSON.stringify(window.__bench, null, 2));
}

const KEYS = {
  ArrowRight: "RIGHT", ArrowLeft: "LEFT", ArrowUp: "UP", ArrowDown: "DOWN",
  KeyZ: "A", KeyX: "B", Enter: "START", ShiftLeft: "SELECT", ShiftRight: "SELECT",
};

async function runDemo() {
  const t0 = performance.now();
  const [wasmBytes, rom] = await Promise.all([fetchBytes(WASM), fetchBytes(ROM)]);
  const t1 = performance.now();
  const module = await WebAssembly.compile(wasmBytes);
  const gbw = await instantiate(module);
  gbw.putFile("/work/rom.gb", rom);
  gbw.open({ dir: "/work", session: "web", rom: "rom.gb" });

  // Held keys persist in the machine between runs, so a press or release
  // goes out once, at frame 0 of the next run.
  const pending = [];
  const held = new Set();
  addEventListener("keydown", (e) => {
    const b = KEYS[e.code];
    if (!b || held.has(b)) return;
    held.add(b);
    pending.push(`${b}@0`);
    e.preventDefault();
  });
  addEventListener("keyup", (e) => {
    const b = KEYS[e.code];
    if (!b || !held.has(b)) return;
    held.delete(b);
    pending.push(`-${b}@0`);
    e.preventDefault();
  });

  let frame = 0;
  let firstDrawn = null;
  let windowStart = performance.now();
  let windowFrames = 0;
  let fps = 0;
  async function tick() {
    const keys = pending.splice(0);
    const r = gbw.request({ id: String(frame), op: "run", args: { frames: 1, keys, shot: { path: "f.png", scale: 1 } } });
    if (!r.ok) {
      say(`run failed: ${JSON.stringify(r.error)}`);
      return;
    }
    await draw(gbw.getFile("/work/f.png"));
    frame++;
    windowFrames++;
    const now = performance.now();
    if (firstDrawn === null) firstDrawn = now;
    if (now - windowStart >= 1000) {
      fps = (windowFrames * 1000) / (now - windowStart);
      windowStart = now;
      windowFrames = 0;
    }
    say(
      `rom ${ROM}\n` +
        `fetch ${(t1 - t0).toFixed(0)} ms, compile to first frame drawn ${(firstDrawn - t1).toFixed(0)} ms\n` +
        `emulated frame ${r.result.stop.time.frame}, ${fps.toFixed(1)} requests/s (one frame + shot each)\n` +
        `keys held: ${[...held].join(" ") || "none"} (arrows, Z=A, X=B, Enter=START, Shift=SELECT)`,
    );
    requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);
}

(params.has("bench") ? runBench() : params.has("check") ? runCheck() : runDemo()).catch((e) => {
  say(`error: ${e.stack ?? e}`);
  window.__error = String(e);
});
