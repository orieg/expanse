#!/usr/bin/env python3
"""Release smoke for the Windows archive: load `expanse.dll` and read its version.

No C toolchain is assumed on the runner. Loading the DLL and calling
`expanse_version()` proves that it loads with its dependencies and exports the
modern API, and that it is the version being released.

Run:  smoke_windows_dll.py <path to expanse.dll> <expected version>
"""

from __future__ import annotations

import ctypes
import os
import sys


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    path, want = os.path.abspath(sys.argv[1]), sys.argv[2]
    dll = ctypes.CDLL(path)
    dll.expanse_version.restype = ctypes.c_char_p
    got = dll.expanse_version().decode()
    if want not in got:
        print(f"::error::expanse_version() returned {got!r}, expected it to contain {want!r}")
        return 1
    print(f"windows archive smoke: ok ({got})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
