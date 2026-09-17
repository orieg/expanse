#!/usr/bin/env python3
"""Workload bounds for the concurrent YCSB suite (#1006, AGENTS.md §8.8 commit 1).

What a Zipfian request stream does to the *targets* of W concurrent threads,
as committed, unit-tested arithmetic: how much of the stream lands on the top-k
keys, how often two threads aim at one key or at one leaf, and what 8 rounds
can resolve. The pre-registration invokes these functions; it does not restate
their outputs from memory.

Scope, stated once. Every probability here is a property of the request stream
at one instant: W threads each holding one independently drawn target. None of
them is a contention rate, a retry rate or a throughput prediction -- those
depend on how long an operation holds a node, which no function here models
and only measurement supplies (the empirical residual of this audit).

The rank law
------------
Rank k in 1..N has probability k^-theta / H(N, theta), with
H(N, s) = sum_{i=1..N} i^-s the generalised harmonic number. This is the law
`crates/expanse/benches/ycsb_common/mod.rs::ZipfianGenerator` names (its
`zeta(n, theta)` is H(n, theta); its ranks are 0-based, so its rank r is this
module's rank r + 1) with `ZIPFIAN_THETA = 0.99`.

That generator does not sample the law exactly. It is the closed-form
approximation of Gray et al.: ranks 1 and 2 (its 0 and 1) get their exact
masses, and every later rank comes from one power-law inversion. Both laws are
here -- `top_k_share` for the exact one and `gray_top_k_share` for what the
generator emits -- so that a rank-histogram test of a harness is held to the
law its generator actually follows, and the gap between the two is a number.

Sources
-------
Gray, Sundaresan, Englert, Baclawski, Weinberger, "Quickly Generating
  Billion-Record Synthetic Databases", SIGMOD 1994. Existence: ACM DL
  doi 10.1145/191839.191886. Content, read from the paper's text: "Integer k
  gets weight proportional to (1/k)^theta where 0 < theta < 1 is the skew", and
  the `zipf(n, theta)` listing -- `alpha = 1/(1-theta)`, `eta = (1 -
  pow(2.0/n, 1-theta)) / (1 - zeta(theta, 2)/zetan)`, `if (uz < 1) return 1;
  if (uz < 1 + pow(0.5, theta)) return 2; return 1 + (int)(n * pow(eta*u - eta
  + 1, alpha));` -- which `ZipfianGenerator::next` reproduces term for term.
Cooper, Silberstein, Tam, Ramakrishnan, Sears, "Benchmarking Cloud Serving
  Systems with YCSB", SoCC 2010. Existence: dblp conf/cloud/CooperSTRS10.
  Content, read from the paper's text: section 4.1 defines the Zipfian and
  Latest distributions; Table 2 lists workloads A (50/50 read/update), B
  (95/5), C (read only), D (95/5 read/insert, Latest) and E (95/5 scan/insert);
  section 5.3 states the generator is "the algorithm ... from Gray et al" and
  that its output is hashed "to scatter items across the keyspace". NOT in the
  paper: the constant 0.99, and a workload F row. Read-modify-write appears
  only as prose in section 6.5 ("similar to workload A ... except that the
  updates are 'read-modify-write' rather than blind writes").
YCSB source tree (github.com/brianfrankcooper/YCSB, master):
  `ZipfianGenerator.ZIPFIAN_CONSTANT = 0.99`, and `workloads/workloadf` with
  `readproportion=0.5`, `readmodifywriteproportion=0.5`,
  `requestdistribution=zipfian`. These two are where theta = 0.99 and workload
  F's mix come from; the paper is not.
Cohen, Statistical Power Analysis for the Behavioral Sciences (1988), ch. 2 --
  the minimum detectable difference, through
  `reader_scaling_bounds.mde_from_rounds`, which this module calls and does not
  reimplement. Cited as that module cites it; not re-read here.

The coincidence probabilities need no citation: they are derived in the
docstrings from the definition of independence, and the tests check them
against brute-force enumeration and against the classical birthday figure
(23 people, 365 days, 0.5073), which the test recomputes from the product
formula rather than quoting.

Usage:
    python3 scripts/ycsb_concurrent_bounds.py              # the table, then the unit tests
    python3 scripts/ycsb_concurrent_bounds.py --self-test  # the unit tests only
    python3 scripts/ycsb_concurrent_bounds.py --table      # the table only
"""

from __future__ import annotations

import itertools
import json
import math
import sys
import unittest
from functools import lru_cache
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from density_poisson import LEAF_CAP, cascade_key_share  # noqa: E402
from reader_scaling_bounds import mde_from_rounds  # noqa: E402

# `ycsb_common::ZIPFIAN_THETA`, and YCSB's `ZIPFIAN_CONSTANT`.
THETA = 0.99
# The population every writer-scaling cell of this suite prefills (2^20).
POPULATION = 1 << 20
# Populated 2-byte-prefix expanses of a uniform 64-bit population that large
# (`density_poisson.EXPANSES_64`), so 16 keys per expanse on average.
LEAF_BINS_64 = 1 << 16
WRITERS = (2, 4, 8)
ROUNDS = 8

# The committed uniform-stream writer sweeps (README §15), whose per-round
# spread is the only measured stand-in for a cell that does not exist yet.
RESULTS = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results"
UNIFORM_BASELINES: tuple[tuple[str, int, Path], ...] = (
    ("0-15", 1, RESULTS / "baseline_writer_scaling_170a4bc3_pin0-15.json"),
    ("0-15", 2, RESULTS / "baseline_writer_scaling_170a4bc3_pin0-15_run2.json"),
    ("0,2,4,6,8,10,12,14", 1, RESULTS / "baseline_writer_scaling_170a4bc3_percore.json"),
    ("0,2,4,6,8,10,12,14", 2, RESULTS / "baseline_writer_scaling_170a4bc3_percore_run2.json"),
)

# Below this many terms a harmonic number is summed directly; above it the tail
# is an Euler-Maclaurin expansion started here.
_DIRECT_TERMS = 1024


# ---------------------------------------------------------------------------
# (a) The rank law and the share of the stream on the top-k keys
# ---------------------------------------------------------------------------

def _direct_sum(a: int, b: int, s: float) -> float:
    """sum_{i=a..b} i^-s, summed from the small terms up."""
    return math.fsum(i ** -s for i in range(b, a - 1, -1))


def _euler_maclaurin(a: int, b: int, s: float) -> float:
    """sum_{i=a..b} i^-s for a >= _DIRECT_TERMS, by Euler-Maclaurin.

    integral + (f(a) + f(b))/2 + (f'(b) - f'(a))/12 - (f'''(b) - f'''(a))/720
    with f(x) = x^-s. The first omitted term is of order s^5 a^(-s-5) / 30240,
    below 1e-16 of the sum for every (a, s) this module passes.
    """
    integral = math.log(b / a) if s == 1.0 else (b ** (1.0 - s) - a ** (1.0 - s)) / (1.0 - s)
    ends = 0.5 * (a ** -s + b ** -s)
    d1 = -s * (b ** (-s - 1.0) - a ** (-s - 1.0))
    d3 = -s * (s + 1.0) * (s + 2.0) * (b ** (-s - 3.0) - a ** (-s - 3.0))
    return integral + ends + d1 / 12.0 - d3 / 720.0


@lru_cache(maxsize=None)
def generalized_harmonic(n: int, s: float) -> float:
    """H(n, s) = sum_{i=1..n} i^-s, the normaliser of the rank law (Gray et al. 1994).

    `ZipfianGenerator::zeta(n, theta)` is this sum taken term by term. Here the
    first 1,024 terms are summed directly and the rest by Euler-Maclaurin, so a
    population of 2^20 or 10^7 costs the same; `test_harmonic_matches_the_direct_sum`
    holds the two to 1e-12 relative at n = 2^20.
    """
    if n < 1:
        raise ValueError(f"n must be at least 1, got {n}")
    if s < 0.0:
        raise ValueError(f"s must be non-negative, got {s}")
    if n <= _DIRECT_TERMS:
        return _direct_sum(1, n, s)
    return _direct_sum(1, _DIRECT_TERMS - 1, s) + _euler_maclaurin(_DIRECT_TERMS, n, s)


def _check_law(n: int, theta: float) -> None:
    if n < 1:
        raise ValueError(f"n must be at least 1, got {n}")
    if not 0.0 <= theta < 1.0:
        # Gray et al. state the law for 0 < theta < 1; theta = 0 is the uniform
        # law and is admitted because the tests use it as the hand-checkable end.
        raise ValueError(f"theta must be in [0, 1), got {theta}")


def zipf_pmf(rank: int, n: int, theta: float) -> float:
    """P(rank), 1-based: rank^-theta / H(n, theta) (Gray et al. 1994)."""
    _check_law(n, theta)
    if not 1 <= rank <= n:
        raise ValueError(f"rank must be in 1..{n}, got {rank}")
    return rank ** -theta / generalized_harmonic(n, theta)


def top_k_share(k: int, n: int, theta: float) -> float:
    """Share of draws landing on the k most popular keys: H(k, theta) / H(n, theta)."""
    _check_law(n, theta)
    if not 0 <= k <= n:
        raise ValueError(f"k must be in 0..{n}, got {k}")
    if k == 0:
        return 0.0
    return generalized_harmonic(k, theta) / generalized_harmonic(n, theta)


def gray_eta(n: int, theta: float) -> float:
    """The generator's `eta` (Gray et al. 1994, `zipf()`; `ZipfianGenerator::new`)."""
    _check_law(n, theta)
    if n < 3:
        raise ValueError("the closed form needs n >= 3")
    zeta2 = generalized_harmonic(2, theta)
    return (1.0 - (2.0 / n) ** (1.0 - theta)) / (1.0 - zeta2 / generalized_harmonic(n, theta))


def gray_next(u: float, n: int, theta: float) -> int:
    """`ZipfianGenerator::next(u)`, transcribed: the 0-based rank for a uniform u in [0, 1).

    A transcription, kept only so `gray_top_k_share` is tested against the
    branch structure the harness runs rather than against its own algebra.
    """
    zeta_n = generalized_harmonic(n, theta)
    uz = u * zeta_n
    if uz < 1.0:
        return 0
    if uz < 1.0 + 0.5 ** theta:
        return 1
    k = int(n * (gray_eta(n, theta) * u - gray_eta(n, theta) + 1.0) ** (1.0 / (1.0 - theta)))
    return min(k, n - 1)


def gray_top_k_share(k: int, n: int, theta: float) -> float:
    """Share of the *generator's* draws on its k lowest ranks.

    Derivation. The generator returns 0 for u < 1/H(n), 1 for u < H(2)/H(n), and
    otherwise floor(n x^alpha) with x = eta u - eta + 1 and alpha = 1/(1-theta).
    floor(n x^alpha) < k iff u < u_k, with
        u_k = 1 - (1 - (k/n)^(1-theta)) / eta,
    and eta is defined so that u_2 = H(2)/H(n) exactly: the power-law branch
    starts where the two exact ranks end. So the share is 1/H(n) at k = 1 and
    u_k for every k >= 2 -- exact at k = 2 and at k = n, an approximation of
    `top_k_share` in between.
    """
    _check_law(n, theta)
    if n < 3:
        raise ValueError("the closed form needs n >= 3")
    if not 0 <= k <= n:
        raise ValueError(f"k must be in 0..{n}, got {k}")
    if k == 0:
        return 0.0
    if k == 1:
        return 1.0 / generalized_harmonic(n, theta)
    return 1.0 - (1.0 - (k / n) ** (1.0 - theta)) / gray_eta(n, theta)


# ---------------------------------------------------------------------------
# (b) Two of W threads on one key, and on one leaf
# ---------------------------------------------------------------------------

def power_sum(n: int, theta: float, m: int) -> float:
    """sum_i p_i^m of the rank law: H(n, m theta) / H(n, theta)^m."""
    _check_law(n, theta)
    if m < 1:
        raise ValueError(f"m must be at least 1, got {m}")
    return generalized_harmonic(n, m * theta) / generalized_harmonic(n, theta) ** m


def pair_collision_probability(n: int, theta: float) -> float:
    """P(two independent draws name the same key) = sum_i p_i^2.

    Independence is the assumption: each thread draws from its own seeded
    stream and no thread's choice depends on another's. At theta = 0 this is
    1/n.
    """
    return power_sum(n, theta, 2)


def any_collision_from_power_sums(sums: list[float], w: int) -> float:
    """P(at least two of w independent draws coincide), from sums[m-1] = sum_i q_i^m.

    Derivation. P(all w distinct) = sum over ordered w-tuples of distinct cells
    of the product of their masses = w! e_w(q), e_w the elementary symmetric
    polynomial. Newton's identities give e_w from the power sums:
        m e_m = sum_{i=1..m} (-1)^(i-1) e_(m-i) P_i,   e_0 = 1.
    Exact for any finite distribution; `test_any_collision_matches_enumeration`
    checks it against brute force and `test_birthday_reference` against the
    classical 23-in-365 figure.
    """
    if w < 1:
        raise ValueError(f"w must be at least 1, got {w}")
    if len(sums) < w:
        raise ValueError(f"need power sums up to order {w}, got {len(sums)}")
    if abs(sums[0] - 1.0) > 1e-9:
        raise ValueError(f"masses must sum to 1, got {sums[0]}")
    e = [1.0]
    for m in range(1, w + 1):
        e.append(sum((-1.0) ** (i - 1) * e[m - i] * sums[i - 1] for i in range(1, m + 1)) / m)
    distinct = math.factorial(w) * e[w]
    return min(1.0, max(0.0, 1.0 - distinct))


def same_key_any_collision(w: int, n: int, theta: float) -> float:
    """P(at least two of w independent Zipfian draws name the same key)."""
    return any_collision_from_power_sums([power_sum(n, theta, m) for m in range(1, w + 1)], w)


def expected_colliding_pairs(w: int, pair_probability: float) -> float:
    """E[number of coinciding pairs among w draws] = C(w, 2) * pair_probability.

    Exact by linearity of expectation, whatever the dependence between pairs,
    and an upper bound on P(at least one coincidence) by Markov's inequality.
    """
    if w < 1:
        raise ValueError(f"w must be at least 1, got {w}")
    if not 0.0 <= pair_probability <= 1.0:
        raise ValueError(f"pair_probability must be in [0, 1], got {pair_probability}")
    return math.comb(w, 2) * pair_probability


def same_leaf_pair_scattered(n: int, theta: float, bins: int) -> float:
    """P(two draws fall in one leaf) when ranks are scattered over `bins` leaves.

    Model: each rank's key sits in a leaf chosen uniformly and independently of
    its rank -- what `ycsb_common` does for the uniform-random key shape, where
    rank r maps to the r-th draw of a uniform 64-bit generator, and what YCSB
    does by hashing the rank (Cooper et al. 2010, section 5.3). Averaged over
    that placement: same key with probability S2 = sum p_i^2, otherwise two
    different keys share a leaf with probability 1/bins:
        S2 + (1 - S2) / bins.
    `bins` is the leaf population parameter in its reciprocal form: bins =
    n / (mean keys per leaf). An expectation over placements; one fixed
    placement can sit above or below it.
    """
    if bins < 1:
        raise ValueError(f"bins must be at least 1, got {bins}")
    s2 = pair_collision_probability(n, theta)
    return s2 + (1.0 - s2) / bins


def leaf_masses_contiguous(n: int, theta: float, leaf_pop: int) -> list[float]:
    """Mass of each leaf when ranks are laid out contiguously, `leaf_pop` per leaf.

    Leaf j holds ranks j*leaf_pop + 1 .. (j+1)*leaf_pop: the popular keys are
    neighbours in the key space, which is Gray's generator unhashed ("the
    popular items are clustered together in the keyspace", Cooper et al. 2010,
    section 5.3) and the upper end of what any rank-to-key mapping can produce.
    """
    _check_law(n, theta)
    if leaf_pop < 1:
        raise ValueError(f"leaf_pop must be at least 1, got {leaf_pop}")
    h = generalized_harmonic(n, theta)
    out = []
    for lo in range(1, n + 1, leaf_pop):
        hi = min(n, lo + leaf_pop - 1)
        if hi - lo < 64 or lo < _DIRECT_TERMS:
            mass = _direct_sum(lo, hi, theta)
        else:
            mass = _euler_maclaurin(lo, hi, theta)
        out.append(mass / h)
    return out


def same_leaf_any_contiguous(w: int, n: int, theta: float, leaf_pop: int) -> float:
    """P(at least two of w draws fall in one leaf), contiguous layout. w = 2 is the pair figure."""
    masses = leaf_masses_contiguous(n, theta, leaf_pop)
    sums = [math.fsum(q ** m for q in masses) for m in range(1, w + 1)]
    return any_collision_from_power_sums(sums, w)


# ---------------------------------------------------------------------------
# (c) Workload D: where monotonic inserts land. A derivation, not a function.
# ---------------------------------------------------------------------------
#
# `ycsb_common` draws workload D's inserted keys from one counter:
# k_j = INSERT_SEQ_BASE + j for j = 1, 2, ..., with INSERT_SEQ_BASE = 2^63.
# Claim: applied in counter order, the fraction of inserts that land in the
# expanse holding the greatest key inserted so far -- the append expanse -- is 1.
#
# Derivation. k_j and k_(j+1) differ by one, so they agree in their top seven
# bytes unless j + 1 is a multiple of 256, where the carry opens the next
# seven-byte prefix. In a digital trie keyed MSB first a key's leaf is named by
# its prefix, so k_(j+1) either joins the leaf that holds k_j, which is the
# current maximum of the sequence, or opens the leaf immediately after it. No
# insert lands anywhere else, for every population and every thread count, as
# long as the sequence is applied in order. The value is the constant 1; a
# function returning it would test nothing, so none is written.
#
# What is NOT 1 by construction, and is left to measurement or to the harness's
# own census:
#   * with T threads each inserting its own arithmetic slice of the counter
#     (k = INSERT_SEQ_BASE + 1 + j*T + t), arrival order across threads is not
#     key order, so an insert can land one leaf behind the current append leaf;
#   * "the rightmost path of the tree" holds only if no population key exceeds
#     the counter. `ycsb_common` clears the top bit of the dense shape's keys,
#     so there the append expanse is the tree's rightmost. It does not clear it
#     for the uniform-random shape, where about half the population lies above
#     2^63: the append path is then one path in the middle of the key space
#     (top byte 0x80, then zeros), shared by every insert, and not the tree's
#     rightmost. The probability that any of N uniform 64-bit population keys
#     falls inside the first 2^24 counter values is N * 2^24 / 2^64, about
#     1e-6 at N = 2^20, so the append subtree holds inserted keys only.
# Cooper et al. 2010 (section 4.1) note that under Latest "the last inserted
# item may not be inserted at the end of the key space": monotonic insert keys
# are this suite's choice, inherited from `ycsb_common`, and not part of YCSB's
# definition of workload D.


# ---------------------------------------------------------------------------
# (d) What the planned rounds can resolve
# ---------------------------------------------------------------------------

def mde_per_unit_sigma(rounds: int) -> float:
    """Minimum detectable difference per unit of per-round sigma, at `rounds` rounds per arm.

    `reader_scaling_bounds.mde_from_rounds` (Cohen 1988, ch. 2) is linear in
    sigma, so it is evaluated on a series whose sample standard deviation is
    exactly 1 -- half the rounds at +a, half at -a, a = sqrt((n-1)/n) -- and
    nothing is reimplemented. At 8 rounds: (1.95996 + 0.84162) * sqrt(2/8) =
    1.40079.
    """
    if rounds < 2 or rounds % 2:
        raise ValueError(f"rounds must be even and at least 2, got {rounds}")
    a = math.sqrt((rounds - 1) / rounds)
    series = [10.0 + a, 10.0 - a] * (rounds // 2)
    return mde_from_rounds(series)["mde"]


def largest_resolvable_cv(effect: float, rounds: int) -> float:
    """The per-round coefficient of variation above which `effect` (relative) is below the MDE."""
    if effect <= 0.0:
        raise ValueError(f"effect must be positive, got {effect}")
    return effect / mde_per_unit_sigma(rounds)


def scaling_series(path: Path, arm: str, writers: int) -> list[float]:
    """Per-round C(W) = writer_mops(W, r) / writer_mops(1, r) of one arm of a writer-sweep artifact.

    Reads `throughput[*].rounds_raw[*].writer_mops`, matched by `round`. Refuses
    a missing cell or a round present in one cell and not the other, so a
    series is never silently shorter than the artifact says (AGENTS.md §8.1).
    """
    data = json.loads(path.read_text())
    by_w = {}
    for w in (1, writers):
        cells = [c for c in data["throughput"] if c.get("arm") == arm and c.get("writers") == w
                 and c.get("readers", 0) == 0]
        if len(cells) != 1:
            raise ValueError(f"{path.name}: expected one {arm} W={w} cell, found {len(cells)}")
        by_w[w] = {int(r["round"]): float(r["writer_mops"]) for r in cells[0]["rounds_raw"]}
    if sorted(by_w[1]) != sorted(by_w[writers]) or not by_w[1]:
        raise ValueError(f"{path.name}: {arm} W=1 and W={writers} do not share their rounds")
    return [by_w[writers][r] / by_w[1][r] for r in sorted(by_w[1])]


def baseline_scaling_mde(path: Path, arm: str, writers: int) -> dict[str, float]:
    """`mde_from_rounds` over `scaling_series`: what the committed uniform-stream cell resolves."""
    return mde_from_rounds(scaling_series(path, arm, writers))


# ---------------------------------------------------------------------------
# The table the pre-registration quotes
# ---------------------------------------------------------------------------

def render_table(n: int = POPULATION, theta: float = THETA) -> str:
    lines = [f"N = {n}, theta = {theta}, H(N, theta) = {generalized_harmonic(n, theta):.6f}", ""]
    lines += ["| k | exact share, `top_k_share` | generator share, `gray_top_k_share` |", "|--:|--:|--:|"]
    for k in (1, 2, 16, 256, 4096, 65536, n // 100):
        lines.append(f"| {k} | {top_k_share(k, n, theta):.6f} | {gray_top_k_share(k, n, theta):.6f} |")
    lines += ["", f"pair on one key, Zipfian: {pair_collision_probability(n, theta):.6e}",
              f"pair on one key, uniform: {pair_collision_probability(n, 0.0):.6e}",
              f"pair on one leaf, scattered over {LEAF_BINS_64} leaves: "
              f"{same_leaf_pair_scattered(n, theta, LEAF_BINS_64):.6e}",
              f"pair on one leaf, scattered, uniform: {same_leaf_pair_scattered(n, 0.0, LEAF_BINS_64):.6e}",
              f"keys in cascaded expanses at 16 per expanse, cap {LEAF_CAP}: "
              f"{cascade_key_share(n / LEAF_BINS_64):.3e}", ""]
    lines += ["| W | any two on one key | same, uniform | any two on one leaf, scattered (upper bound) "
              "| any two on one leaf, contiguous 16 | contiguous 256 |", "|--:|--:|--:|--:|--:|--:|"]
    for w in WRITERS:
        scattered = min(1.0, expected_colliding_pairs(w, same_leaf_pair_scattered(n, theta, LEAF_BINS_64)))
        lines.append(
            f"| {w} | {same_key_any_collision(w, n, theta):.6f} | {same_key_any_collision(w, n, 0.0):.3e} "
            f"| {scattered:.6f} | {same_leaf_any_contiguous(w, n, theta, 16):.6f} "
            f"| {same_leaf_any_contiguous(w, n, theta, 256):.6f} |")
    lines += ["", f"MDE per unit sigma at {ROUNDS} rounds: {mde_per_unit_sigma(ROUNDS):.5f}"]
    for effect in (0.05, 0.10, 0.25):
        lines.append(f"largest per-round CV resolving a {effect:.0%} effect: "
                     f"{largest_resolvable_cv(effect, ROUNDS):.4f}")
    lines += ["", "| arm | pin | run | W | mean C(W) | per-round sigma | MDE, relative |", "|---|---|--:|--:|--:|--:|--:|"]
    for arm in ("map",):
        for pin, run, path in UNIFORM_BASELINES:
            for w in WRITERS:
                m = baseline_scaling_mde(path, arm, w)
                mean = m["mde"] / m["relative"]
                lines.append(f"| `{arm}` | `{pin}` | {run} | {w} | {mean:.4f} | {m['sigma']:.5f} | {m['relative']:.2%} |")
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

class HarmonicTests(unittest.TestCase):
    def test_hand_values(self):
        # H(4, 1) = 1 + 1/2 + 1/3 + 1/4 = 25/12; H(n, 0) = n; H(3, 0.5) by hand.
        self.assertAlmostEqual(generalized_harmonic(4, 1.0), 25.0 / 12.0, places=14)
        self.assertEqual(generalized_harmonic(7, 0.0), 7.0)
        self.assertAlmostEqual(generalized_harmonic(3, 0.5), 1 + 2 ** -0.5 + 3 ** -0.5, places=14)

    def test_harmonic_matches_the_direct_sum(self):
        # The Euler-Maclaurin tail against term-by-term summation, which is what
        # `ZipfianGenerator::zeta` does, at the suite population and at every
        # exponent `same_key_any_collision` reaches for W = 8.
        for s in (0.99, 1.0, 1.98, 7.92):
            direct = _direct_sum(1, POPULATION, s)
            self.assertAlmostEqual(generalized_harmonic(POPULATION, s) / direct, 1.0, places=12)

    def test_basel_limit(self):
        # H(n, 2) -> pi^2 / 6, and the remainder is 1/n to first order.
        n = 10 ** 7
        self.assertAlmostEqual(generalized_harmonic(n, 2.0), math.pi ** 2 / 6 - 1 / n + 1 / (2 * n * n), places=13)


class RankLawTests(unittest.TestCase):
    def test_pmf_sums_to_one_and_is_monotone(self):
        p = [zipf_pmf(k, 50, 0.99) for k in range(1, 51)]
        self.assertAlmostEqual(math.fsum(p), 1.0, places=14)
        self.assertTrue(all(a > b for a, b in zip(p, p[1:])))

    def test_top_k_hand_values(self):
        # N = 4, theta = 0.5: H = 1 + 0.70711 + 0.57735 + 0.5 = 2.78446;
        # top-1 = 1/H = 0.35914, top-2 = 1.70711/H = 0.61308.
        self.assertAlmostEqual(top_k_share(1, 4, 0.5), 0.35914, places=5)
        self.assertAlmostEqual(top_k_share(2, 4, 0.5), 0.61308, places=5)
        self.assertEqual(top_k_share(0, 4, 0.5), 0.0)
        self.assertEqual(top_k_share(4, 4, 0.5), 1.0)
        # Uniform end: k/n.
        self.assertAlmostEqual(top_k_share(25, 100, 0.0), 0.25, places=14)

    def test_top_k_reference_values_at_the_suite_population(self):
        # Pinned so the pre-registration's table cannot drift from this module.
        self.assertAlmostEqual(generalized_harmonic(POPULATION, THETA), 15.446323, places=5)
        self.assertAlmostEqual(top_k_share(1, POPULATION, THETA), 0.064740, places=5)
        self.assertAlmostEqual(top_k_share(256, POPULATION, THETA), 0.406592, places=5)
        self.assertAlmostEqual(top_k_share(POPULATION // 100, POPULATION, THETA), 0.665291, places=5)

    def test_gray_share_is_exact_at_its_anchors(self):
        n, th = POPULATION, THETA
        self.assertAlmostEqual(gray_top_k_share(1, n, th), top_k_share(1, n, th), places=14)
        self.assertAlmostEqual(gray_top_k_share(2, n, th), top_k_share(2, n, th), places=12)
        self.assertAlmostEqual(gray_top_k_share(n, n, th), 1.0, places=14)

    def test_gray_share_matches_the_transcribed_generator(self):
        # A deterministic grid of u, pushed through the generator's own branch
        # structure: the closed form must agree to the grid's resolution.
        n, th, grid = 100_000, THETA, 20_000
        ranks = [gray_next((i + 0.5) / grid, n, th) for i in range(grid)]
        self.assertEqual(min(ranks), 0)
        self.assertLessEqual(max(ranks), n - 1)
        for k in (1, 2, 3, 10, 100, 1000, 50_000):
            observed = sum(r < k for r in ranks) / grid
            self.assertAlmostEqual(observed, gray_top_k_share(k, n, th), delta=1.0 / grid)

    def test_generator_departs_from_the_exact_law_by_a_bounded_amount(self):
        # The generator is an approximation between its anchors. The gap is
        # pinned as a bound, so a harness histogram test knows which law to use
        # and how far apart they are.
        gaps = [abs(gray_top_k_share(k, POPULATION, THETA) - top_k_share(k, POPULATION, THETA))
                for k in (3, 16, 256, 4096, 65536)]
        self.assertGreater(max(gaps), 1e-4)
        self.assertLess(max(gaps), 0.02)


class CollisionTests(unittest.TestCase):
    def test_pair_uniform_is_one_over_n(self):
        self.assertAlmostEqual(pair_collision_probability(1000, 0.0), 1e-3, places=15)

    def test_pair_hand_value(self):
        # N = 2, theta = 0.5: p = (1, 0.70711)/1.70711 = (0.58579, 0.41421);
        # sum p^2 = 0.34315 + 0.17157 = 0.51472.
        self.assertAlmostEqual(pair_collision_probability(2, 0.5), 0.51472, places=5)

    def test_birthday_reference(self):
        # 23 people, 365 equiprobable days: 1 - prod_{i<23} (365 - i)/365.
        product = 1.0
        for i in range(23):
            product *= (365 - i) / 365
        got = same_key_any_collision(23, 365, 0.0)
        self.assertAlmostEqual(got, 1.0 - product, places=12)
        self.assertAlmostEqual(got, 0.5073, places=4)

    def test_any_collision_matches_enumeration(self):
        n, th = 6, 0.99
        p = [zipf_pmf(k, n, th) for k in range(1, n + 1)]
        for w in (2, 3, 4):
            distinct = math.fsum(math.prod(p[i] for i in t) for t in itertools.permutations(range(n), w))
            self.assertAlmostEqual(same_key_any_collision(w, n, th), 1.0 - distinct, places=12)
        # w = 2 is the pair figure; more draws than keys must coincide.
        self.assertAlmostEqual(same_key_any_collision(2, n, th), pair_collision_probability(n, th), places=14)
        self.assertAlmostEqual(same_key_any_collision(7, n, th), 1.0, places=9)
        self.assertEqual(same_key_any_collision(1, n, th), 0.0)

    def test_expected_pairs_bounds_any_collision(self):
        for w in WRITERS:
            exact = same_key_any_collision(w, POPULATION, THETA)
            bound = expected_colliding_pairs(w, pair_collision_probability(POPULATION, THETA))
            self.assertLessEqual(exact, bound)
        self.assertEqual(expected_colliding_pairs(8, 0.5), 14.0)

    def test_scattered_leaf_hand_value_and_limits(self):
        # One bin: every pair shares it. bins = n, uniform: 1/n + (1 - 1/n)/n.
        self.assertEqual(same_leaf_pair_scattered(100, 0.5, 1), 1.0)
        self.assertAlmostEqual(same_leaf_pair_scattered(10, 0.0, 10), 0.1 + 0.9 / 10, places=15)
        # Never below the same-key figure.
        self.assertGreater(same_leaf_pair_scattered(POPULATION, THETA, LEAF_BINS_64),
                           pair_collision_probability(POPULATION, THETA))

    def test_contiguous_leaf_hand_value_and_limits(self):
        # N = 4, theta = 0.5, two per leaf: masses (1.70711, 1.07735)/2.78446 =
        # (0.61308, 0.38692); same leaf = 0.37587 + 0.14971 = 0.52558.
        masses = leaf_masses_contiguous(4, 0.5, 2)
        self.assertAlmostEqual(masses[0], 0.61308, places=5)
        self.assertAlmostEqual(math.fsum(masses), 1.0, places=14)
        self.assertAlmostEqual(same_leaf_any_contiguous(2, 4, 0.5, 2), 0.52558, places=5)
        # leaf_pop = 1 is the same-key figure; leaf_pop = n is certainty.
        self.assertAlmostEqual(same_leaf_any_contiguous(4, 50, 0.99, 1), same_key_any_collision(4, 50, 0.99), places=12)
        self.assertAlmostEqual(same_leaf_any_contiguous(2, 50, 0.99, 50), 1.0, places=12)
        # A ragged last leaf still sums to one.
        self.assertAlmostEqual(math.fsum(leaf_masses_contiguous(10, 0.99, 4)), 1.0, places=14)

    def test_contiguous_masses_agree_across_the_summation_switch(self):
        # Blocks above the direct-summation threshold take the Euler-Maclaurin
        # branch; both must give the same leaf mass.
        n, th, pop = 1 << 16, THETA, 256
        masses = leaf_masses_contiguous(n, th, pop)
        h = generalized_harmonic(n, th)
        for j in (3, 4, 5, 100, 255):
            lo = j * pop + 1
            self.assertAlmostEqual(masses[j], _direct_sum(lo, lo + pop - 1, th) / h, places=14)

    def test_reference_values_at_the_suite_population(self):
        n, th = POPULATION, THETA
        self.assertAlmostEqual(pair_collision_probability(n, th), 6.974716e-3, delta=5e-9)
        self.assertAlmostEqual(same_key_any_collision(8, n, th), 0.157279, places=5)
        self.assertAlmostEqual(same_leaf_any_contiguous(8, n, th, 16), 0.619047, places=5)
        # Skew multiplies the instantaneous same-key pair rate by n * sum p^2.
        self.assertAlmostEqual(pair_collision_probability(n, th) * n, 7313.5, delta=0.05)


class DetectabilityTests(unittest.TestCase):
    def test_mde_per_unit_sigma_hand_value(self):
        # (1.959964 + 0.841621) * sqrt(2/8) = 2.801585 * 0.5 = 1.400793.
        self.assertAlmostEqual(mde_per_unit_sigma(8), 1.400793, places=5)
        # And it is `mde_from_rounds`, not a second implementation: sigma 2 doubles it.
        a = 2.0 * math.sqrt(7 / 8)
        self.assertAlmostEqual(mde_from_rounds([5 + a, 5 - a] * 4)["mde"], 2 * mde_per_unit_sigma(8), places=12)

    def test_largest_resolvable_cv(self):
        self.assertAlmostEqual(largest_resolvable_cv(0.10, 8), 0.10 / 1.400793, places=6)

    def test_render_table_runs_and_names_every_writer_count(self):
        text = render_table()
        for w in WRITERS:
            self.assertIn(f"| {w} |", text)
        self.assertIn("MDE per unit sigma at 8 rounds", text)


class ArtifactTests(unittest.TestCase):
    def test_reducer_reproduces_the_published_str_row(self):
        # METHODOLOGY §17.6, first row: `str`, pin `0-15`, run 1, W = 2 --
        # sigma 0.01497, MDE 0.02096, 3.29%. The same reduction, same artifact.
        m = baseline_scaling_mde(UNIFORM_BASELINES[0][2], "str", 2)
        self.assertAlmostEqual(m["sigma"], 0.01497, places=5)
        self.assertAlmostEqual(m["mde"], 0.02096, places=5)
        self.assertAlmostEqual(m["relative"], 0.0329, places=4)
        self.assertEqual(m["n"], 8.0)

    def test_every_uniform_baseline_cell_reduces(self):
        for _pin, _run, path in UNIFORM_BASELINES:
            for arm in ("map", "set"):
                for w in WRITERS:
                    series = scaling_series(path, arm, w)
                    self.assertEqual(len(series), ROUNDS)
                    self.assertTrue(all(x > 0 for x in series))

    def test_reducer_refuses_a_missing_cell(self):
        with self.assertRaises(ValueError):
            scaling_series(UNIFORM_BASELINES[0][2], "map", 3)
        with self.assertRaises(ValueError):
            scaling_series(UNIFORM_BASELINES[0][2], "no_such_arm", 2)


class ArgumentTests(unittest.TestCase):
    def test_invalid_arguments_raise(self):
        for call in (
            lambda: generalized_harmonic(0, 0.99),
            lambda: generalized_harmonic(5, -1.0),
            lambda: zipf_pmf(0, 5, 0.5),
            lambda: zipf_pmf(6, 5, 0.5),
            lambda: zipf_pmf(1, 5, 1.0),
            lambda: top_k_share(6, 5, 0.5),
            lambda: gray_top_k_share(1, 2, 0.5),
            lambda: power_sum(5, 0.5, 0),
            lambda: any_collision_from_power_sums([1.0], 2),
            lambda: any_collision_from_power_sums([0.9, 0.5], 2),
            lambda: any_collision_from_power_sums([1.0], 0),
            lambda: expected_colliding_pairs(0, 0.1),
            lambda: expected_colliding_pairs(2, 1.5),
            lambda: same_leaf_pair_scattered(5, 0.5, 0),
            lambda: leaf_masses_contiguous(5, 0.5, 0),
            lambda: mde_per_unit_sigma(7),
            lambda: largest_resolvable_cv(0.0, 8),
        ):
            with self.assertRaises(ValueError):
                call()


if __name__ == "__main__":
    if "--table" in sys.argv:
        print(render_table())
        sys.exit(0)
    if "--self-test" not in sys.argv:
        print(render_table())
        print()
    sys.argv = [sys.argv[0]]
    unittest.main()
