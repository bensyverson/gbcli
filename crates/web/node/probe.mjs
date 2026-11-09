// Web spike probe (leaf 3zNDPj): sends a list of protocol requests to one
// .wasm handler under Node's WASI and prints each op's outcome, to find the
// ops that fail, or abort the module, on wasm32.
//
//   node probe.mjs WASM ROM
import { readFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { WASI } from "node:wasi";

const [wasmPath, romPath] = process.argv.slice(2);
const work = await mkdtemp(join(tmpdir(), "gbcli-probe-"));
const wasi = new WASI({ version: "preview1", args: [], env: {}, preopens: { "/work": work }, returnOnExit: true });
const module = await WebAssembly.compile(await readFile(wasmPath));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
wasi.initialize(instance);
const x = instance.exports;
const enc = new TextEncoder();
const dec = new TextDecoder();
function call(fn, ...buffers) {
  const args = buffers.flatMap((bytes) => {
    const ptr = x.gbw_alloc(bytes.length);
    new Uint8Array(x.memory.buffer, ptr, bytes.length).set(bytes);
    return [ptr, bytes.length];
  });
  const packed = BigInt.asUintN(64, fn(...args));
  for (let i = 0; i < args.length; i += 2) x.gbw_free(args[i], args[i + 1]);
  if (packed === 0xffffffffffffffffn) return null;
  const ptr = Number(packed >> 32n);
  const len = Number(packed & 0xffffffffn);
  const out = new Uint8Array(x.memory.buffer, ptr, len).slice();
  x.gbw_free(ptr, len);
  return dec.decode(out);
}

call(x.gbw_put_file, enc.encode("/work/rom.gb"), await readFile(romPath));
console.log("open", call(x.gbw_open, enc.encode(JSON.stringify({ dir: "/work", session: "web", rom: "rom.gb" }))));

const requests = [
  { op: "version" },
  { op: "run", args: { frames: 60 } },
  { op: "show" },
  { op: "checkpoint", args: { name: "a" } },
  { op: "run", args: { frames: 30 } },
  { op: "restore", args: { name: "a" } },
  { op: "undo" },
  { op: "checkpoints" },
  { op: "events", args: {} },
  { op: "step", args: {} },
  { op: "read", args: { reads: ["pc", "[$FF44]"] } },
  { op: "write", args: { writes: [{ kind: "poke", target: "$C000", bytes: "01" }] } },
  { op: "shot", args: { shot: { path: "s.png" } } },
  { op: "view", args: { what: "tiles", png: { path: "t.png" } } },
  { op: "view", args: { what: "memory", addr: "$C000", len: 16 } },
  { op: "measure", args: { from: "$0204", to: "$0204" } },
  { op: "sessions" },
  { op: "gaps" },
  { op: "opcode", args: { query: "ld a, b" } },
  { op: "doctor" },
  { op: "build", args: {} },
  { op: "new", args: { dir: "game" } },
  { op: "build", args: { dir: "game" } },
  { op: "power_cycle", args: {} },
  { op: "sav", args: { path: "x.sav" } },
  { op: "prune", args: { unused_for: "7d" } },
  // Last: it aborts the module (std::env::temp_dir panics on WASI).
  { op: "assets", args: { dir: "game" } },
];
let n = 0;
for (const r of requests) {
  const line = JSON.stringify({ id: String(++n), ...r });
  let out;
  try {
    out = call(x.gbw_request, enc.encode(line));
  } catch (e) {
    console.log(`${r.op.padEnd(12)} ABORTED the module: ${e.message}`);
    break;
  }
  const res = JSON.parse(out);
  const what = res.ok ? "ok" : `${res.error.code}: ${res.error.message}`;
  console.log(`${r.op.padEnd(12)} ${what.slice(0, 220)}`);
}
await rm(work, { recursive: true, force: true });
