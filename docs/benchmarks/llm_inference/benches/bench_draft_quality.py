#!/usr/bin/env python3
"""
Pillar A — Speculative Draft Quality on Authentic Reference Token Streams.

Compares candidate generation policies on reference-continuation acceptance alpha:
1. HuggingFace Adaptive Lookup (adaptive N -> 1 fallback, prompt-only) — Strongest Baseline
2. HuggingFace Fixed 3-gram Lookup (fixed N=3)
3. HuggingFace Fixed 2-gram Lookup (fixed N=2)
4. Expanse Variable-Length Longest Suffix Match (ExpanseStrMap 2-neighbour LCP, 1 key/pos)
5. Native Suffix Array (cdivsufsort / pydivsufsort equivalent exact match semantics)

Outputs per-task distributions, macro alpha with BCa 95% bootstrap CIs over tasks,
steps saved, candidate lookup latencies, and Step 0 speedup ceilings vs HF Adaptive.
"""

import sys
import json
import time
from pathlib import Path
from typing import List, Tuple, Dict, Any

import numpy as np
from scipy.stats import bootstrap

import expanse_trie

DATA_DIR = Path(__file__).resolve().parent.parent / "data"
RESULTS_DIR = Path(__file__).resolve().parent.parent / "results"
RESULTS_DIR.mkdir(parents=True, exist_ok=True)

MAX_MATCH_LEN = 16
DRAFT_LEN = 4


def encode_key_nul_free(tokens: List[int]) -> bytes:
    """Encode uint32 tokens into 7-bit base-128 bytes with 0x80 offset (NUL-free)."""
    buf = bytearray()
    for tok in tokens:
        v = tok & 0xFFFFFFFF
        b0 = (v & 0x7F) + 0x80
        b1 = ((v >> 7) & 0x7F) + 0x80
        b2 = ((v >> 14) & 0x7F) + 0x80
        b3 = ((v >> 21) & 0x7F) + 0x80
        b4 = ((v >> 28) & 0x0F) + 0x80
        buf.extend([b0, b1, b2, b3, b4])
    return bytes(buf)


def compute_bca_ci(per_task_alphas: List[float]) -> Tuple[float, float]:
    """Computes BCa 95% bootstrap confidence interval over per-task alpha values."""
    if len(per_task_alphas) < 3:
        m = float(np.mean(per_task_alphas)) if per_task_alphas else 0.0
        return m, m
    data = (np.array(per_task_alphas),)
    try:
        res = bootstrap(data, np.mean, confidence_level=0.95, n_resamples=1000, method="BCa")
        return float(res.confidence_interval.low), float(res.confidence_interval.high)
    except Exception:
        try:
            res = bootstrap(data, np.mean, confidence_level=0.95, n_resamples=1000, method="percentile")
            return float(res.confidence_interval.low), float(res.confidence_interval.high)
        except Exception:
            m = float(np.mean(per_task_alphas))
            return m, m


class ExpanseLongestSuffixMatcher:
    """
    ExpanseStrMap with 1 key per token position and 2-neighbour LCP search.
    Inserts reversed windows: key = reversed(tokens[i - MAX_MATCH_LEN + 1 : i + 1]).
    Query: q = reversed(history[-MAX_MATCH_LEN:]). 2-neighbour LCP finds maximal prefix.
    """
    def __init__(self, initial_tokens: List[int]):
        self.trie = expanse_trie.ExpanseStrMap()
        self.tokens = list(initial_tokens)
        self.indexed_up_to = 0
        self._index_tokens()

    def _index_tokens(self):
        while self.indexed_up_to < len(self.tokens) - 1:
            i = self.indexed_up_to
            start = max(0, i - MAX_MATCH_LEN + 1)
            window = self.tokens[start : i + 1][::-1]
            key_bytes = encode_key_nul_free(window)
            self.trie.insert(key_bytes, i)
            self.indexed_up_to += 1

    def append_and_update(self, new_tokens: List[int]):
        self.tokens.extend(new_tokens)
        self._index_tokens()

    def propose(self, history: List[int], draft_len: int = DRAFT_LEN) -> List[int]:
        if not history:
            return []
        q_window = history[-MAX_MATCH_LEN:][::-1]
        q_bytes = encode_key_nul_free(q_window)

        cand_idx = -1
        best_match_len = 0

        p = self.trie.prev_at_or_before(q_bytes)
        if p is not None:
            p_key, p_idx = p
            if isinstance(p_key, str):
                p_key = p_key.encode("latin1")
            lcp_len = 0
            for a, b in zip(q_bytes, p_key):
                if a == b:
                    lcp_len += 1
                else:
                    break
            tok_match = lcp_len // 5
            if tok_match > best_match_len and p_idx + 1 < len(self.tokens):
                best_match_len = tok_match
                cand_idx = p_idx

        n = self.trie.next_at_or_after(q_bytes)
        if n is not None:
            n_key, n_idx = n
            if isinstance(n_key, str):
                n_key = n_key.encode("latin1")
            lcp_len = 0
            for a, b in zip(q_bytes, n_key):
                if a == b:
                    lcp_len += 1
                else:
                    break
            tok_match = lcp_len // 5
            if tok_match > best_match_len and n_idx + 1 < len(self.tokens):
                best_match_len = tok_match
                cand_idx = n_idx

        if best_match_len >= 1 and cand_idx >= 0:
            draft_start = cand_idx + 1
            draft_end = min(len(self.tokens), draft_start + draft_len)
            return self.tokens[draft_start:draft_end]
        return []


class SuffixArrayMatcher:
    """Exact native Suffix Array twin with identical longest-match semantics."""
    def __init__(self, initial_tokens: List[int]):
        self.tokens = list(initial_tokens)
        self.pos_map: Dict[Tuple[int, ...], int] = {}
        self.indexed_up_to = 0
        self._index_tokens()

    def _index_tokens(self):
        while self.indexed_up_to < len(self.tokens) - 1:
            i = self.indexed_up_to
            for l in range(1, min(MAX_MATCH_LEN + 1, i + 2)):
                k = tuple(self.tokens[i - l + 1 : i + 1])
                if k not in self.pos_map:
                    self.pos_map[k] = i
            self.indexed_up_to += 1

    def append_and_update(self, new_tokens: List[int]):
        self.tokens.extend(new_tokens)
        self._index_tokens()

    def propose(self, history: List[int], draft_len: int = DRAFT_LEN) -> List[int]:
        if not history or not self.tokens:
            return []
        max_q = min(len(history), MAX_MATCH_LEN)
        for q_len in range(max_q, 0, -1):
            k = tuple(history[-q_len:])
            if k in self.pos_map:
                cand_idx = self.pos_map[k]
                draft_start = cand_idx + 1
                draft_end = min(len(self.tokens), draft_start + draft_len)
                return self.tokens[draft_start:draft_end]
        return []


class HFAdaptivePromptLookupMatcher:
    """HuggingFace PromptLookup with adaptive N -> 1 fallback (Strongest Baseline)."""
    def __init__(self, initial_tokens: List[int], max_ngram_size: int = 3):
        self.tokens = list(initial_tokens)
        self.max_ngram_size = max_ngram_size
        self.pos_map: Dict[Tuple[int, ...], int] = {}
        self.indexed_up_to = 0
        self._index_tokens()

    def _index_tokens(self):
        while self.indexed_up_to < len(self.tokens) - 1:
            i = self.indexed_up_to
            for l in range(1, min(self.max_ngram_size + 1, i + 2)):
                k = tuple(self.tokens[i - l + 1 : i + 1])
                if k not in self.pos_map:
                    self.pos_map[k] = i
            self.indexed_up_to += 1

    def append_and_update(self, new_tokens: List[int]):
        self.tokens.extend(new_tokens)
        self._index_tokens()

    def propose(self, history: List[int], draft_len: int = DRAFT_LEN) -> List[int]:
        if not history or not self.tokens:
            return []
        max_n = min(len(history), self.max_ngram_size)
        for ngram_size in range(max_n, 0, -1):
            k = tuple(history[-ngram_size:])
            if k in self.pos_map:
                cand_idx = self.pos_map[k]
                draft_start = cand_idx + 1
                draft_end = min(len(self.tokens), draft_start + draft_len)
                return self.tokens[draft_start:draft_end]
        return []


class HFFixedPromptLookupMatcher:
    """HuggingFace PromptLookup with fixed N-gram size."""
    def __init__(self, initial_tokens: List[int], ngram_size: int = 3):
        self.tokens = list(initial_tokens)
        self.ngram_size = ngram_size
        self.pos_map: Dict[Tuple[int, ...], int] = {}
        self.indexed_up_to = 0
        self._index_tokens()

    def _index_tokens(self):
        while self.indexed_up_to < len(self.tokens) - 1:
            i = self.indexed_up_to
            if i >= self.ngram_size - 1:
                k = tuple(self.tokens[i - self.ngram_size + 1 : i + 1])
                if k not in self.pos_map:
                    self.pos_map[k] = i
            self.indexed_up_to += 1

    def append_and_update(self, new_tokens: List[int]):
        self.tokens.extend(new_tokens)
        self._index_tokens()

    def propose(self, history: List[int], draft_len: int = DRAFT_LEN) -> List[int]:
        if len(history) < self.ngram_size or not self.tokens:
            return []
        k = tuple(history[-self.ngram_size:])
        if k in self.pos_map:
            cand_idx = self.pos_map[k]
            draft_start = cand_idx + 1
            draft_end = min(len(self.tokens), draft_start + draft_len)
            return self.tokens[draft_start:draft_end]
        return []


def simulate_speculation(matcher_cls, matcher_kwargs, prompt_tokens, ground_truth_tokens) -> Tuple[int, int, int, float, float]:
    matcher = matcher_cls(prompt_tokens, **matcher_kwargs)
    history = list(prompt_tokens)
    pos = 0
    gt_len = len(ground_truth_tokens)
    
    speculation_steps = 0
    total_accepted = 0
    total_drafted = 0
    
    t0 = time.perf_counter()
    lat_queries = 0
    
    while pos < gt_len:
        speculation_steps += 1
        draft = matcher.propose(history, draft_len=DRAFT_LEN)
        lat_queries += 1
        
        accepted = 0
        for d in draft:
            if pos + accepted < gt_len and d == ground_truth_tokens[pos + accepted]:
                accepted += 1
            else:
                break
        
        total_drafted += len(draft)
        total_accepted += (accepted + 1)
        
        advance_count = min(gt_len - pos, accepted + 1)
        new_tokens = ground_truth_tokens[pos : pos + advance_count]
        pos += advance_count
        history.extend(new_tokens)
        matcher.append_and_update(new_tokens)

    t1 = time.perf_counter()
    avg_latency_us = ((t1 - t0) / max(1, lat_queries)) * 1e6
    alpha = (total_accepted / max(1, speculation_steps)) - 1.0

    return speculation_steps, total_accepted, total_drafted, alpha, avg_latency_us


def run_benchmark():
    datasets = [
        ("humaneval_code", "humaneval_reference_tokens.json"),
        ("summarization", "summary_reference_tokens.json"),
        ("json_schemas", "json_reference_tokens.json"),
    ]

    arms = [
        ("hf_adaptive_lookup", HFAdaptivePromptLookupMatcher, {"max_ngram_size": 3}),
        ("hf_fixed_3gram", HFFixedPromptLookupMatcher, {"ngram_size": 3}),
        ("hf_fixed_2gram", HFFixedPromptLookupMatcher, {"ngram_size": 2}),
        ("expanse_longest_suffix", ExpanseLongestSuffixMatcher, {}),
        ("suffix_array_baseline", SuffixArrayMatcher, {}),
    ]

    results = {}

    for workload_name, filename in datasets:
        json_file = DATA_DIR / filename
        if not json_file.exists():
            print(f"Skipping {workload_name}: {json_file} does not exist.")
            continue

        with open(json_file, "r", encoding="utf-8") as f:
            data = json.load(f)

        tasks = data.get("tasks", [])
        prompt_tokens = data.get("prompt_tokens", [])
        ground_truth_tokens = data.get("ground_truth_tokens", [])

        print(f"\n==> Evaluating {workload_name} ({len(tasks)} tasks, {len(prompt_tokens)} prompt toks, {len(ground_truth_tokens)} ref toks)...")

        workload_res = {}

        for arm_name, cls_type, kwargs in arms:
            # 1. Full-stream execution
            steps, accepted, drafted, alpha, lat_us = simulate_speculation(
                cls_type, kwargs, prompt_tokens, ground_truth_tokens
            )

            # 2. Per-task evaluation for BCa 95% bootstrap CI
            per_task_alphas = []
            if tasks:
                for task in tasks:
                    t_p = task["prompt_tokens"]
                    t_gt = task["reference_tokens"]
                    if not t_p or not t_gt:
                        continue
                    _, _, _, t_alpha, _ = simulate_speculation(cls_type, kwargs, t_p, t_gt)
                    per_task_alphas.append(t_alpha)

            ci_low, ci_high = compute_bca_ci(per_task_alphas) if per_task_alphas else (alpha, alpha)

            workload_res[arm_name] = {
                "speculation_steps": steps,
                "total_accepted_tokens": accepted,
                "total_drafted_tokens": drafted,
                "mean_acceptance_length_alpha": round(alpha, 3),
                "bca_95_ci": [round(ci_low, 3), round(ci_high, 3)],
                "acceptance_rate": round(accepted / max(1, drafted), 4) if drafted > 0 else 0.0,
                "candidate_lookup_latency_us": round(lat_us, 3),
            }
            print(f"  [{arm_name:24s}] alpha = {alpha:.3f} (95% CI [{ci_low:.3f}, {ci_high:.3f}]), steps = {steps}, lat = {lat_us:.2f} us")

        # Step 0 Speedup Ceiling vs Ground Baseline (HF Adaptive Lookup)
        base_alpha = workload_res["hf_adaptive_lookup"]["mean_acceptance_length_alpha"]
        exp_alpha = workload_res["expanse_longest_suffix"]["mean_acceptance_length_alpha"]
        sa_alpha = workload_res["suffix_array_baseline"]["mean_acceptance_length_alpha"]
        
        gain_pct = round(((exp_alpha - base_alpha) / (1.0 + base_alpha)) * 100.0, 2)
        ceiling = round((1.0 + exp_alpha) / (1.0 + base_alpha), 3)
        passes_gate = bool(gain_pct >= 5.0)

        if passes_gate:
            boundary = f"PASS: Expanse wins by {gain_pct:+.1f}% alpha over HF Adaptive (Speedup ceiling: {ceiling:.3f}x)"
        else:
            boundary = f"BOUNDARY RESULT: Expanse alpha gain vs HF Adaptive is {gain_pct:+.1f}% (< 5.0% gate); prompt lookup does not justify digital trie"

        workload_res["_speculative_ceiling"] = {
            "baseline_arm": "hf_adaptive_lookup",
            "baseline_alpha": base_alpha,
            "expanse_alpha": exp_alpha,
            "suffix_array_alpha": sa_alpha,
            "alpha_gain_pct_vs_adaptive": gain_pct,
            "theoretical_tok_per_sec_ceiling": ceiling,
            "passes_pillar_c_gate": passes_gate,
            "boundary_result": boundary
        }

        results[workload_name] = workload_res

    out_file = RESULTS_DIR / "bench_draft_quality.json"
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)
    print(f"\nPillar A results written to {out_file}")


if __name__ == "__main__":
    run_benchmark()
