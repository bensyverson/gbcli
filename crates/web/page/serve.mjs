// Web spike (Stage 12, leaf 0Mfl87): serves the repo root read-only on
// localhost so the page can fetch the .wasm and a ROM.
//
//   node crates/web/page/serve.mjs [PORT]    then open http://localhost:PORT/crates/web/page/
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = resolve(fileURLToPath(import.meta.url), "../../../..");
export const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm", ".png": "image/png" };

/// The bytes and type for a URL path under ROOT, or null.
export async function lookup(urlPath) {
  let path = normalize(decodeURIComponent(urlPath.split("?")[0]));
  if (path.endsWith("/")) path += "index.html";
  const file = join(ROOT, path);
  if (!file.startsWith(ROOT)) return null;
  try {
    return { body: await readFile(file), type: TYPES[extname(file)] ?? "application/octet-stream" };
  } catch {
    return null;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const port = Number(process.argv[2] ?? 8137);
  createServer(async (req, res) => {
    const hit = await lookup(req.url);
    if (!hit) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { "content-type": hit.type }).end(hit.body);
  }).listen(port, "127.0.0.1", () => console.log(`http://localhost:${port}/crates/web/page/`));
}
