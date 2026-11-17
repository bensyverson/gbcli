"""Rebuilds everything after a change, then checks it: the one command.

    python3 tools/make.py [--gbcli PATH] [--no-check] [--prove]

1. tools/gen_toys.py: data/toys.txt and art/toys/ -> the toy include
2. tools/gen_clock.py: include/tuning.inc -> the clock's rotations
3. gbcli build, then gbcli build --gbc (build/factory.gb, build/gbc/factory.gbc)
4. tools/rules_check.py: one scenario per rule of the controls table
   (skipped with --no-check)
5. with --prove, tools/prove_screen.py: the belts, key column, clock and
   frame budget claims (about a minute)
Stops at the first step that fails.
"""

import subprocess
import sys
from pathlib import Path

import gbcli as g

TOOLS = Path(__file__).resolve().parent


def step(name, cmd):
    print(f"== {name}", flush=True)
    if subprocess.run(cmd).returncode:
        sys.exit(f"make: {name} failed")


def main():
    p = g.arg_parser(__doc__.splitlines()[0])
    p.add_argument("--no-check", action="store_true", help="skip the rules check")
    p.add_argument("--prove", action="store_true", help="also run tools/prove_screen.py")
    args = p.parse_args()
    py = sys.executable
    step("toys", [py, str(TOOLS / "gen_toys.py")])
    step("clock", [py, str(TOOLS / "gen_clock.py")])
    step("build", [args.gbcli, "build", str(g.PROJECT)])
    step("build --gbc", [args.gbcli, "build", str(g.PROJECT), "--gbc"])
    if not args.no_check and (TOOLS / "rules_check.py").exists():
        step("rules check", [py, str(TOOLS / "rules_check.py"), "--gbcli", args.gbcli])
    if args.prove:
        step("prove screen", [py, str(TOOLS / "prove_screen.py"), "--gbcli", args.gbcli])


if __name__ == "__main__":
    main()
