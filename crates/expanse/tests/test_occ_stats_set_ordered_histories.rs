//! G36.1, the forms clause (#1194; `docs/benchmarks/concurrency/METHODOLOGY.md`
//! §36.4): over the rounds of each live ordered history on a shared set,
//! the walk visited every node form the history is registered for.
//!
//! `tests/linearizability.rs` checks the same histories against a
//! sequential set. That check cannot say which nodes the reads went
//! through, and the two histories that change a leaf's form on purpose
//! cannot be checked by the tree's state before and after: it is the same.
//! The visit counters of the walk, read as deltas over a whole history,
//! can. They also show that every read went through the optimistic
//! protocol: `ReadOps` moves by the number of reads the scripts hold.
//!
//! Its own test binary, and one single test inside it, for the reason
//! `test_occ_stats_readers.rs` gives: the counters are process-global, and
//! a neighbouring test doing reads would move every delta measured here.
//! Adding a second `#[test]` to this file reintroduces that.
#![cfg(feature = "occ-stats")]
// Excluded from the nightly Miri lane rather than given a shard there: that
// lane builds without `occ-stats`, so a shard for this target would run
// zero tests and report green.
#![cfg(not(miri))]

use expanse_trie::occ_stats::{self, Stat};

mod set_ordered_histories;

use set_ordered_histories::{History, ROUNDS, SetOp, SetRet};

/// The counters of the terminal forms inside a tree.
const TREE_TERMINALS: [Stat; 12] = [
    Stat::OrderedVisitImmed,
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

/// The reads each scripted phase of a phased history makes in a form the
/// barriers guarantee: two writers, two reads each, two cycles.
const PHASE_READS: u64 = 8;

/// Runs `ROUNDS` rounds of `h` and requires, of the deltas over all of
/// them: one read operation per read in the scripts, and for each group of
/// `forms` at least its stated number of visits per round (a group is
/// satisfied by any of its counters). A phased history states
/// `PHASE_READS`: the scripted reads made while the barriers hold the tree
/// in that form, which no schedule can take away. The other histories
/// state 0, which asks for one visit over all rounds.
fn visited(h: &History, forms: &[(&str, &[Stat], u64)]) {
    let before = occ_stats::snapshot();
    let mut answered = 0u64;
    for _ in 0..ROUNDS {
        let (events, _) = set_ordered_histories::run(h);
        for e in &events {
            match (e.op, e.ret) {
                (op, SetRet::Found(_)) if op.is_read() => answered += 1,
                (SetOp::Insert(_) | SetOp::Remove(_), SetRet::Changed(_)) => {}
                (SetOp::Clear | SetOp::Compact, SetRet::Done) => {}
                other => panic!("{}: {other:?}", h.name),
            }
        }
    }
    let after = occ_stats::snapshot();
    let d = |s: Stat| after[s as usize] - before[s as usize];
    let reads = (ROUNDS * h.reads()) as u64;
    assert!(reads > 0, "{}: the history holds no read", h.name);
    assert_eq!(answered, reads, "{}: reads answered", h.name);
    assert_eq!(
        d(Stat::ReadOps),
        reads,
        "{}: every read is one optimistic read operation",
        h.name
    );
    assert!(
        d(Stat::ReadAttempts) >= reads,
        "{}: at least one attempt per read",
        h.name
    );
    for (what, group, per_round) in forms {
        let n: u64 = group.iter().map(|&s| d(s)).sum();
        let floor = (per_round * ROUNDS as u64).max(1);
        assert!(
            n >= floor,
            "{}: {n} visits of {what} in {ROUNDS} rounds, at least {floor} expected",
            h.name
        );
    }
}

#[test]
fn set_ordered_histories_visit_the_forms_they_are_registered_for() {
    assert!(occ_stats::enabled());
    let immed: (&str, &[Stat], u64) = ("an immediate", &[Stat::OrderedVisitImmed], 0);
    let bitmap: (&str, &[Stat], u64) = (
        "a bitmap leaf",
        &[Stat::OrderedVisitBitmapLeaf],
        PHASE_READS,
    );

    // One key under each of 64 digits: immediates under a wide branch.
    visited(&set_ordered_histories::tree_rooted(), &[immed]);
    visited(&set_ordered_histories::hotspot(), &[immed]);
    // Both sides of `ROOT_LEAF_CAP`: the root leaf, and the tree the
    // inserts make of it.
    visited(
        &set_ordered_histories::root_leaf(),
        &[
            ("the root leaf", &[Stat::OrderedVisitRootLeaf], PHASE_READS),
            ("a terminal of the tree", &TREE_TERMINALS, PHASE_READS),
        ],
    );
    // Both sides of `LEAFB1_DOWN`: the bitmap leaf, and the one-byte linear
    // leaf the removals demote it to.
    visited(
        &set_ordered_histories::bitmap(),
        &[
            bitmap,
            (
                "a one-byte linear leaf",
                &[Stat::OrderedVisitLeaf1],
                PHASE_READS,
            ),
        ],
    );
    // The history that changes form on purpose: both forms, as registered.
    visited(
        &set_ordered_histories::full_expanse(),
        &[
            bitmap,
            (
                "a full expanse",
                &[Stat::OrderedVisitFullExpanse],
                PHASE_READS,
            ),
        ],
    );
    // `compact` and `clear` are writer operations, not forms: the set is a
    // tree before the first `clear` and a root leaf after it.
    visited(
        &set_ordered_histories::compact_clear(),
        &[(
            "the root leaf or a terminal of the tree",
            &[
                Stat::OrderedVisitRootLeaf,
                Stat::OrderedVisitImmed,
                Stat::OrderedVisitLeaf1,
            ],
            0,
        )],
    );
}
