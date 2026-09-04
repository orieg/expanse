#!/usr/bin/env python3
"""docs/benchmarks/rocksdb_memtable/scripts/generate_charts.py

Standard suite entrypoint that delegates to integrations/rocksdb/scripts/generate_bench_svg.py.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
SCRIPT = REPO_ROOT / "integrations" / "rocksdb" / "scripts" / "generate_bench_svg.py"

if __name__ == "__main__":
    res = subprocess.run([sys.executable, str(SCRIPT)] + sys.argv[1:], check=True)
    sys.exit(res.returncode)
