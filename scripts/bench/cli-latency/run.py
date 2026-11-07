#!/usr/bin/env python3
"""Wall time of one CLI request against a live session: process start, argv
parsing, bundle open, session lock, head load, the request, the reply.

Builds the release binary, makes a temporary project with the committed V1
test ROM, creates a session and runs it one frame so it has a head, then
times N invocations of each request and prints min, median and p95 in ms.

    python3 -I scripts/bench/cli-latency/run.py [N]     # N per request, default 200
"""

import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
ROM = os.path.join(REPO, "crates", "engine", "tests", "roms", "rebuild_v1.gb")
SYM = os.path.join(REPO, "crates", "engine", "tests", "roms", "rebuild_v1.sym")
REQUESTS = [
    ["version"],
    ["show", "--session", "bench"],
    ["read", "u16[wFrame]", "--session", "bench"],
    ["step", "--session", "bench"],
]


def main() -> None:
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 200
    subprocess.run(
        ["cargo", "build", "--quiet", "--release", "-p", "gbcli"], cwd=REPO, check=True
    )
    binary = os.path.join(REPO, "target", "release", "gbcli")
    env = {k: v for k, v in os.environ.items() if not k.startswith("GBCLI_")}
    project = tempfile.mkdtemp(prefix="gbcli-cli-latency-")
    try:
        shutil.copy(ROM, os.path.join(project, "demo.gb"))
        shutil.copy(SYM, os.path.join(project, "demo.sym"))
        setup = [binary, "run", "demo.gb", "--session", "bench", "--frames", "1"]
        subprocess.run(setup, cwd=project, env=env, check=True, capture_output=True)
        print(f"host: {platform.platform()}, {platform.machine()}, {os.cpu_count()} CPUs")
        print(f"load: {os.getloadavg()[0]:.2f}  n={n} per request")
        for args in REQUESTS:
            samples = []
            for _ in range(n):
                start = time.perf_counter()
                done = subprocess.run(
                    [binary, *args], cwd=project, env=env, capture_output=True
                )
                samples.append((time.perf_counter() - start) * 1000)
                if done.returncode != 0:
                    sys.exit(f"{args}: exit {done.returncode}: {done.stderr.decode()}")
            samples.sort()
            p95 = samples[int(len(samples) * 0.95) - 1]
            print(
                f"gbcli {' '.join(args):<40} min {samples[0]:6.2f}  "
                f"median {statistics.median(samples):6.2f}  p95 {p95:6.2f} ms"
            )
    finally:
        shutil.rmtree(project, ignore_errors=True)


if __name__ == "__main__":
    main()
