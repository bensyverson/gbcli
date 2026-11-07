#!/usr/bin/env python3
"""Checks every Pan Docs and RGBDS docs link in gbcli's help against the pages.

Finds each https://gbdev.io/pandocs/PAGE.html#ANCHOR and
https://rgbds.gbdev.io/docs/... link in crates/cli/src/help/,
fetches each page once with curl (urllib through the sandbox proxy can end
early; project/gotchas.md), and fails naming every link whose page doesn't
load or whose anchor isn't an id on it. It checks that the section exists,
not that its text still says what the help says: re-read a section when this
flags it, and when a figure is changed.

    python3 -I scripts/help/check-doc-links.py

Needs the network (gbdev.io, rgbds.gbdev.io); in the Bash sandbox, allow both.
"""

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
HELP = REPO / "crates/cli/src/help"
LINK = re.compile(r"https://(?:gbdev\.io/pandocs|rgbds\.gbdev\.io/docs)/[A-Za-z0-9_./-]+(?:#[A-Za-z0-9_-]+)?")


def links():
    found = {}
    for path in sorted(HELP.glob("*.rs")):
        for link in LINK.findall(path.read_text()):
            found.setdefault(link, path.relative_to(REPO))
    return found


def fetch(url):
    run = subprocess.run(
        ["curl", "-sS", "-f", "-L", url], capture_output=True, text=True, check=False
    )
    return run.stdout if run.returncode == 0 else None


def main():
    found = links()
    pages = {}
    bad = []
    for link, where in found.items():
        page, _, anchor = link.partition("#")
        if page not in pages:
            pages[page] = fetch(page)
        html = pages[page]
        if html is None:
            bad.append(f"{where}: {link}: the page didn't load")
        elif not anchor:
            bad.append(f"{where}: {link}: names no section")
        elif f'id="{anchor}"' not in html and f"id={anchor}" not in html:
            bad.append(f"{where}: {link}: no section #{anchor} on the page")
    for line in bad:
        print(line)
    print(f"{len(found)} links on {len(pages)} pages, {len(bad)} bad")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
