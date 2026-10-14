//! G36.3 (#1194; `docs/benchmarks/concurrency/METHODOLOGY.md` §36.4, as
//! amended by §36.13 item 2): on quiescent sets every ordered read of
//! `SetReader`, `OwnedSetReader` and `DetachedSetReader` agrees with a
//! `BTreeSet`, answers through the optimistic protocol and never from the
//! lock, and between them the reads visit every node form a shared set has.
//!
//! The counter clauses are what make this more than a differential: without
//! them the test passes on a build whose every answer comes from the locked
//! fallback, which is correct by construction. Over a block of calls of
//! which `w` reach the walk (all but `next_after(u64::MAX)` and
//! `prev_before(0)`, which answer `None` first), `ReadOps` and
//! `ReadAttempts` each move by exactly `w` and `ReadFallbacks` by 0.
//!
//! Its own test binary, and one single test inside it, for the reason
//! `test_occ_stats_readers.rs` gives: the counters are process-global, the
//! harness runs `#[test]` functions on parallel threads, and a neighbouring
//! test doing reads would move every delta measured here. Adding a second
//! `#[test]` to this file reintroduces that.
#![cfg(feature = "occ-stats")]
// Excluded from the nightly Miri lane rather than given a shard there: that
// lane builds without `occ-stats`, so a shard for this target would run zero
// tests and report green. The reads it makes are run under Miri by
// `sync::miri_tests::set_ordered_reads_under_miri` and the
// `sync::miri_ub_sites::set_ordered_*` workloads.
#![cfg(not(miri))]

use std::collections::BTreeSet;
use std::ops::Bound::{Excluded, Included, Unbounded};
use std::sync::Arc;

use expanse_trie::occ_stats::{self, NUM_STATS, Stat};
use expanse_trie::sync::{DetachedSetReader, OwnedSetReader, SetReader, SyncExpanseSet};
use expanse_trie::types::{LEAF_CAP, LEAF1_CAP, LEAFB1_DOWN, ROOT_LEAF_CAP};

/// The six ordered reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    First,
    Last,
    NextAtOrAfter,
    NextAfter,
    PrevAtOrBefore,
    PrevBefore,
}

const OPS: [Op; 6] = [
    Op::First,
    Op::Last,
    Op::NextAtOrAfter,
    Op::NextAfter,
    Op::PrevAtOrBefore,
    Op::PrevBefore,
];

/// The model's answer, each read written with its own bounds.
fn model(m: &BTreeSet<u64>, op: Op, key: u64) -> Option<u64> {
    match op {
        Op::First => m.first().copied(),
        Op::Last => m.last().copied(),
        Op::NextAtOrAfter => m.range((Included(key), Unbounded)).next().copied(),
        Op::NextAfter => m.range((Excluded(key), Unbounded)).next().copied(),
        Op::PrevAtOrBefore => m.range((Unbounded, Included(key))).next_back().copied(),
        Op::PrevBefore => m.range((Unbounded, Excluded(key))).next_back().copied(),
    }
}

/// Whether a call reaches the walk. `next_after(u64::MAX)` and
/// `prev_before(0)` answer `None` before it and count no read operation.
fn walks(op: Op, key: u64) -> bool {
    !matches!((op, key), (Op::NextAfter, u64::MAX) | (Op::PrevBefore, 0))
}

/// The three handle forms, each with its six methods: eighteen in all.
enum Handle<'a> {
    Borrowed(SetReader<'a>),
    Owned(OwnedSetReader),
    Detached(DetachedSetReader),
}

const FORMS: [&str; 3] = ["SetReader", "OwnedSetReader", "DetachedSetReader"];

impl Handle<'_> {
    fn read(&self, set: &SyncExpanseSet, op: Op, key: u64) -> Option<u64> {
        match self {
            Handle::Borrowed(r) => match op {
                Op::First => r.first(),
                Op::Last => r.last(),
                Op::NextAtOrAfter => r.next_at_or_after(key),
                Op::NextAfter => r.next_after(key),
                Op::PrevAtOrBefore => r.prev_at_or_before(key),
                Op::PrevBefore => r.prev_before(key),
            },
            Handle::Owned(r) => match op {
                Op::First => r.first(),
                Op::Last => r.last(),
                Op::NextAtOrAfter => r.next_at_or_after(key),
                Op::NextAfter => r.next_after(key),
                Op::PrevAtOrBefore => r.prev_at_or_before(key),
                Op::PrevBefore => r.prev_before(key),
            },
            Handle::Detached(r) => match op {
                Op::First => r.first(set),
                Op::Last => r.last(set),
                Op::NextAtOrAfter => r.next_at_or_after(set, key),
                Op::NextAfter => r.next_after(set, key),
                Op::PrevAtOrBefore => r.prev_at_or_before(set, key),
                Op::PrevBefore => r.prev_before(set, key),
            },
        }
    }
}

/// The counters of the terminal forms the walk visits (§36.4, G36.3).
const VISITS: [Stat; 14] = [
    Stat::OrderedVisitRootLeaf,
    Stat::OrderedVisitImmed,
    Stat::OrderedVisitImmedFull,
    Stat::OrderedVisitLeaf1,
    Stat::OrderedVisitLeaf2,
    Stat::OrderedVisitLeaf3,
    Stat::OrderedVisitLeaf4,
    Stat::OrderedVisitLeaf5,
    Stat::OrderedVisitLeaf6,
    Stat::OrderedVisitLeaf7,
    Stat::OrderedVisitLeafNarrow,
    Stat::OrderedVisitBitmapLeaf,
    Stat::OrderedVisitBitmapLeafNarrow,
    Stat::OrderedVisitFullExpanse,
];

/// A shared set and its model.
struct Fixture {
    set: Arc<SyncExpanseSet>,
    model: BTreeSet<u64>,
}

fn build(keys: impl IntoIterator<Item = u64>) -> Fixture {
    let set = Arc::new(SyncExpanseSet::new());
    let mut model = BTreeSet::new();
    for k in keys {
        assert_eq!(set.insert(k), model.insert(k), "insert {k:#x}");
    }
    Fixture { set, model }
}

impl Fixture {
    /// Removes `key` through `SyncExpanseSet::remove`.
    fn take(&mut self, key: u64) {
        assert!(self.model.remove(&key), "the model holds {key:#x}");
        assert!(self.set.remove(key), "the set holds {key:#x}");
    }
}

/// 0, `u64::MAX`, and each of `keys` with its two neighbours: present keys,
/// absent keys and both ends of the key space.
fn probes_around(keys: impl IntoIterator<Item = u64>) -> Vec<u64> {
    let mut p: BTreeSet<u64> = [0, u64::MAX].into_iter().collect();
    for k in keys {
        p.extend([k.wrapping_sub(1), k, k.wrapping_add(1)]);
    }
    p.into_iter().collect()
}

/// `b` repeated in the low `n` bytes.
fn rep_byte(b: u8, n: u8) -> u64 {
    (0..n).fold(0, |k, _| (k << 8) | u64::from(b))
}

/// The key whose bytes above level `kb + 1` are `0x11`, whose digit at level
/// `kb + 1` is `d`, and whose low `kb` bytes are `low`.
fn key_under(d: u8, kb: u8, low: u64) -> u64 {
    (((rep_byte(0x11, 7 - kb) << 8) | u64::from(d)) << (8 * kb)) | low
}

/// Per-handle totals of every counter, over the whole test.
type Totals = [[u64; NUM_STATS]; 3];

/// Every read of every handle form over every probe, against the model, with
/// the counter clauses per handle form: the `w` calls that reach the walk are
/// `w` read operations and `w` attempts, and none falls back.
fn counted(what: &str, f: &Fixture, probes: &[u64], totals: &mut Totals) {
    let set = &*f.set;
    let handles = [
        Handle::Borrowed(set.reader()),
        Handle::Owned(f.set.owned_reader()),
        Handle::Detached(set.detached_reader()),
    ];
    for (form, h) in handles.iter().enumerate() {
        let name = FORMS[form];
        let before = occ_stats::snapshot();
        let mut w = 0u64;
        // `first` and `last` take no key: once per block.
        let calls = [(Op::First, 0), (Op::Last, 0)].into_iter().chain(
            probes
                .iter()
                .flat_map(|&p| OPS[2..].iter().map(move |&op| (op, p))),
        );
        for (op, p) in calls {
            assert_eq!(
                h.read(set, op, p),
                model(&f.model, op, p),
                "{what}: {name}, {op:?}({p:#x}) against the model"
            );
            w += u64::from(walks(op, p));
        }
        let after = occ_stats::snapshot();
        let d = |s: Stat| after[s as usize] - before[s as usize];
        assert_eq!(
            d(Stat::ReadFallbacks),
            0,
            "{what}: {name}: a quiescent read fell back to the lock"
        );
        assert_eq!(d(Stat::ReadOps), w, "{what}: {name}: read operations");
        assert_eq!(
            d(Stat::ReadAttempts),
            w,
            "{what}: {name}: one attempt per read on a quiescent set"
        );
        for (t, (a, b)) in totals[form].iter_mut().zip(after.iter().zip(&before)) {
            *t += a - b;
        }
    }
}

/// The generator of the Callgrind arms' `keys` (`benches/instructions.rs`).
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// The `random`, `sequential` and `clustered` distributions of the Callgrind
/// arms' `keys`, from the same generator and seed, at a population that
/// keeps a debug-build run short.
fn distribution(dist: &str) -> Vec<u64> {
    const POP: u64 = 6_000;
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    match dist {
        "sequential" => (0..POP).collect(),
        "random" => (0..POP).map(|_| rng.next()).collect(),
        "clustered" => {
            let mut base = 0;
            (0..POP)
                .map(|i| {
                    if i % 256 == 0 {
                        base = rng.next() & !0xFF;
                    }
                    base + (i % 256)
                })
                .collect()
        }
        other => panic!("unknown distribution {other}"),
    }
}

/// The fixtures built for one node form each, by the recipes of
/// `sync::set_ordered_fixtures` (which an integration target cannot reach):
/// name, fixture, probes.
fn form_fixtures() -> Vec<(String, Fixture, Vec<u64>)> {
    let mut rows = Vec::new();
    let all = |f: &Fixture| probes_around(f.model.iter().copied());
    let mut push = |name: String, f: Fixture, probes: Vec<u64>| rows.push((name, f, probes));

    let f = build([]);
    push("empty".into(), f, vec![0, 1, u64::MAX]);
    for pop in [1, 8, ROOT_LEAF_CAP] {
        let f = build((1..=pop as u64).map(|i| i * rep_byte(1, 8)));
        let p = all(&f);
        push(format!("root leaf of {pop}"), f, p);
    }

    // Immediates of 1, 2, 7 and 15 one-byte keys under one level-2 branch,
    // and one of 2 seven-byte keys under the top branch.
    let mut keys = Vec::new();
    for (d, n) in [(0x21u8, 1u64), (0x22, 2), (0x23, 15), (0x24, 7), (0x25, 15)] {
        keys.extend((0..n).map(|i| key_under(d, 1, 0x41 + i)));
    }
    keys.extend([
        (0x31 << 56) | rep_byte(0x41, 7),
        (0x31 << 56) | rep_byte(0x42, 7),
    ]);
    let f = build(keys);
    let p = all(&f);
    push("immediates".into(), f, p);

    // Three linear leaves per key width: the smallest a shared set makes
    // (§36.13 item 1), a capacity-class boundary, and the cap.
    for kb in 1..=7u8 {
        let smallest = [16usize, 8, 6, 4, 4, 3, 3][kb as usize - 1];
        let (boundary, cap) = if kb == 1 {
            (24, LEAF1_CAP)
        } else {
            (16, LEAF_CAP)
        };
        let pops = [smallest, boundary, cap];
        let f = build((0..3usize).flat_map(|j| {
            (0..pops[j]).map(move |i| key_under(0x21 + j as u8, kb, rep_byte(0x41 + i as u8, kb)))
        }));
        let p = all(&f);
        push(format!("Leaf{kb} of {pops:?}"), f, p);
    }

    // One `LeafBitmap1` of 28 keys in a level-1 slot, beside an immediate.
    let f = build(
        (0x10..=0x1Du64)
            .chain(0x90..=0x9D)
            .map(|low| key_under(0x21, 1, low))
            .chain((0..5).map(|i| key_under(0x22, 1, 0x41 + i))),
    );
    let p = all(&f);
    push("LeafBitmap1".into(), f, p);

    // 33 keys sharing their upper seven bytes: a bitmap leaf behind a narrow
    // pointer; and what removals leave of it below `LEAFB1_DOWN`, a
    // one-byte linear leaf that keeps the skip (§36.13 item 8).
    let narrow = || (0..33u64).map(|i| key_under(0x21, 1, 0x10 + 7 * i));
    let f = build(narrow());
    let p = all(&f);
    push("narrow LeafBitmap1".into(), f, p);
    for pop in [3usize, LEAFB1_DOWN - 1] {
        let mut f = build(narrow());
        for k in narrow().take(33 - pop) {
            f.take(k);
        }
        let p = all(&f);
        push(format!("narrow Leaf1 of {pop}"), f, p);
    }

    // Two adjacent full expanses, one key below them and three above.
    let f = build(
        std::iter::once(key_under(0x20, 1, 0x41))
            .chain((0x21..=0x22u8).flat_map(|d| (0..=0xFFu64).map(move |low| key_under(d, 1, low))))
            .chain((0..3).map(|i| key_under(0x23, 1, 0x41 + i))),
    );
    let ends = [
        key_under(0x21, 1, 0),
        key_under(0x21, 1, 0xFF),
        key_under(0x22, 1, 0),
        key_under(0x22, 1, 0xFF),
    ];
    let p = probes_around(f.model.iter().copied().step_by(17).chain(ends));
    push("full expanses".into(), f, p);

    // A `BranchU` and a `BranchB` over immediates.
    let f = build((0..200u8).map(|d| key_under(0x20 + d, 1, 0x40)));
    let p = all(&f);
    push("uncompressed branch".into(), f, p);
    let f = build((0..12u8).flat_map(|i| {
        let d = 0x40 + 2 * i;
        let n = if d == 0x44 { 1 } else { 3 };
        (0..n).map(move |j| key_under(d, 1, 0x41 + j))
    }));
    let p = all(&f);
    push("bitmap branch".into(), f, p);
    rows
}

/// The read-set bound (`scripts/olc_bounds.py::ordered_read_set_branches`):
/// a branch at every level on the probe's path and on its sibling's, so a
/// search that finds nothing under one top digit and answers under the other
/// retains 1 + 6 + 6 = 13 versions. The recipe is
/// `sync::set_ordered_fixtures::bound_fixture`.
fn bound_fixture() -> Fixture {
    let mut keys = Vec::new();
    for (top, main, side) in [(0x30u64, 0x02u8, 0x01u64), (0x40, 0x01, 0x02)] {
        let path = (top << 56) | (rep_byte(main, 5) << 16);
        for d in [0x01u64, 0x02] {
            keys.extend((0..17).map(|i| path | (d << 8) | (0x41 + i)));
        }
        for j in 3..=7u8 {
            let above = path >> (8 * j) << (8 * j);
            keys.push(above | (side << (8 * (j - 1))) | rep_byte(0x01, j - 1));
        }
    }
    build(keys)
}

#[test]
fn set_reader_ordered_reads_answer_optimistically_on_every_form() {
    assert!(occ_stats::enabled());
    let mut totals: Totals = [[0; NUM_STATS]; 3];

    // One node form per fixture, as built and after `compact`.
    for (what, f, probes) in form_fixtures() {
        counted(&what, &f, &probes, &mut totals);
        f.set.compact();
        counted(&format!("{what}, compacted"), &f, &probes, &mut totals);
    }

    // A tree over each `keys` distribution; a dense set holding bitmap
    // leaves and full expanses; each also after `compact`, and as what
    // `SyncExpanseSet::remove` leaves of it when most keys are removed.
    let dense: Vec<u64> = (0..1024u64)
        .chain((0..8u64).flat_map(|b| (0..40u64).map(move |i| 0x7000_0000 + (b << 8) + 3 * i)))
        .chain((0..200u64).map(|i| 0x9900_0000_0000 + i))
        .collect();
    let shapes = [
        ("random", distribution("random")),
        ("sequential", distribution("sequential")),
        ("clustered", distribution("clustered")),
        ("dense", dense),
    ];
    for (what, keys) in shapes {
        let mut f = build(keys.iter().copied());
        let probes = probes_around(keys.iter().copied().step_by(7));
        counted(what, &f, &probes, &mut totals);
        if what == "dense" {
            let counts = f.set.with_locked(|s| s.stats().node_counts);
            assert!(
                counts.leaf_bitmap > 0 && counts.full_expanse > 0,
                "the dense set must hold bitmap leaves and full expanses: {counts:?}"
            );
        }
        f.set.compact();
        counted(&format!("{what}, compacted"), &f, &probes, &mut totals);
        for (i, &k) in keys.iter().enumerate() {
            if i % 10 != 0 && f.model.contains(&k) {
                f.take(k);
            }
        }
        counted(&format!("{what}, mostly removed"), &f, &probes, &mut totals);
        f.set.compact();
        counted(
            &format!("{what}, mostly removed, compacted"),
            &f,
            &probes,
            &mut totals,
        );
    }

    // Every form was visited, through each handle form.
    for (form, name) in FORMS.iter().enumerate() {
        for s in VISITS {
            assert!(
                totals[form][s as usize] > 0,
                "no {name} read visited {}",
                occ_stats::NAMES[s as usize]
            );
        }
    }

    // The bound: a successor search that finds nothing under `0x30` and a
    // predecessor search that finds nothing under `0x40` each retain exactly
    // 13 versions and answer on the first attempt. The high-water mark is a
    // gauge, so it is zeroed first.
    let f = bound_fixture();
    let set = &*f.set;
    let handles = [
        Handle::Borrowed(set.reader()),
        Handle::Owned(f.set.owned_reader()),
        Handle::Detached(set.detached_reader()),
    ];
    let bound_probes = [
        (
            Op::NextAtOrAfter,
            (0x30 << 56) | (rep_byte(0x02, 6) << 8) | 0x7F,
        ),
        (
            Op::PrevAtOrBefore,
            (0x40 << 56) | (rep_byte(0x01, 6) << 8) | 0x10,
        ),
    ];
    for (form, h) in handles.iter().enumerate() {
        for (op, probe) in bound_probes {
            let name = FORMS[form];
            occ_stats::reset();
            assert_eq!(
                h.read(set, op, probe),
                model(&f.model, op, probe),
                "{name}, {op:?}"
            );
            let s = occ_stats::snapshot();
            assert_eq!(
                s[Stat::OrderedReadSetHwm as usize],
                13,
                "{name}, {op:?}: versions retained"
            );
            assert_eq!(s[Stat::ReadOps as usize], 1, "{name}, {op:?}");
            assert_eq!(s[Stat::ReadAttempts as usize], 1, "{name}, {op:?}");
            assert_eq!(s[Stat::ReadFallbacks as usize], 0, "{name}, {op:?}");
        }
    }
}
