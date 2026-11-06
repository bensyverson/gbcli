# Backlog

Work decided against, parked rather than dropped in silence. Nothing here is scheduled or blocking — active work lives in `job`. Read it before proposing something that sounds novel; it may already have been weighed and parked.

- **One dated H2 per item:** what it is, why it's parked, and *what would un-park it*.
- **Delete an item when it lands, or when it stops being plausible.** A long list is one nobody reads.
- **Something parked that turns out to be needed becomes a task in `job`** — move it, don't work it from here.

Format: one dated H2 per entry, a headline, then what it is, why it's parked, and the trigger that would revive it.

---

## 2026-11-06 — GBDK (C) support in `build`

What: `build` and `new` support for GBDK-2020 projects, so game logic can be written in C around hand-tuned assembly. Why parked: the target use case is extreme DMG performance, which is an assembly job; RGBDS comes first (user ruling, 2026-11-06). Un-park when: the RGBDS loop has shipped and a user wants C, or the agent eval shows agents need C for non-hot-path logic.

## 2026-11-06 — docboy as the emulator backend

What: wrap docboy (MIT, C++17) instead of SameBoy. Why parked: lost the bake-off. It may be slightly more accurate, but it is 3.5× slower (6.2× vs 21.8× realtime, measured), selects the model at compile time, keeps state private behind friend classes, has an unversioned state format, and its debugger blocks on `std::cin`. See project/2026-11-06-backend-evaluation.md. Un-park when: SameBoy shows a fidelity gap on a DMG edge our users hit that docboy gets right, or as a third opinion when a bespoke core and SameBoy disagree.

## 2026-11-06 — MiSTer, GateBoy and PyBoy as backends

What: the other candidates from the original brief. Why parked: MiSTer has a T80 CPU, a ModelSim-only simulator, no debugger and no accuracy advantage; GateBoy has a non-die-traced CPU, known test failures, MBC1 only and no license; PyBoy's save states are exact only at frame boundaries and it has no watchpoints. Details in project/2026-11-06-backend-evaluation.md. Un-park when: one of them publishes results beating SameBoy on the DMG tests we care about (GateBoy would also need a license).
