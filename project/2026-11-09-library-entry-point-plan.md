# Plan: the library entry point (Stage 13)

2026-11-09. Ben chose this as the next work after the web spike closed ("1 (library) seems like something we should do regardless"), together with replacing rgbgfx (leaf `zkoPft`, filed separately under the root). It is the first step [2026-11-09-web-target.md](2026-11-09-web-target.md) ▸ When the work starts names, and the spike's Recommendation 3 in [2026-11-09-web-spike-findings.md](2026-11-09-web-spike-findings.md) ▸ Recommendation.

## Goal

A host that isn't a shell can drive gbcli in-process: a Rust agent tool first, a web page later. Concretely, three things exist when the stage closes:

- `gbcli-server`'s `Handler` is constructible from an explicit host, store and interrupt, with `Host::from_process` and the watchdog thread left to the caller.
- The session bundle sits behind a key-and-blob store trait with two backings: today's filesystem, and memory.
- Help, the primer, recipes and the schema are reachable as text from a library crate, not only by building argv.

The CLI and `gbcli serve` keep answering exactly as they do today. Nothing user-visible changes; this is a seam, not a feature.

## What is already true

- Every verb is a protocol op whose decision lives in a library crate, and `gbcli-server`'s `Handler::handle` answers them in-process. The CLI and `serve` are thin clients of it.
- `gbcli_cli::run(args, &Env, Io)` is a process-free entry point with explicit environment and streams; the binary is what builds `Env::from_process`.
- The CLI crate, help included, builds for `wasm32-wasip1` unchanged, and the spike ran real verbs through it (findings ▸ Help).
- The protocol types are shared, so a library caller compares and prints the same data the CLI does.

## What changes

Three pieces, each its own leaf below.

**1. The store trait.** Today `gbcli-bundle` is the filesystem: it holds paths (`Bundle::project`, `SessionStore::dir`), opens and renames files, takes `flock`, reads mtimes as last use, uses a per-process tag in temp names, and lists directories. The trait is at keys and blobs, below the bundle's decisions (`Bundle`, session creation, the history ring, checkpoints, pruning, events, the gaps ledger). Sketch from the findings:

- `get(key) -> Option<bytes>`
- `put(key, bytes)` — atomic replace
- `append(key, bytes)`
- `remove(key)`
- `list(prefix) -> keys`
- `lock(session) -> Guard`
- `touch(session, now)` and `last_used(session)`

The key is a path-like name under the bundle, and `now` comes from the host. The file backing decides how atomicity and locks are done (temp-and-rename, flock); the memory backing is a map, with no lock for one owner. Last use is the trait's concern, so the file backing may keep mtime under it. Incarnations become a counter or a random value, never a process id.

The tricky mapping, to settle in leaf 1: `SessionFile.rom` and `RomSource` are paths relative to the project, and a session's ROM is a project file, not a bundle blob. The memory backing must let a caller hand the ROM in as bytes and later read its header without a project directory, without changing the protocol type.

**2. The constructor and interrupt.** `Handler::new` today builds its own `Watchdog` thread and calls `Host::from_process()`. It should take the host, the store and an interrupt the caller controls; `from_process` and the watchdog belong to the CLI and `serve`. The wall guard stays as it is natively (`wall_guard_s`, PROTOCOL_VERSION 5): the native caller starts the watchdog around the interrupt; a host that cannot spawn a thread supplies its own.

**3. Help as an API.** `crates/cli/src/help/` is pure (`primer()`, `verb(&VerbSpec)`, the topics, `RECIPES`, `SCHEMA`) but private to the crate. The leaning is `pub mod help` rather than a new `gbcli-help` crate: the CLI crate already builds for wasm32 and links into the web spike at 543 KB gzipped, and the verb table the help depends on lives there too. Leaf 3 decides and says why.

## Order

The store trait first: it changes every call site in `gbcli-bundle`, and the constructor and the memory backing are written once against its final shape. Help is independent and can run beside it. The stage closes on a library-caller proof, which is also its acceptance.

## Out of scope

The web page, host-side persistence, the RGBDS plan-run-finish seam and rgbgfx's web answer: the web track is paused (findings ▸ Ben's decisions). Embedded scripting stays parked. No new verb, no protocol change, no behavior change to the CLI or `serve`.

## Risks

- **The refactor touches everything in the bundle.** Its tests already pin crash consistency, the history ring, locks and pruning, so they are the safety net; a change to an expected behavior has to be argued, not absorbed.
- **A public interface.** The trait shape is expensive to change once callers exist. The findings fixed its level; leaf 1 settles the remaining details and records them in `docs/architecture.md`.
- **One session may not be enough** for leaf 1. The agent works as far as it can and reports; a follow-up rides with the next leaf rather than a second agent.

## Tasks

Imported 2026-11-09 under the root `9b5RBA`; the tracker is the plan from here.

```yaml
tasks:
  - title: 'Stage 13: The library entry point'
    ref: stage13
    desc: |
      Why: a host that isn't a shell must be able to drive gbcli in-process — a Rust agent tool first, a web page later (Ben, 2026-11-09; project/2026-11-09-web-target.md). The web spike showed the handler is nearly there and named the three gaps (project/2026-11-09-web-spike-findings.md ▸ Recommendation). This stage closes them without changing anything a user sees.

      What: `gbcli-server`'s Handler is built from an explicit host, store and interrupt; the session bundle sits behind a key-and-blob store trait with a filesystem backing (today's behavior) and a memory backing; help, the primer, recipes and the schema are reachable from a library crate. Closing proof: a test drives a session end-to-end through `Handler::handle` with an in-memory store, an explicit host and no subprocess.
    children:
      - title: 'The bundle store trait, with the filesystem as its first backing'
        ref: store
        desc: |
          Why: the bundle is the filesystem today — paths, open/rename, flock, mtimes as last use, a per-process temp tag, readdir — so a memory store, or a browser's storage, has nothing to implement. This leaf puts every storage decision behind a key-and-blob trait and moves today's behavior into a filesystem backing unchanged. It is the spine of the stage: the constructor and the memory backing in the next leaf are written against it.

          What: a store trait in `gbcli-bundle`, at keys and blobs, below the bundle's decisions (Bundle, session creation, the history ring, checkpoints, pruning, events, the gaps ledger, listing, the ROM stamp). The operations the findings name: `get(key) -> Option<bytes>`, `put(key, bytes)` (atomic replace), `append(key, bytes)`, `remove(key)`, `list(prefix) -> keys`, `lock(session) -> Guard`, `touch(session, now)` and `last_used(session)`. A filesystem backing implements it with exactly today's semantics: temp-and-rename, flock, the lock file's mtime as last use, and directory listing. Incarnations come from a counter or a random value inside the store, never `std::process::id()`.

          Already decided: the trait's level and shape sketch are in project/2026-11-09-web-spike-findings.md ▸ What the bundle store needs to become; the ruling that a library crate's decision takes data rather than a path it opens is project/2026-11-09-web-target.md ▸ What the ruling asks of new work, item 1. File access stays at the edges; the backing is the edge.

          Seams: `crates/bundle/src/{bundle,session,store,history,checkpoints,events,prune,listing,lock,stamp,boot,io}.rs`. `Bundle` must stay non-generic above the trait (a trait object, or an enum of the two backings) so `Handler`, the CLI and `serve` do not grow generics; the agent chooses and says why. Update `docs/architecture.md` with where the trait sits.

          Out of scope: the memory backing (next leaf), the Handler constructor, help. No protocol type changes: `SessionFile.rom` and `RomSource` keep their shapes.

          Open, the agent's call: the exact key layout; whether `put` is atomic by contract or the backing's choice; how `touch`/`last_used` are stored for the file backing. The one wrinkle to settle and record: a session's ROM is a project file, not a bundle blob, so the memory backing must accept ROM bytes and answer the header read later without a project directory, without changing the protocol type.

          Risks: the refactor touches every module in the crate. The existing tests pin crash consistency, the history ring, locks, pruning and listing; they are the safety net, and a changed expectation must be argued in the report, not absorbed. One session may not finish; report where it stopped and let the remainder ride with the next leaf.
        criteria:
          - '`cargo test -p gbcli-bundle` passes, with no test expectation changed except where a constructor now names a backing'
          - 'file I/O (`std::fs`, `File`, `rename`, `read_dir`) is confined to the filesystem backing and `io.rs`; no bundle decision module opens a path itself'
          - '`cargo test` for the server and CLI goldens passes unchanged, so `serve` and the CLI answer as before'
          - 'the trait, the key layout and the ROM wrinkle are recorded in `docs/architecture.md`'
      - title: 'A memory store, and a Handler built from a host, a store and an interrupt'
        ref: handler
        blockedBy: [store]
        desc: |
          Why: a caller with no filesystem — a Rust tool with throwaway sessions, a web page with browser storage — needs the bundle in memory; and `Handler::new` today builds its own watchdog thread and calls `Host::from_process()`, which a host that isn't a process cannot supply (findings ▸ The host and the watchdog). This leaf adds both, and moves `from_process` to the edges.

          What: a memory backing for the store trait (a map, no lock for one owner, last use a field) that can create a session from ROM bytes; and `Handler::new` taking the host, the store and an interrupt the caller controls, with `Host::from_process()` called only by the CLI binary and `serve`. The native path still starts the watchdog thread around the interrupt, so `wall_guard_s` (PROTOCOL_VERSION 5) keeps its behavior; a caller that starts no watchdog still answers short requests.

          Already decided: the interrupt replaces the thread in the core path, not the wall guard itself (findings ▸ How the handler gets a host without a process, and runs without the watchdog thread); the ruling that no new feature may need a subprocess, a thread, a socket or the system clock in its core path is project/2026-11-09-web-target.md item 2.

          Seams: `crates/server/src/handle/mod.rs` (`Handler::new`, `set_host`), `crates/server/src/watchdog.rs`, `crates/bundle` (the memory backing), and the two call sites that build a handler (the CLI's `start_handler` and `gbcli serve`). The `Host` type already exists; pass it, don't re-derive it.

          Out of scope: help, the closing proof, host-side persistence of the memory store. No behavior change to any request.
        criteria:
          - 'a session in a memory store is created from ROM bytes, runs, takes a shot, checkpoints, restores and undoes, and nothing on that path touches the filesystem'
          - '`Handler::new` takes the host, the store and the interrupt; `Host::from_process()` appears only in the CLI binary and `serve`'
          - 'the CLI and `serve` goldens pass unchanged, and a native `run` still enforces `wall_guard_s`'
      - title: 'Help, the primer, recipes and the schema as a public API'
        ref: help
        desc: |
          Why: an agent tool that calls the handler still needs the help as text, and a library caller must not build argv to get it (findings ▸ What the help needs to become reachable; project/2026-11-09-web-target.md item 6). The functions are pure but private to `gbcli-cli`.

          What: `primer()`, `verb(&VerbSpec)`, the help topics, `RECIPES` and `SCHEMA` are reachable from a library crate without argv, a CLI `Env` or a process. The leaning is `pub mod help` on `gbcli-cli`, not a new crate: the CLI crate already builds for wasm32 and links into the web spike (543 KB gzipped), and the verb table the help depends on lives there. The agent decides and says why; a new crate is only right if a caller genuinely cannot take the CLI dependency.

          Already decided: the protocol stays the one interface, so no `help` protocol op is added for this (project/2026-11-09-web-target.md item 5).

          Seams: `crates/cli/src/help/`, `crates/cli/src/lib.rs` (`mod help` at line 11), `crates/cli/src/verbs.rs`. The CLI's own rendering must not change.

          Out of scope: a `help` op for JSONL clients; moving help out of the CLI crate into a new crate unless the leaf argues it.
        criteria:
          - 'a library caller reads the primer, a help topic, help for a verb, `RECIPES` and `SCHEMA` with no argv and no `Env`'
          - 'the CLI renders help byte-identically to today, by the existing goldens'
      - title: 'Closing: a library caller drives a session with no CLI and no subprocess'
        ref: proof
        blockedBy: [handler, help]
        desc: |
          Why: the stage exists so a host that isn't a shell can drive gbcli. This leaf is the proof and the stage's acceptance: it exercises the seams end to end, the way a Rust agent tool would, and documents the entry point for the next caller. Without it the other leaves could all pass and the composition still not work.

          What: a test (or a small example plus a test) that builds `Handler` with an explicit `Host`, an in-memory store and a caller-supplied interrupt, then drives one session entirely through `Handler::handle`: create it from ROM bytes, run frames with keys, read memory, take a shot, checkpoint, restore, undo. It must not use the CLI, a subprocess, `PATH` or a project directory. Its doc comment is the documented entry point.

          Already decided: the protocol is the one interface (project/2026-11-09-web-target.md item 5); a Rust caller calls `Handler::handle` with typed requests and responses.

          Seams: `crates/server/tests/` or `crates/server/examples/`; whichever keeps `cargo test --workspace` under its 30 s warm budget. Use an existing test ROM from `crates/*/tests/roms/`, read as bytes.

          Out of scope: the web build; flash carts; the build verb in memory.
        criteria:
          - 'one test drives a session from creation through run, read, shot, checkpoint, restore and undo through `Handler::handle` alone'
          - 'the test uses no CLI, no subprocess and no project directory, and passes with the host naming no `PATH`'
          - 'the entry point and its required construction are documented where a library caller will find them'
```
