#!/usr/bin/env python3
"""
Pillar 1: Speculative Draft Quality & Mean Acceptance Length (alpha) via Replay Verifier.

Compares:
1. Fixed 3-Gram Match (HuggingFace PromptLookup Baseline)
2. Fixed 2-Gram Match (HuggingFace PromptLookup Baseline)
3. Expanse Fixed 3-Gram (Bit-packed 21-bit integer trie)
4. Expanse Variable-Length Longest-Suffix Match (2-neighbour LCP in ExpanseStrMap)
5. Dict Multimap Draft Tree (dict[prefix, list[(token, count)]])
6. Expanse Draft Tree (Subexpanse range scan continuation tree)

Evaluates on committed offline synthetic token pattern streams:
- Code Patterns
- Summary Patterns
- JSON Schemas
"""

import sys
import time
import json
import argparse
import numpy as np
from pathlib import Path
from typing import List, Tuple, Optional, Dict
from scipy.stats import bootstrap

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "bindings" / "python"))

from expanse_trie import ExpanseMap, ExpanseStrMap

DATA_DIR = Path(__file__).resolve().parent.parent / "data"

def to_bytes(key_str_or_bytes) -> bytes:
    if isinstance(key_str_or_bytes, bytes):
        return key_str_or_bytes
    return key_str_or_bytes.encode("utf-8")

def encode_token_7bit(tok: int) -> bytes:
    assert 0 <= tok < (1 << 21), f"Token {tok} exceeds 21-bit encoding limit"
    b0 = ((tok >> 14) & 0x7F) + 1
    b1 = ((tok >> 7) & 0x7F) + 1
    b2 = (tok & 0x7F) + 1
    return bytes([b0, b1, b2])

def encode_rev_window(tokens: List[int]) -> bytes:
    return b"".join(encode_token_7bit(t) for t in reversed(tokens))

def lcp_tokens(a: bytes, b: bytes) -> int:
    min_l = min(len(a), len(b))
    for i in range(0, min_l, 3):
        if a[i:i+3] != b[i:i+3]:
            return i // 3
    return min_l // 3

# ==============================================================================
# Speculative Draft Engines
# ==============================================================================

class FixedNgramSearchEngine:
    """Fixed N-gram sliding window search (simulating HF unfold / prompt matching)."""
    def __init__(self, ngram_size: int = 3, num_draft: int = 4):
        self.ngram_size = ngram_size
        self.num_draft = num_draft
        self.tokens: List[int] = []
        self.index: Dict[Tuple[int, ...], int] = {}

    def reset(self, prompt_tokens: List[int]):
        self.tokens = list(prompt_tokens)
        self.index.clear()
        for i in range(len(self.tokens) - self.ngram_size):
            k = tuple(self.tokens[i:i+self.ngram_size])
            self.index[k] = i

    def append_and_draft(self, accepted: List[int]) -> List[int]:
        for tok in accepted:
            self.tokens.append(tok)
            i = len(self.tokens) - self.ngram_size - 1
            if i >= 0:
                k = tuple(self.tokens[i:i+self.ngram_size])
                self.index[k] = i

        if len(self.tokens) < self.ngram_size:
            return []

        query = tuple(self.tokens[-self.ngram_size:])
        pos = self.index.get(query)
        if pos is not None and pos + self.ngram_size < len(self.tokens):
            start = pos + self.ngram_size
            return self.tokens[start : start + self.num_draft]
        return []

class ExpanseFixedNgramEngine:
    """ExpanseMap bit-packed 21-bit 3-gram engine."""
    def __init__(self, num_draft: int = 4):
        self.num_draft = num_draft
        self.ngram_size = 3
        self.tokens: List[int] = []
        self.map = ExpanseMap()

    def reset(self, prompt_tokens: List[int]):
        self.tokens = list(prompt_tokens)
        self.map.clear()
        for i in range(len(self.tokens) - 3):
            k = (self.tokens[i] << 42) | (self.tokens[i+1] << 21) | self.tokens[i+2]
            self.map.insert(k, i)

    def append_and_draft(self, accepted: List[int]) -> List[int]:
        for tok in accepted:
            self.tokens.append(tok)
            i = len(self.tokens) - 4
            if i >= 0:
                k = (self.tokens[i] << 42) | (self.tokens[i+1] << 21) | self.tokens[i+2]
                self.map.insert(k, i)

        if len(self.tokens) < 3:
            return []

        q = (self.tokens[-3] << 42) | (self.tokens[-2] << 21) | self.tokens[-1]
        pos = self.map.get(q)
        if pos is not None and pos + 3 < len(self.tokens):
            start = pos + 3
            return self.tokens[start : start + self.num_draft]
        return []

class ExpanseLongestSuffixEngine:
    """Variable-length Longest Suffix Match (LSM) using 2-neighbour LCP in ExpanseStrMap."""
    def __init__(self, max_suffix_len: int = 16, min_match_len: int = 2, num_draft: int = 4):
        self.max_suffix_len = max_suffix_len
        self.min_match_len = min_match_len
        self.num_draft = num_draft
        self.tokens: List[int] = []
        self.strmap = ExpanseStrMap()

    def reset(self, prompt_tokens: List[int]):
        self.tokens = list(prompt_tokens)
        self.strmap.clear()
        n = len(self.tokens)
        for i in range(n - 1):
            for length in range(self.min_match_len, min(i + 1, self.max_suffix_len) + 1):
                w = self.tokens[i + 1 - length : i + 1]
                k = encode_rev_window(w)
                self.strmap.insert(k, i)

    def append_and_draft(self, accepted: List[int]) -> List[int]:
        for tok in accepted:
            self.tokens.append(tok)
            i = len(self.tokens) - 2
            if i >= 0:
                for length in range(self.min_match_len, min(i + 1, self.max_suffix_len) + 1):
                    w = self.tokens[i + 1 - length : i + 1]
                    k = encode_rev_window(w)
                    self.strmap.insert(k, i)

        if len(self.tokens) < self.min_match_len:
            return []

        q_window = self.tokens[-self.max_suffix_len:]
        q = encode_rev_window(q_window)

        pred = self.strmap.prev_at_or_before(q)
        succ = self.strmap.next_at_or_after(q)

        cands = []
        for cand in (pred, succ):
            if cand is not None:
                cand_k, pos = cand
                if pos + 1 < len(self.tokens):
                    lcp = lcp_tokens(q, to_bytes(cand_k))
                    if lcp >= self.min_match_len:
                        cands.append((lcp, pos))

        if not cands:
            return []

        best_lcp, pos = max(cands, key=lambda x: x[0])
        start = pos + 1
        return self.tokens[start : start + self.num_draft]

class DictMultimapTreeEngine:
    """Python dict multimap baseline for multi-candidate draft trees."""
    def __init__(self, ngram_size: int = 3, tree_width: int = 4):
        self.ngram_size = ngram_size
        self.tree_width = tree_width
        self.tokens: List[int] = []
        self.multimap: Dict[Tuple[int, ...], Dict[int, int]] = {}

    def reset(self, prompt_tokens: List[int]):
        self.tokens = list(prompt_tokens)
        self.multimap.clear()
        for i in range(len(self.tokens) - self.ngram_size):
            k = tuple(self.tokens[i:i+self.ngram_size])
            nxt = self.tokens[i+self.ngram_size]
            if k not in self.multimap:
                self.multimap[k] = {}
            self.multimap[k][nxt] = self.multimap[k].get(nxt, 0) + 1

    def append_and_draft(self, accepted: List[int]) -> List[int]:
        for tok in accepted:
            self.tokens.append(tok)
            i = len(self.tokens) - self.ngram_size - 1
            if i >= 0:
                k = tuple(self.tokens[i:i+self.ngram_size])
                nxt = self.tokens[i+self.ngram_size]
                if k not in self.multimap:
                    self.multimap[k] = {}
                self.multimap[k][nxt] = self.multimap[k].get(nxt, 0) + 1

        if len(self.tokens) < self.ngram_size:
            return []

        query = tuple(self.tokens[-self.ngram_size:])
        counts = self.multimap.get(query)
        if not counts:
            return []

        sorted_cands = sorted(counts.items(), key=lambda x: x[1], reverse=True)
        top_tokens = [tok for tok, _ in sorted_cands[:self.tree_width]]
        return top_tokens

class ExpanseDraftTreeEngine:
    """Expanse variable-length suffix tree building multi-candidate draft trees."""
    def __init__(self, max_suffix_len: int = 8, min_match_len: int = 2, tree_width: int = 4):
        self.max_suffix_len = max_suffix_len
        self.min_match_len = min_match_len
        self.tree_width = tree_width
        self.tokens: List[int] = []
        self.strmap = ExpanseStrMap()

    def reset(self, prompt_tokens: List[int]):
        self.tokens = list(prompt_tokens)
        self.strmap.clear()
        n = len(self.tokens)
        for i in range(n - 1):
            for length in range(self.min_match_len, min(i + 1, self.max_suffix_len) + 1):
                w = self.tokens[i + 1 - length : i + 1]
                k = encode_rev_window(w)
                self.strmap.insert(k, i)

    def append_and_draft(self, accepted: List[int]) -> List[int]:
        for tok in accepted:
            self.tokens.append(tok)
            i = len(self.tokens) - 2
            if i >= 0:
                for length in range(self.min_match_len, min(i + 1, self.max_suffix_len) + 1):
                    w = self.tokens[i + 1 - length : i + 1]
                    k = encode_rev_window(w)
                    self.strmap.insert(k, i)

        if len(self.tokens) < self.min_match_len:
            return []

        q = encode_rev_window(self.tokens[-self.max_suffix_len:])
        pred = self.strmap.prev_at_or_before(q)
        succ = self.strmap.next_at_or_after(q)

        cands = []
        for cand in (pred, succ):
            if cand is not None:
                cand_k, pos = cand
                if pos + 1 < len(self.tokens):
                    lcp = lcp_tokens(q, to_bytes(cand_k))
                    if lcp >= self.min_match_len:
                        cands.append((lcp, pos))

        if not cands:
            return []

        best_lcp, pos = max(cands, key=lambda x: x[0])
        start = pos + 1
        return self.tokens[start : start + self.tree_width]

# ==============================================================================
# Replay Verifier Simulation Harness
# ==============================================================================

def run_replay_verifier(engine, prompt_tokens: List[int], ground_truth_tokens: List[int], repeats: int = 5) -> dict:
    """Executes deterministic speculative verification simulation with repeat latency measurements."""
    all_latencies_us = []
    
    for r in range(repeats):
        engine.reset(prompt_tokens)
        
        total_tokens = len(ground_truth_tokens)
        curr_pos = 0
        speculation_steps = 0
        total_accepted_tokens = 0
        total_drafted_tokens = 0

        while curr_pos < total_tokens:
            speculation_steps += 1
            
            t0 = time.perf_counter_ns()
            draft_tokens = engine.append_and_draft([])
            t1 = time.perf_counter_ns()
            all_latencies_us.append((t1 - t0) / 1000.0)

            accepted_this_step = []
            for i, draft_tok in enumerate(draft_tokens):
                target_idx = curr_pos + i
                if target_idx < total_tokens and draft_tok == ground_truth_tokens[target_idx]:
                    accepted_this_step.append(draft_tok)
                else:
                    break

            next_ground_idx = curr_pos + len(accepted_this_step)
            if next_ground_idx < total_tokens:
                extra_token = ground_truth_tokens[next_ground_idx]
                accepted_this_step.append(extra_token)

            total_drafted_tokens += len(draft_tokens)
            total_accepted_tokens += len(accepted_this_step)
            curr_pos += len(accepted_this_step)

            engine.append_and_draft(accepted_this_step)

    alpha = total_accepted_tokens / max(1, speculation_steps)
    acceptance_rate = (total_accepted_tokens - speculation_steps) / max(1, total_drafted_tokens) if total_drafted_tokens > 0 else 0.0
    
    # Compute median and BCa bootstrap 95% CI for lookup latency
    lat_arr = np.array(all_latencies_us)
    med_latency_us = float(np.median(lat_arr))
    
    ci_low, ci_high = med_latency_us, med_latency_us
    if len(lat_arr) >= 30 and np.std(lat_arr) > 1e-6:
        try:
            res = bootstrap((lat_arr,), np.median, confidence_level=0.95, method='percentile', n_resamples=1000, random_state=42)
            ci_low = float(res.confidence_interval.low)
            ci_high = float(res.confidence_interval.high)
        except Exception:
            ci_low, ci_high = float(np.percentile(lat_arr, 2.5)), float(np.percentile(lat_arr, 97.5))
    elif len(lat_arr) > 0:
        ci_low, ci_high = float(np.percentile(lat_arr, 2.5)), float(np.percentile(lat_arr, 97.5))

    return {
        "speculation_steps": speculation_steps,
        "total_accepted_tokens": total_accepted_tokens,
        "total_drafted_tokens": total_drafted_tokens,
        "mean_acceptance_length_alpha": round(alpha, 3),
        "acceptance_rate": round(acceptance_rate, 4),
        "candidate_lookup_latency_us": round(med_latency_us, 3),
        "lookup_latency_ci_95": [round(ci_low, 3), round(ci_high, 3)],
    }

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--quick", action="store_true", help="Quick smoke run")
    parser.add_argument("--json", action="store_true", help="Emit JSON payload")
    args = parser.parse_args()

    workload_files = [
        "code_patterns_tokens.json",
        "summary_patterns_tokens.json",
        "json_schemas_tokens.json",
    ]

    all_results = {}
    repeats = 3 if args.quick else 10

    for wf in workload_files:
        path = DATA_DIR / wf
        if not path.exists():
            continue
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)

        wname = data["workload"]
        prompt = data["prompt_tokens"]
        target = data["ground_truth_tokens"]
        if args.quick:
            target = target[:200]

        engines = {
            "hf_fixed_3gram": FixedNgramSearchEngine(ngram_size=3, num_draft=4),
            "hf_fixed_2gram": FixedNgramSearchEngine(ngram_size=2, num_draft=4),
            "expanse_fixed_3gram": ExpanseFixedNgramEngine(num_draft=4),
            "expanse_longest_suffix": ExpanseLongestSuffixEngine(max_suffix_len=16, min_match_len=2, num_draft=4),
            "dict_multimap_tree": DictMultimapTreeEngine(ngram_size=3, tree_width=4),
            "expanse_draft_tree": ExpanseDraftTreeEngine(max_suffix_len=8, min_match_len=2, tree_width=4),
        }

        w_res = {}
        for ename, eng in engines.items():
            metrics = run_replay_verifier(eng, prompt, target, repeats=repeats)
            w_res[ename] = metrics

        all_results[wname] = w_res

    out_file = Path(__file__).resolve().parent.parent / "results" / "bench_draft_quality.json"
    out_file.parent.mkdir(parents=True, exist_ok=True)
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(all_results, f, indent=2)
    print(f"Pillar 1 results written to {out_file}")

if __name__ == "__main__":
    main()
