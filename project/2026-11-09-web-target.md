# Decision: design for a web build of gbcli, and for gbcli as a library

2026-11-09, seventeenth session. **Ruling (Ben): a web IDE that embeds gbcli is a real goal, though not an immediate one, so nothing new may be designed in a way that can't work there.** Ben expects SameBoy and RGBDS to compile to WebAssembly, and if either doesn't, to be tractable to replace. No WASM work is scheduled; this doc says what the ruling asks of work done before then.

**Second ruling, same day (Ben): everything the CLI does must eventually be exposed as a Rust library that an agent loop can call as a tool.** Not needed now, but nothing may be designed so that only the CLI can reach it. It shares the web build's shape: a host that isn't a shell, calling gbcli in-process.

## Why now

Lantern's bot (examples/raycaster/tools/bot.py) settled whether driving gbcli from outside is fast enough: 108 calls, about 4 s for a win. Ben then asked what happens in a web IDE, where there is no Python and no shell. The question that matters there is where bot code, and anything else that drives the emulator, runs: it has to be a language and an API the browser can host.

## What carries over already

- **The library is nearly there.** The CLI answers every verb in-process through `gbcli-server`'s `Handler`, with typed requests and responses from `gbcli-protocol`. An agent loop could call `Handler::handle` today. What's missing is a deliberate, documented entry point for it, not a new layer.
- **The protocol is the seam.** Library crates hold every decision. The CLI and `gbcli serve` turn argv or JSON Lines into one typed request and print its response. In WASM, a page would call the same handler in-process from JavaScript, with no process spawned per call.
- **Determinism.** Runs depend only on the ROM, the inputs and the seed, never on wall time, so a web run replays like a CLI run.

## What doesn't, today (known debt, not to be fixed for its own sake)

- **Some of what an agent needs lives only in the CLI crate:** the help topics, recipes, primer and per-verb help (`crates/cli/src/help/`), and the rendering of results as rows. A tool-calling agent needs the help as text it can be given, and the results as data, which the protocol already is.
- **The host comes from the process:** `Host::from_process` reads the working directory and `PATH` (`crates/server/src/handle/build.rs`). That's fine at the edge, but a library caller must be able to pass its own.

- **The build runs RGBDS as subprocesses** (`crates/build/src/toolchain.rs`, `project/tool.rs`), and `doctor` reads `PATH` and git.
- **The bundle store is the filesystem** (`.gbcli/` sessions, checkpoints, caches, `crates/bundle`), with file locks (`crates/bundle/src/lock.rs`).
- **Font authoring's save decides on files** (`crates/build/src/font_author/`): which glyphs come from the working copy and which comments and properties from the `.yaff` is interleaved with reading and writing them, and the newer-copy check reads file times. The conversions themselves (`gbcli-font`) are data in, data out. Found by review on 2026-11-09 (leaf 2xiOiv); Resolved by that fix: `base::drift` and `save::merged` take data, and only the file reads and writes stay around them (the hash replaced file times).
- **`serve`'s watchdog is a thread** (`crates/server/src/watchdog.rs`).
- **Example tools and bots are Python.** That's fine for examples, but it means a bot isn't portable to a web host.

## What the ruling asks of new work

1. **A library crate's new decision takes its inputs as data, not as a process or a path it opens itself.** File access stays at the edges: the CLI, `serve`, or a store type that a web build can back with memory or browser storage. Reading the file and deciding about its contents are separate functions, as the font, pixels and song readers already are.
2. **No new feature may need a subprocess, a thread, a socket, or the system clock in its core path.** Where an external tool is unavoidable (RGBDS), it stays behind one seam that a WASM build could swap for an in-process build of the same tool.
3. **New dependencies must build for `wasm32`.** Check before proposing one; say so in the ask.
4. **Anything that drives the emulator from a script must be usable from a browser.** A feature whose only client is a Python or shell script isn't enough on its own. This weighs on the parked embedded-scripting entry (backlog.md ▸ Embedded scripting for input bots), whose un-park condition now names the web.
5. **The protocol stays the one interface.** A web binding, or a Rust tool in an agent loop, is another thin client of it, never a second API.
6. **A new verb is a protocol op first.** Its decision lives in a library crate and its request and result are protocol types; the CLI only parses argv and renders rows. Anything an agent needs to use a verb (its help, a remedy) is text a library caller can reach, not something only the CLI prints. Help moving out of the CLI crate is debt for when the library entry point is built, not for now.

## When the work starts

- **The library:** a documented entry point (a `Handler` built with an explicit host and store), and the help reachable from a library crate.
- **The web:** first a time-boxed spike, with the engine, SameBoy and the protocol handler built for `wasm32` behind an in-memory store, running a ROM headless from JavaScript. Then RGBDS in WASM, or a plan to replace it. The spike's findings decide the store's design and the scripting question.
