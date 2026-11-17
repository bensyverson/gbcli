"""Runs gbcli for the tools, in this project, and reads its JSON.

Every tool takes --gbcli PATH (default: gbcli on PATH) and passes it here.
"""

import json
import subprocess
import sys

import tuning

PROJECT = tuning.PROJECT
ROM = "build/factory.gb"
ROM_GBC = "build/gbc/factory.gbc"


class Gbcli:
    def __init__(self, exe="gbcli", verbose=False):
        self.exe = exe
        self.verbose = verbose
        self.calls = 0

    def run(self, *args, check=True):
        """Runs one gbcli command with --json; returns the parsed reply."""
        cmd = [self.exe, *args, "--json", "--dir", str(PROJECT)]
        self.calls += 1
        if self.verbose:
            print("$ gbcli " + " ".join(args), file=sys.stderr)
        out = subprocess.run(cmd, capture_output=True, text=True)
        try:
            reply = json.loads(out.stdout)
        except json.JSONDecodeError:
            sys.exit(f"gbcli {' '.join(args)}: no JSON (exit {out.returncode})\n{out.stdout}{out.stderr}")
        if check and not reply.get("ok"):
            sys.exit(f"gbcli {' '.join(args)}: {json.dumps(reply.get('error', reply))}")
        return reply

    def text(self, *args):
        """Runs one gbcli command for its rows; returns (exit code, output)."""
        cmd = [self.exe, *args, "--dir", str(PROJECT)]
        self.calls += 1
        out = subprocess.run(cmd, capture_output=True, text=True)
        return out.returncode, out.stdout + out.stderr


def arg_parser(description):
    import argparse
    p = argparse.ArgumentParser(description=description)
    p.add_argument("--gbcli", default="gbcli", help="the gbcli binary (default: gbcli on PATH)")
    p.add_argument("-v", "--verbose", action="store_true", help="print each gbcli command")
    return p
