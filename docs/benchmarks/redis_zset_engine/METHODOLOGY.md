# Redis ZSET Engine — Methodology & Design Verdict

This suite evaluates whether an Expanse digital trie can serve as a Redis Sorted
Set (ZSET) engine, and how it compares to Redis's actual design: a **skip list**
(for score ordering and rank) plus a **hash-table dict** (for `O(1)` member →
score lookup).

The crux of issue #330 is a **design claim to verify, not assume**: that Expanse
does in a *single* structure what Redis does in two. This document settles that
first (Step 0), derives the claims ceiling and the cells Expanse is *expected to
lose*, then specifies the measurement harness. Numbers live in
[`README.md`](README.md); this file is design and method.

---

## 1. What a ZSET actually requires

A sorted set maps `member -> score` and supports, at minimum:

| Command | Access path it needs |
|---|---|
| `ZADD` / `ZINCRBY` | member → score (to find/replace), plus ordered insert |
| `ZSCORE` | member → score |
| `ZREM` | member → score (to locate the ordered entry to delete) |
| `ZRANK` | member → its position in `(score, member)` order |
| `ZRANGEBYSCORE` / `ZREVRANGEBYSCORE` | score-ordered iteration |
| `ZRANGE` (by rank) | position → member (select) |
| `ZCOUNT` | cardinality of a score window |

Two *distinct* keyings are unavoidable: one **by member** (for `ZSCORE`,
`ZREM`, `ZINCRBY`, and the score half of `ZRANK`) and one **by `(score,
member)`** (for ordering, range, rank, count). Redis serves the first with a
dict and the second with a skip list.

## 2. Modeling choices

* **Members and scores are `u32`.** Members are dense IDs `0..N`; scores are
  drawn from a bounded domain (default `[0, 1_000_000)`), so many members share
  a score and the **member tie-break is genuinely exercised** — a realistic
  leaderboard, not an all-distinct-scores best case.
* **Composite ordering key** `(score << 32) | member : u64`. Ordering by this
  `u64` is *exactly* Redis's total order (score first, member as tie-break).
  This is the key `ExpanseMap` and the reference skip list both order by.
* **Score domain limits.** The `u32`×`u32` pack fits `ExpanseMap`'s `u64` key
  space. Redis scores are IEEE doubles; a full-fidelity port would need a
  128-bit composite key (which Expanse does not expose) or a monotonic double→
  u64 encoding. Stated plainly so the model's ceiling is not overread.

## 3. Step 0 — design verdict (the claim under test)

**Verdict: the "single structure" framing is REFUTED for the full ZSET command
surface. Expanse needs two structures, exactly like Redis — but a *better* two.**

A single composite-key `ExpanseMap` (keyed by `(score, member)`) supports
`ZRANGEBYSCORE`, `ZREVRANGEBYSCORE`, `ZRANK`-by-score, `ZCOUNT`, and
`ZRANGE`-by-rank. It **cannot** answer `ZSCORE(member)`, `ZREM(member)`,
`ZINCRBY(member)`, or `ZRANK(member)` without first knowing the member's score —
member is in the *low* bits of the key, not a prefix, so a member lookup would
be an `O(n)` scan. A single member-keyed map has the inverse problem: no score
order. **No single Expanse structure serves both keyings.**

So the Expanse ZSET is **dual**: a composite-key `order` map + a `member ->
score` map. What that dual buys over Redis's dual is real and is the actual
thesis to test:

1. **Homogeneous, cache-conscious.** Both halves are the same compact digital
   trie, versus Redis's heterogeneous skip list (random-height pointer nodes)
   plus dict.
2. **Rank without span bookkeeping.** The ordered half answers `ZRANK`/`ZCOUNT`
   via `count_below` / `count_range` — an `O(depth)` walk that sums *precomputed*
   subtree population counters (`ExpanseMap::count_below`, `crate::nav::count_below`).
   Redis must maintain per-level **span** counters on every insert/delete to get
   `O(log n)` rank; Expanse gets rank as a property of the structure it already
   has.
3. **Select** (`ZRANGE` by rank) via `by_count` (`crate::nav::by_count`), again
   from the population counters.

The primitive check the issue asks for: **a count-in-range / key→rank primitive
already exists** (`count_below`, `count_range`, `by_count`), verified to be
`O(depth)` over population counters, not an `O(n)` traversal. `ZRANK` is
therefore genuinely sublinear on Expanse — this is not a cell where Expanse is
forced into a linear-scan fallback.

## 4. Claims ceiling & expected losses (pre-registered)

Derived from the structures *before* measuring. A cell named a loss here is
published regardless of outcome.

| Cell | Prior | Reason |
|---|---|---|
| `ZSCORE` random member | **Expected LOSS** *(target — not yet measured: no pillar exercises ZSCORE and no ZSCORE cell exists in `results/`; this row is a structural prior, not a result)* | The dict is a flat hash probe (~12 ns at 1M, per the repo's stdlib table); the Expanse member map is a trie descent (~38 ns random at 1M). This is the hash table's home turf; the Expanse member half cannot match it on uniform-random members. |
| `ZREVRANGEBYSCORE` | **Expected LOSS** (prior); **resolved WIN** post-#341 | Prior (pre-#341): `ExpanseMap` had no reverse iterator, so descending iteration was repeated `prev_at_or_before`, `O(depth)` per element, versus the skip list's `O(1)` level-0 backward pointers. Resolution: the `range_rev` reverse ordered iterator (#341) amortizes descent to `O(1)` per member and flips both cells to wins (2.70× / 1.63× over the skip list). The emulated re-descent is retained as a suite arm for the emulated-vs-native comparison. |
| `ZRANGE` by rank / select | **Toss-up, lean LOSS** | `by_count` re-descends summing populations; the skip list's span descent is a tight `O(log n)` pointer walk. |
| `ZRANK` | **Toss-up** | `count_below` `O(depth)` vs span `O(log n)`. Both sublinear; decided by measurement, not structure. |
| `ZCOUNT` | **Toss-up** | Two rank primitives on each side. |
| `ZRANGEBYSCORE` forward | **Lean WIN** | Expanse's stack-based range iterator streams contiguous leaves; the repo already measures dense forward iteration at or above `BTreeMap`. |
| `ZADD` / churn | **Lean WIN** | Trie inserts avoid the skip list's random-height allocation and pointer chasing, though Expanse updates two structures. |
| Memory / member | **Lean WIN** | Two compact tries vs skip-list fat nodes + dict. And the baseline here is *conservative* (below). |

## 5. Reference implementation (the Redis side)

A **span-augmented skip list** (William Pugh, *Skip Lists: A Probabilistic
Alternative to Balanced Trees*, 1990) plus the standard order-statistic **span**
counters that give `O(log n)` `ZRANK`/`ZRANGE`-by-rank, next to a
`hashbrown::HashMap<u32, u32>` dict. Written **clean-room from the published
skip-list algorithm — no Redis source consulted** (Redis is BSD, unrelated to
this repo's clean-room concern with LGPL libjudy, but the discipline is kept).
`p = 1/4`, max 32 levels — Redis's own parameters.

**The skip list is arena-backed** (nodes in a `Vec`, `u32` index links, a boxed
per-node level array) with a free list. This is a *conservative* model of the
baseline in two ways, both of which make an Expanse memory win a **floor**, not
a ceiling:

* No per-node `malloc` header or allocator fragmentation (a production
  pointer-node skip list pays both).
* Members are `u32` inline, not Redis's heap-allocated `sds` strings.

## 6. Measurement discipline (per `docs/BENCHMARKING.md`)

* **Rule 0 — measured region.** Only the operation/query loop is timed. Member
  permutations (Fisher–Yates shuffle so no engine sees a favorable monotonic
  build), score streams, query streams, and pre-population are built in setup.
  Each bench's module doc states its timed window.
* **Rule 1 — interleaved A/B.** Every round runs Expanse then SkipList+Dict for
  the same scenario before advancing, so runner/thermal drift hits both arms.
* **Rule 5 — distributions, medians.** Throughput is the median over rounds
  (5 on the reference host, 2 in `--quick`).
* **Rule 6 — fixed seeds.** Every bench names its xorshift64 seed and
  populations; runs are bit-reproducible.
* **Rule 2 — load hygiene.** The reference-host run snapshots load before and
  between arms; a non-target process above ~100% CPU voids the run.
* **Correctness gate.** Every bench calls `zset_common::validate()` before
  timing: it builds a small randomized set in both engines plus a brute-force
  `BTreeSet` oracle and asserts `zrank`/`zscore`/`zcount`/`zrangebyscore`/
  `zrevrangebyscore`/`zrange_by_rank` agree. A broken span counter or composite
  encoding panics instead of publishing wrong numbers.

## 7. Pillars

1. **ZADD churn** — `fresh_insert`, `score_update` (delete+insert of the
   composite key), `zincrby`, `mixed_churn`. Throughput (M ops/sec).
2. **Range iteration** — `ZRANGEBYSCORE` and `ZREVRANGEBYSCORE`, small (~64) and
   large (~8192) windows. Throughput (M elements/sec).
3. **Rank & count** — `ZRANK`, `ZCOUNT`, `ZRANGE`-by-rank (select). Throughput
   (M queries/sec) and ns/query.
4. **Memory** — bytes/member at 100k and 1M, random and sequential scores, with
   the order/members and list/dict sub-structure split.

## 8. Reproduction

```bash
./docs/benchmarks/redis_zset_engine/run.sh            # full suite + charts
./docs/benchmarks/redis_zset_engine/run.sh --quick    # fast smoke (small N)
```

Per-bench:

```bash
cargo bench -p expanse-trie --bench zset_rank -- --json
```
