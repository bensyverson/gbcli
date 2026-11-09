// Web spike (Stage 12, leaf 0Mfl87): drives the page in Playwright's
// headless Chromium, serving the repo root through request interception (no
// port). `demo` runs the page for a few seconds with some key presses and
// saves a screenshot; `bench` runs ?bench and prints its JSON; `check` runs
// check.mjs's request in the page and compares it with the native build's
// (target/release/examples/native_ref), exiting 1 on any difference.
//
//   PLAYWRIGHT_BROWSERS_PATH=target/web-spike/ms-playwright \
//     node crates/web/page/browser.mjs demo OUT.png [ROM]
//   PLAYWRIGHT_BROWSERS_PATH=target/web-spike/ms-playwright \
//     node crates/web/page/browser.mjs bench [ROM] [FRAMES] [REPS]
//   PLAYWRIGHT_BROWSERS_PATH=target/web-spike/ms-playwright \
//     node crates/web/page/browser.mjs check ROM target/release/examples/native_ref [FRAMES]
//
// ROM is a path from the repo root.
import { execFileSync } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";
import { ROOT, lookup } from "./serve.mjs";

const [mode, ...rest] = process.argv.slice(2);
const ORIGIN = "https://gbw.local"; // https: crypto.subtle needs a secure context
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 560, height: 640 }, deviceScaleFactor: 2 });
page.on("console", (m) => console.error(`[page] ${m.text()}`));
page.on("pageerror", (e) => console.error(`[page error] ${e}`));
await page.route(`${ORIGIN}/**`, async (route) => {
  const hit = await lookup(new URL(route.request().url()).pathname);
  if (!hit) return route.fulfill({ status: 404 });
  return route.fulfill({ status: 200, contentType: hit.type, body: hit.body });
});
const romParam = (rom) => (rom ? `rom=/${rom}` : "");

if (mode === "demo") {
  const [out, rom] = rest;
  await page.goto(`${ORIGIN}/crates/web/page/?${romParam(rom)}`);
  await page.waitForTimeout(2500);
  for (const key of ["Enter", "KeyZ", "ArrowRight"]) {
    await page.keyboard.down(key);
    await page.waitForTimeout(150);
    await page.keyboard.up(key);
  }
  await page.waitForTimeout(1500);
  const error = await page.evaluate(() => window.__error ?? null);
  console.log(await page.locator("#status").textContent());
  await page.screenshot({ path: out });
  if (error) process.exitCode = 1;
} else if (mode === "bench") {
  const [rom, frames = "3000", reps = "5"] = rest;
  await page.goto(`${ORIGIN}/crates/web/page/?bench&frames=${frames}&reps=${reps}&${romParam(rom)}`);
  await page.waitForFunction(() => window.__bench || window.__error, null, { timeout: 600000 });
  const out = await page.evaluate(() => window.__bench ?? { error: window.__error });
  console.log(JSON.stringify({ browser: `chromium ${browser.version()}`, ...out }));
  if (out.error) process.exitCode = 1;
} else if (mode === "check") {
  // The browser's answer to check.mjs's request against the native build's.
  const [rom, nativeRef, frames = "600"] = rest;
  await page.goto(`${ORIGIN}/crates/web/page/?check&frames=${frames}&${romParam(rom)}`);
  await page.waitForFunction(() => window.__bench || window.__error, null, { timeout: 600000 });
  const web = await page.evaluate(() => window.__bench ?? { error: window.__error });
  if (web.error) throw new Error(web.error);
  const dir = await mkdtemp(join(tmpdir(), "gbcli-native-"));
  const native = JSON.parse(execFileSync(nativeRef, [dir, join(ROOT, rom), web.line, "-"], { encoding: "utf8" }));
  await rm(dir, { recursive: true, force: true });
  let ok = web.response_line === native.response_line;
  console.log(`rom ${rom} frames ${frames} in chromium ${browser.version()}`);
  console.log(`response lines  ${ok ? "identical" : "DIFFER"}`);
  for (const name of ["shot.png", "mix.wav"]) {
    const same = web.files[name] !== null && web.files[name] === native.files[name];
    ok &&= same;
    console.log(`${name.padEnd(9)} browser ${web.files[name]} native ${native.files[name]} ${same ? "equal" : "DIFFER"}`);
  }
  console.log(ok ? "PASS" : "FAIL");
  if (!ok) process.exitCode = 1;
} else {
  console.error("usage: browser.mjs demo OUT.png [ROM] | bench [ROM] [FRAMES] [REPS] | check ROM NATIVE_REF [FRAMES]");
  process.exitCode = 2;
}
await browser.close();
