#!/usr/bin/env python3
"""Assert that every registry actually serves a released version.

Every publish job in release.yml can report success while a registry ends
up not serving the version: the job checks that the upload command exited
zero, not that the package is resolvable. v0.5.0 is the worked example --
all publish jobs were green, and the only way the PHP library's state was
discovered was by querying registries by hand afterwards.

Two rules this encodes, both learned from that release:

1. RETRY. Registries ingest asynchronously. The PHP library was absent
   from Packagist minutes after the tag and present later; a single-shot
   check would have failed a correct release. Absence is only meaningful
   after the backoff is exhausted.

2. QUERY THE ENDPOINT THE INSTALLER USES. For Packagist that is
   repo.packagist.org/p2/, which Composer and PIE resolve against -- not
   packagist.org/packages/<name>.json, a cached web view that lagged for
   BOTH packages and produced a wrong conclusion when checked by hand.

Usage:
  python3 scripts/verify_release_registries.py --version 0.5.0
  python3 scripts/verify_release_registries.py --version 0.5.0 --only crates.io,PyPI
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.request

UA = "expanse-release-verifier (+https://github.com/orieg/expanse)"
TIMEOUT = 20


def fetch(url: str, token: str | None = None) -> tuple[int, object]:
    """Returns (status, parsed-json-or-None). Never raises on HTTP status."""
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "application/json"})
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            body = resp.read().decode("utf-8", "replace")
            try:
                return resp.status, json.loads(body)
            except json.JSONDecodeError:
                return resp.status, None
    except urllib.error.HTTPError as exc:
        return exc.code, None
    except (urllib.error.URLError, TimeoutError, OSError):
        return 0, None


def norm(v: str) -> str:
    return v.lstrip("vV").strip()


# Each probe returns the list of versions the registry currently serves.
def _crates(_v: str) -> list[str]:
    _, d = fetch("https://crates.io/api/v1/crates/expanse-trie")
    return [x["num"] for x in d.get("versions", [])] if isinstance(d, dict) else []


def _pypi(_v: str) -> list[str]:
    _, d = fetch("https://pypi.org/pypi/expanse-trie/json")
    return list(d.get("releases", {})) if isinstance(d, dict) else []


def _npm(pkg: str):
    def probe(_v: str) -> list[str]:
        _, d = fetch(f"https://registry.npmjs.org/{pkg.replace('/', '%2F')}")
        return list(d.get("versions", {})) if isinstance(d, dict) else []

    return probe


def _rubygems(_v: str) -> list[str]:
    _, d = fetch("https://rubygems.org/api/v1/versions/expanse.json")
    return [x["number"] for x in d] if isinstance(d, list) else []


def _nuget(_v: str) -> list[str]:
    _, d = fetch("https://api.nuget.org/v3-flatcontainer/orieg.expanse/index.json")
    return list(d.get("versions", [])) if isinstance(d, dict) else []


def _packagist(pkg: str):
    def probe(_v: str) -> list[str]:
        # repo.packagist.org/p2 is what Composer and PIE resolve against.
        _, d = fetch(f"https://repo.packagist.org/p2/{pkg}.json")
        if not isinstance(d, dict):
            return []
        out: list[str] = []
        for releases in d.get("packages", {}).values():
            out += [r["version"] for r in releases if isinstance(r, dict) and "version" in r]
        return out

    return probe


def _go_tag(version: str) -> list[str]:
    """Go resolves bindings/go by a nested tag, not a registry."""
    token = os.environ.get("GITHUB_TOKEN")
    status, _ = fetch(
        f"https://api.github.com/repos/orieg/expanse/git/ref/tags/bindings/go/v{version}", token
    )
    return [version] if status == 200 else []


PROBES = [
    ("crates.io", "expanse-trie", _crates),
    ("PyPI", "expanse-trie", _pypi),
    ("npm", "@orieg/expanse", _npm("@orieg/expanse")),
    ("npm (wasm)", "@orieg/expanse-wasm", _npm("@orieg/expanse-wasm")),
    ("RubyGems", "expanse", _rubygems),
    ("NuGet", "Orieg.Expanse", _nuget),
    ("Packagist", "orieg/expanse", _packagist("orieg/expanse")),
    ("Packagist", "orieg/expanse-extension", _packagist("orieg/expanse-extension")),
    ("Go module tag", "bindings/go", _go_tag),
]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", required=True, help="release version, with or without leading v")
    ap.add_argument("--attempts", type=int, default=6)
    ap.add_argument("--delay", type=int, default=30, help="seconds; doubles up to --max-delay")
    ap.add_argument("--max-delay", type=int, default=240)
    ap.add_argument("--only", default=None, help="comma-separated registry names to check")
    args = ap.parse_args()

    want = norm(args.version)
    probes = list(PROBES)
    if args.only:
        keep = {s.strip().lower() for s in args.only.split(",")}
        probes = [p for p in probes if p[0].lower() in keep]

    print(f"Verifying every registry serves {want}\n")
    pending = {(name, pkg): fn for name, pkg, fn in probes}
    found: dict[tuple[str, str], str] = {}
    delay = args.delay

    for attempt in range(1, args.attempts + 1):
        for key in list(pending):
            name, pkg = key
            try:
                served = [norm(v) for v in pending[key](want)]
            except Exception as exc:  # a probe bug must not read as absence
                print(f"  [{name}] {pkg}: probe error: {exc}")
                continue
            if want in served:
                found[key] = "ok"
                del pending[key]
                print(f"  OK       {name:16s} {pkg}")
        if not pending:
            break
        if attempt < args.attempts:
            print(
                f"  ... {len(pending)} not yet serving {want}; "
                f"attempt {attempt}/{args.attempts}, retrying in {delay}s"
            )
            time.sleep(delay)
            delay = min(delay * 2, args.max_delay)

    print()
    if pending:
        print(f"FAILED: {len(pending)} registry/registries do not serve {want}", file=sys.stderr)
        for name, pkg in pending:
            print(f"  MISSING  {name:16s} {pkg}", file=sys.stderr)
        print(
            "\nThe publish job for each of these reported success. Either the upload did not "
            "happen, or ingestion is slower than the retry budget -- check the package page "
            "before assuming the release is complete.",
            file=sys.stderr,
        )
        return 1

    print(f"All {len(found)} registries serve {want}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
