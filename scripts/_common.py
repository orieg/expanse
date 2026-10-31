#!/usr/bin/env python3
"""Helpers shared by several scripts under `scripts/`.

- `gh_output` writes `key=value` lines to `$GITHUB_OUTPUT`. It replaced three
  copies (`bench_governor.py`, `bench_host_guard.py`, `bench_lock.py`) that had
  drifted: only `bench_lock`'s passed each value through `output_value`. The
  sanitising form is the correct one. `$GITHUB_OUTPUT` is a line-oriented file,
  so a value carrying a newline writes a second `key=value` line of its own
  choosing, and the values that reach it include text other processes wrote
  (the lock holder's owner record, an exception message). The constant reason
  codes the other two scripts write pass through unchanged.
- `repo_root` is the checkout this file sits in, found from its own path. It
  replaced the `get_repo_root()` copies, which all computed exactly that.
- `git_toplevel` is the top level of the git work tree containing the current
  directory. It is a different question from `repo_root`: a copy of `scripts/`
  extracted outside the checkout (the bare-metal workflow's `bench-tools`) has
  no repository around it, and the scripts that ask it mean the checkout they
  are run in. It fails loudly outside a work tree (AGENTS.md 8.1).

Scripts import this module by name: running `python3 scripts/<name>.py` puts
`scripts/` first on `sys.path`.

    python3 scripts/_common.py --self-test
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

# The character set a workflow output value may carry: no newline, no
# backtick (a value can end up inside a markdown code fence in a PR comment).
_OUTPUT_UNSAFE = re.compile(r"[^A-Za-z0-9_.:=/ +\[\]-]")


def output_value(text: str, limit: int = 300) -> str:
    """`text` reduced to one line of a closed character set, bounded in length.

    The first line only, with every character outside the set replaced by `?`;
    an empty result reads `unknown`, never an empty value.
    """
    stripped = text.strip()
    line = stripped.splitlines()[0] if stripped else ""
    return _OUTPUT_UNSAFE.sub("?", line)[:limit] or "unknown"


def gh_output(**kv: str) -> None:
    """Append each `key=value` to `$GITHUB_OUTPUT`; a no-op when it is unset."""
    out = os.environ.get("GITHUB_OUTPUT")
    if not out:
        return
    with open(out, "a", encoding="utf-8") as fh:
        for k, v in kv.items():
            fh.write(f"{k}={output_value(v)}\n")


def repo_root() -> Path:
    """The checkout this file belongs to: the parent of `scripts/`."""
    return Path(__file__).resolve().parent.parent


def git_toplevel() -> Path:
    """`git rev-parse --show-toplevel` from the current directory.

    Raises `subprocess.CalledProcessError` outside a git work tree.
    """
    return Path(subprocess.run(["git", "rev-parse", "--show-toplevel"],
                               capture_output=True, text=True, check=True).stdout.strip())


def _self_test() -> int:
    assert output_value("host_busy") == "host_busy"
    assert output_value("governor_unavailable") == "governor_unavailable"
    assert output_value("a`b\nsecond") == "a?b"
    assert output_value("  \n ") == "unknown"
    assert output_value("x" * 400) == "x" * 300
    assert output_value("pid=12 host=a/b [x]") == "pid=12 host=a/b [x]"

    with tempfile.TemporaryDirectory() as d:
        out = Path(d) / "out"
        old = os.environ.get("GITHUB_OUTPUT")
        os.environ["GITHUB_OUTPUT"] = str(out)
        try:
            gh_output(fail_reason="lock_held", lock_owner="pid=1\ninjected=true")
            gh_output(fail_reason="no_huge_pages")
        finally:
            if old is None:
                del os.environ["GITHUB_OUTPUT"]
            else:
                os.environ["GITHUB_OUTPUT"] = old
        # A newline in a value must not write a line of its own.
        assert out.read_text(encoding="utf-8") == (
            "fail_reason=lock_held\nlock_owner=pid=1\nfail_reason=no_huge_pages\n"
        ), out.read_text(encoding="utf-8")
        os.environ.pop("GITHUB_OUTPUT", None)
        gh_output(fail_reason="x")  # unset: writes nothing, raises nothing
        if old is not None:
            os.environ["GITHUB_OUTPUT"] = old

    assert (repo_root() / "scripts" / "_common.py").is_file()
    print("_common self-test: ok")
    return 0


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        sys.exit(_self_test())
    sys.exit("usage: _common.py --self-test")
