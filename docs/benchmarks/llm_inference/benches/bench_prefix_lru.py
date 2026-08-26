#!/usr/bin/env python3
"""
Pillar 4: Prefix-Cache Indexing & Ordered LRU Eviction.

Measures:
1. Touch Throughput (touches/sec) — updating access timestamps.
2. LRU Eviction Throughput (evictions/sec) — evicting oldest blocks.
3. Rank-Threshold Eviction Throughput (evictions/sec) — batch eviction below timestamp.
4. Total Index Memory Footprint (MB) at scale.

Competitors:
- collections.OrderedDict (Doubly-Linked List + Hash Table, standard vLLM/SGLang pattern)
- ExpanseMap Ordered Table ((monotonic_ts << 32) | block_id, exact digital trie)
"""

import sys
import time
import json
import argparse
import tracemalloc
from collections import OrderedDict
from pathlib import Path
from typing import List, Dict

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "bindings" / "python"))

from expanse_trie import ExpanseMap

def run_prefix_lru(block_counts: List[int]) -> Dict[str, dict]:
    results = {}

    for N in block_counts:
        print(f"  --> Benchmarking Prefix LRU Cache with N = {N:,} blocks...")

        # ---------------------------------------------------------------------
        # 1. collections.OrderedDict
        # ---------------------------------------------------------------------
        tracemalloc.start()
        od = OrderedDict()
        for b_id in range(N):
            od[b_id] = (b_id * 16, b_id * 32)  # dummy physical KV slot pointers
        od_current, od_peak = tracemalloc.get_traced_memory()
        tracemalloc.stop()

        od_mem_mb = od_peak / (1024 * 1024)

        # Measure Touch Throughput (updating 10,000 blocks to most recently used)
        touch_keys = [i * 3 % N for i in range(min(10000, N))]
        t0 = time.perf_counter()
        for k in touch_keys:
            od.move_to_end(k)
        t1 = time.perf_counter()
        od_touch_tps = len(touch_keys) / max(1e-9, t1 - t0)

        # Measure LRU Eviction Throughput (evicting oldest 2,000 blocks)
        evict_count = min(2000, N // 4)
        t0 = time.perf_counter()
        for _ in range(evict_count):
            od.popitem(last=False)
        t1 = time.perf_counter()
        od_evict_tps = evict_count / max(1e-9, t1 - t0)

        del od

        # ---------------------------------------------------------------------
        # 2. ExpanseMap Ordered Table
        # ---------------------------------------------------------------------
        # Key: (monotonic_ts << 32) | block_id
        # Value: physical KV slot offset
        # Side map: block_id -> monotonic_ts
        exp_table = ExpanseMap()
        block_to_ts = [0] * N
        monotonic_ts = 0

        for b_id in range(N):
            monotonic_ts += 1
            block_to_ts[b_id] = monotonic_ts
            k = (monotonic_ts << 32) | b_id
            exp_table.insert(k, b_id * 16)

        exp_mem_bytes = exp_table.mem_used()
        exp_mem_mb = exp_mem_bytes / (1024 * 1024)

        # Measure Touch Throughput (remove old timestamp key, insert new timestamp key)
        t0 = time.perf_counter()
        for b_id in touch_keys:
            old_ts = block_to_ts[b_id]
            exp_table.remove((old_ts << 32) | b_id)
            monotonic_ts += 1
            block_to_ts[b_id] = monotonic_ts
            exp_table.insert((monotonic_ts << 32) | b_id, b_id * 16)
        t1 = time.perf_counter()
        exp_touch_tps = len(touch_keys) / max(1e-9, t1 - t0)

        # Measure LRU Eviction Throughput via first() + remove()
        t0 = time.perf_counter()
        for _ in range(evict_count):
            oldest = exp_table.first()
            if oldest is not None:
                exp_table.remove(oldest[0])
        t1 = time.perf_counter()
        exp_evict_tps = evict_count / max(1e-9, t1 - t0)

        # Measure Rank-Threshold Eviction via count_below()
        cutoff_ts = monotonic_ts // 2
        t0 = time.perf_counter()
        items_below = exp_table.count_below(cutoff_ts << 32)
        # Bounded bulk prune
        prune_keys = []
        cur = exp_table.first()
        while cur is not None and (cur[0] >> 32) < cutoff_ts and len(prune_keys) < 1000:
            prune_keys.append(cur[0])
            cur = exp_table.next_after(cur[0])
        for pk in prune_keys:
            exp_table.remove(pk)
        t1 = time.perf_counter()
        exp_rank_evict_tps = len(prune_keys) / max(1e-9, t1 - t0)

        del exp_table, block_to_ts

        results[str(N)] = {
            "num_blocks": N,
            "ordered_dict_lru": {
                "memory_mb": round(od_mem_mb, 2),
                "touch_throughput_tps": round(od_touch_tps, 0),
                "eviction_throughput_tps": round(od_evict_tps, 0),
                "rank_eviction_supported": False,
            },
            "expanse_ordered_table": {
                "memory_mb": round(exp_mem_mb, 2),
                "touch_throughput_tps": round(exp_touch_tps, 0),
                "eviction_throughput_tps": round(exp_evict_tps, 0),
                "rank_eviction_tps": round(exp_rank_evict_tps, 0),
                "rank_eviction_supported": True,
                "memory_reduction_vs_od_x": round(od_mem_mb / max(1e-9, exp_mem_mb), 2),
            },
        }

    return results

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--quick", action="store_true", help="Quick smoke run")
    parser.add_argument("--json", action="store_true", help="Emit JSON payload")
    args = parser.parse_args()

    if args.quick:
        blocks = [10_000, 50_000, 100_000]
    else:
        blocks = [100_000, 500_000, 1_000_000]

    print(f"Running Pillar 4 Prefix LRU Benchmark (blocks={blocks})...")
    results = run_prefix_lru(blocks)

    out_file = Path(__file__).resolve().parent.parent / "results" / "bench_prefix_lru.json"
    out_file.parent.mkdir(parents=True, exist_ok=True)
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)
    print(f"Pillar 4 results written to {out_file}")

if __name__ == "__main__":
    main()
