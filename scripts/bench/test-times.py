#!/usr/bin/env python3
"""Times the warm test suite: each test binary, then each test in the slow ones.

Usage: python3 -I scripts/bench/test-times.py [--per-test-over SECONDS] [-- CARGO_ARGS...]

Builds the tests first (`cargo test --no-run`), so the figures exclude compile
time. Each binary runs as `cargo test` would run it, with the default thread
count. For every binary slower than --per-test-over (default 2 s), each test
is then run alone, one at a time, to find the slow ones; its ignored
(pre-release) tests are skipped, as the warm suite skips them. Needs the Bash
sandbox off (cargo's registry cache lives outside it) and the corpus fetched.
"""

import argparse
import json
import os
import resource
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def package(package_id):
    """The package name in a package id: `path+file:///…/crates/build#gbcli-build@0.1.0`,
    or `path+file:///…/crates/gbcli#0.1.0` when the name is the directory's."""
    path, _, fragment = package_id.partition("#")
    if "@" in fragment:
        return fragment.split("@")[0]
    return path.rstrip("/").rsplit("/", 1)[-1]


def executables(cargo_args):
    out = subprocess.run(
        ["cargo", "test", "--workspace", "--no-run", "--message-format=json", *cargo_args],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    found = []
    for line in out.splitlines():
        msg = json.loads(line)
        if msg.get("reason") == "compiler-artifact" and msg.get("executable") and msg["profile"]["test"]:
            target = msg["target"]
            pkg = package(msg["package_id"])
            found.append((f"{pkg}::{target['name']}", msg["executable"], msg["manifest_path"]))
    return found


def listed(exe, cwd, *extra):
    """The binary's test names (`--ignored`: only its ignored ones)."""
    out = subprocess.run([exe, "--list", "--format=terse", *extra], cwd=cwd,
                         capture_output=True, text=True).stdout
    return [l.rsplit(": ", 1)[0] for l in out.splitlines() if l.endswith(": test")]


def cpu():
    r = resource.getrusage(resource.RUSAGE_CHILDREN)
    return r.ru_utime + r.ru_stime


def timed(cmd, cwd):
    """Wall and CPU (user + system) seconds. CPU is the steadier figure on a
    loaded machine; wall is what the feedback loop feels."""
    start, start_cpu = time.monotonic(), cpu()
    proc = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    return time.monotonic() - start, cpu() - start_cpu, proc


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--per-test-over", type=float, default=2.0)
    ap.add_argument("cargo_args", nargs="*")
    args = ap.parse_args()

    rows = []
    for name, exe, manifest in executables(args.cargo_args):
        cwd = os.path.dirname(manifest)  # cargo runs tests from the package root
        secs, cpu_secs, proc = timed([exe, "-q"], cwd)
        if proc.returncode != 0:
            print(f"FAILED {name}\n{proc.stdout[-2000:]}{proc.stderr[-2000:]}", file=sys.stderr)
        rows.append((secs, cpu_secs, name, exe, cwd))

    rows.sort(reverse=True)
    total = sum(r[0] for r in rows)
    total_cpu = sum(r[1] for r in rows)
    print(f"load average at start: {os.getloadavg()[0]:.1f} on {os.cpu_count()} CPUs")
    print(f"binaries: {len(rows)}, sum of wall {total:.1f} s, sum of CPU {total_cpu:.1f} s"
          " (cargo runs binaries one after another)")
    print("    wall       CPU  binary")
    for secs, cpu_secs, name, _, _ in rows:
        print(f"{secs:8.2f} s {cpu_secs:7.2f} s  {name}")

    for secs, _, name, exe, cwd in rows:
        if secs < args.per_test_over:
            continue
        # The warm suite skips ignored (pre-release) tests, so this does too.
        ignored = set(listed(exe, cwd, "--ignored"))
        tests = [t for t in listed(exe, cwd) if t not in ignored]
        per = []
        for t in tests:
            s, c, _ = timed([exe, "--exact", t, "-q"], cwd)
            per.append((s, c, t))
        per.sort(reverse=True)
        print(f"\n{name} ({secs:.1f} s, {len(tests)} tests, {len(ignored)} ignored not timed),"
              " each run alone (wall, CPU):")
        for s, c, t in per:
            print(f"{s:8.2f} s {c:7.2f} s  {t}")


if __name__ == "__main__":
    main()
