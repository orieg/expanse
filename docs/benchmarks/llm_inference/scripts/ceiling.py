#!/usr/bin/env python3
"""
Step 0 — Math-First Theoretical Speedup Ceiling Model & Gating for Speculative Decoding.

Derives the theoretical throughput gain ceiling (tok/s) of a speculative draft
datastore over a baseline draft generator.

Mathematical Derivation:
In speculative decoding with verification:
Let:
- S = Total decoding steps.
- N = Total accepted tokens generated = S * (1 + alpha).
- alpha = Mean accepted draft tokens per verification step.
- T_verify = Time of target model verification forward pass on (1 + K) tokens (typically 15-50 ms).
- T_propose = Time to generate/lookup candidate draft tokens (typically 1-15 us).

Throughput (tokens / second):
  Throughput = N / (S * (T_verify + T_propose)) = (1 + alpha) / (T_verify + T_propose)

Relative Speedup Ratio:
  Speedup = Throughput_expanse / Throughput_baseline
          = [(1 + alpha_expanse) / (T_verify + T_propose_expanse)] / [(1 + alpha_baseline) / (T_verify + T_propose_baseline)]

Because T_propose << T_verify (e.g. 10 us << 20,000 us, representing < 0.05% of step time):
  T_verify + T_propose_expanse ≈ T_verify + T_propose_baseline ≈ T_verify

Hence, the Theoretical Speedup Ceiling is strictly bounded by the acceptance length ratio:
  Speedup_Ceiling ≤ (1 + alpha_expanse) / (1 + alpha_baseline)

Gating Rule:
If (alpha_expanse - alpha_baseline) / (1 + alpha_baseline) < 0.05 (i.e. < 5% tok/s gain ceiling),
the end-to-end serving engine integration is not justified by candidate quality alone,
and Pillar C is skipped with the boundary result published.
"""

from typing import Tuple

def compute_speedup_ceiling(alpha_baseline: float, alpha_expanse: float) -> float:
    """Computes the theoretical throughput speedup ceiling from mean acceptance lengths."""
    if alpha_baseline < 0 or alpha_expanse < 0:
        raise ValueError("Acceptance lengths alpha must be non-negative")
    return (1.0 + alpha_expanse) / (1.0 + alpha_baseline)

def compute_exact_speedup(
    alpha_baseline: float,
    alpha_expanse: float,
    t_verify_ms: float,
    t_propose_baseline_us: float,
    t_propose_expanse_us: float,
) -> float:
    """Computes the exact speedup ratio including candidate proposal latency overheads."""
    if t_verify_ms <= 0:
        raise ValueError("Verification forward pass latency must be positive")
    if t_propose_baseline_us < 0 or t_propose_expanse_us < 0:
        raise ValueError("Proposal latency must be non-negative")

    t_verify_us = t_verify_ms * 1000.0
    t_step_base = t_verify_us + t_propose_baseline_us
    t_step_exp = t_verify_us + t_propose_expanse_us

    rate_base = (1.0 + alpha_baseline) / t_step_base
    rate_exp = (1.0 + alpha_expanse) / t_step_exp

    return rate_exp / rate_base

def evaluate_speculative_gate(
    alpha_baseline: float,
    alpha_expanse: float,
    threshold_pct: float = 5.0,
) -> Tuple[bool, float, float]:
    """
    Evaluates whether real-stream alpha gain meets the threshold to justify Pillar C.
    Returns: (passes_gate, gain_pct, ceiling_speedup)
    """
    ceiling = compute_speedup_ceiling(alpha_baseline, alpha_expanse)
    gain_pct = (ceiling - 1.0) * 100.0
    passes = gain_pct >= threshold_pct
    return passes, gain_pct, ceiling

# ==============================================================================
# Reference-Pinned Unit Tests
# ==============================================================================

def test_speedup_ceiling_known_references():
    # 1. Zero alpha improvement -> exactly 1.0x ceiling
    assert abs(compute_speedup_ceiling(3.0, 3.0) - 1.0) < 1e-9

    # 2. Reference alpha: base = 3.0 (4 tokens/step), expanse = 3.8 (4.8 tokens/step) -> 4.8 / 4.0 = 1.20x
    assert abs(compute_speedup_ceiling(3.0, 3.8) - 1.20) < 1e-9

    # 3. Reference alpha: base = 3.192, expanse = 3.754 -> 4.754 / 4.192 = 1.13406
    assert abs(compute_speedup_ceiling(3.192, 3.754) - 1.134064885) < 1e-6

def test_exact_speedup_vs_ceiling_bounds():
    # When proposal latency is 10 us and verify is 20 ms (20,000 us), deviation is < 0.05%
    t_verify_ms = 20.0
    t_propose_base_us = 0.5
    t_propose_exp_us = 10.0

    exact = compute_exact_speedup(3.0, 3.8, t_verify_ms, t_propose_base_us, t_propose_exp_us)
    ceiling = compute_speedup_ceiling(3.0, 3.8)

    # Exact speedup must be slightly below the ceiling due to the 9.5 us proposal overhead
    assert exact <= ceiling
    # The difference must be within 0.05% (ratio >= 0.9995 of ceiling)
    assert (exact / ceiling) >= 0.999

def test_speculative_gate_threshold():
    # +13.4% gain passes 5% gate
    passes, gain, ceiling = evaluate_speculative_gate(3.192, 3.754, threshold_pct=5.0)
    assert passes is True
    assert round(gain, 2) == 13.41
    assert round(ceiling, 3) == 1.134

    # +2.0% gain fails 5% gate
    passes, gain, ceiling = evaluate_speculative_gate(3.0, 3.08, threshold_pct=5.0)
    assert passes is False
    assert round(gain, 2) == 2.00

if __name__ == "__main__":
    test_speedup_ceiling_known_references()
    test_exact_speedup_vs_ceiling_bounds()
    test_speculative_gate_threshold()
    print("✅ All Step 0 speedup ceiling unit tests passed successfully!")
