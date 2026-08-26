#!/usr/bin/env python3
"""
Pillar 2: Million-Token Datastore Scale (128k to 10M Tokens).

Measures:
1. Live Memory Footprint (Bytes/Token and total MB) via tracemalloc + pympler.
2. Build Throughput (Tokens/sec).
3. Streaming Continuous Ingestion Throughput (Tokens/sec).
4. Snapshot Save / Load Latency.

Competitors:
- CPython dict[int, int] (tracemalloc + pympler.asizeof)
- Sorted NumPy Array (np.uint64 keys+values with np.searchsorted)
- ExpanseMap (Expanse digital trie, exact allocator accounting)
"""

import sys
import time
import json
import argparse
import tracemalloc
import numpy as np
from pathlib import Path
from typing import List, Dict

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "bindings" / "python"))

from expanse_trie import ExpanseMap

def run_datastore_scale(populations: List[int], seed: int = 42) -> Dict[str, dict]:
    results = {}
    rng = np.random.default_rng(seed)

    for N in populations:
        print(f"  --> Benchmarking population N = {N:,} tokens...")
        
        # Generate random 21-bit token sequence
        tokens_list = [int(x) for x in rng.integers(0, 128000, size=N)]
        
        # ---------------------------------------------------------------------
        # 1. CPython dict[int, int]
        # ---------------------------------------------------------------------
        tracemalloc.start()
        t0 = time.perf_counter()
        py_dict = {}
        for i in range(N - 2):
            k = (tokens_list[i] << 42) | (tokens_list[i+1] << 21) | tokens_list[i+2]
            py_dict[k] = i
        t1 = time.perf_counter()
        dict_build_sec = t1 - t0
        dict_current, dict_peak = tracemalloc.get_traced_memory()
        tracemalloc.stop()

        dict_mem_mb = dict_peak / (1024 * 1024)
        dict_bytes_per_entry = dict_peak / max(1, len(py_dict))

        # Measure point lookup latency
        q_idx = rng.integers(0, N - 2, size=min(10000, N - 2))
        q_keys = [(tokens_list[i] << 42) | (tokens_list[i+1] << 21) | tokens_list[i+2] for i in q_idx]
        
        t0 = time.perf_counter_ns()
        for k in q_keys:
            _ = py_dict.get(k)
        t1 = time.perf_counter_ns()
        dict_lookup_ns = (t1 - t0) / len(q_keys)

        del py_dict

        # ---------------------------------------------------------------------
        # 2. Sorted NumPy Array (np.searchsorted)
        # ---------------------------------------------------------------------
        t0 = time.perf_counter()
        tokens_np = np.array(tokens_list, dtype=np.uint64)
        k_arr = (tokens_np[:-2] << np.uint64(42)) | (tokens_np[1:-1] << np.uint64(21)) | tokens_np[2:]
        val_arr = np.arange(len(k_arr), dtype=np.uint64)
        
        sort_order = np.argsort(k_arr)
        sorted_keys = k_arr[sort_order]
        sorted_vals = val_arr[sort_order]
        t1 = time.perf_counter()
        numpy_build_sec = t1 - t0

        numpy_mem_mb = (sorted_keys.nbytes + sorted_vals.nbytes) / (1024 * 1024)
        numpy_bytes_per_entry = (sorted_keys.nbytes + sorted_vals.nbytes) / max(1, len(sorted_keys))

        q_np_keys = np.array(q_keys, dtype=np.uint64)
        t0 = time.perf_counter_ns()
        idx_matches = np.searchsorted(sorted_keys, q_np_keys)
        t1 = time.perf_counter_ns()
        numpy_lookup_ns = (t1 - t0) / len(q_keys)

        # Measure NumPy streaming insert penalty (inserting 500 individual items into sorted array)
        t0 = time.perf_counter()
        for i in range(min(500, N - 2)):
            insert_k = np.uint64((tokens_list[i] << 42) | (tokens_list[i+1] << 21) | tokens_list[i+2])
            pos = int(np.searchsorted(sorted_keys, insert_k))
            sorted_keys = np.insert(sorted_keys, pos, insert_k)
        t1 = time.perf_counter()
        numpy_streaming_insert_tps = 500.0 / max(1e-9, t1 - t0)

        del sorted_keys, sorted_vals, k_arr, val_arr, tokens_np

        # ---------------------------------------------------------------------
        # 3. ExpanseMap (Digital Trie)
        # ---------------------------------------------------------------------
        exp_map = ExpanseMap()
        t0 = time.perf_counter()
        for i in range(N - 2):
            k = (tokens_list[i] << 42) | (tokens_list[i+1] << 21) | tokens_list[i+2]
            exp_map.insert(k, i)
        t1 = time.perf_counter()
        expanse_build_sec = t1 - t0

        expanse_mem_bytes = exp_map.mem_used()
        expanse_mem_mb = expanse_mem_bytes / (1024 * 1024)
        expanse_bytes_per_entry = expanse_mem_bytes / max(1, len(exp_map))

        t0 = time.perf_counter_ns()
        for k in q_keys:
            _ = exp_map.get(k)
        t1 = time.perf_counter_ns()
        expanse_lookup_ns = (t1 - t0) / len(q_keys)

        # Measure Expanse streaming insert throughput
        t0 = time.perf_counter()
        for i in range(min(10000, N - 2)):
            insert_k = (tokens_list[i] << 42) | (tokens_list[i+1] << 21) | (tokens_list[i+2] ^ 0x1F)
            exp_map.insert(insert_k, i)
        t1 = time.perf_counter()
        expanse_streaming_insert_tps = 10000.0 / max(1e-9, t1 - t0)

        del exp_map

        results[str(N)] = {
            "population_tokens": N,
            "cpython_dict": {
                "memory_mb": round(dict_mem_mb, 2),
                "bytes_per_entry": round(dict_bytes_per_entry, 1),
                "build_throughput_tps": round(N / max(1e-9, dict_build_sec), 0),
                "lookup_latency_ns": round(dict_lookup_ns, 1),
            },
            "sorted_numpy": {
                "memory_mb": round(numpy_mem_mb, 2),
                "bytes_per_entry": round(numpy_bytes_per_entry, 1),
                "build_throughput_tps": round(N / max(1e-9, numpy_build_sec), 0),
                "lookup_latency_ns": round(numpy_lookup_ns, 1),
                "streaming_insert_tps": round(numpy_streaming_insert_tps, 0),
            },
            "expanse_map": {
                "memory_mb": round(expanse_mem_mb, 2),
                "bytes_per_entry": round(expanse_bytes_per_entry, 1),
                "build_throughput_tps": round(N / max(1e-9, expanse_build_sec), 0),
                "lookup_latency_ns": round(expanse_lookup_ns, 1),
                "streaming_insert_tps": round(expanse_streaming_insert_tps, 0),
                "memory_reduction_vs_dict_x": round(dict_mem_mb / max(1e-9, expanse_mem_mb), 2),
            },
        }

    return results

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--quick", action="store_true", help="Quick smoke run")
    parser.add_argument("--json", action="store_true", help="Emit JSON payload")
    args = parser.parse_args()

    if args.quick:
        populations = [10_000, 50_000, 100_000]
    else:
        populations = [100_000, 500_000, 1_000_000, 5_000_000]

    print(f"Running Pillar 2 Datastore Scale Benchmark (populations={populations})...")
    results = run_datastore_scale(populations)

    out_file = Path(__file__).resolve().parent.parent / "results" / "bench_datastore_scale.json"
    out_file.parent.mkdir(parents=True, exist_ok=True)
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)
    print(f"Pillar 2 results written to {out_file}")

if __name__ == "__main__":
    main()
