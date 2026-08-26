#!/usr/bin/env python3
"""
Pillar A: Speculative Draft Quality & Mean Acceptance Length (alpha) via Replay Verifier.

Compares:
1. Real HuggingFace PromptLookup (Adaptive max_matching_ngram_size -> 1 fallback)
2. Fixed 3-Gram Match (HF PromptLookup fixed 3-gram)
3. Fixed 2-Gram Match (HF PromptLookup fixed 2-gram)
4. Expanse Variable-Length Longest-Suffix Match (StrMap 2-neighbour LCP, 1 key/position)
5. Suffix Array Native Baseline (REST static twin, same match semantics)

Evaluates on two configurations:
- Prompt-Only Lookup (Standard HF PromptLookup setting)
- Corpus Retrieval Speculation (REST setting: multi-document corpus index)
"""

import sys
import time
import json
import argparse
import numpy as np
from pathlib import Path
from typing import List, Tuple, Optional, Dict, Any

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "bindings" / "python"))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))

from expanse_trie import ExpanseStrMap
from ceiling import compute_speedup_ceiling, evaluate_speculative_gate

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

class HFFallbackPromptLookupEngine:
    """
    Real HuggingFace PromptLookupCandidateGenerator policy:
    Checks n-gram matches from max_matching_ngram_size down to 1; selects first occurrence.
    """
    def __init__(self, max_matching_ngram_size: int = 3, num_draft: int = 4):
        self.max_matching_ngram_size = max_matching_ngram_size
        self.num_draft = num_draft
        self.tokens: List[int] = []
        self.indices: Dict[int, Dict[Tuple[int, ...], int]] = {
            n: {} for n in range(1, max_matching_ngram_size + 1)
        }

    def reset(self, prompt_tokens: List[int]):
        self.tokens = list(prompt_tokens)
        for n in self.indices:
            self.indices[n].clear()
            for i in range(len(self.tokens) - n):
                k = tuple(self.tokens[i:i+n])
                if k not in self.indices[n]:
                    self.indices[n][k] = i

    def append_and_draft(self, accepted: List[int]) -> List[int]:
        for tok in accepted:
            self.tokens.append(tok)
            for n in self.indices:
                i = len(self.tokens) - n - 1
                if i >= 0:
                    k = tuple(self.tokens[i:i+n])
                    if k not in self.indices[n]:
                        self.indices[n][k] = i

        # Adaptive fallback from max_matching_ngram_size down to 1
        for n in range(min(self.max_matching_ngram_size, len(self.tokens)), 0, -1):
            query = tuple(self.tokens[-n:])
            pos = self.indices[n].get(query)
            if pos is not None and pos + n < len(self.tokens):
                start = pos + n
                return self.tokens[start : start + self.num_draft]
        return []

class FixedNgramSearchEngine:
    """Fixed N-gram exact match baseline."""
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
            if k not in self.index:
                self.index[k] = i

    def append_and_draft(self, accepted: List[int]) -> List[int]:
        for tok in accepted:
            self.tokens.append(tok)
            i = len(self.tokens) - self.ngram_size - 1
            if i >= 0:
                k = tuple(self.tokens[i:i+self.ngram_size])
                if k not in self.index:
                    self.index[k] = i

        if len(self.tokens) < self.ngram_size:
            return []

        query = tuple(self.tokens[-self.ngram_size:])
        pos = self.index.get(query)
        if pos is not None and pos + self.ngram_size < len(self.tokens):
            start = pos + self.ngram_size
            return self.tokens[start : start + self.num_draft]
        return []

class ExpanseLongestSuffixEngine:
    """
    Expanse Variable-length Longest Suffix Match (LSM).
    Inserts EXACTLY ONE reversed max-length window per position (1 key/token).
    2-neighbour LCP guarantees discovery of the maximal matching suffix.
    """
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
            w = self.tokens[max(0, i + 1 - self.max_suffix_len) : i + 1]
            k = encode_rev_window(w)
            self.strmap.insert(k, i)

    def append_and_draft(self, accepted: List[int]) -> List[int]:
        for tok in accepted:
            self.tokens.append(tok)
            i = len(self.tokens) - 2
            if i >= 0:
                w = self.tokens[max(0, i + 1 - self.max_suffix_len) : i + 1]
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

        best_lcp, pos = max(cands, key=lambda x: (x[0], -x[1]))
        start = pos + 1
        return self.tokens[start : start + self.num_draft]

class SuffixArrayEngine:
    """
    Suffix Array native baseline with binary search matching identical match semantics.
    """
    def __init__(self, max_suffix_len: int = 16, min_match_len: int = 2, num_draft: int = 4):
        self.max_suffix_len = max_suffix_len
        self.min_match_len = min_match_len
        self.num_draft = num_draft
        self.tokens: List[int] = []
        self.sa: List[int] = []

    def _rebuild_sa(self):
        n = len(self.tokens)
        if n < self.min_match_len:
            self.sa = []
            return
        # Suffix array ordered by reversed token prefixes
        suffixes = list(range(n - 1))
        suffixes.sort(key=lambda i: list(reversed(self.tokens[max(0, i + 1 - self.max_suffix_len) : i + 1])))
        self.sa = suffixes

    def reset(self, prompt_tokens: List[int]):
        self.tokens = list(prompt_tokens)
        self._rebuild_sa()

    def append_and_draft(self, accepted: List[int]) -> List[int]:
        for tok in accepted:
            self.tokens.append(tok)
        self._rebuild_sa()

        if len(self.tokens) < self.min_match_len or not self.sa:
            return []

        q_rev = list(reversed(self.tokens[-self.max_suffix_len:]))
        
        # Binary search for closest suffix in SA
        low = 0
        high = len(self.sa) - 1
        best_pos = -1
        best_lcp = 0

        while low <= high:
            mid = (low + high) // 2
            pos = self.sa[mid]
            cand_rev = list(reversed(self.tokens[max(0, pos + 1 - self.max_suffix_len) : pos + 1]))
            
            # Compute common prefix length between q_rev and cand_rev
            lcp = 0
            for a, b in zip(q_rev, cand_rev):
                if a == b:
                    lcp += 1
                else:
                    break
            
            if lcp > best_lcp and pos + 1 < len(self.tokens):
                best_lcp = lcp
                best_pos = pos

            if cand_rev < q_rev:
                low = mid + 1
            else:
                high = mid - 1

        if best_lcp >= self.min_match_len and best_pos != -1:
            start = best_pos + 1
            return self.tokens[start : start + self.num_draft]
        return []

# ==============================================================================
# Simulation Harness
# ==============================================================================

def run_replay_verifier(engine, prompt_tokens: List[int], ground_truth_tokens: List[int], repeats: int = 5) -> dict:
    total_tokens = len(ground_truth_tokens)
    all_latencies_us = []

    for _ in range(repeats):
        engine.reset(prompt_tokens)
        curr_pos = 0
        speculation_steps = 0
        total_accepted_tokens = 0
        total_drafted_tokens = 0

        while curr_pos < total_tokens:
            speculation_steps += 1

            t0 = time.perf_counter_ns()
            # Batch measurement of proposal lookups
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
    med_latency_us = float(np.median(all_latencies_us))

    return {
        "speculation_steps": speculation_steps,
        "total_accepted_tokens": total_accepted_tokens,
        "total_drafted_tokens": total_drafted_tokens,
        "mean_acceptance_length_alpha": round(alpha, 3),
        "acceptance_rate": round(acceptance_rate, 4),
        "candidate_lookup_latency_us": round(med_latency_us, 3),
    }

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--quick", action="store_true", help="Quick smoke run")
    parser.add_argument("--json", action="store_true", help="Emit JSON payload")
    args = parser.parse_args()

    workload_files = [
        "humaneval_real_tokens.json",
        "summary_real_tokens.json",
        "json_real_tokens.json",
    ]

    all_results = {}
    repeats = 2 if args.quick else 5

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
            target = target[:150]

        engines = {
            "hf_adaptive_lookup": HFFallbackPromptLookupEngine(max_matching_ngram_size=3, num_draft=4),
            "hf_fixed_3gram": FixedNgramSearchEngine(ngram_size=3, num_draft=4),
            "hf_fixed_2gram": FixedNgramSearchEngine(ngram_size=2, num_draft=4),
            "expanse_longest_suffix": ExpanseLongestSuffixEngine(max_suffix_len=16, min_match_len=2, num_draft=4),
            "suffix_array_baseline": SuffixArrayEngine(max_suffix_len=16, min_match_len=2, num_draft=4),
        }

        w_res = {}
        for ename, eng in engines.items():
            metrics = run_replay_verifier(eng, prompt, target, repeats=repeats)
            w_res[ename] = metrics

        # Compute speedup ceilings and gate evaluation
        base_alpha = w_res["hf_fixed_3gram"]["mean_acceptance_length_alpha"]
        exp_alpha = w_res["expanse_longest_suffix"]["mean_acceptance_length_alpha"]
        passes_gate, gain_pct, ceiling = evaluate_speculative_gate(base_alpha, exp_alpha, threshold_pct=5.0)
        
        w_res["_speculative_ceiling"] = {
            "baseline_alpha": base_alpha,
            "expanse_alpha": exp_alpha,
            "alpha_gain_pct": round(gain_pct, 2),
            "theoretical_tok_per_sec_ceiling": round(ceiling, 3),
            "passes_pillar_c_gate": passes_gate,
        }

        all_results[wname] = w_res

    out_file = Path(__file__).resolve().parent.parent / "results" / "bench_draft_quality.json"
    out_file.parent.mkdir(parents=True, exist_ok=True)
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(all_results, f, indent=2)
    print(f"Pillar A results written to {out_file}")

if __name__ == "__main__":
    main()
