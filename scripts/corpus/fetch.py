#!/usr/bin/env python3
"""Fetch the pinned test-ROM corpus and verify every file by SHA-256.

Reads tests/corpus/manifest.json (sources, and which archive paths land where)
and tests/corpus/SHA256SUMS (one line per corpus file). Downloads each source
archive once into a cache directory, checks the archive's own SHA-256 where the
manifest pins one, extracts the selected paths into the corpus directory, then
verifies every file against SHA256SUMS: a missing, changed or unexpected file
fails the run.

    python3 -I scripts/corpus/fetch.py                  # whole corpus
    python3 -I scripts/corpus/fetch.py --only mooneye --only blargg
    python3 -I scripts/corpus/fetch.py --verify-only    # re-check an existing tree

Stdlib only. Run from anywhere; paths default to the repo this script lives in.
"""

import argparse
import hashlib
import http.client
import json
import os
import shutil
import sys
import tarfile
import time
import urllib.error
import urllib.request
import zipfile

REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
MANIFEST = os.path.join(REPO, "tests", "corpus", "manifest.json")
SUMS = os.path.join(REPO, "tests", "corpus", "SHA256SUMS")
DEFAULT_DEST = os.path.join(REPO, "tests", "corpus", "fetched")
DEFAULT_CACHE = os.path.join(REPO, "tests", "corpus", ".downloads")
USER_AGENT = "gbcli-corpus-fetch/1"
DOWNLOAD_ATTEMPTS = 5


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def load_manifest(path=MANIFEST):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def load_sums(path=SUMS):
    sums = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if not line:
                continue
            digest, rel = line.split("  ", 1)
            sums[rel] = digest
    return sums


def _download_resuming(label, url, tmp):
    """Download url to tmp, resuming with HTTP Range when a connection ends early.

    A dropped connection can end the body without an error (observed through a
    filtering proxy), so completeness is judged by Content-Length when the server
    sends one; the archive and per-file checksums catch anything this misses.
    """
    have, expected = 0, None
    for attempt in range(1, DOWNLOAD_ATTEMPTS + 1):
        headers = {"User-Agent": USER_AGENT}
        if have:
            headers["Range"] = f"bytes={have}-"
        complete = False
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=60) as r:
                if have and r.status != 206:  # server ignored Range: start over
                    have = 0
                length = r.headers.get("Content-Length")
                if have == 0:
                    expected = int(length) if length else None
                with open(tmp, "ab" if have else "wb") as out:
                    shutil.copyfileobj(r, out)
                complete = True
        except (http.client.IncompleteRead, urllib.error.URLError, ConnectionError, TimeoutError) as e:
            print(f"{label}: {type(e).__name__} (attempt {attempt})", file=sys.stderr)
        have = os.path.getsize(tmp) if os.path.exists(tmp) else 0
        if expected is not None and have == expected:
            return
        if expected is None and complete:
            return
        if expected is not None:
            print(f"{label}: {have}/{expected} bytes, resuming (attempt {attempt})", file=sys.stderr)
        elif not complete:
            have = 0  # no length to resume against
        time.sleep(attempt)
    raise SystemExit(f"{label}: download kept ending early ({url})")


def download(source, cache_dir, check_archive=True):
    """Return the local path of the source archive, downloading it if needed."""
    os.makedirs(cache_dir, exist_ok=True)
    path = os.path.join(cache_dir, source["cache_name"])
    pinned = source.get("archive_sha256")
    if os.path.exists(path) and (not pinned or not check_archive or sha256_file(path) == pinned):
        return path
    tmp = path + ".part"
    _download_resuming(source["id"], source["url"], tmp)
    os.replace(tmp, path)
    if pinned and check_archive:
        got = sha256_file(path)
        if got != pinned:
            raise SystemExit(f"{source['id']}: archive SHA-256 {got} != pinned {pinned} ({source['url']})")
    return path


def _members(archive_path, kind):
    """Yield (name, reader) for every regular file in the archive."""
    if kind == "zip":
        with zipfile.ZipFile(archive_path) as z:
            for info in z.infolist():
                if not info.is_dir():
                    yield info.filename, (lambda i=info: z.read(i))
    elif kind == "tar.gz":
        with tarfile.open(archive_path, "r:gz") as t:
            for m in t:
                if m.isfile():
                    yield m.name, (lambda m=m: t.extractfile(m).read())
    else:
        raise SystemExit(f"unknown archive kind {kind!r}")


def _safe_rel(rel):
    parts = rel.split("/")
    if rel.startswith("/") or any(p in ("", "..") for p in parts):
        raise SystemExit(f"refusing unsafe archive path {rel!r}")
    return rel


def extract(source, archive_path, dest, suites):
    """Extract the source's mapped paths for the selected suites; return files written."""
    strip = source.get("strip_components", 0)
    rules = [r for r in source["extract"] if r["suite"] in suites]
    if not rules:
        return 0
    written = 0
    for name, read in _members(archive_path, source["kind"]):
        parts = name.split("/")
        if len(parts) <= strip:
            continue
        inner = "/".join(parts[strip:])
        for rule in rules:
            frm, to = rule["from"], rule["to"]
            if inner == frm or (frm.endswith("/") and inner.startswith(frm)):
                rel = _safe_rel(to + inner[len(frm):] if frm.endswith("/") else to)
                out = os.path.join(dest, rel)
                os.makedirs(os.path.dirname(out), exist_ok=True)
                with open(out, "wb") as f:
                    f.write(read())
                written += 1
                break
    return written


def verify(dest, sums, suites):
    """Check every expected file of the selected suites; return (ok, files, bytes, problems)."""
    prefixes = tuple(s + "/" for s in suites)
    expected = {rel: d for rel, d in sums.items() if rel.startswith(prefixes)}
    problems = []
    total = 0
    for rel, digest in sorted(expected.items()):
        path = os.path.join(dest, rel)
        if not os.path.exists(path):
            problems.append(f"missing  {rel}")
            continue
        total += os.path.getsize(path)
        got = sha256_file(path)
        if got != digest:
            problems.append(f"changed  {rel} ({got})")
    for suite in suites:
        root = os.path.join(dest, suite)
        for dirpath, _, files in os.walk(root):
            for fn in files:
                rel = os.path.relpath(os.path.join(dirpath, fn), dest).replace(os.sep, "/")
                if rel not in expected:
                    problems.append(f"extra    {rel}")
    return not problems, len(expected), total, problems


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dest", default=DEFAULT_DEST, help="corpus directory (default tests/corpus/fetched)")
    ap.add_argument("--cache", default=DEFAULT_CACHE, help="archive cache (default tests/corpus/.downloads)")
    ap.add_argument("--only", action="append", metavar="SUITE", help="fetch only this suite (repeatable)")
    ap.add_argument("--verify-only", action="store_true", help="skip downloads; verify the existing tree")
    args = ap.parse_args()

    start = time.monotonic()
    manifest = load_manifest()
    sums = load_sums()
    all_suites = list(manifest["suites"])
    suites = args.only or all_suites
    unknown = [s for s in suites if s not in manifest["suites"]]
    if unknown:
        raise SystemExit(f"unknown suite(s) {unknown}; have {all_suites}")

    if not args.verify_only:
        for suite in suites:
            shutil.rmtree(os.path.join(args.dest, suite), ignore_errors=True)
        for source in manifest["sources"]:
            if not any(r["suite"] in suites for r in source["extract"]):
                continue
            path = download(source, args.cache)
            n = extract(source, path, args.dest, suites)
            print(f"{source['id']}: {n} files", file=sys.stderr)

    ok, files, size, problems = verify(args.dest, sums, suites)
    elapsed = time.monotonic() - start
    for p in problems:
        print(p)
    status = "OK" if ok else f"FAILED ({len(problems)} problems; unpinned archives are reused from {args.cache}, delete one there to re-download it)"
    print(f"{status}: {files} files, {size} bytes ({size / 1e6:.1f} MB) in {args.dest}; {elapsed:.1f} s")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
