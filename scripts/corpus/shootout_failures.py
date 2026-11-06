#!/usr/bin/env python3
"""Extract per-test results for chosen emulators from the gbdev emulator shootout page.

The page (https://gbdev.io/GBEmulatorShootout/) is one ~6 MB HTML table with an
inline PNG per cell, so it is parsed, never read whole. Download it first:

    curl -sSL -o shootout.html https://gbdev.io/GBEmulatorShootout/
    python3 -I scripts/corpus/shootout_failures.py shootout.html
    python3 -I scripts/corpus/shootout_failures.py shootout.html --emulator SameBoy --json

Output: the page's "Updated On" stamp, each chosen emulator's header score, and
every test that emulator FAILs or was not run on (an empty cell), plus a
comparison of the first two emulators. Stdlib only.
"""

import argparse
import json
import sys
from html.parser import HTMLParser


class ShootoutParser(HTMLParser):
    """Collects the header row (emulators) and one record per test row."""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.updated_on = None
        self.emulators = []  # [(name, score_text)]
        self.rows = []  # [(test_name, [result, ...])]
        self._row = None
        self._cell = None  # ("th"|"td", class, text parts)
        self._in_tooltip = False
        self._header_done = False

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "tr":
            self._row = {"th": [], "td": []}
        elif tag in ("th", "td") and self._row is not None:
            self._cell = (tag, attrs.get("class", ""), [])
        elif tag == "span" and "tooltiptext" in (attrs.get("class") or ""):
            self._in_tooltip = True
        elif tag == "br" and self._cell is not None:
            self._cell[2].append("\n")

    def handle_endtag(self, tag):
        if tag == "span" and self._in_tooltip:
            self._in_tooltip = False
        elif tag in ("th", "td") and self._cell is not None:
            kind, cls, parts = self._cell
            text = "".join(parts).replace("​", "").strip()
            self._row[kind].append((cls, text))
            self._cell = None
        elif tag == "tr" and self._row is not None:
            self._finish_row(self._row)
            self._row = None

    def handle_data(self, data):
        if self._cell is not None and not self._in_tooltip:
            self._cell[2].append(data)

    def _finish_row(self, row):
        if not self._header_done:
            first = row["th"][0][1] if row["th"] else ""
            if first.startswith("Updated On"):
                self.updated_on = first.split("\n", 1)[1].strip() if "\n" in first else first
                for _, text in row["th"][1:]:
                    name, _, score = text.rpartition(" ")
                    self.emulators.append((name.strip(), score.strip("()")))
                self._header_done = True
            return
        if not row["th"]:
            return
        test = row["th"][0][1]
        results = []
        for cls, text in row["td"]:
            results.append(cls if cls else "NOT_RUN")
        if len(results) != len(self.emulators):
            raise SystemExit(
                f"row {test!r} has {len(results)} cells for {len(self.emulators)} emulators; page layout changed"
            )
        self.rows.append((test, results))


def find_emulator(emulators, wanted):
    for i, (name, _) in enumerate(emulators):
        if name.lower() == wanted.lower():
            return i
    raise SystemExit(f"emulator {wanted!r} not on the page; have: {', '.join(n for n, _ in emulators)}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("html", help="downloaded shootout page")
    ap.add_argument("--emulator", action="append", help="emulator column (repeatable); default SameBoy and docboy")
    ap.add_argument("--json", action="store_true", help="emit JSON instead of Markdown")
    args = ap.parse_args()
    wanted = args.emulator or ["SameBoy", "docboy"]

    p = ShootoutParser()
    with open(args.html, encoding="utf-8") as f:
        p.feed(f.read())
    if not p.emulators or not p.rows:
        raise SystemExit("no header or no test rows found; page layout changed")

    cols = [find_emulator(p.emulators, w) for w in wanted]
    report = {
        "updated_on": p.updated_on,
        "test_rows": len(p.rows),
        "emulators": {},
    }
    for c in cols:
        name, score = p.emulators[c]
        tally = {}
        for _, res in p.rows:
            tally[res[c]] = tally.get(res[c], 0) + 1
        report["emulators"][name] = {
            "score": score,
            "tally": tally,
            "fail": [t for t, res in p.rows if res[c] == "FAIL"],
            "not_run": [t for t, res in p.rows if res[c] == "NOT_RUN"],
        }

    if args.json:
        json.dump(report, sys.stdout, indent=2)
        print()
        return

    print(f"Shootout updated on: {p.updated_on}; {len(p.rows)} test rows; {len(p.emulators)} emulators")
    for name, e in report["emulators"].items():
        print(f"\n## {name} ({e['score']}) tally={e['tally']}\n")
        print(f"FAIL ({len(e['fail'])}):")
        for t in e["fail"]:
            print(f"- {t}")
        print(f"\nNot run ({len(e['not_run'])}):")
        for t in e["not_run"]:
            print(f"- {t}")
    if len(cols) >= 2:
        a, b = cols[0], cols[1]
        na, nb = p.emulators[a][0], p.emulators[b][0]
        print(f"\n## Comparison: {na} vs {nb}\n")
        print(f"| Test | {na} | {nb} |\n|---|---|---|")
        for t, res in p.rows:
            if res[a] != res[b] or res[a] == "FAIL":
                print(f"| {t} | {res[a]} | {res[b]} |")


if __name__ == "__main__":
    main()
