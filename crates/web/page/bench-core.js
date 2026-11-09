// Web spike (Stage 12, leaf 0Mfl87): the one benchmark the browser page
// (?bench) and Node (bench.mjs) both run, so their figures come from the
// same code. Spike code: disposable.
//
// Start-up: compile the module's bytes, instantiate it, put the ROM, open
// the session, and run to the first frame with a shot read back as PNG
// bytes: what a page waits for before it can draw.
// Throughput: after a warm-up run, `reps` runs of `frames` frames each with
// no shot or audio; fps is frames over each run's wall time (median, min and
// max reported). Interactive: `loop` runs of one frame with a shot each, the
// request a page sends per animation frame.

const median = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};
const round = (x, d = 1) => Math.round(x * 10 ** d) / 10 ** d;

export async function benchmark({ wasmBytes, instantiate, rom, frames = 3000, reps = 5, warmup = 600, loop = 300, now }) {
  const t0 = now();
  const module = await WebAssembly.compile(wasmBytes);
  const t1 = now();
  const gbw = await instantiate(module);
  const t2 = now();
  gbw.putFile("/work/rom.gb", rom);
  gbw.open({ dir: "/work", session: "web", rom: "rom.gb" });
  const t3 = now();
  const first = gbw.request({ id: "f", op: "run", args: { frames: 1, shot: { path: "f.png", scale: 1 } } });
  if (!first.ok) throw new Error(JSON.stringify(first));
  const png = gbw.getFile("/work/f.png");
  if (!png) throw new Error("no first frame");
  const t4 = now();

  const warm = gbw.request({ id: "w", op: "run", args: { frames: warmup } });
  if (!warm.ok) throw new Error(JSON.stringify(warm));

  const fps = [];
  for (let i = 0; i < reps; i++) {
    const a = now();
    const r = gbw.request({ id: `r${i}`, op: "run", args: { frames } });
    const b = now();
    if (!r.ok || r.result.stop.budget.frames !== frames) throw new Error(JSON.stringify(r));
    fps.push(frames / ((b - a) / 1000));
  }

  const la = now();
  for (let i = 0; i < loop; i++) {
    const r = gbw.request({ id: `l${i}`, op: "run", args: { frames: 1, shot: { path: "f.png", scale: 1 } } });
    if (!r.ok) throw new Error(JSON.stringify(r));
    gbw.getFile("/work/f.png");
  }
  const lb = now();

  return {
    startup_ms: {
      compile: round(t1 - t0),
      instantiate: round(t2 - t1),
      open: round(t3 - t2),
      first_frame: round(t4 - t3),
      total: round(t4 - t0),
    },
    first_png_bytes: png.length,
    throughput: { frames, reps, fps_median: round(median(fps)), fps_min: round(Math.min(...fps)), fps_max: round(Math.max(...fps)) },
    interactive: { requests: loop, per_request_ms: round((lb - la) / loop, 2), fps: round(loop / ((lb - la) / 1000)) },
  };
}
