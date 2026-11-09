// Web spike (Stage 12, leaf 0Mfl87): runs the benchmark on every host, in
// interleaved rounds, so machine load hits them alike, and prints each
// host's figures per round and the median across rounds. Expects
// scripts/web/build-spike.sh to have built the .wasm and native_bench, and
// the page's npm dependencies and Playwright's Chromium (findings doc ▸ The
// page). Run from the repo root:
//
//   node scripts/web/bench-spike.mjs [ROM] [ROUNDS] [FRAMES] [REPS]
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { loadavg, tmpdir } from "node:os";
import { join } from "node:path";

const [rom = "crates/sameboy/tests/roms/layers.gb", rounds = "3", frames = "3000", reps = "5"] = process.argv.slice(2);
const WASM = "target/wasm32-wasip1/release/gbcli_web.wasm";
const env = { ...process.env, PLAYWRIGHT_BROWSERS_PATH: join(process.cwd(), "target/web-spike/ms-playwright") };
const json = (cmd, args) => JSON.parse(execFileSync(cmd, args, { encoding: "utf8", env, stdio: ["ignore", "pipe", "ignore"] }).trim().split("\n").pop());

const hosts = {
  native: () => {
    // Outside the repo: a bundle search from inside it finds the checkout's .gbcli/.
    const dir = mkdtempSync(join(tmpdir(), "gbcli-native-bench-"));
    try {
      return json("target/release/examples/native_bench", [join(dir, "p"), rom, frames, reps]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  },
  "node:wasi": () => json("node", ["--no-warnings", "crates/web/page/bench.mjs", WASM, rom, "node", frames, reps]),
  "node+shim": () => json("node", ["--no-warnings", "crates/web/page/bench.mjs", WASM, rom, "shim", frames, reps]),
  chromium: () => json("node", ["crates/web/page/browser.mjs", "bench", rom, frames, reps]),
};

const results = Object.fromEntries(Object.keys(hosts).map((h) => [h, []]));
for (let r = 0; r < Number(rounds); r++) {
  for (const [name, run] of Object.entries(hosts)) {
    const before = loadavg()[0].toFixed(1);
    const out = run();
    results[name].push(out);
    console.log(
      `round ${r + 1} ${name.padEnd(10)} load ${before.padStart(5)}  fps ${out.throughput.fps_median}` +
        `  startup ${out.startup_ms.total} ms  per-request ${out.interactive.per_request_ms} ms`,
    );
  }
}
const median = (xs) => [...xs].sort((a, b) => a - b)[xs.length >> 1];
console.log(`\nmedian of ${rounds} rounds (${rom}, ${frames} frames x ${reps} reps per round):`);
for (const [name, outs] of Object.entries(results)) {
  const m = (f) => median(outs.map(f));
  const s = outs[0].startup_ms;
  console.log(
    `${name.padEnd(10)} fps ${m((o) => o.throughput.fps_median)}  startup ${m((o) => o.startup_ms.total)} ms` +
      (s.compile !== undefined
        ? ` (compile ${m((o) => o.startup_ms.compile)}, instantiate ${m((o) => o.startup_ms.instantiate)}, open ${m((o) => o.startup_ms.open)}, first frame ${m((o) => o.startup_ms.first_frame)})`
        : ` (open ${m((o) => o.startup_ms.open)}, first frame ${m((o) => o.startup_ms.first_frame)})`) +
      `  per-request ${m((o) => o.interactive.per_request_ms)} ms`,
  );
}
