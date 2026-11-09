// Web spike check (leaf 3zNDPj): runs a ROM N frames with keys through the
// .wasm build of gbcli's handler under Node's WASI, and through the native
// build of the same calls, and checks that the protocol response lines and
// the SHA-256 of the shot (PNG) and the audio mix (WAV) are identical.
//
//   node check.mjs WASM ROM NATIVE_REF [FRAMES] [MODEL]
//
// NATIVE_REF is target/release/examples/native_ref (crates/web/examples);
// MODEL is a model's wire name (dmg, gbc, ...), the bundle default if absent.
// Exits 0 when everything is identical, else 1.
import { readFile, mkdtemp, rm } from "node:fs/promises";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { WASI } from "node:wasi";

const [wasmPath, romPath, nativeRef, framesArg, model] = process.argv.slice(2);
if (!nativeRef) {
  console.error("usage: node check.mjs WASM ROM NATIVE_REF [FRAMES] [MODEL]");
  process.exit(2);
}
const frames = Number(framesArg ?? 600);
const OUTPUTS = ["shot.png", "mix.wav"];
const request = {
  id: "1",
  op: "run",
  args: {
    frames,
    keys: ["START@10", "-START@12", "A@60", "-A@70", "RIGHT@100", "-RIGHT@160"],
    read: ["a", "pc", "[$FF40]"],
    shot: { path: "shot.png", scale: 1 },
    audio: { wav: { path: "mix.wav" } },
  },
};
const line = JSON.stringify(request);
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

// --- the .wasm, under WASI with a temp directory preopened as /work ---
const work = await mkdtemp(join(tmpdir(), "gbcli-web-"));
const wasi = new WASI({
  version: "preview1",
  args: ["gbcli-web"],
  env: {},
  preopens: { "/work": work },
  returnOnExit: true,
});
const module = await WebAssembly.compile(await readFile(wasmPath));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
wasi.initialize(instance);
const x = instance.exports;

const enc = new TextEncoder();
const dec = new TextDecoder();
function put(bytes) {
  const ptr = x.gbw_alloc(bytes.length);
  new Uint8Array(x.memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length];
}
function take(signed) {
  // A wasm i64 reaches JS as a signed BigInt.
  const packed = BigInt.asUintN(64, signed);
  if (packed === 0xffffffffffffffffn) return null;
  const ptr = Number(packed >> 32n);
  const len = Number(packed & 0xffffffffn);
  const out = new Uint8Array(x.memory.buffer, ptr, len).slice();
  x.gbw_free(ptr, len);
  return out;
}
function call(fn, ...buffers) {
  const args = buffers.flatMap(put);
  const out = take(fn(...args));
  for (let i = 0; i < args.length; i += 2) x.gbw_free(args[i], args[i + 1]);
  return out;
}

const rom = await readFile(romPath);
const putErr = dec.decode(call(x.gbw_put_file, enc.encode("/work/rom.gb"), rom));
if (putErr) throw new Error(`put_file: ${putErr}`);
const open = { dir: "/work", session: "web", rom: "rom.gb", ...(model ? { model } : {}) };
const opened = JSON.parse(dec.decode(call(x.gbw_open, enc.encode(JSON.stringify(open)))));
if (!opened.ok) throw new Error(`open: ${JSON.stringify(opened.error)}`);
const responseBytes = call(x.gbw_request, enc.encode(line));
if (!responseBytes) throw new Error("request: no handler");
const wasmLine = dec.decode(responseBytes);
const wasmFiles = {};
for (const name of OUTPUTS) {
  const bytes = call(x.gbw_get_file, enc.encode(`/work/${name}`));
  wasmFiles[name] = bytes ? sha256(bytes) : null;
}
await rm(work, { recursive: true, force: true });

// --- the native build of the same calls ---
const nativeDir = await mkdtemp(join(tmpdir(), "gbcli-native-"));
const native = JSON.parse(
  execFileSync(nativeRef, [nativeDir, romPath, line, model ?? "-"], { encoding: "utf8" }),
);
await rm(nativeDir, { recursive: true, force: true });

let ok = wasmLine === native.response_line;
console.log(`rom ${romPath} frames ${frames} model ${model ?? "default"}`);
console.log(`request         ${line}`);
console.log(`wasm response   ${wasmLine}`);
if (!ok) console.log(`native response ${native.response_line}`);
console.log(`response lines  ${ok ? "identical" : "DIFFER"}`);
for (const name of OUTPUTS) {
  const same = wasmFiles[name] !== null && wasmFiles[name] === native.files[name];
  ok &&= same;
  console.log(`${name.padEnd(9)} wasm ${wasmFiles[name]} native ${native.files[name]} ${same ? "equal" : "DIFFER"}`);
}
console.log(ok ? "PASS" : "FAIL");
process.exit(ok ? 0 : 1);
