#!/usr/bin/env python3
"""
Master runner for the LLM Inference & Speculative Decoding Benchmark Suite.

Executes all 4 benchmark pillars:
1. bench_draft_quality.py (Replay verifier mean acceptance length alpha)
2. bench_datastore_scale.py (128k to 10M token datastore scale & memory)
3. bench_llama_lookup (Native C++ llama.cpp lookup cache benchmark)
4. bench_prefix_lru.py (Prefix-cache indexing & ordered LRU eviction)

Generates JSON telemetry in results/ and renders dual-theme SVG charts.
"""

import sys
import subprocess
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE_DIR.parent.parent
BENCHES_DIR = BASE_DIR / "benches"
SCRIPTS_DIR = BASE_DIR / "scripts"
RESULTS_DIR = BASE_DIR / "results"

def main():
    quick = "--quick" in sys.argv or "-q" in sys.argv
    print("========================================================================")
    print(f" Running LLM Inference & Speculative Decoding Benchmark Suite (quick={quick})")
    print("========================================================================")

    env_opts = {"PYTHONPATH": str(REPO_ROOT / "bindings" / "python")}
    import os
    merged_env = {**os.environ, **env_opts}

    # Pillar 1: Speculative Draft Quality (alpha)
    print("\n==> [1/4] Running Pillar 1: Speculative Draft Quality (Replay Verifier)...")
    cmd = [sys.executable, str(BENCHES_DIR / "bench_draft_quality.py")]
    if quick:
        cmd.append("--quick")
    subprocess.run(cmd, env=merged_env, check=True)

    # Pillar 2: Million-Token Datastore Scale
    print("\n==> [2/4] Running Pillar 2: Million-Token Datastore Scale...")
    cmd = [sys.executable, str(BENCHES_DIR / "bench_datastore_scale.py")]
    if quick:
        cmd.append("--quick")
    subprocess.run(cmd, env=merged_env, check=True)

    # Pillar 3: Native C++ llama.cpp Lookup Benchmark
    print("\n==> [3/4] Running Pillar 3: Native C++ llama.cpp Lookup Benchmark...")
    cpp_bin = BENCHES_DIR / "bench_llama_lookup"
    if not cpp_bin.exists():
        print("    Building native C++ bench_llama_lookup binary...")
        subprocess.run(["make", "-C", str(BASE_DIR)], check=True)
    cmd = [str(cpp_bin)]
    if quick:
        cmd.append("--quick")
    subprocess.run(cmd, cwd=REPO_ROOT, check=True)

    # Pillar 4: Prefix-Cache LRU Eviction & Memory
    print("\n==> [4/4] Running Pillar 4: Prefix-Cache Indexing & Ordered LRU Eviction...")
    cmd = [sys.executable, str(BENCHES_DIR / "bench_prefix_lru.py")]
    if quick:
        cmd.append("--quick")
    subprocess.run(cmd, env=merged_env, check=True)

    # Generate SVG charts
    print("\n==> Generating Dual-Theme SVG Comparison Charts...")
    subprocess.run([sys.executable, str(SCRIPTS_DIR / "generate_charts.py")], env=merged_env, check=True)

    print("\n========================================================================")
    print(" All LLM Inference benchmarks completed successfully!")
    print(f" Results written to: {RESULTS_DIR}")
    print("========================================================================")

if __name__ == "__main__":
    main()
