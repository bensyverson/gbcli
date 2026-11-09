// Web spike (Stage 12, leaf 0Mfl87): counts the WASI calls one-frame
// requests make under browser_wasi_shim, and the time spent in them, to tell
// host cost from time inside the module; then lists the in-memory bundle.
// `shot` adds a shot to each request. Run from the repo root:
//
//   node scripts/web/wasi-calls.mjs target/wasm32-wasip1/release/gbcli_web.wasm ROM [shot]
import { readFile } from "node:fs/promises";
import * as shim from "../../crates/web/page/node_modules/@bjorn3/browser_wasi_shim/dist/index.js";
import { Gbw } from "../../crates/web/page/gbw.js";

const [wasmPath, romPath, shotArg] = process.argv.slice(2);
const fds = [
  new shim.OpenFile(new shim.File([])),
  shim.ConsoleStdout.lineBuffered((m) => console.log(m)),
  shim.ConsoleStdout.lineBuffered((m) => console.log(m)),
  new shim.PreopenDirectory("/work", []),
];
const wasi = new shim.WASI([], [], fds, { debug: false });
const stats = {};
let paths = [];
for (const [name, fn] of Object.entries(wasi.wasiImport)) {
  wasi.wasiImport[name] = (...a) => {
    const t = performance.now();
    const r = fn(...a);
    const s = (stats[name] ??= { n: 0, ms: 0 });
    s.n++;
    s.ms += performance.now() - t;
    if (name === "path_open") {
      paths.push(new TextDecoder().decode(new Uint8Array(wasi.inst.exports.memory.buffer, a[2], a[3])));
    }
    return r;
  };
}
// The store sets file times, which the shim refuses (see gbw.js addTimes).
wasi.wasiImport.fd_filestat_set_times = () => 0;
const module = await WebAssembly.compile(await readFile(wasmPath));
const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: wasi.wasiImport });
wasi.initialize(instance);
const g = new Gbw(instance);
g.putFile("/work/rom.gb", new Uint8Array(await readFile(romPath)));
g.open({ dir: "/work", session: "web", rom: "rom.gb" });
g.request({ id: "a", op: "run", args: { frames: 60 } });
for (const k of Object.keys(stats)) delete stats[k];
paths = [];
const N = 20;
const t = performance.now();
for (let i = 0; i < N; i++) {
  const args = { frames: 1 };
  if (shotArg) args.shot = { path: "f.png", scale: 1 };
  g.request({ id: "b", op: "run", args });
}
const total = performance.now() - t;
console.log("per request ms", (total / N).toFixed(2));
for (const [k, s] of Object.entries(stats).sort((a, b) => b[1].ms - a[1].ms)) {
  console.log(k.padEnd(24), (s.n / N).toFixed(1), "calls/req", (s.ms / N).toFixed(2), "ms/req");
}
const counts = {};
for (const p of paths) counts[p] = (counts[p] ?? 0) + 1;
console.log(Object.entries(counts).slice(0, 40));
function walk(dir, pre) {
  for (const [n, e] of dir.contents) {
    if (e.contents) walk(e, pre + n + "/");
    else console.log(pre + n, e.data.length);
  }
}
walk(fds[3].dir, "/work/");
