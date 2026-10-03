#!/usr/bin/env python3
"""Builds both APT and RPM package repositories for Expanse.

When --sign-key KEYID is provided, signs the APT Release file (generating
InRelease and Release.gpg), exports the binary public keyring to
apt/expanse-archive-keyring.gpg, signs the RPM repomd.xml (generating
repomd.xml.asc), and exports the armored public key to
rpm/RPM-GPG-KEY-expanse.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys

# Ensure local scripts directory is in sys.path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from build_apt_repo import build_apt_repo  # noqa: E402
from build_rpm_repo import build_rpm_repo  # noqa: E402


def get_default_version(repo_root: str | None = None) -> str:
    """Inspects workspace and crate Cargo.toml files to extract the package version."""
    if repo_root is None:
        repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    for toml_path in [
        os.path.join(repo_root, "Cargo.toml"),
        os.path.join(repo_root, "crates", "expanse", "Cargo.toml"),
        os.path.join(repo_root, "crates", "expanse-capi", "Cargo.toml"),
    ]:
        if os.path.isfile(toml_path):
            with open(toml_path, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line.startswith("version = ") or line.startswith("version="):
                        v = line.split("=", 1)[1].strip().strip('"\'')
                        if v and not v.endswith(".workspace"):
                            return v
    return "0.10.2"


def gpg(*argv: str) -> None:
    """Run gpg non-interactively. A passphrase-protected key reads its passphrase from
    REPO_SIGNING_PASSPHRASE through loopback pinentry: a CI runner has no TTY."""
    passphrase = os.environ.get("REPO_SIGNING_PASSPHRASE")
    extra = ["--pinentry-mode", "loopback", "--passphrase-fd", "0"] if passphrase else []
    subprocess.run(
        ["gpg", "--batch", "--yes", *extra, *argv],
        check=True,
        input=passphrase.encode() if passphrase else None,
    )


def sign_repositories(apt_out: str, rpm_out: str, key: str) -> None:
    """Sign the APT Release file and the RPM repomd.xml, and publish the public key."""
    release = os.path.join(apt_out, "dists", "stable", "Release")
    if not os.path.exists(release):
        sys.exit(f"error: {release} is missing; nothing to sign")
    gpg("--local-user", key, "--clearsign", "--output", os.path.join(os.path.dirname(release), "InRelease"), release)
    gpg("--local-user", key, "--armor", "--detach-sign", "--output", release + ".gpg", release)
    with open(os.path.join(apt_out, "expanse-archive-keyring.gpg"), "wb") as f:
        f.write(subprocess.run(["gpg", "--batch", "--export", key], check=True, capture_output=True).stdout)

    repomd = os.path.join(rpm_out, "repodata", "repomd.xml")
    if not os.path.exists(repomd):
        sys.exit(f"error: {repomd} is missing; nothing to sign")
    gpg("--local-user", key, "--armor", "--detach-sign", "--output", repomd + ".asc", repomd)
    with open(os.path.join(rpm_out, "RPM-GPG-KEY-expanse"), "wb") as f:
        f.write(subprocess.run(["gpg", "--batch", "--armor", "--export", key], check=True, capture_output=True).stdout)
    print(f"Signed APT Release and RPM repomd.xml with {key}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Build both APT and RPM package repositories for Expanse."
    )
    parser.add_argument(
        "--artifacts-dir",
        default="artifacts",
        help="Input directory containing .deb and .rpm release packages (default: artifacts)",
    )
    parser.add_argument(
        "--docs-dir",
        default="docs",
        help="Target docs root directory containing apt/ and rpm/ (default: docs)",
    )
    parser.add_argument(
        "--allow-empty",
        action="store_true",
        help="Permit building initial repository metadata when artifacts directory has no packages yet.",
    )
    parser.add_argument(
        "--version",
        default=None,
        help="Package version string (defaults to Cargo.toml version).",
    )
    parser.add_argument(
        "--sign-key",
        default=None,
        help="GPG key id that signs the APT Release file and the RPM repomd.xml. Required unless --unsigned.",
    )
    parser.add_argument(
        "--unsigned",
        action="store_true",
        help="Build without signatures (local previews only; never publish an unsigned repository).",
    )
    args = parser.parse_args()
    if not args.sign_key and not args.unsigned:
        sys.exit("error: pass --sign-key KEYID (or --unsigned for a local preview)")

    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    ver = args.version or get_default_version(repo_root)
    apt_out = os.path.join(args.docs_dir, "apt")
    rpm_out = os.path.join(args.docs_dir, "rpm")

    print(f"Building Expanse APT repository (v{ver}) into {apt_out}...")
    build_apt_repo(args.artifacts_dir, apt_out, allow_empty=args.allow_empty, version=ver)

    print(f"Building Expanse RPM repository (v{ver}) into {rpm_out}...")
    build_rpm_repo(args.artifacts_dir, rpm_out, allow_empty=args.allow_empty, version=ver)

    if args.sign_key:
        sign_repositories(apt_out, rpm_out, args.sign_key)

    print("All package repositories successfully generated.")


if __name__ == "__main__":
    main()
