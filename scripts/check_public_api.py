#!/usr/bin/env python3
"""scripts/check_public_api.py — the Rust public API surface is a reviewed diff (#797).

AGENTS.md section 6.5 gates the C ABI symbol set (`check_abi_parity.py`), the
version lockstep, the test-count floor and the deletion rationale. It did not
gate the **Rust** surface, which is what crates.io consumers actually compile
against, and that is how two public items reached a published v0.6.0 with no
caller, no test and no documented contract:

  * `ExpanseBlobMap::arena_mut`   — shipped v0.4.0, withdrawn by #763/#796
  * `DomainOrdinal::new`          — shipped v0.6.0, withdrawn by #763/#796

Both were found by a person reading the code. Nothing objected.

The gate is symmetric, and the reverse direction is the one that costs users
rather than embarrassing the project: an *accidental* removal or signature
change ships just as quietly today.

This is a snapshot diff, not a semver classifier. It makes a surface change a
visible, reviewable line in the pull request; it does not decide whether that
change is major or minor -- a reviewer does, against the Cargo 0.x rule that
`^0.6` spans every 0.6.x, so a breaking change needs 0.7.0 and never a patch.

Regenerating is the intended workflow for a deliberate change:

    python3 scripts/check_public_api.py --write

Fail-loud (AGENTS.md section 8.1): a missing `cargo public-api`, a missing
toolchain or a failed subprocess is an error naming the cause, never a
silently skipped check that reports success.

Usage:
  python3 scripts/check_public_api.py
  python3 scripts/check_public_api.py --write
  python3 scripts/check_public_api.py --toolchain nightly-2026-09-03
  python3 scripts/check_public_api.py --self-test
"""

from __future__ import annotations

import argparse
import difflib
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

SNAPSHOT_DIR = REPO_ROOT / ".github" / "public-api"

# Scope. `expanse-trie` is the crate a Rust consumer depends on; the C ABI
# surface of `expanse-capi` is already pinned symbol-by-symbol by
# `check_abi_parity.py`, and the binding crates (`-py`, `-node`, `-wasm`,
# `-php`) export to their host language rather than to Rust. Adding a crate
# here means adding its snapshot with --write, and nothing else.
CRATES = ("expanse-trie",)

# The default toolchain. `cargo public-api` reads rustdoc JSON, which is
# nightly-only. CI passes its pinned channel explicitly so a rustdoc format
# change lands as one deliberate regeneration rather than a surprise red run.
DEFAULT_TOOLCHAIN = "nightly"

# Items #763 withdrew. They must never silently return: a snapshot that
# contains one of these again is either a revert nobody reviewed or a
# regenerate-to-green (AGENTS.md section 8.12.3 — pin the motivating defect).
WITHDRAWN = (
    "ExpanseBlobMap::arena_mut",
    "DomainOrdinal::new(",
)


def snapshot_path(crate: str) -> Path:
    return SNAPSHOT_DIR / f"{crate}.txt"


def render(crate: str, toolchain: str) -> str:
    """The crate's public API as `cargo public-api` prints it."""
    if shutil.which("cargo") is None:
        raise RuntimeError("cargo is not on PATH")
    probe = subprocess.run(
        ["cargo", f"+{toolchain}", "public-api", "--version"],
        capture_output=True,
        text=True,
        cwd=REPO_ROOT,
    )
    if probe.returncode != 0:
        raise RuntimeError(
            "`cargo +%s public-api` is unavailable: %s\n"
            "Install it with `cargo install cargo-public-api --locked` and the "
            "toolchain with `rustup toolchain install %s`."
            % (toolchain, (probe.stderr or probe.stdout).strip().splitlines()[-1:] or "", toolchain)
        )
    proc = subprocess.run(
        ["cargo", f"+{toolchain}", "public-api", "-p", crate, "--simplified"],
        capture_output=True,
        text=True,
        cwd=REPO_ROOT,
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"`cargo +{toolchain} public-api -p {crate}` failed with rc={proc.returncode}:\n{proc.stderr}"
        )
    lines = [ln.rstrip() for ln in proc.stdout.splitlines() if ln.strip()]
    if not lines:
        raise RuntimeError(
            f"`cargo public-api -p {crate}` produced an empty surface, which is "
            "not a plausible result for this crate -- refusing to treat it as a pass."
        )
    return "\n".join(lines) + "\n"


HEADER = (
    "# The public Rust API of `%s`, as `cargo public-api --simplified` prints it.\n"
    "#\n"
    "# Generated -- do not hand-edit. Regenerate a deliberate change with:\n"
    "#     python3 scripts/check_public_api.py --write\n"
    "#\n"
    "# A diff here is a public surface change. Under Cargo 0.x semver `^0.6`\n"
    "# spans every 0.6.x, so a removal or a signature change needs 0.7.0 and\n"
    "# never a patch release. See AGENTS.md section 6.5 and issue #797.\n"
)


def body(text: str) -> list[str]:
    """The snapshot's API lines, with the generated header stripped."""
    return [ln for ln in text.splitlines() if not ln.startswith("#")]


def withdrawn_items(lines: list[str]) -> list[str]:
    return [w for w in WITHDRAWN if any(w in ln for ln in lines)]


def diff_lines(crate: str, want: list[str], got: list[str]) -> list[str]:
    return list(
        difflib.unified_diff(
            want,
            got,
            fromfile=f"{snapshot_path(crate).relative_to(REPO_ROOT)} (committed)",
            tofile=f"{crate} (current)",
            lineterm="",
            n=1,
        )
    )


def check(toolchain: str, write: bool) -> list[str]:
    problems: list[str] = []
    SNAPSHOT_DIR.mkdir(parents=True, exist_ok=True)
    for crate in CRATES:
        current = render(crate, toolchain)
        path = snapshot_path(crate)

        returned = withdrawn_items(body(current))
        if returned:
            problems.append(
                f"{crate}: withdrawn public item(s) are back on the surface: "
                f"{', '.join(returned)}. These were removed by #763/#796; a "
                f"deliberate reinstatement edits WITHDRAWN in this script and "
                f"says why in the pull request."
            )

        want = HEADER % crate + current
        if write:
            path.write_text(want)
            print(f"check_public_api.py --write: {path.relative_to(REPO_ROOT)} "
                  f"({len(body(want))} public items)")
            continue

        if not path.exists():
            problems.append(
                f"{crate}: no committed snapshot at "
                f"{path.relative_to(REPO_ROOT)} -- generate it with "
                f"`python3 scripts/check_public_api.py --write`"
            )
            continue

        committed = body(path.read_text())
        observed = body(want)
        if committed != observed:
            d = diff_lines(crate, committed, observed)
            added = sum(1 for ln in d if ln.startswith("+") and not ln.startswith("+++"))
            removed = sum(1 for ln in d if ln.startswith("-") and not ln.startswith("---"))
            problems.append(
                f"{crate}: public API surface changed ({added} added, {removed} removed) "
                f"and the committed snapshot does not reflect it.\n"
                + "\n".join(d)
                + f"\n\nIf the change is deliberate, regenerate with "
                f"`python3 scripts/check_public_api.py --write` and commit "
                f"{path.relative_to(REPO_ROOT)}. Under Cargo 0.x semver a removal "
                f"or a signature change needs a 0.7.0, never a patch."
            )
    return problems


def self_test() -> None:
    """Fail-then-pass on the logic that decides, not on a helper beside it."""
    crate = "expanse-trie"

    # 1. The diff must see an addition the snapshot does not have. This is the
    #    #763 shape: a public item reaching a release unreviewed.
    base = ["pub fn expanse_trie::blobmap::ExpanseBlobMap::len(&self) -> usize"]
    added = base + ["pub fn expanse_trie::blobmap::ExpanseBlobMap::arena_mut(&mut self) -> &mut BlobArena"]
    d = diff_lines(crate, base, added)
    assert any(ln.startswith("+") and "arena_mut" in ln for ln in d), d
    assert base != added

    # 2. And a removal, which is the direction that costs users.
    d = diff_lines(crate, added, base)
    assert any(ln.startswith("-") and "arena_mut" in ln for ln in d), d

    # 3. The withdrawn-item tripwire fires on each historical item verbatim,
    #    and not on an unrelated surface.
    assert withdrawn_items(["pub fn expanse_trie::blobmap::ExpanseBlobMap::arena_mut(&mut self) -> ()"]) == [
        "ExpanseBlobMap::arena_mut"
    ]
    assert withdrawn_items(["pub const fn expanse_trie::domain::DomainOrdinal::new(u64, u64) -> Self"]) == [
        "DomainOrdinal::new("
    ]
    # `new_set` must not trip the `DomainOrdinal::new(` pattern -- the trailing
    # paren is what makes it a call site rather than a prefix.
    assert withdrawn_items(["pub fn expanse_trie::domain::ExpanseDomainDict::new_set(&self) -> DomainSet"]) == []
    assert withdrawn_items(base) == []

    # 4. The header must not leak into the compared body, or every regenerate
    #    would diff against itself.
    assert body(HEADER % crate + "pub mod expanse_trie\n") == ["pub mod expanse_trie"]

    # 5. The committed snapshot, if present, is a real surface and carries
    #    neither withdrawn item.
    path = snapshot_path(crate)
    if path.exists():
        lines = body(path.read_text())
        assert len(lines) > 100, f"{path} has only {len(lines)} items -- not a plausible surface"
        assert withdrawn_items(lines) == [], withdrawn_items(lines)

    print("check_public_api.py --self-test: all checks passed")


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--write", action="store_true", help="regenerate the committed snapshots")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument(
        "--toolchain",
        default=os.environ.get("EXPANSE_PUBLIC_API_TOOLCHAIN", DEFAULT_TOOLCHAIN),
        help=f"rustup toolchain to render with (default: {DEFAULT_TOOLCHAIN})",
    )
    args = ap.parse_args()

    if args.self_test:
        self_test()
        return 0

    try:
        problems = check(args.toolchain, args.write)
    except RuntimeError as exc:
        print(f"::error::{exc}", file=sys.stderr)
        return 1

    for p in problems:
        print(f"::error::{p}", file=sys.stderr)
    if problems:
        print(
            f"\ncheck_public_api.py: {len(problems)} crate(s) have an unreviewed public API change.",
            file=sys.stderr,
        )
        return 1
    if not args.write:
        print(
            "check_public_api.py: the public Rust API matches its committed snapshot "
            f"for {', '.join(CRATES)}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
