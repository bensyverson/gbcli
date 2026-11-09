// Lists a .wasm module's imports and exports: node imports.mjs FILE.wasm
import { readFile } from "node:fs/promises";

const module = await WebAssembly.compile(await readFile(process.argv[2]));
for (const i of WebAssembly.Module.imports(module)) {
  console.log(`import ${i.module}.${i.name} (${i.kind})`);
}
for (const e of WebAssembly.Module.exports(module)) {
  console.log(`export ${e.name} (${e.kind})`);
}
