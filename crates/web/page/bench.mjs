// Web spike (Stage 12, leaf 0Mfl87): the benchmark (bench-core.js) in Node.
//
//   node bench.mjs WASM ROM [shim|node] [FRAMES] [REPS]
//
// `shim` (the default) runs the module under @bjorn3/browser_wasi_shim with
// an in-memory filesystem, as the page does; `node` under Node's node:wasi
// with a temp directory preopened as /work. Prints one JSON object.
import { readFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { WASI } from "node:wasi";
import * as shim from "@bjorn3/browser_wasi_shim";
import { Gbw, instantiateWithShim } from "./gbw.js";
import { benchmark } from "./bench-core.js";

const [wasmPath, romPath, host = "shim", framesArg, repsArg] = process.argv.slice(2);
if (!romPath) {
  console.error("usage: node bench.mjs WASM ROM [shim|node] [FRAMES] [REPS]");
  process.exit(2);
}
let work = null;
const instantiate =
  host === "node"
    ? async (module) => {
        work = await mkdtemp(join(tmpdir(), "gbcli-bench-"));
        const wasi = new WASI({ version: "preview1", args: [], env: {}, preopens: { "/work": work }, returnOnExit: true });
        const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
        wasi.initialize(instance);
        return new Gbw(instance);
      }
    : async (module) => (await instantiateWithShim(shim, module)).gbw;

const result = await benchmark({
  wasmBytes: await readFile(wasmPath),
  instantiate,
  rom: new Uint8Array(await readFile(romPath)),
  frames: Number(framesArg ?? 3000),
  reps: Number(repsArg ?? 5),
  now: () => performance.now(),
});
if (work) await rm(work, { recursive: true, force: true });
console.log(JSON.stringify({ host: `node ${process.version} ${host === "node" ? "node:wasi" : "browser_wasi_shim"}`, rom: romPath, ...result }));
