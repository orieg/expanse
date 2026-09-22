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
   For Maven Central it is the repository's maven-metadata.xml, which
   Maven and Gradle read -- not the search.maven.org index, which after
   v0.7.0 returned no versions at all for an artifact the repository
   served.

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
import xml.etree.ElementTree as ET

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


def fetch_text(url: str) -> tuple[int, str | None]:
    """Returns (status, body-or-None) for a non-JSON endpoint. Never raises on HTTP status."""
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            return resp.status, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as exc:
        return exc.code, None
    except (urllib.error.URLError, TimeoutError, OSError):
        return 0, None


def norm(v: str) -> str:
    return v.strip().lstrip("vV").strip()


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


MAVEN_METADATA = "https://repo1.maven.org/maven2/io/github/orieg/expanse-java/maven-metadata.xml"


def parse_maven_metadata(body: str | None) -> list[str]:
    """The <versioning><versions><version> list of a maven-metadata.xml."""
    if not body:
        return []
    try:
        root = ET.fromstring(body)
    except ET.ParseError:
        return []
    return [v.text.strip() for v in root.findall("./versioning/versions/version") if v.text]


def _maven(_v: str) -> list[str]:
    """Maven Central repository metadata for io.github.orieg:expanse-java."""
    _, body = fetch_text(MAVEN_METADATA)
    return parse_maven_metadata(body)


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
    ("Maven Central", "io.github.orieg:expanse-java", _maven),
    ("Packagist", "orieg/expanse", _packagist("orieg/expanse")),
    ("Packagist", "orieg/expanse-extension", _packagist("orieg/expanse-extension")),
    ("Go module tag", "bindings/go", _go_tag),
]


def run_self_test() -> int:
    assert norm("v0.5.0") == "0.5.0"
    assert norm("0.5.0") == "0.5.0"
    assert norm(" V1.2.3 ") == "1.2.3"
    # The Maven probe reads the repository metadata Maven and Gradle resolve
    # against; pinned on the shape repo1 served for v0.7.0.
    sample = (
        '<?xml version="1.0" encoding="UTF-8"?>\n<metadata><groupId>io.github.orieg</groupId>'
        "<artifactId>expanse-java</artifactId><versioning><latest>0.7.0</latest>"
        "<release>0.7.0</release><versions><version>0.6.0</version>"
        "<version>0.7.0</version></versions></versioning></metadata>"
    )
    assert parse_maven_metadata(sample) == ["0.6.0", "0.7.0"], parse_maven_metadata(sample)
    assert parse_maven_metadata("<metadata/>") == []
    assert parse_maven_metadata("not xml") == []
    assert parse_maven_metadata(None) == []
    # The probe itself must read that endpoint through that parser; a parser
    # test alone stays green if the probe goes back to the search index.
    import inspect

    maven_src = inspect.getsource(_maven)
    assert "fetch_text(MAVEN_METADATA)" in maven_src, "_maven must read MAVEN_METADATA"
    assert "parse_maven_metadata(" in maven_src, "_maven must parse the metadata"
    assert MAVEN_METADATA.startswith("https://repo1.maven.org/maven2/"), MAVEN_METADATA
    assert MAVEN_METADATA.endswith("/io/github/orieg/expanse-java/maven-metadata.xml")
    assert len(PROBES) == 10
    names = [p[0] for p in PROBES]
    assert "Maven Central" in names
    assert "crates.io" in names
    assert "NuGet" in names
    print("verify_release_registries.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", default=None, help="release version, with or without leading v")
    ap.add_argument("--attempts", type=int, default=6)
    ap.add_argument("--delay", type=int, default=30, help="seconds; doubles up to --max-delay")
    ap.add_argument("--max-delay", type=int, default=240)
    ap.add_argument("--only", default=None, help="comma-separated registry names to check")
    ap.add_argument("--self-test", action="store_true", help="Run internal unit self-tests and exit")
    args = ap.parse_args()

    if args.self_test:
        return run_self_test()

    if not args.version:
        ap.error("--version is required when not running --self-test")

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
