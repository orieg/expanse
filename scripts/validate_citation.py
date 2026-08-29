#!/usr/bin/env python3
"""Validate CITATION.cff and keep it consistent with .zenodo.json.

Nothing checked these two files, so a malformed edit merged green and a
rename could move one without the other. They are the project's citation
record: GitHub reads CITATION.cff for "Cite this repository", and Zenodo
reads .zenodo.json when minting a DOI, so a divergence publishes two
different titles for the same software under the same DOI family.

Checks:
  1. Both files parse.
  2. CITATION.cff carries the keys CFF 1.2.0 requires, plus the ones this
     project relies on (title/authors/license/repository-code).
  3. Any DOI is a well-formed Zenodo DOI, and `doi` is the CONCEPT DOI --
     it must also appear in `identifiers` described as the concept DOI, so
     the file cannot silently pin a single version DOI as the project DOI.
  4. `version` matches the workspace version, so a release bump cannot
     leave the citation record behind.
  5. The two files agree on title and on the author list.

Usage: python3 scripts/validate_citation.py [--expected-version X.Y.Z]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover
    print("PyYAML is required: pip install pyyaml", file=sys.stderr)
    raise SystemExit(2)

ROOT = Path(__file__).resolve().parent.parent
CFF_PATH = ROOT / "CITATION.cff"
ZENODO_PATH = ROOT / ".zenodo.json"

ZENODO_DOI = re.compile(r"^10\.5281/zenodo\.\d+$")
ISO_DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
REQUIRED = ("cff-version", "message", "title", "type", "authors", "license", "repository-code")


def workspace_version() -> str | None:
    """Reads [workspace.package] version from the root Cargo.toml."""
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    in_section = False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            in_section = stripped == "[workspace.package]"
            continue
        if in_section:
            m = re.match(r'version\s*=\s*"([^"]+)"', stripped)
            if m:
                return m.group(1)
    return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--expected-version", default=None)
    args = ap.parse_args()

    errors: list[str] = []

    if not CFF_PATH.is_file():
        print("FATAL: CITATION.cff is missing", file=sys.stderr)
        return 1

    try:
        cff = yaml.safe_load(CFF_PATH.read_text(encoding="utf-8"))
    except yaml.YAMLError as exc:
        print(f"FATAL: CITATION.cff is not valid YAML: {exc}", file=sys.stderr)
        return 1
    if not isinstance(cff, dict):
        print("FATAL: CITATION.cff must be a mapping", file=sys.stderr)
        return 1

    for key in REQUIRED:
        if not cff.get(key):
            errors.append(f"CITATION.cff: missing required key `{key}`")

    if cff.get("cff-version") != "1.2.0":
        errors.append(f"CITATION.cff: cff-version is {cff.get('cff-version')!r}, expected '1.2.0'")

    authors = cff.get("authors")
    if isinstance(authors, list):
        for i, a in enumerate(authors):
            if not isinstance(a, dict) or not (a.get("family-names") or a.get("name")):
                errors.append(f"CITATION.cff: authors[{i}] needs family-names or name")
    elif authors is not None:
        errors.append("CITATION.cff: authors must be a list")

    released = cff.get("date-released")
    if released is not None and not ISO_DATE.match(str(released)):
        errors.append(f"CITATION.cff: date-released {released!r} is not YYYY-MM-DD")

    # --- DOI shape, and concept-vs-version discipline ---
    identifiers = cff.get("identifiers") or []
    if not isinstance(identifiers, list):
        errors.append("CITATION.cff: identifiers must be a list")
        identifiers = []

    id_dois: dict[str, str] = {}
    for i, ident in enumerate(identifiers):
        if not isinstance(ident, dict):
            errors.append(f"CITATION.cff: identifiers[{i}] must be a mapping")
            continue
        if ident.get("type") != "doi":
            continue
        value = str(ident.get("value", ""))
        if not ZENODO_DOI.match(value):
            errors.append(f"CITATION.cff: identifiers[{i}] value {value!r} is not a Zenodo DOI")
            continue
        id_dois[value] = str(ident.get("description", ""))

    doi = cff.get("doi")
    if doi is not None:
        if not ZENODO_DOI.match(str(doi)):
            errors.append(f"CITATION.cff: doi {doi!r} is not a Zenodo DOI (10.5281/zenodo.N)")
        elif str(doi) not in id_dois:
            errors.append(
                f"CITATION.cff: doi {doi} is not listed under identifiers; "
                "list it and describe it as the concept DOI"
            )
        elif "concept" not in id_dois[str(doi)].lower():
            errors.append(
                f"CITATION.cff: doi {doi} must be the CONCEPT DOI (all versions), but its "
                f"identifiers description reads {id_dois[str(doi)]!r}. A version DOI here "
                "pins every citation to one release."
            )

    # --- version lockstep with the workspace ---
    expected = args.expected_version or workspace_version()
    actual = cff.get("version")
    if expected and actual is not None and str(actual) != expected:
        errors.append(
            f"CITATION.cff: version is {actual!r} but the workspace is {expected!r}. "
            "Run scripts/bump_version.py so the citation record tracks the release."
        )

    # --- agreement with .zenodo.json ---
    if ZENODO_PATH.is_file():
        try:
            zen = json.loads(ZENODO_PATH.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            errors.append(f".zenodo.json is not valid JSON: {exc}")
            zen = None
        if isinstance(zen, dict):
            if zen.get("title") != cff.get("title"):
                errors.append(
                    "title differs between CITATION.cff and .zenodo.json:\n"
                    f"    cff    : {cff.get('title')!r}\n"
                    f"    zenodo : {zen.get('title')!r}\n"
                    "    Zenodo mints the DOI from .zenodo.json and GitHub cites CITATION.cff; "
                    "they must name the same work."
                )
            cff_names = {
                f"{a.get('family-names')}, {a.get('given-names')}"
                for a in (authors or [])
                if isinstance(a, dict) and a.get("family-names")
            }
            zen_names = {
                str(c.get("name")) for c in (zen.get("creators") or []) if isinstance(c, dict)
            }
            if cff_names and zen_names and cff_names != zen_names:
                errors.append(
                    f"authors differ: CITATION.cff {sorted(cff_names)} vs "
                    f".zenodo.json {sorted(zen_names)}"
                )

    if errors:
        print(f"validate_citation.py: {len(errors)} error(s)\n", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1

    print(
        f"validate_citation.py: OK (title={cff.get('title')!r}, "
        f"version={cff.get('version')}, doi={cff.get('doi')})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
