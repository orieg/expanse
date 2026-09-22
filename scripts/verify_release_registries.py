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
   For Maven Central it is the repository itself, not the search.maven.org
   index, which after v0.7.0 returned no versions at all for an artifact
   the repository served. A build that pins a version fetches that
   version's POM directly and never reads maven-metadata.xml, so the POM
   is the primary probe and the metadata's version list the fallback.

3. BYPASS THE CDN CACHE. repo1.maven.org sits behind Cloudflare, which
   serves maven-metadata.xml and 404s from its edge cache. After v0.7.0
   the metadata carried a 23:57 UTC stamp and the repository served the
   POM, yet three verify attempts up to 03:09 UTC reported the version
   missing. A cache-busting query string makes every probe a cache MISS
   at the edge (checked against that origin: cf-cache-status HIT without
   it, MISS with it).

4. SOME REGISTRIES ARE SLOWER. Central's sync outlives the shared retry
   budget where the others do not, so a registry can carry extra attempts
   of its own (EXTRA_ATTEMPTS) and only that registry keeps the job
   waiting once the shared budget is spent.

Usage:
  python3 scripts/verify_release_registries.py --version 0.5.0
  python3 scripts/verify_release_registries.py --version 0.5.0 --only crates.io,PyPI
"""

from __future__ import annotations

import argparse
import itertools
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


def cache_busted(url: str) -> str:
    """The URL with a unique query string, so a CDN edge cannot answer from its cache.

    Cloudflare keys its cache on the full URL including the query string, and a
    request-side Cache-Control header alone is not honoured; the query string is
    what turns a HIT into a MISS.
    """
    sep = "&" if "?" in url else "?"
    # The counter keeps two calls in one clock tick distinct.
    return f"{url}{sep}nocache={time.time_ns()}-{next(_NONCE)}"


_NONCE = itertools.count()


def fetch_text(url: str, bypass_cache: bool = False) -> tuple[int, str | None]:
    """Returns (status, body-or-None) for a non-JSON endpoint. Never raises on HTTP status."""
    if bypass_cache:
        url = cache_busted(url)
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Cache-Control": "no-cache", "Pragma": "no-cache"})
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


MAVEN_REPO = "https://repo1.maven.org/maven2/io/github/orieg/expanse-java"
MAVEN_METADATA = f"{MAVEN_REPO}/maven-metadata.xml"


def maven_pom_url(version: str) -> str:
    """The POM a build pinned to `version` resolves, the same URL Maven and Gradle GET."""
    return f"{MAVEN_REPO}/{version}/expanse-java-{version}.pom"


def parse_maven_metadata(body: str | None) -> list[str]:
    """The <versioning><versions><version> list of a maven-metadata.xml."""
    if not body:
        return []
    try:
        root = ET.fromstring(body)
    except ET.ParseError:
        return []
    return [v.text.strip() for v in root.findall("./versioning/versions/version") if v.text]


def _maven(version: str) -> list[str]:
    """Maven Central repository for io.github.orieg:expanse-java.

    The version's own POM first: a pinned dependency resolves that URL and never
    consults the metadata, so a 200 there is the release being installable.
    The metadata's version list is the fallback. Both bypass the CDN cache.
    """
    status, _ = fetch_text(maven_pom_url(version), bypass_cache=True)
    served = [version] if status == 200 else []
    _, body = fetch_text(MAVEN_METADATA, bypass_cache=True)
    return served + parse_maven_metadata(body)


def _go_tag(version: str) -> list[str]:
    """Go resolves bindings/go by a nested tag, not a registry."""
    token = os.environ.get("GITHUB_TOKEN")
    status, _ = fetch(f"https://api.github.com/repos/orieg/expanse/git/ref/tags/bindings/go/v{version}", token)
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

# Attempts a registry gets beyond the shared --attempts budget, each waiting
# --max-delay. Only these registries keep the job waiting past that budget.
# Maven Central: the v0.7.0 release was absent from the repository for every
# attempt of a 12-minute budget run three times over 3.5 hours (release.yml
# run 35669177044), with every other registry served on the first attempt.
EXTRA_ATTEMPTS = {"Maven Central": 4}


def attempts_for(name: str, base: int) -> int:
    """Total attempts registry `name` gets: the shared budget plus its own extra."""
    return base + EXTRA_ATTEMPTS.get(name, 0)


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
    assert (
        "fetch_text(MAVEN_METADATA, bypass_cache=True)" in maven_src
    ), "_maven must read MAVEN_METADATA past the CDN cache"
    assert (
        "fetch_text(maven_pom_url(version), bypass_cache=True)" in maven_src
    ), "_maven must probe the version's POM past the CDN cache"
    assert "parse_maven_metadata(" in maven_src, "_maven must parse the metadata"
    assert MAVEN_METADATA.startswith("https://repo1.maven.org/maven2/"), MAVEN_METADATA
    assert MAVEN_METADATA.endswith("/io/github/orieg/expanse-java/maven-metadata.xml")
    assert maven_pom_url("0.7.0") == (
        "https://repo1.maven.org/maven2/io/github/orieg/expanse-java/0.7.0/expanse-java-0.7.0.pom"
    ), maven_pom_url("0.7.0")
    # The cache-buster varies per call and survives an existing query string.
    a, b = cache_busted("https://x/y.xml"), cache_busted("https://x/y.xml")
    assert a.startswith("https://x/y.xml?nocache=") and a != b, (a, b)
    assert cache_busted("https://x/y?k=v").startswith("https://x/y?k=v&nocache="), cache_busted("https://x/y?k=v")
    # fetch_text must apply it when asked, not merely accept the flag.
    assert "url = cache_busted(url)" in inspect.getsource(fetch_text)
    # The per-registry budget: Maven gets more, an unlisted registry exactly the base.
    assert attempts_for("Maven Central", 6) == 10, attempts_for("Maven Central", 6)
    assert attempts_for("crates.io", 6) == 6
    assert set(EXTRA_ATTEMPTS) <= {p[0] for p in PROBES}, "EXTRA_ATTEMPTS names a real registry"
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
    ap.add_argument(
        "--attempts",
        type=int,
        default=6,
        help="shared retry budget; EXTRA_ATTEMPTS registries get more on top",
    )
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

    last_attempt = max((attempts_for(name, args.attempts) for name, _, _ in probes), default=0)
    for attempt in range(1, last_attempt + 1):
        # Past the shared budget only registries with extra attempts are probed;
        # the others are already final and are reported below.
        due = [k for k in pending if attempt <= attempts_for(k[0], args.attempts)]
        if not due:
            break
        for key in due:
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
        still_due = [k for k in pending if attempt < attempts_for(k[0], args.attempts)]
        if still_due:
            budget = max(attempts_for(k[0], args.attempts) for k in still_due)
            print(
                f"  ... {len(pending)} not yet serving {want}; "
                f"attempt {attempt}/{budget}, retrying in {delay}s"
                + (f" ({len(still_due)} still within budget)" if len(still_due) < len(pending) else "")
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
