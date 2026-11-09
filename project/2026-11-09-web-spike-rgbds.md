# Finding: RGBDS on the web (web spike, leaf LMjWWK)

2026-11-09, nineteenth session, agent rgbweb (Sonnet), research only by Ben's ruling ([2026-11-09-web-spike-plan.md](2026-11-09-web-spike-plan.md)). Spike 2 folds this into the web spike's findings doc.

**Short answer: unpatched RGBDS 1.0.4 builds to wasm with Emscripten and runs.** rgbasm, rgblink and rgbfix together are about 0.9 MB raw and 0.37 MB gzip. rgbgfx isn't shipped for the web by anyone, and gbcli shouldn't need it there: its own image pipeline is Rust (the `png` crate builds for `wasm32`), so the open question is whether the web build reproduces rgbgfx's decisions in Rust. That is Ben's call.

## How rgbds-live does it (read)

- **The repo is [gbdev/rgbds-live](https://github.com/gbdev/rgbds-live)** (daid's original URL redirects), MIT. Cloned at a8eb8bb (2026-10-21). Its `rgbds` submodule is exactly v1.0.4, the version gbcli pins.
- **Toolchain:** Emscripten 4.0.8 ([build.yml](https://github.com/gbdev/rgbds-live/blob/master/.github/workflows/build.yml)), CMake through `emcmake` with upstream's own CMake ([CMakeLists.txt](https://github.com/gbdev/rgbds-live/blob/master/CMakeLists.txt)), building only `rgbasm rgblink rgbfix`. Compile flags `-O3 -flto -msimd128 -ffast-math -DNDEBUG`. Link flags `-sEXPORT_ES6=1 -sALLOW_MEMORY_GROWTH=1 -sENVIRONMENT=web -sMODULARIZE=1 -sEXPORTED_RUNTIME_METHODS=['FS']`, with one factory per tool.
- **Its one patch isn't a port:** [patches/rgbds.patch](https://github.com/gbdev/rgbds-live/blob/master/patches/rgbds.patch) (60 lines) emits hidden `__SEC_<n>_<line>_<file>` labels so the IDE can map ROM addresses to source lines. Nothing in RGBDS needed porting.
- **Files go through Emscripten's in-memory filesystem (MEMFS)** ([js/compiler.js](https://github.com/gbdev/rgbds-live/blob/master/js/compiler.js)). Each tool run is a fresh module instance: `preRun` writes every project file, `main` runs at instantiation, and `FS.readFile` reads the output. There is no `main()` re-entry and no global state to reset. An instance costs a few milliseconds.
- **Exit handling is weak:** with the default `-sEXIT_RUNTIME=0` the exit code isn't observable, so failure is detected by the output file being missing, and diagnostics are scraped from stderr.
- **No worker and no timeout** on a stuck tool. rgbgfx isn't built; the editor draws graphics in its own JS.

## Upstream RGBDS (read)

No official wasm or Emscripten support: nothing in the 1.0.4 sources, no wasm release asset, and no issue or PR found by a `gh` search and a web search, so this could miss something. Since 1.0 the build is C++20. It needs bison 3.0 or later at build time, and libpng and zlib for rgbgfx only.

## Verified: a build of 1.0.4 (run by rgbweb)

- **Built with Emscripten 7.0.0 (Homebrew) without CMake:** upstream's CMake always fetches zlib and libpng, the zlib URL returned 404, and rgbds-live's shim didn't stop that with CMake 4.4. The script is [scripts/web/build-rgbds-wasm.sh](../scripts/web/build-rgbds-wasm.sh): bison, then `em++ -O3 -flto -std=c++20` over upstream's source lists, linked with `-sEXIT_RUNTIME=1`.
- **Checked by [scripts/web/test-rgbds-wasm.mjs](../scripts/web/test-rgbds-wasm.mjs) in Node, through MEMFS:** good source gives exit 0 and a 188-byte `.o`. Bad source gives exit 1 with upstream-format diagnostics on stderr. rgblink writes a 16 KB ROM, `.sym` (`-n`) and `.map` (`-m`). rgbfix gives a 32 KB ROM and its `-Woverwrite` warning. A second rgbasm on a fresh instance works.
- **`-sEXIT_RUNTIME=1` is required:** only with it does `onExit(code)` report the real status.
- **Sizes** (bytes, `-O3 -flto`, no SIMD or fast-math, default exit handling; with `-sEXIT_RUNTIME=1` raw is 449,020 / 296,140 / 157,091):

  | Tool | raw | gzip -9 | brotli |
  |---|---|---|---|
  | rgbasm | 442,036 | 177,689 | 142,748 |
  | rgblink | 290,771 | 108,542 | 88,050 |
  | rgbfix | 155,606 | 62,180 | 51,604 |

  The live rgbds-live site's wasm is the same size class: 456,682 / 327,924 / 183,727 raw.
- **Not tested:** rgbgfx, a real browser or Worker, rgbds-live's SIMD and fast-math flags.

## What gbcli's build seam would need (rgbweb's proposal, for the decision leaf)

- **One seam below the build's decisions:** a runner that takes a tool, its args and its input files as data and returns the exit code, stderr and output files. Native runs it as a subprocess in a temp dir; the web host runs the wasm tools. Staging, the asm/link/fix order and diagnostics parsing stay shared. Today `crates/build/src/project/tool.rs` mixes the runner with `Command`, a stderr thread, a clock deadline and process-group kill, which is exactly what the web ruling keeps out of a core path.
- **The Rust core can't instantiate the tools itself:** a `wasm32` Rust module can't load another wasm module, so the JS host provides the runner. The API is then async, or one host call runs the whole asm, link and fix chain.
- **Timeouts belong to the host:** a Web Worker per tool, terminated on a deadline. A runaway macro loop would otherwise block the page.
- **Inputs are the whole project tree** (every `INCLUDE`/`INCBIN`, include paths, hardware.inc), since rgbasm reads files itself. The toolchain's `--version` probe becomes a fixed pinned version on the web.
- **rgbgfx:** reproduce its decisions in Rust (gbcli already checks them after the fact, `agree.rs`), or offer no `assets` verb on the web. Ben's call.

## Side effects on the machine

`brew install emscripten cmake bison` upgraded several existing Homebrew formulas as dependencies, including **node and python@3.15**. No project files were touched.
