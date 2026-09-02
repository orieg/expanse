#!/usr/bin/env python3
"""Compile, run, and verify the EXAMPLES program in each Section 3 man page.

`check_man_pages.py` validates *form*: the pages exist, the troff is clean, and
every C ABI symbol is mentioned somewhere. It cannot tell whether the prose is
true. It passed a page that documented `expanse_set_by_count` as 1-indexed when
the implementation is 0-based, and whose worked example printed the 3rd key
while claiming to print the 2nd (#419).

Compiling the example would not have caught that either: the wrong program
compiled cleanly and exited 0. What catches it is running the example and
diffing its output against the output the page *documents*.

So each page with an EXAMPLES program also carries an `Example Output` block,
and this script asserts the two agree. A documented line may contain an
`<angle-bracket>` placeholder, which matches any text on that line — used for
values that legitimately vary, such as the version string.

Usage:
  python3 scripts/check_man_examples.py [--lib-dir target/release]
  python3 scripts/check_man_examples.py --self-test
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# Pages that are pure reference (types, error codes, conventions) and carry no
# runnable program. Listed explicitly so a page that *loses* its example is a
# failure rather than a silent skip.
NO_EXAMPLE_PAGES = {"Judy.3"}

# Pages whose symbols exist only in a 32-bit libexpanse (the
# `!EXPANSE_WIDE_SURFACE` block of expanse.h). Their EXAMPLES program must be
# present — it is the reference usage — but it cannot link against the
# 64-bit host library this checker builds against; the i686 CI lane compiles
# and runs the same shape with -m32 (crates/expanse-capi/smoke/narrow_api_smoke.c).
NARROW_SURFACE_PAGES = {"expanse_sync32.3"}

OUTPUT_HEADING = "Example Output"


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def extract_example_code(src: str) -> str | None:
    """First .nf/.fi block after .SH EXAMPLES, unescaped to compilable C."""
    i = src.find(".SH EXAMPLES")
    if i < 0:
        return None
    # Not necessarily adjacent: a page may carry prose between the heading and
    # the code block (expanse.3 does).
    m = re.search(r"\.nf\n(.*?)\n\.fi", src[i:], re.S)
    if not m:
        return None
    code = m.group(1).replace("\\\\", "\\")
    code = re.sub(r"\\f[BIRP]", "", code)
    return code if "int main" in code else None


def extract_documented_output(src: str) -> str | None:
    """The .nf/.fi block under the `Example Output` subsection."""
    m = re.search(
        rf"\.SS {re.escape(OUTPUT_HEADING)}\n(?:.*?\n)??\.nf\n(.*?)\n\.fi",
        src,
        re.S,
    )
    if not m:
        return None
    return re.sub(r"\\f[BIRP]", "", m.group(1).replace("\\\\", "\\"))


def outputs_match(documented: str, actual: str) -> bool:
    """Line-for-line, with `<placeholder>` matching any text on that line."""
    d_lines = documented.rstrip("\n").split("\n")
    a_lines = actual.rstrip("\n").split("\n")
    if len(d_lines) != len(a_lines):
        return False
    for d, a in zip(d_lines, a_lines):
        if "<" in d and ">" in d:
            pattern = "".join(
                ".*" if part.startswith("<") and part.endswith(">") else re.escape(part)
                for part in re.split(r"(<[^<>]*>)", d)
            )
            if not re.fullmatch(pattern, a):
                return False
        elif d != a:
            return False
    return True


def build_and_run(code: str, include_dir: Path, lib_dir: Path) -> tuple[int, str, str]:
    with tempfile.TemporaryDirectory() as td:
        c_file = Path(td) / "example.c"
        c_file.write_text(code, encoding="utf-8")
        binary = Path(td) / "example"
        cc = os.environ.get("CC", "cc")
        compile_res = subprocess.run(
            [cc, "-I", str(include_dir), str(c_file), "-L", str(lib_dir), "-lexpanse", "-o", str(binary)],
            capture_output=True,
            text=True,
        )
        if compile_res.returncode != 0:
            return compile_res.returncode, "", compile_res.stderr
        env = dict(os.environ)
        # The example links the shared library out of the build directory.
        for var in ("LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH"):
            env[var] = f"{lib_dir}{os.pathsep}{env.get(var, '')}".rstrip(os.pathsep)
        run_res = subprocess.run([str(binary)], capture_output=True, text=True, env=env, timeout=120)
        return run_res.returncode, run_res.stdout, run_res.stderr


def self_test() -> int:
    assert outputs_match("a\nb", "a\nb")
    assert not outputs_match("a\nb", "a\nc"), "differing line must fail"
    assert not outputs_match("a", "a\nb"), "extra output line must fail"
    assert not outputs_match("a\nb", "a"), "missing output line must fail"
    assert outputs_match("version: <v>", "version: 0.4.1-dev (v0.4.1-55-gabc)"), "placeholder must match"
    assert not outputs_match("version: <v>", "release: 0.4.1"), "placeholder must not match a different prefix"
    # The regression this gate exists for: the pre-#419 page documented the 2nd
    # key as 100, which is the 3rd. A pinned output makes that a failure.
    assert not outputs_match("2nd key: 50", "2nd key: 100"), "wrong documented value must fail"

    src = (
        ".SH EXAMPLES\nprose\n.PP\n.nf\nint main(void) { return 0; }\n.fi\n"
        ".SS Example Output\n.nf\nhello\n.fi\n"
    )
    assert extract_example_code(src) == "int main(void) { return 0; }", "code extraction"
    assert extract_documented_output(src) == "hello", "output extraction"
    assert extract_example_code(".SH NAME\nx\n") is None, "page without EXAMPLES"

    print("check_man_examples.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--lib-dir", default="target/release", help="directory holding libexpanse (default: target/release)")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()

    root = repo_root()
    include_dir = root / "include"
    lib_dir = (root / args.lib_dir).resolve()

    if not any(lib_dir.glob("libexpanse.*")):
        print(f"::error::no libexpanse found in {lib_dir} — run `cargo build --release -p expanse-capi` first")
        return 1
    if shutil.which(os.environ.get("CC", "cc")) is None:
        print("::error::no C compiler found; the man-page examples cannot be verified")
        return 1

    errors: list[str] = []
    checked = 0
    for page in sorted((root / "man" / "man3").glob("*.3")):
        src = page.read_text(encoding="utf-8")
        code = extract_example_code(src)
        if code is None:
            if page.name not in NO_EXAMPLE_PAGES:
                errors.append(f"{page.name}: no runnable EXAMPLES program found (add one, or list the page in NO_EXAMPLE_PAGES)")
            continue

        if page.name in NARROW_SURFACE_PAGES:
            # Present and syntactically a program, but only linkable at 32-bit.
            if "int main(" not in code:
                errors.append(f"{page.name}: narrow-surface EXAMPLES block is not a program")
            print(f"  {page.name}: 32-bit-only surface — example present, verified on the i686 lane, not built here")
            continue

        documented = extract_documented_output(src)
        if documented is None:
            errors.append(f"{page.name}: EXAMPLES program has no '.SS {OUTPUT_HEADING}' block to verify it against")
            continue

        rc, stdout, stderr = build_and_run(code, include_dir, lib_dir)
        if rc != 0:
            errors.append(f"{page.name}: example failed (exit {rc}): {stderr.strip().splitlines()[:3]}")
            continue

        if not outputs_match(documented, stdout):
            errors.append(
                f"{page.name}: documented output does not match what the example prints.\n"
                f"      documented:\n{documented}\n"
                f"      actual:\n{stdout.rstrip()}"
            )
            continue
        checked += 1

    if errors:
        print("::error::Man page example verification failed:")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(f"✓ {checked} man page example programs compile, run, and print their documented output.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
