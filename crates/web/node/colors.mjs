// Web spike (leaf 3zNDPj): SameBoy's color conversion for all 32768 RGB555
// colors on a GBC, wasm against native. It runs libm's pow, the one place a
// wasm libm (wasi-libc's) could round differently from Apple's.
//
//   node colors.mjs WASM ROM NATIVE_COLORS
//
// NATIVE_COLORS is target/release/examples/native_colors. Exits 0 on equal.
import { readFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { WASI } from "node:wasi";

const [wasmPath, romPath, nativeColors] = process.argv.slice(2);
const wasi = new WASI({ version: "preview1", args: [], env: {}, returnOnExit: true });
const module = await WebAssembly.compile(await readFile(wasmPath));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
wasi.initialize(instance);
const x = instance.exports;

const rom = await readFile(romPath);
const ptr = x.gbw_alloc(rom.length);
new Uint8Array(x.memory.buffer, ptr, rom.length).set(rom);
const packed = BigInt.asUintN(64, x.gbw_color_table_sha256(ptr, rom.length));
x.gbw_free(ptr, rom.length);
const outPtr = Number(packed >> 32n);
const outLen = Number(packed & 0xffffffffn);
const wasm = new TextDecoder().decode(new Uint8Array(x.memory.buffer, outPtr, outLen).slice());
x.gbw_free(outPtr, outLen);

const native = execFileSync(nativeColors, [romPath], { encoding: "utf8" }).trim();
console.log(`wasm   ${wasm}`);
console.log(`native ${native}`);
const ok = wasm === native;
console.log(ok ? "PASS: all 32768 colors convert identically" : "FAIL");
process.exit(ok ? 0 : 1);
