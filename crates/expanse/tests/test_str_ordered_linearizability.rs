//! Linearizability of the validated string ordered reads (#1143,
//! `docs/benchmarks/concurrency/METHODOLOGY.md` §33.3 and §33.8).
//!
//! The string twin of the whole-map ordered checker in `linearizability.rs`
//! (#900): an ordered answer depends on keys other than its argument, so a
//! history is checked against one sequential `BTreeMap<Vec<u8>, u64>` with
//! Wing & Gong's search, memoised on (operations applied, map state).
//!
//! A history that never reaches a cross-level answer would pass vacuously,
//! so the keys straddle the 8-byte chunk edge (lengths 7, 8, 9 and 16 that
//! share prefixes), a pair differs only in a terminal against a continuation
//! byte (`abcdefg` and `abcdefgh`), removals empty child nodes, and the run
//! asserts the `str_ordered_cross_level` counter — validated optimistic
//! answers taken after a `StrNode` level unwound — moved, and that fallbacks
//! stayed a small share of the reads.
//!
//! The counters are compiled out without the `occ-stats` feature (AGENTS.md
//! §2.1 invariant 5), so this file is empty in the default build and
//! `scripts/test_occ_stats.sh` runs it with the feature on.
#![cfg(feature = "occ-stats")]
// Excluded from the nightly Miri lane rather than given a shard there: that
// lane builds without `occ-stats`, so a shard would run zero tests. The
// reads it makes are covered by `sync::str_ordered_tests` and the
// `str_ordered_reader_writer` UB-site workload.
#![cfg(not(miri))]

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Instant;

use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::SyncExpanseStrMap;

type Key = Vec<u8>;
type State = BTreeMap<Key, u64>;

#[derive(Clone, Debug, PartialEq)]
enum Op {
    Insert(Key, u64),
    Remove(Key),
    NextAtOrAfter(Key),
    NextAfter(Key),
    PrevAtOrBefore(Key),
    PrevBefore(Key),
}

#[derive(Clone, Debug, PartialEq)]
enum Ret {
    Old(Option<u64>),
    Found(Option<(Key, u64)>),
}

#[derive(Clone, Debug)]
struct Event {
    op: Op,
    ret: Ret,
    start: Instant,
    end: Instant,
}

fn nf(k: &[u8]) -> &NulFreeStr {
    NulFreeStr::new(k).expect("NUL-free test key")
}

fn entry((k, v): (&Key, &u64)) -> (Key, u64) {
    (k.clone(), *v)
}

/// The map after `op`, if the sequential map in `state` returns `ret`.
fn apply(state: &State, op: &Op, ret: &Ret) -> Option<State> {
    let found = |got: Option<(Key, u64)>| match ret {
        Ret::Found(r) => (*r == got).then(|| state.clone()),
        Ret::Old(_) => None,
    };
    match op {
        Op::Insert(k, v) => match ret {
            Ret::Old(old) => (state.get(k).copied() == *old).then(|| {
                let mut s = state.clone();
                s.insert(k.clone(), *v);
                s
            }),
            Ret::Found(_) => None,
        },
        Op::Remove(k) => match ret {
            Ret::Old(old) => (state.get(k).copied() == *old).then(|| {
                let mut s = state.clone();
                s.remove(k);
                s
            }),
            Ret::Found(_) => None,
        },
        Op::NextAtOrAfter(k) => found(state.range(k.clone()..).next().map(entry)),
        Op::NextAfter(k) => {
            let mut succ = k.clone();
            succ.push(1);
            found(state.range(succ..).next().map(entry))
        }
        Op::PrevAtOrBefore(k) => found(state.range(..=k.clone()).next_back().map(entry)),
        Op::PrevBefore(k) => found(state.range(..k.clone()).next_back().map(entry)),
    }
}

fn linearizable(events: &[Event], initial: &State) -> bool {
    assert!(events.len() <= 64, "the search memoises on a u64 mask");
    fn search(
        events: &[Event],
        used: u64,
        state: &State,
        seen: &mut HashSet<(u64, State)>,
    ) -> bool {
        if used.count_ones() as usize == events.len() {
            return true;
        }
        if !seen.insert((used, state.clone())) {
            return false;
        }
        let min_end = events
            .iter()
            .enumerate()
            .filter(|(i, _)| used & (1u64 << i) == 0)
            .map(|(_, e)| e.end)
            .min();
        for (i, e) in events.iter().enumerate() {
            if used & (1u64 << i) != 0 {
                continue;
            }
            if let Some(me) = min_end
                && me < e.start
            {
                continue;
            }
            if let Some(next) = apply(state, &e.op, &e.ret)
                && search(events, used | (1u64 << i), &next, seen)
            {
                return true;
            }
        }
        false
    }
    search(events, 0, initial, &mut HashSet::new())
}

/// The checker rejects the answer a walk that forgets a passed level can
/// return: two inserts, one into the level the read passed over and one
/// that becomes the sibling's minimum, and a read overlapping both that
/// returns the latter.
#[test]
fn checker_rejects_a_key_that_was_never_the_successor() {
    let initial: State = [(b"bbbbbbbbM".to_vec(), 2)].into_iter().collect();
    let mut t = vec![Instant::now()];
    while t.len() < 8 {
        let n = Instant::now();
        if n > *t.last().unwrap() {
            t.push(n);
        }
    }
    let writes = [
        Event {
            op: Op::Insert(b"aaaaaaaaZZ".to_vec(), 10),
            ret: Ret::Old(None),
            start: t[1],
            end: t[2],
        },
        Event {
            op: Op::Insert(b"bbbbbbbbA".to_vec(), 11),
            ret: Ret::Old(None),
            start: t[3],
            end: t[4],
        },
    ];
    let read = |got: Option<(Key, u64)>| Event {
        op: Op::NextAtOrAfter(b"aaaaaaaaZ".to_vec()),
        ret: Ret::Found(got),
        start: t[0],
        end: t[5],
    };
    let bad = [
        writes[0].clone(),
        writes[1].clone(),
        read(Some((b"bbbbbbbbA".to_vec(), 11))),
    ];
    assert!(!linearizable(&bad, &initial));
    for got in [
        Some((b"bbbbbbbbM".to_vec(), 2)),
        Some((b"aaaaaaaaZZ".to_vec(), 10)),
    ] {
        let good = [writes[0].clone(), writes[1].clone(), read(got.clone())];
        assert!(linearizable(&good, &initial), "{got:?}");
    }
}

/// Keys that straddle the chunk edge and share prefixes, so ordered answers
/// cross `StrNode` levels.
fn keys() -> Vec<Key> {
    [
        &b"abcdefg"[..],
        b"abcdefgh",
        b"abcdefghi",
        b"abcdefghij",
        b"abcdefghijklmnop",
        b"abcdefghijklmnopq",
        b"abcdefgz",
        b"abcdefgzz",
    ]
    .iter()
    .map(|k| k.to_vec())
    .collect()
}

/// Probes that land inside a child node with nothing at or after (or
/// before) them there, so the answer comes from another level.
fn probes() -> Vec<Key> {
    [
        &b"abcdefgh\xFF"[..],
        b"abcdefghijklmnop\xFF",
        b"abcdefgh",
        b"abcdefg\x01",
        b"abcdefgz\x01",
        b"abcdefghi\x01",
    ]
    .iter()
    .map(|k| k.to_vec())
    .collect()
}

fn history(threads: usize, per_thread: usize, round: usize) -> (Vec<Event>, State) {
    let map = Arc::new(SyncExpanseStrMap::new());
    let mut initial = State::new();
    // A permanent neighbour on each side keeps answers defined; the keys are
    // inserted and removed by the threads.
    for (k, v) in [(b"a".to_vec(), 1u64), (b"b".to_vec(), 2)] {
        map.insert(nf(&k), v);
        initial.insert(k, v);
    }
    for (i, k) in keys()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| (i + round).is_multiple_of(2))
    {
        map.insert(nf(&k), 100 + i as u64);
        initial.insert(k, 100 + i as u64);
    }
    let (keys, probes) = (Arc::new(keys()), Arc::new(probes()));
    let out = Arc::new(Mutex::new(Vec::new()));
    // The threads start their operations together, so they overlap.
    let start_line = Arc::new(Barrier::new(threads));
    let hs: Vec<_> = (0..threads)
        .map(|t| {
            let (map, keys, probes, out, start_line) = (
                Arc::clone(&map),
                Arc::clone(&keys),
                Arc::clone(&probes),
                Arc::clone(&out),
                Arc::clone(&start_line),
            );
            thread::spawn(move || {
                let rd = map.reader();
                let mut local = Vec::with_capacity(per_thread);
                start_line.wait();
                for i in 0..per_thread {
                    let k = keys[(t * 5 + i + round) % keys.len()].clone();
                    let p = probes[(t * 3 + i + round) % probes.len()].clone();
                    let op = match (t + i + round) % 6 {
                        0 => Op::Insert(k, (t * 1000 + i) as u64),
                        1 => Op::Remove(k),
                        2 => Op::NextAtOrAfter(p),
                        3 => Op::NextAfter(p),
                        4 => Op::PrevAtOrBefore(p),
                        _ => Op::PrevBefore(p),
                    };
                    let start = Instant::now();
                    let ret = match &op {
                        Op::Insert(k, v) => Ret::Old(map.insert(nf(k), *v)),
                        Op::Remove(k) => Ret::Old(map.remove(nf(k))),
                        Op::NextAtOrAfter(k) => Ret::Found(rd.next_at_or_after(nf(k))),
                        Op::NextAfter(k) => Ret::Found(rd.next_after(nf(k))),
                        Op::PrevAtOrBefore(k) => Ret::Found(rd.prev_at_or_before(nf(k))),
                        Op::PrevBefore(k) => Ret::Found(rd.prev_before(nf(k))),
                    };
                    let end = Instant::now();
                    local.push(Event {
                        op,
                        ret,
                        start,
                        end,
                    });
                }
                drop(rd);
                out.lock().unwrap().extend(local);
            })
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
    let events = out.lock().unwrap().clone();
    assert_eq!(events.len(), threads * per_thread);
    (events, initial)
}

/// G33.3 for the point ops: every recorded history is linearizable, the
/// histories contain validated optimistic cross-level answers, and the
/// reads fall back rarely.
#[test]
fn str_ordered_reads_are_linearizable() {
    let before = occ_stats::snapshot();
    let rounds = 1000;
    for round in 0..rounds {
        let (events, initial) = history(3, 16, round);
        assert!(
            linearizable(&events, &initial),
            "round {round}: a string ordered history is not linearizable: {events:#?}"
        );
    }
    let after = occ_stats::snapshot();
    let d = |s: Stat| after[s as usize] - before[s as usize];
    let (ops, cross, fallbacks, answers, overflows) = (
        d(Stat::ReadOps),
        d(Stat::StrOrderedCrossLevel),
        d(Stat::ReadFallbacks),
        d(Stat::StrOrderedAnswers),
        d(Stat::StrOrderedOverflows),
    );
    eprintln!(
        "str ordered: read_ops={ops} answers={answers} cross_level={cross} \
         fallbacks={fallbacks} overflows={overflows}"
    );
    assert!(
        cross > 0,
        "no validated optimistic cross-level answer: the history is vacuous"
    );
    assert_eq!(overflows, 0, "these keys are at most 3 levels deep");
    assert!(
        fallbacks * 20 < ops,
        "fallbacks must be under 5% of reads: {fallbacks} of {ops}"
    );
    assert_eq!(
        answers + fallbacks,
        ops,
        "every read is an answer or a fallback"
    );
}
