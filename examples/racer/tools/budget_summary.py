"""Summarizes `gbcli run --frame-budget --json` output, skipping startup frames.

The race's startup (LCD off, clearing memory) never halts, so the frame
budget counts its first frames as lag; this reports the frames after it.

Run: python3 -I tools/budget_summary.py FILE [SKIP]   (SKIP defaults to 10)
"""

import json
import sys

FRAME_T = 70224


def frames(node):
    """Finds the per-frame list wherever the response nests it."""
    if isinstance(node, dict):
        for k, v in node.items():
            if k == "frames" and isinstance(v, list) and v and isinstance(v[0], dict):
                return v
            found = frames(v)
            if found:
                return found
    elif isinstance(node, list):
        for v in node:
            found = frames(v)
            if found:
                return found
    return None


def main():
    data = json.load(open(sys.argv[1]))
    skip = int(sys.argv[2]) if len(sys.argv) > 2 else 10
    fs = frames(data)
    if fs is None:
        raise SystemExit("no per-frame list in the response; keys at the top: " + ", ".join(data))
    print("fields:", ", ".join(fs[0]))
    rest = fs[skip:]
    active = [f["active_t"] for f in rest]
    lag = [i + skip for i, f in enumerate(rest) if f.get("lag")]
    worst = max(range(len(active)), key=active.__getitem__)
    print(f"frames {len(rest)} after skipping {skip}: lag {len(lag)} {lag[:10]}")
    print(f"active max {max(active)} ({100 * max(active) / FRAME_T:.1f}%) at index {worst + skip}, "
          f"mean {sum(active) / len(active):.0f} ({100 * sum(active) / len(active) / FRAME_T:.1f}%)")


if __name__ == "__main__":
    main()
