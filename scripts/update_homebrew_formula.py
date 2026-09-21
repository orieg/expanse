#!/usr/bin/env python3
"""Render the Homebrew formula and the MacPorts Portfile for a release.

Both are templates (`extra/homebrew/expanse.rb.in`, `extra/macports/Portfile.in`)
with `@VERSION@`, `@SHA256_<TARGET>@` and `@SIZE_<TARGET>@` placeholders, filled
from the release archives themselves. Neither template carries a version, so
neither can drift from the manifests `scripts/bump_version.py` keeps in lockstep.

Fail-loud by construction (AGENTS.md section 8.1): a missing archive, a
placeholder left unfilled or a checksum that is not 64 hex digits is an error.
A formula with a placeholder checksum would install nothing and say why only
at `brew install`, on a user's machine.

Usage:
  update_homebrew_formula.py --version 0.7.0 --artifacts-dir artifacts \
      --formula-out dist/expanse.rb --portfile-out dist/Portfile
  update_homebrew_formula.py --version 0.7.0 --artifacts-dir artifacts \
      --push-to-tap orieg/homebrew-tap          # deploy key in HOMEBREW_TAP_DEPLOY_KEY
  update_homebrew_formula.py --self-test
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
FORMULA_TEMPLATE = REPO_ROOT / "extra" / "homebrew" / "expanse.rb.in"
PORTFILE_TEMPLATE = REPO_ROOT / "extra" / "macports" / "Portfile.in"
DEPLOY_KEY_ENV = "HOMEBREW_TAP_DEPLOY_KEY"

PLACEHOLDER = re.compile(r"@([A-Z0-9_]+)@")
TARGET_FIELD = re.compile(r"^(SHA256|SIZE)_([A-Z0-9_]+)$")
VERSION_RE = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")


class RenderError(RuntimeError):
    """A template could not be filled completely and correctly."""


def placeholder_target(field: str) -> str:
    """`AARCH64_APPLE_DARWIN` -> `aarch64-apple-darwin`.

    `x86_64` keeps its underscore: it is the one target component that has one.
    """
    return field.lower().replace("x86_64", "x86~64").replace("_", "-").replace("x86~64", "x86_64")


def archive_name(version: str, target: str) -> str:
    return f"expanse-{version}-{target}.tar.gz"


def archive_facts(artifacts_dir: Path, version: str, target: str) -> dict[str, str]:
    path = artifacts_dir / archive_name(version, target)
    if not path.is_file():
        raise RenderError(f"release archive not found: {path}")
    data = path.read_bytes()
    if not data:
        raise RenderError(f"release archive is empty: {path}")
    return {"SHA256": hashlib.sha256(data).hexdigest(), "SIZE": str(len(data))}


def render(template: str, version: str, artifacts_dir: Path) -> str:
    """Fill every placeholder; raise if one is unknown or an archive is absent."""
    if not VERSION_RE.match(version):
        raise RenderError(f"not a release version: {version!r}")
    cache: dict[str, dict[str, str]] = {}

    def fill(match: re.Match[str]) -> str:
        name = match.group(1)
        if name == "VERSION":
            return version
        field = TARGET_FIELD.match(name)
        if not field:
            raise RenderError(f"unknown placeholder @{name}@")
        target = placeholder_target(field.group(2))
        if target not in cache:
            cache[target] = archive_facts(artifacts_dir, version, target)
        return cache[target][field.group(1)]

    out = PLACEHOLDER.sub(fill, template)
    if not cache:
        raise RenderError("template names no release archive")
    for target, facts in cache.items():
        if not re.fullmatch(r"[0-9a-f]{64}", facts["SHA256"]):
            raise RenderError(f"bad sha256 for {target}")
    return out


def check_ruby_syntax(content: str) -> None:
    """`ruby -c` when ruby is present; its absence is reported, never a pass."""
    try:
        proc = subprocess.run(["ruby", "-c"], input=content, text=True, capture_output=True, check=False)
    except FileNotFoundError:
        print("::notice::ruby not found; formula syntax check skipped", file=sys.stderr)
        return
    if proc.returncode != 0:
        raise RenderError(f"rendered formula is not valid Ruby:\n{proc.stderr}")


def push_to_tap(tap_repo: str, deploy_key: str, formula: str, version: str) -> None:
    """Commit `Formula/expanse.rb` to the tap over SSH with a deploy key."""
    with tempfile.TemporaryDirectory() as tmp:
        key_file = Path(tmp) / "id_deploy"
        key_file.write_text(deploy_key.strip() + "\n", encoding="utf-8")
        os.chmod(key_file, 0o600)
        env = os.environ.copy()
        env["GIT_SSH_COMMAND"] = f"ssh -i {key_file} -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new"
        local = tap_repo.startswith("/") or tap_repo.startswith("file://")
        url = tap_repo if local else f"git@github.com:{tap_repo}.git"
        repo = Path(tmp) / "tap"

        def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
            return subprocess.run(["git", "-C", str(repo), *args], env=env, text=True,
                                  capture_output=True, check=check)

        subprocess.run(["git", "clone", "--depth", "1", url, str(repo)], env=env, check=True,
                       text=True, capture_output=True)
        (repo / "Formula").mkdir(exist_ok=True)
        (repo / "Formula" / "expanse.rb").write_text(formula, encoding="utf-8")
        git("config", "user.name", "github-actions[bot]")
        git("config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com")
        git("add", "Formula/expanse.rb")
        if git("diff", "--staged", "--quiet", check=False).returncode == 0:
            print(f"Formula/expanse.rb in {tap_repo} is already at v{version}")
            return
        git("commit", "-m", f"chore(expanse): bump formula to v{version}")
        git("push", "origin", "HEAD")
        print(f"pushed Formula/expanse.rb v{version} to {tap_repo}")


def self_test() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        art = Path(tmp)
        targets = ["aarch64-apple-darwin", "x86_64-apple-darwin",
                   "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]
        for i, target in enumerate(targets):
            (art / archive_name("1.2.3", target)).write_bytes(b"archive-%d" % i)

        assert placeholder_target("X86_64_APPLE_DARWIN") == "x86_64-apple-darwin"
        assert placeholder_target("AARCH64_UNKNOWN_LINUX_GNU") == "aarch64-unknown-linux-gnu"

        for template_path in (FORMULA_TEMPLATE, PORTFILE_TEMPLATE):
            template = template_path.read_text(encoding="utf-8")
            out = render(template, "1.2.3", art)
            assert not PLACEHOLDER.search(out), f"{template_path.name}: placeholder survived"
            assert "1.2.3" in out
            want = hashlib.sha256(b"archive-0").hexdigest()
            assert want in out, f"{template_path.name}: arm64 macOS checksum missing"
        portfile = render(PORTFILE_TEMPLATE.read_text(encoding="utf-8"), "1.2.3", art)
        assert f"size    {len(b'archive-0')}" in portfile
        check_ruby_syntax(render(FORMULA_TEMPLATE.read_text(encoding="utf-8"), "1.2.3", art))

        # Fail-loud controls: each must raise, and for its own reason.
        (art / archive_name("1.2.3", "x86_64-apple-darwin")).unlink()
        for bad_template, version, needle in (
            (FORMULA_TEMPLATE.read_text(encoding="utf-8"), "1.2.3", "archive not found"),
            ("@SHA256_AARCH64_APPLE_DARWIN@ @BOGUS@", "1.2.3", "unknown placeholder"),
            ("@VERSION@", "1.2.3", "names no release archive"),
            ("@SHA256_AARCH64_APPLE_DARWIN@", "v1.2.3", "not a release version"),
        ):
            try:
                render(bad_template, version, art)
            except RenderError as err:
                assert needle in str(err), f"wrong failure: {err}"
            else:
                raise AssertionError(f"render accepted a bad input ({needle})")

        # Push path against a local bare repository: commit lands, re-push is a no-op.
        bare = Path(tmp) / "tap.git"
        subprocess.run(["git", "init", "--bare", "-q", "-b", "main", str(bare)], check=True)
        seed = Path(tmp) / "seed"
        subprocess.run(["git", "clone", "-q", str(bare), str(seed)], check=True, capture_output=True)
        (seed / "README.md").write_text("tap\n", encoding="utf-8")
        for cmd in (["add", "."], ["-c", "user.name=t", "-c", "user.email=t@example.invalid",
                                   "commit", "-q", "-m", "seed"], ["push", "-q", "origin", "HEAD:main"]):
            subprocess.run(["git", "-C", str(seed), *cmd], check=True, capture_output=True)
        for _ in range(2):
            push_to_tap(str(bare), "not-a-real-key", "class Expanse < Formula\nend\n", "1.2.3")
        log = subprocess.run(["git", "-C", str(bare), "log", "--format=%s", "main"],
                             check=True, text=True, capture_output=True).stdout.splitlines()
        assert log == ["chore(expanse): bump formula to v1.2.3", "seed"], log
    print("update_homebrew_formula.py self-test: ok")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--version", help="release version without the leading v")
    parser.add_argument("--artifacts-dir", type=Path, help="directory holding the release archives")
    parser.add_argument("--formula-out", type=Path)
    parser.add_argument("--portfile-out", type=Path)
    parser.add_argument("--push-to-tap", metavar="OWNER/REPO",
                        help=f"push the formula to this tap; the deploy key is read from ${DEPLOY_KEY_ENV}")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if not args.version or not args.artifacts_dir:
        parser.error("--version and --artifacts-dir are required")
    try:
        formula = render(FORMULA_TEMPLATE.read_text(encoding="utf-8"), args.version, args.artifacts_dir)
        portfile = render(PORTFILE_TEMPLATE.read_text(encoding="utf-8"), args.version, args.artifacts_dir)
        check_ruby_syntax(formula)
    except RenderError as err:
        print(f"::error::{err}", file=sys.stderr)
        return 1
    for out, text in ((args.formula_out, formula), (args.portfile_out, portfile)):
        if out:
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_text(text, encoding="utf-8")
            print(f"wrote {out}")
    if args.push_to_tap:
        key = os.environ.get(DEPLOY_KEY_ENV, "")
        if not key.strip():
            print(f"::error::--push-to-tap needs the deploy key in ${DEPLOY_KEY_ENV}", file=sys.stderr)
            return 1
        push_to_tap(args.push_to_tap, key, formula, args.version)
    return 0


if __name__ == "__main__":
    sys.exit(main())
