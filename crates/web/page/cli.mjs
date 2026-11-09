// Web spike (Stage 12, leaf 0Mfl87): the CLI itself, in-process in the
// .wasm under browser_wasi_shim (gbw_cli): help, recipes and rows, with no
// process. Prints each invocation's exit code and the first lines of its
// output.
//
//   node cli.mjs WASM ROM
import { readFile } from "node:fs/promises";
import * as shim from "@bjorn3/browser_wasi_shim";
import { instantiateWithShim } from "./gbw.js";

const [wasmPath, romPath] = process.argv.slice(2);
const { gbw } = await instantiateWithShim(shim, await WebAssembly.compile(await readFile(wasmPath)));
gbw.putFile("/work/game.gb", new Uint8Array(await readFile(romPath)));
const calls = [
  [],
  ["help", "traps"],
  ["run", "--help"],
  ["recipes"],
  ["run", "game.gb", "--session", "demo", "--frames", "120", "--shot", "first.png"],
  ["show", "--session", "demo"],
  ["read", "pc", "frame", "--session", "demo"],
  ["doctor"],
];
let failed = false;
for (const args of calls) {
  const t = performance.now();
  const { code, out, err } = gbw.cli(args);
  const ms = (performance.now() - t).toFixed(1);
  const text = (out + err).trimEnd().split("\n");
  console.log(`$ gbcli ${args.join(" ")}    [exit ${code}, ${ms} ms, ${text.length} lines]`);
  console.log(text.slice(0, 6).map((l) => `  ${l}`).join("\n"));
  if (code !== 0 && args[0] !== "doctor") failed = true;
}
const shot = gbw.getFile("/work/first.png");
console.log(`first.png: ${shot ? shot.length + " bytes" : "missing"}`);
process.exit(failed || !shot ? 1 : 0);
