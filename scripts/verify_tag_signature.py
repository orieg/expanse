#!/usr/bin/env python3
"""Release gate: the release tag must be signed by a key committed to this repository.

`release.yml` runs on a pushed `v*` tag and publishes to registries that cannot
take a version back. The tag is the maintainer's: `scripts/tag_release.py`
creates it with `git tag -s`. This check is the other half. It fails unless

  1. the tag is an annotated tag (a lightweight tag carries no signature);
  2. its signature is good, by a key that is neither expired nor revoked;
  3. the signing key's fingerprint is one of the keys in the committed keyring;
  4. the signed object names this tag (a signed `v1.0.0` object placed at
     another ref name is not a signature of that name); and
  5. with `--sha`, it peels to that commit: the one the run is building. A
     tag moved after the push is then refused, whatever it was moved to.

Only the committed keyring is trusted. The check runs `git verify-tag` with a
fresh, empty GnuPG home into which that one file is imported, so a key that
happens to be on the runner, or one fetched from a key server, cannot make a
tag pass. The verdict needs both `git verify-tag` exiting 0 and a
`VALIDSIG <fingerprint>` status line: GnuPG prints `VALIDSIG` for a
signature by an expired or revoked key too, and reports those through the
exit status and `EXPKEYSIG` / `REVKEYSIG`.

Run:  verify_tag_signature.py --tag vX.Y.Z [--sha COMMIT] [--keyring FILE] [--repo DIR]
      verify_tag_signature.py --self-test
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

DEFAULT_KEYRING = ".github/release-signing-keys.asc"


class Refused(Exception):
    """The tag is not an acceptable release tag."""


def _run(cmd: list[str], cwd: Path | None = None, env: dict | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True)


def _gpg_env(home: Path) -> dict:
    env = dict(os.environ)
    env["GNUPGHOME"] = str(home)
    return env


def keyring_fingerprints(keyring: Path, home: Path) -> set[str]:
    """Imports `keyring` into the empty GnuPG home `home`; returns its primary-key fingerprints."""
    if not keyring.is_file():
        raise Refused(f"keyring {keyring} does not exist")
    env = _gpg_env(home)
    imp = _run(["gpg", "--batch", "--quiet", "--import", str(keyring)], env=env)
    if imp.returncode != 0:
        raise Refused(f"cannot import {keyring}: {imp.stderr.strip()}")
    listing = _run(["gpg", "--batch", "--with-colons", "--fingerprint", "--list-keys"], env=env)
    if listing.returncode != 0:
        raise Refused(f"cannot list the imported keys: {listing.stderr.strip()}")
    fingerprints: set[str] = set()
    after_pub = False
    for line in listing.stdout.splitlines():
        fields = line.split(":")
        if fields[0] == "pub":
            after_pub = True
        elif fields[0] == "sub":
            after_pub = False
        elif fields[0] == "fpr" and after_pub:
            fingerprints.add(fields[9])
            after_pub = False
    if not fingerprints:
        raise Refused(f"{keyring} holds no public key")
    return fingerprints


def valid_signers(status: str) -> set[str]:
    """Primary-key fingerprints GnuPG reports a valid signature from.

    A `VALIDSIG` status line ends with the primary key's fingerprint; its
    second field is the fingerprint of the key that signed, which is a subkey
    when one was used. Either identifies the owner; the primary is what a
    keyring lists.
    """
    signers: set[str] = set()
    for line in status.splitlines():
        parts = line.split()
        if len(parts) >= 3 and parts[0] == "[GNUPG:]" and parts[1] == "VALIDSIG":
            signers.add(parts[-1])
    return signers


REFUSING_STATUS = {
    "EXPKEYSIG": "the signing key has expired",
    "REVKEYSIG": "the signing key is revoked",
    "EXPSIG": "the signature has expired",
    "BADSIG": "the signature does not match the tag",
}


def verify(repo: Path, tag: str, keyring: Path, sha: str | None = None) -> str:
    """Returns the fingerprint that signed `tag`, or raises `Refused`."""
    kind = _run(["git", "cat-file", "-t", f"refs/tags/{tag}"], cwd=repo)
    if kind.returncode != 0:
        raise Refused(f"tag {tag} does not exist in {repo}. Create the tag with scripts/tag_release.py and push it")
    if kind.stdout.strip() != "tag":
        raise Refused(f"{tag} is a lightweight tag ({kind.stdout.strip()}): a release tag is annotated and signed. "
                      f"Re-create the tag with scripts/tag_release.py, which creates a signed annotated tag")
    with tempfile.TemporaryDirectory(prefix="verify-tag-") as tmp:
        home = Path(tmp)
        home.chmod(0o700)
        trusted = keyring_fingerprints(keyring, home)
        checked = _run(["git", "verify-tag", "--raw", tag], cwd=repo, env=_gpg_env(home))
    # `--raw` prints GnuPG's status lines on stderr.
    statuses = {parts[1] for parts in (line.split() for line in checked.stderr.splitlines())
                if len(parts) >= 2 and parts[0] == "[GNUPG:]"}
    for status, meaning in REFUSING_STATUS.items():
        if status in statuses:
            raise Refused(f"{tag}: {meaning} ({status}). Nothing is published yet: update "
                          f"{keyring.name} or sign again with a current key, under a new tag")
    signers = valid_signers(checked.stderr)
    if not signers:
        unsigned = "no signature found" in checked.stderr or "NODATA" in checked.stderr
        reason = "is not signed" if unsigned else "has no valid signature from a key in the keyring"
        raise Refused(f"{tag} {reason}. Create the tag with scripts/tag_release.py, which signs it; "
                      f"if already signed, ensure the key is committed in {keyring.name} and not expired, and that gpg is installed")
    if checked.returncode != 0:
        raise Refused(f"{tag}: git verify-tag exited {checked.returncode} although a signature was found: "
                      f"{' '.join(sorted(statuses)) or checked.stderr.strip()[:200]}. Check GnuPG status lines or re-sign the tag")
    accepted = signers & trusted
    if not accepted:
        raise Refused(f"{tag} is signed by {', '.join(sorted(signers))}, which is not in {keyring.name}. "
                      f"Add the signing key to {keyring.name} or sign with a trusted key under a new tag")
    # The signature covers the tag object, whose `tag` line is the name it
    # was signed as; a ref can point at any object.
    obj = _run(["git", "cat-file", "tag", f"refs/tags/{tag}"], cwd=repo).stdout
    named = next((line[4:] for line in obj.split("\n\n", 1)[0].splitlines() if line.startswith("tag ")), None)
    if named != tag:
        raise Refused(f"the object at refs/tags/{tag} is a signed tag named {named!r}, not {tag!r}. "
                      f"Re-tag with the correct name or re-create {tag} with scripts/tag_release.py")
    if sha is not None:
        peeled = _run(["git", "rev-parse", "--verify", "--quiet", f"refs/tags/{tag}^{{commit}}"], cwd=repo).stdout.strip()
        if peeled != sha:
            raise Refused(f"{tag} points at {peeled[:12] or 'no commit'}, and this run builds {sha[:12]}: "
                          f"the tag moved after it was pushed. A release tag is never re-pushed; bump the version")
    return sorted(accepted)[0]


def self_test() -> int:
    failures: list[str] = []

    def refused(label: str, fn, needle: str) -> None:
        try:
            got = fn()
        except Refused as exc:
            if needle not in str(exc):
                failures.append(f"{label}: refused for the wrong reason: {exc}")
            return
        failures.append(f"{label}: was accepted ({got})")

    # Parsing, with no GnuPG involved. A subkey signature names the primary last.
    primary, subkey = "A" * 40, "B" * 40
    status = (f"[GNUPG:] NEWSIG\n[GNUPG:] GOODSIG {subkey[-16:]} someone\n"
              f"[GNUPG:] VALIDSIG {subkey} 2026-10-05 1 0 4 1 10 00 {primary}\n")
    if valid_signers(status) != {primary}:
        failures.append(f"VALIDSIG parse: got {valid_signers(status)}")
    if valid_signers("[GNUPG:] BADSIG 0123 someone\n") or valid_signers("[GNUPG:] ERRSIG 0123 1 10 00 0 9\n"):
        failures.append("a bad or unverifiable signature was read as valid")
    if valid_signers("gpg: Good signature from someone\n"):
        failures.append("prose was read as a status line")

    # End to end, with two throwaway keys in a throwaway GnuPG home.
    with tempfile.TemporaryDirectory(prefix="verify-tag-test-") as tmp:
        root = Path(tmp)
        gpg_home = root / "gnupg"
        gpg_home.mkdir(mode=0o700)
        env = _gpg_env(gpg_home)

        def make_key(name: str) -> str:
            proc = _run(["gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                         "--quick-generate-key", f"{name} <{name}@example.invalid>", "ed25519", "sign", "never"],
                        env=env)
            if proc.returncode != 0:
                raise RuntimeError(f"cannot generate a test key: {proc.stderr.strip()}")
            out = _run(["gpg", "--batch", "--with-colons", "--list-keys", f"{name}@example.invalid"], env=env).stdout
            return next(l.split(":")[9] for l in out.splitlines() if l.startswith("fpr:"))

        try:
            maintainer, other = make_key("maintainer"), make_key("other")
        except (RuntimeError, OSError, StopIteration) as exc:
            print(f"::error::verify_tag_signature self-test: {exc}")
            return 1

        keyring = root / "keys.asc"
        keyring.write_text(_run(["gpg", "--batch", "--armor", "--export", maintainer], env=env).stdout)
        empty = root / "empty.asc"
        empty.write_text("")

        repo = root / "repo"
        repo.mkdir()

        def git(*args: str) -> None:
            proc = _run(["git", *args], cwd=repo, env=env)
            if proc.returncode != 0:
                raise RuntimeError(f"git {' '.join(args)}: {proc.stderr.strip()}")

        git("init", "--quiet")
        git("config", "user.name", "t")
        git("config", "user.email", "t@example.invalid")
        git("config", "commit.gpgsign", "false")
        (repo / "f").write_text("x")
        git("add", "f")
        git("commit", "--quiet", "-m", "c")
        git("tag", "-s", "-u", maintainer, "-m", "signed", "v1.0.0")
        git("tag", "-s", "-u", other, "-m", "signed by another key", "v1.0.1")
        git("tag", "-a", "--no-sign", "-m", "annotated, unsigned", "v1.0.2")
        git("tag", "--no-sign", "v1.0.3")

        try:
            got = verify(repo, "v1.0.0", keyring)
            if got != maintainer:
                failures.append(f"maintainer-signed tag: accepted as {got}, want {maintainer}")
        except Refused as exc:
            failures.append(f"maintainer-signed tag was refused: {exc}")
        refused("signed by a key outside the keyring", lambda: verify(repo, "v1.0.1", keyring),
                "no valid signature from a key in the keyring")
        refused("annotated but unsigned", lambda: verify(repo, "v1.0.2", keyring), "is not signed")
        refused("lightweight tag", lambda: verify(repo, "v1.0.3", keyring), "lightweight tag")
        refused("missing tag", lambda: verify(repo, "v9.9.9", keyring), "does not exist")
        refused("missing keyring", lambda: verify(repo, "v1.0.0", root / "nope.asc"), "does not exist")

        head = _run(["git", "rev-parse", "HEAD"], cwd=repo).stdout.strip()
        (repo / "g").write_text("y")
        git("add", "g")
        git("commit", "--quiet", "-m", "d")
        later = _run(["git", "rev-parse", "HEAD"], cwd=repo).stdout.strip()
        # Bound to the commit being built.
        try:
            if verify(repo, "v1.0.0", keyring, head) != maintainer:
                failures.append("the tag on the built commit was not accepted")
        except Refused as exc:
            failures.append(f"the tag on the built commit was refused: {exc}")
        refused("a tag that points at another commit than the one built",
                lambda: verify(repo, "v1.0.0", keyring, later), "the tag moved after it was pushed")
        # A signed object under another ref name is not a signature of that name.
        obj = _run(["git", "rev-parse", "refs/tags/v1.0.0"], cwd=repo).stdout.strip()
        git("update-ref", "refs/tags/v2.0.0", obj)
        refused("the signed v1.0.0 object placed at refs/tags/v2.0.0",
                lambda: verify(repo, "v2.0.0", keyring), "is a signed tag named 'v1.0.0'")
        # THE DEFECT: an expired key. GnuPG still prints VALIDSIG for it.
        expiring = make_key("expiring")
        exp_ring = root / "expiring.asc"
        git("tag", "-s", "-u", expiring, "-m", "signed before expiry", "v1.1.0")
        proc = _run(["gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                     "--quick-set-expire", expiring, "seconds=1"], env=env)
        if proc.returncode != 0:
            failures.append(f"cannot expire the test key: {proc.stderr.strip()}")
        import time
        time.sleep(2.5)
        exp_ring.write_text(_run(["gpg", "--batch", "--armor", "--export", expiring], env=env).stdout)
        raw = _run(["git", "verify-tag", "--raw", "v1.1.0"], cwd=repo, env=env).stderr
        if "VALIDSIG" not in raw or "EXPKEYSIG" not in raw:
            failures.append(f"the fixture did not produce VALIDSIG with EXPKEYSIG: {raw!r}")
        refused("signed by a key that has since expired", lambda: verify(repo, "v1.1.0", exp_ring),
                "the signing key has expired")
        refused("empty keyring", lambda: verify(repo, "v1.0.0", empty), "cannot import")

        # The runner's own keys are not consulted: with the signer's key in the
        # ambient GnuPG home and a keyring holding another key, the tag is refused.
        other_ring = root / "other.asc"
        other_ring.write_text(_run(["gpg", "--batch", "--armor", "--export", other], env=env).stdout)
        saved = os.environ.get("GNUPGHOME")
        os.environ["GNUPGHOME"] = str(gpg_home)
        try:
            refused("signer known to the runner but absent from the keyring",
                    lambda: verify(repo, "v1.0.0", other_ring), "no valid signature from a key in the keyring")
        finally:
            if saved is None:
                del os.environ["GNUPGHOME"]
            else:
                os.environ["GNUPGHOME"] = saved

    if failures:
        for f in failures:
            print(f"::error::verify_tag_signature self-test: {f}")
        return 1
    print("verify_tag_signature --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tag")
    ap.add_argument("--sha", help="the commit the tag must peel to (the one being built)")
    ap.add_argument("--keyring", type=Path)
    ap.add_argument("--repo", type=Path, default=Path(__file__).resolve().parent.parent)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.tag:
        print("::error::--tag is required", file=sys.stderr)
        return 1
    keyring = args.keyring or args.repo / DEFAULT_KEYRING
    try:
        signer = verify(args.repo, args.tag, keyring, args.sha)
    except Refused as exc:
        print(f"::error::{exc}")
        return 1
    print(f"{args.tag} is signed by {signer}, a key in {keyring.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
