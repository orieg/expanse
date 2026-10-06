#!/usr/bin/env python3
"""Release assets: every build leg produced its archive, and what was uploaded is what was built.

`release.yml` builds one archive per target, collects them with the packages
and the SBOMs into `artifacts/`, and uploads that directory to the GitHub
Release. Two things could go wrong without anything failing:

  - a build leg uploads nothing, and the release is published one platform short;
  - an upload is truncated or replaced, and the published asset is not the file
    whose checksum is in `SHA256SUMS`.

`check-local` runs before the release is created (and in the dry-run canary):
each target of the build matrix in `release.yml` has exactly one archive, the
fixed files are present, and every file is listed in `SHA256SUMS` with the
digest it has.

`check-readback` runs on the assets downloaded back from the draft release,
before the draft is published: the downloaded set is exactly the uploaded set,
and every downloaded file has the digest `SHA256SUMS` records.

Run:  release_assets.py check-local --dir artifacts [--workflow FILE]
      release_assets.py check-readback --dir readback --local artifacts
      release_assets.py --self-test
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
import tempfile
from pathlib import Path

SUMS = "SHA256SUMS"
# Written after SHA256SUMS, so not listed in it.
UNLISTED = frozenset({SUMS, "expanse.intoto.jsonl"})
REQUIRED = (SUMS, "expanse.cdx.json", "expanse-trie.cdx.json", "expanse.intoto.jsonl")
ARCHIVE_SUFFIXES = (".tar.gz", ".zip")
BUILD_JOB = "build-release-artifacts"


def build_targets(workflow: Path) -> list[str]:
    """The `target:` of every matrix leg of the build job in `release.yml`."""
    text = workflow.read_text(encoding="utf-8")
    m = re.search(rf"^  {re.escape(BUILD_JOB)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)", text, re.S | re.M)
    if not m:
        raise ValueError(f"{workflow} has no `{BUILD_JOB}` job")
    targets = re.findall(r"^\s+- target: (\S+)\s*$", m.group(1), re.M)
    if not targets:
        raise ValueError(f"the `{BUILD_JOB}` job in {workflow} lists no matrix targets")
    return targets


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def read_sums(path: Path) -> dict[str, str]:
    """`{file name: digest}` from a `sha256sum` listing (`<hex>  ./name`)."""
    sums: dict[str, str] = {}
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        m = re.fullmatch(r"([0-9a-f]{64}) [ *](?:\./)?(.+)", line)
        if not m:
            raise ValueError(f"{path.name}:{n}: not a sha256sum line: {line!r}")
        if m.group(2) in sums:
            raise ValueError(f"{path.name}:{n}: {m.group(2)} is listed twice")
        sums[m.group(2)] = m.group(1)
    return sums


def files_in(directory: Path) -> set[str]:
    return {p.name for p in directory.iterdir() if p.is_file()}


def checksum_problems(directory: Path) -> list[str]:
    """Every file is listed with its digest; every listed file exists."""
    sums_path = directory / SUMS
    if not sums_path.is_file():
        return [f"{directory.name}/ has no {SUMS}"]
    try:
        sums = read_sums(sums_path)
    except ValueError as exc:
        return [str(exc)]
    present = files_in(directory)
    out = [f"{name} is in {SUMS} but not in {directory.name}/" for name in sorted(set(sums) - present)]
    out += [f"{name} is in {directory.name}/ but not in {SUMS}"
            for name in sorted(present - set(sums) - UNLISTED)]
    for name in sorted(set(sums) & present):
        got = digest(directory / name)
        if got != sums[name]:
            out.append(f"{name}: digest {got[:16]}… does not match {SUMS} ({sums[name][:16]}…)")
    return out


def local_problems(directory: Path, targets: list[str]) -> list[str]:
    present = files_in(directory)
    out = [f"{name} is missing from {directory.name}/" for name in REQUIRED if name not in present]
    for target in targets:
        archives = sorted(n for n in present if n.endswith(tuple(f"-{target}{s}" for s in ARCHIVE_SUFFIXES)))
        if len(archives) != 1:
            found = ", ".join(archives) if archives else "none"
            out.append(f"build target {target}: expected one archive, found {found}")
    return out + checksum_problems(directory)


def readback_problems(readback: Path, local: Path) -> list[str]:
    uploaded, downloaded = files_in(local), files_in(readback)
    out = [f"{name} was uploaded but is not on the release" for name in sorted(uploaded - downloaded)]
    out += [f"{name} is on the release but was not uploaded by this run" for name in sorted(downloaded - uploaded)]
    return out + checksum_problems(readback)


def self_test() -> int:
    failures: list[str] = []

    def check(label, got, want):
        if got != want:
            failures.append(f"{label}: got {got!r}, want {want!r}")

    with tempfile.TemporaryDirectory(prefix="release-assets-test-") as tmp:
        root = Path(tmp)
        wf = root / "release.yml"
        wf.write_text(
            "jobs:\n  release-gate:\n    steps:\n      - target: not-a-build-leg\n"
            f"  {BUILD_JOB}:\n    strategy:\n      matrix:\n        include:\n"
            "          - target: x86_64-unknown-linux-gnu\n            os: ubuntu-latest\n"
            "          - target: x86_64-pc-windows-msvc\n            os: windows-latest\n"
            "  github-release:\n    steps:\n      - target: also-not\n"
        )
        targets = build_targets(wf)
        check("targets come from the build job only", targets,
              ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"])

        def make(name: str) -> Path:
            d = root / name
            d.mkdir()
            for f, body in (("expanse-1.0.0-x86_64-unknown-linux-gnu.tar.gz", b"linux"),
                            ("expanse-v1.0.0-x86_64-pc-windows-msvc.zip", b"windows"),
                            ("libexpanse1_1.0.0_amd64.deb", b"deb"),
                            ("expanse.cdx.json", b"{}"), ("expanse-trie.cdx.json", b"{}")):
                (d / f).write_bytes(body)
            (d / SUMS).write_text("".join(
                f"{digest(d / n)}  ./{n}\n" for n in sorted(files_in(d))))
            (d / "expanse.intoto.jsonl").write_bytes(b"bundle")
            return d

        good = make("artifacts")
        check("a complete directory", local_problems(good, targets), [])

        missing_leg = make("missing-leg")
        (missing_leg / "expanse-v1.0.0-x86_64-pc-windows-msvc.zip").unlink()
        got = local_problems(missing_leg, targets)
        check("a build leg that uploaded nothing", got[0],
              "build target x86_64-pc-windows-msvc: expected one archive, found none")

        two = make("two-archives")
        (two / "expanse-1.0.1-x86_64-unknown-linux-gnu.tar.gz").write_bytes(b"stale")
        check("two archives for one target", local_problems(two, targets)[0].split(":")[0],
              "build target x86_64-unknown-linux-gnu")

        no_sbom = make("no-sbom")
        (no_sbom / "expanse.cdx.json").unlink()
        check("a missing fixed file", local_problems(no_sbom, targets)[0],
              "expanse.cdx.json is missing from no-sbom/")

        unlisted = make("unlisted")
        (unlisted / "extra.rpm").write_bytes(b"rpm")
        check("a file SHA256SUMS does not list", local_problems(unlisted, targets),
              [f"extra.rpm is in unlisted/ but not in {SUMS}"])

        # Read-back: same names and same bytes pass; a truncated or missing
        # download, or an asset this run did not upload, fails.
        same = make("readback-same")
        check("an identical read-back", readback_problems(same, good), [])
        truncated = make("readback-truncated")
        (truncated / "libexpanse1_1.0.0_amd64.deb").write_bytes(b"de")
        got = readback_problems(truncated, good)
        check("a truncated asset", len(got) == 1 and got[0].startswith("libexpanse1_1.0.0_amd64.deb: digest"), True)
        short = make("readback-short")
        (short / "expanse-1.0.0-x86_64-unknown-linux-gnu.tar.gz").unlink()
        check("an asset missing from the release", readback_problems(short, good)[0],
              "expanse-1.0.0-x86_64-unknown-linux-gnu.tar.gz was uploaded but is not on the release")
        foreign = make("readback-foreign")
        (foreign / "old.zip").write_bytes(b"x")
        check("an asset this run did not upload", readback_problems(foreign, good)[0],
              "old.zip is on the release but was not uploaded by this run")

        bad_sums = make("bad-sums")
        (bad_sums / SUMS).write_text("not a checksum line\n")
        check("a malformed SHA256SUMS", "not a sha256sum line" in checksum_problems(bad_sums)[0], True)

    # The real workflow names the legs this check will hold a release to.
    real = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "release.yml"
    try:
        n = len(build_targets(real))
        if n < 5:
            failures.append(f"release.yml lists only {n} build targets")
    except (OSError, ValueError) as exc:
        failures.append(f"cannot read the build targets of release.yml: {exc}")

    if failures:
        for f in failures:
            print(f"::error::release_assets self-test: {f}")
        return 1
    print("release_assets --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", nargs="?", choices=["check-local", "check-readback"])
    ap.add_argument("--dir", type=Path)
    ap.add_argument("--local", type=Path)
    ap.add_argument("--workflow", type=Path,
                    default=Path(__file__).resolve().parent.parent / ".github" / "workflows" / "release.yml")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.command or not args.dir or not args.dir.is_dir():
        print("::error::a command and an existing --dir are required", file=sys.stderr)
        return 1
    if args.command == "check-local":
        try:
            targets = build_targets(args.workflow)
        except (OSError, ValueError) as exc:
            print(f"::error::{exc}")
            return 1
        found = local_problems(args.dir, targets)
        ok = f"{len(targets)} build target(s) each have one archive; {len(files_in(args.dir))} file(s) match {SUMS}"
    else:
        if not args.local or not args.local.is_dir():
            print("::error::check-readback needs --local, the uploaded directory", file=sys.stderr)
            return 1
        found = readback_problems(args.dir, args.local)
        ok = f"{len(files_in(args.dir))} asset(s) read back from the release match what was uploaded"
    for p in found:
        print(f"::error::{p}")
    if found:
        return 1
    print(f"release_assets {args.command}: {ok}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
