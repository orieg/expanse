# DNS-Shaped Keys: Methodology and Pre-Registration

Status: pre-registered, no data (AGENTS.md §8.8 commit 2; refs #1800). This
file is frozen once the first reference-host run is dispatched: later changes
are appended as dated amendments and nothing above them is edited. It is done
when the suite's `README.md` carries a verdict for every prediction in §4.

## 1. Question

Does an ordered byte-key container of Expanse have a measured advantage, on
keys shaped like domain names, over the two structures a name server would
otherwise use for its zone index? The suite measures the index alone: no wire
parsing, no sockets, no zone-file loading.

The answer gates any DNS server work (§6). No such server exists in this
repository and none is proposed here.

## 2. Arms

| Arm | Structure | Key form | Role |
|---|---|---|---|
| `expanse_ordered` | `ExpanseOrderedBytesMap` | `key_nul` | subject |
| `expanse_direct` | `ExpanseStrMap` | `key_direct` | measures what the ordered map's escape pass costs |
| `btreemap` | `std::collections::BTreeMap<Vec<u8>, u64>` | `key_nul` | twin |
| `qp_trie` | [`qp-trie`](https://github.com/sdleffler/qp-trie-rs) `=0.8.2` (MPL-2.0), `Trie<Vec<u8>, u64>` | `key_nul` | twin |

Every arm stores the same `u64` payload (§8.16). `qp-trie` is already pinned
as a dev-dependency of `expanse-trie` with a crate-scoped licence exception in
`deny.toml`; no dependency is added.

Both key forms are defined and tested in `scripts/dns_name_bounds.py`:
labels lowercased, reversed, each followed by a separator, so byte order is
the canonical name order of RFC 4034 §6.1. `key_nul` separates with `0x00`
and escapes nothing. `key_direct` is NUL-free, which `ExpanseStrMap` requires:
it separates with `0x01` and escapes label octets `0x00..=0x02`.
`ExpanseOrderedBytesMap` stores `key_nul` under its own escape, so every probe
of that arm is scanned and re-encoded into a stack buffer before the lookup
(`with_encoded_key`, `crates/expanse/src/ordered_bytesmap.rs`); `expanse_direct`
skips that pass and is the control for it.

**Which arm can do what.** `qp-trie` 0.8.2 has no predecessor or range call
(`trie.rs`: `get`, `iter_prefix`, `subtrie`, `longest_common_prefix`), and its
traversal is not in byte order (`docs/benchmarks/patricia_comparison/METHODOLOGY.md`
§1). It therefore cannot answer the predecessor operation and is recorded
`NOT_APPLICABLE` there, never as a loss. It runs the other three operations.

**Where a twin can win (§8.3).** Neither twin is denied the metric on the
operations it runs. A B-tree's predecessor is its ordinary search and returns
a borrowed key, while Expanse must rebuild the key bytes from the path; that
is the cell where `btreemap` is most likely to win. `qp-trie` branches on the
first differing nybble and so reads fewer key bytes than Expanse on a miss
that diverges early.

## 3. Workload

**Names.** `scripts/dns_name_bounds.py::gen_names(n)`, seed `NAME_SEED`: six
TLDs, a second-level label of 3 to 16 characters drawn uniformly from
`[a-z0-9]`, and zero to three host names (`www`, `mail`, `ns1`) under it. The
populations are prefixes of one stream. `crates/expanse/tests/test_dns_name_bounds.rs`
reproduces the generator in Rust and pins it against the script.

**The names are synthetic, and that bounds every claim (§6).** Uniform random
labels share no prefix beyond the TLD, so the set has less shared structure
than a real zone. No real name list is used: none was found whose licence and
reproducibility were verified. A real list is a separate amendment, registered
before its data.

**Populations.** n ∈ {10^3, 10^4, 10^5, 10^6}. Predictions are evaluated at
10^5 and 10^6; the two smaller populations are reported as `NOT_PREREGISTERED`.

**Insertion order: both (§8.12.4).** `sorted` is ascending key order, a
B-tree's append path. `shuffled` is a Fisher–Yates permutation from the suite
PRNG. Every result row records its order, every timing prediction is evaluated
on each order separately, and a verdict needs both.

**Probes.** Built and encoded outside the timed region, shuffled, and reused
across rounds. Each probe class is a row of its own; two classes are never
averaged into one figure (§8.6).

| Class | Definition |
|---|---|
| `hit` | a uniformly drawn half of the population |
| `miss_new_sld` | names from the continuation of the generator stream whose second-level name is absent from the population |
| `miss_under_sld` | `<label>.<sld>.<tld>`: a present two-label name drawn uniformly, with one label in front drawn by the generator's label rule from a second seeded stream, rejected on membership |
| `encloser` | a `miss_under_sld` name with k − 1 further labels in front, k uniform in 1..=3 |

`miss_new_sld` is the §8.6 form: same generator, rejected on membership.
`miss_under_sld` is the shape of a name-error answer inside a served zone. The
two end at different depths, which is why they are separate rows.

**Operations.**

| `workload_id` | Operation | Probes | Per arm |
|---|---|---|---|
| `dns_get` | exact lookup, 50% hit | `hit` with `miss_new_sld`; `hit` with `miss_under_sld` | `get` |
| `dns_predecessor` | largest name at or before an absent name, key and value returned | `miss_new_sld`; `miss_under_sld` | `prev_at_or_before_entry_decode_into` into a caller buffer; `ExpanseStrMap::prev_at_or_before_into`; `BTreeMap::range(..=k).next_back()`; `qp_trie` `NOT_APPLICABLE` |
| `dns_closest_encloser` | strip the leftmost label until a lookup hits | `encloser` | one `get` per label; k + 1 lookups per probe, the last one a hit |
| `dns_memory` | live heap after the build | none | requested bytes from an allocator hook on every arm; `mem_used()` beside it for the Expanse arms |
| `dns_predecessor_concurrent` | `dns_predecessor` on `miss_under_sld` at W readers, n = 10^6, sorted build | as above | §5 |

No arm has a longest-prefix-match call, so the closest encloser is the same
loop on every arm. Stripping a label is a prefix slice of the reversed key, at
offsets computed in setup. The predecessor sink folds the value, the returned
key length and the key's last byte, so no arm's key is dead code. The
allocating form `prev_at_or_before_entry` is timed as an extra row
`expanse_ordered_owned` with no prediction.

**Measured region.** Probe loop only. Builds, probe generation, encoding and
`Drop` are outside it.

**Rounds and statistics.** 12 rounds per cell, one process per round, arm
order rotated each round, repetitions calibrated to the 20 ms `MIN_WINDOW` of
`benches/patricia_common`. A timing figure is the geometric mean of the
per-round ratio (Expanse ns ÷ twin ns) with its BCa 95% interval over per-round
log ratios, 2,000 resamples. Reference host, the hybrid-host core pin
(`scripts/bench_pin.sh`), under the bench lock, with load snapshots and the
busy-CPU delta in the artifact (§8.17). A contaminated run is discarded and the
discard disclosed.

## 4. Pre-registration

**What was seen before these predictions were written.** No timing and no
memory figure of any arm on these keys. The inputs are the derived bounds
below, and the `patricia_comparison` suite's results for `ExpanseStrMap`
against `qp-trie` on path keys. The predictions are therefore blind on DNS
keys and not blind on the string engine in general. No result here has had
independent review.

**Derived inputs** (`python3 scripts/dns_name_bounds.py --report`; exact for
the engine, pinned at n = 24 and 2,000 by the Rust test):

| n | key form | stored B/key | shells | suffix leaves | mean levels | lower bound on `mem_used`, B/name | sub-maps above `ROOT_LEAF_CAP` (entries) |
|---|---|---|---|---|---|---|---|
| 10^5 | `key_direct` | 16.59 | 40,636 | 38,193 | 2.22 | 47.51 | 1 (42,209) |
| 10^5 | `key_nul` as stored | 19.19 | 45,328 | 53,153 | 2.39 | 56.16 | 1 (38,629) |
| 10^6 | `key_direct` | 16.62 | 406,032 | 381,697 | 2.23 | 47.62 | 1 (416,923) |
| 10^6 | `key_nul` as stored | 19.22 | 403,542 | 520,527 | 2.42 | 54.56 | 1 (302,744) |

The bound leaves out the one sub-map above `ROOT_LEAF_CAP`, whose bytes the
script does not derive. For `BTreeMap<Vec<u8>, u64>` the script gives
52.6 to 112.0 B/name at 10^5 and 52.7 to 112.0 at 10^6 for `key_nul`,
depending on node fill. The two ranges overlap, so the bounds do not decide
the memory comparison; M1 below is a hypothesis.

**Verdict labels.** For a timing prediction, per cell:
- `PASS`: the interval lies wholly on the predicted side of 1.0, in two
  independent dispatches.
- `REFUTED`: wholly on the other side, in two dispatches.
- `BOUNDARY_RESULT`: the interval spans 1.0 in either dispatch.
- `INTERMEDIATE`: the two dispatches disagree, or only one exists.
- `NOT_APPLICABLE`: the arm has no call for the operation.
- `UNPREDICTED_LOSS`: a cell with no prediction whose interval lies wholly
  above 1.0.

A prediction's overall verdict is `PASS` only if every one of its cells is.
Exact predictions (census, bytes) are `PASS` or `REFUTED` on integer
comparison. Changing a threshold, a probe class, the rounds or the populations
after the first dispatch relabels the affected verdicts `INTERMEDIATE` and
needs fresh runs (§8.19).

**Predictions.** Cells are n ∈ {10^5, 10^6} × {sorted, shuffled} × each probe
class of the operation, unless stated.

| ID | Operation | Against | Prediction |
|---|---|---|---|
| E1 | build | the script | `stats()` shells and suffix leaves of `expanse_direct` and `expanse_ordered` equal the table above; `mem_used()` is at least the script's lower bound; in both orders |
| G1 | `dns_get` | `btreemap` | `expanse_ordered` faster |
| G2 | `dns_get` | `qp_trie` | `expanse_ordered` faster |
| G3 | `dns_get` | `expanse_direct` | `expanse_ordered` slower: the escape pass has a cost |
| N1 | `dns_predecessor` | `btreemap` | `expanse_ordered` faster at n = 10^6 |
| N2 | `dns_predecessor` | `qp_trie` | `NOT_APPLICABLE` |
| L1 | `dns_closest_encloser` | `btreemap` | `expanse_ordered` faster |
| L2 | `dns_closest_encloser` | `qp_trie` | `expanse_ordered` faster |
| M1 | `dns_memory` | `btreemap` | `expanse_ordered` fewer requested bytes in the `sorted` build |
| M2 | `dns_memory` | `qp_trie` | `expanse_ordered` fewer requested bytes, both orders |
| M3 | `dns_memory` | `expanse_direct` | `expanse_direct` has the smaller `mem_used()`, both orders |

**Basis, and what would refute it.**
- G2, L2: on path keys with an 8-byte shared prefix, `ExpanseStrMap` measured
  0.7055 [0.6905, 0.7135] of `qp_trie`'s time at n = 100,000 and
  0.4455 [0.4444, 0.4465] at n = 1,000,000, generator order (workload:
  patricia_string_lookup; measured: i9-12900F reference host, c1b604ad). DNS
  keys are short and share little, which is the regime of that cell.
  `expanse_ordered` adds the escape pass that cell did not have; G2 fails if
  that pass costs more than the margin.
- G1, L1: a hypothesis, unmeasured. A B-tree lookup compares the probe with
  several heap-allocated keys per level; Expanse descends 2.2 to 2.4 string
  levels and compares one suffix. Nothing here measures either cost.
- N1: a hypothesis, unmeasured, and the least certain timing prediction. It
  is stated only at 10^6; the 10^5 cells carry no direction.
- M1: the `sorted` build is where a B-tree's leaves are least full. The
  `shuffled` cells carry no direction.
- M2: `qp_trie` requested 90.59 B/key against 57.36 for `ExpanseStrMap` on the
  8-byte-prefix path keys at n = 1,000,000 (workload: patricia_memory; same
  run). Those keys are longer than DNS keys, so the margin here is unknown.

**Expected losses.** Cells with no direction, where a loss is plausible and
would be reported as measured: `dns_predecessor` at 10^5 against `btreemap`;
`dns_memory` against `btreemap` in the `shuffled` build; `expanse_ordered_owned`
on every predecessor cell, since it allocates per call. G3 is a predicted loss
of the subject arm against its own control.

## 5. Concurrent readers

`SyncExpanseOrderedBytesMap::prev_at_or_before_decode_into` runs under the
writer lock; only its point `get` is optimistic (`crates/expanse/src/sync.rs`).
A server answering from zone snapshots would instead share an immutable
`ExpanseOrderedBytesMap` by reference. This harness sizes the difference. It
is a separate file (§8.15), read-only, with no writer thread.

| Arm | Call |
|---|---|
| `shared_ref` | `ExpanseOrderedBytesMap::prev_at_or_before_entry_decode_into` through `&` from W threads |
| `sync_locked` | `SyncExpanseOrderedBytesMap::prev_at_or_before_decode_into` |
| `sync_get` (control) | `SyncExpanseOrderedBytesMap::get` on `hit` probes |

W ∈ {1, 2, 4, 8}, builds interleaved within each round, 8 rounds, pin `0-15`
(the reference host's P-core logical CPUs) for every arm. The pin is part of
each figure: a lock-serialised arm moves with the pin (§8.20.5), so these
cells are comparable with each other and with nothing taken under another pin.
Throughput is operations per second summed over readers; S(W) is throughput at
W over throughput at 1, with a BCa 95% interval over rounds. Verdicts need two
dispatches, as in §4.

| ID | Arm | Prediction |
|---|---|---|
| C1 | `sync_locked` | does not scale: the upper bound of S(8) is below 2 |
| C2 | `shared_ref` | scales: the lower bound of S(8) is above 4 |
| C3 | `sync_get` | scales: the lower bound of S(8) is above 4 |

The bounds 2 and 4 are chosen, not derived: 4 is half of linear at 8 readers.
C3 is the control. If C3 fails, the host or the pin limits scaling and C1 says
nothing about the lock. The harness also reports `shared_ref` throughput over
`sync_locked` throughput at W = 8 with its interval. An optimistic predecessor
(rejected in hand-over-hand form in #900) is worth reopening only if the lower
bound of that ratio is at least 2 and a caller cannot use snapshots; this suite
measures the first condition and not the second.

## 6. Decision and claims ceiling

- **No winning cell ends the DNS line of work.** A winning cell is a `PASS` on
  G1, N1 or L1 together with no `REFUTED` against `qp_trie` on the same
  operation.
- **"A measured advantage to build on"** requires G1 and N1 both `PASS`: exact
  lookup and the predecessor are the two index operations an authoritative
  answer and its denial of existence use. L1, M1 and M2 are reported and do not
  enter the decision.
- **Ceiling.** Every verdict is about these synthetic names, these two twin
  implementations at these versions, one host, and single-threaded reads
  (§5 apart). None is a claim about real zones, about B-trees or QP-tries in
  general, or about a name server's throughput.

## 7. Not in this commit

The harnesses, their `# Workload shape` tables, the `.github/bench-suites.json`
entries, `run.sh`, the suite `README.md` and every result arrive with the data
(§8.8 commit 3).
