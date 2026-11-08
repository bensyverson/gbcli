#!/usr/bin/env python3
"""Times the first and later execs of a freshly written shell script.

Usage: python3 -I scripts/bench/fresh-exec.py. On macOS the first exec of a new
executable is checked by the OS, which is what tests that write fake tools pay.
"""
import os, subprocess, time, uuid

# Scratch under target/, which is gitignored.
d = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "target", "fresh-exec")
os.makedirs(d, exist_ok=True)
for trial in range(3):
    p = os.path.join(d, f"tool-{uuid.uuid4().hex}")
    open(p, "w").write("#!/bin/sh\necho 'rgbasm v1.0.4'\n")
    os.chmod(p, 0o755)
    times = []
    for _ in range(3):
        t = time.monotonic()
        subprocess.run([p, "--version"], capture_output=True)
        times.append(time.monotonic() - t)
    print("fresh script, execs 1-3:", " ".join(f"{x*1000:.0f} ms" for x in times))
t = time.monotonic()
subprocess.run(["/bin/sh", "-c", "echo hi"], capture_output=True)
print(f"/bin/sh -c: {(time.monotonic()-t)*1000:.0f} ms")
