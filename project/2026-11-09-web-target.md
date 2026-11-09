# Decision: design for a web build of gbcli

2026-11-09, seventeenth session. **Ruling (Ben): a web IDE that embeds gbcli is a real goal, though not an immediate one, so nothing new may be designed in a way that can't work there.** Ben expects SameBoy and RGBDS to compile to WebAssembly, and if either doesn't, to be tractable to replace. No WASM work is scheduled; this doc says what the ruling asks of work done before then.

## Why now

Lantern's bot (examples/raycaster/tools/bot.py) settled whether driving gbcli from outside is fast enough: 108 calls, about 4 s for a win. Ben then asked what happens in a web IDE, where there is no Python and no shell. The question that matters there is where bot code, and anything else that drives the emulator, runs: it has to be a language and an API the browser can host.

## What carries over already

- **The protocol is the seam.** Library crates hold every decision. The CLI and `gbcli serve` turn argv or JSON Lines into one typed request and print its response. In WASM, a page would call the same handler in-process from JavaScript, with no process spawned per call.
- **Determinism.** Runs depend only on the ROM, the inputs and the seed, never on wall time, so a web run replays like a CLI run.

## What doesn't, today (known debt, not to be fixed for its own sake)

- **The build runs RGBDS as subprocesses** (`crates/build/src/toolchain.rs`, `project/tool.rs`), and `doctor` reads `PATH` and git.
- **The bundle store is the filesystem** (`.gbcli/` sessions, checkpoints, caches, `crates/bundle`), with file locks (`crates/bundle/src/lock.rs`).
- **`serve`'s watchdog is a thread** (`crates/server/src/watchdog.rs`).
- **Example tools and bots are Python.** That's fine for examples, but it means a bot isn't portable to a web host.

## What the ruling asks of new work

1. **A library crate's new decision takes its inputs as data, not as a process or a path it opens itself.** File access stays at the edges: the CLI, `serve`, or a store type that a web build can back with memory or browser storage. Reading the file and deciding about its contents are separate functions, as the font, pixels and song readers already are.
2. **No new feature may need a subprocess, a thread, a socket, or the system clock in its core path.** Where an external tool is unavoidable (RGBDS), it stays behind one seam that a WASM build could swap for an in-process build of the same tool.
3. **New dependencies must build for `wasm32`.** Check before proposing one; say so in the ask.
4. **Anything that drives the emulator from a script must be usable from a browser.** A feature whose only client is a Python or shell script isn't enough on its own. This weighs on the parked embedded-scripting entry (backlog.md ▸ Embedded scripting for input bots), whose un-park condition now names the web.
5. **The protocol stays the one interface.** A web binding is another thin client of it, never a second API.

## When the web work starts

The first step is a time-boxed spike: the engine, SameBoy and the protocol handler built for `wasm32` behind an in-memory store, running a ROM headless from JavaScript. Then RGBDS in WASM, or a plan to replace it. The spike's findings decide the store's design and the scripting question.
