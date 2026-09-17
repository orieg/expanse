//! Threaded stress for the 32-bit single-writer/many-reader wrapper: one
//! writer churns while readers hammer the optimistic path. Excluded under
//! Miri — the validated racy reads are the documented seqlock trade the
//! `sync32` module docs cover, and are exercised under ASan/TSan in CI
//! instead.
#![cfg(not(miri))]

use std::collections::BTreeMap;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use expanse_trie::sync32::{Busy, SyncExpanseMap32, SyncExpanseSet32, WriteError};

/// Stable keys are inserted before the readers start and never mutated,
/// so every read of one must observe exactly its value — or `Busy`.
const STABLE: u32 = 512;
/// Churn keys live above the stable range and are inserted/removed
/// continuously while readers run.
const CHURN_ROUNDS: u32 = 40_000;

fn lcg(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *state
}

#[test]
fn map_readers_see_stable_keys_under_writer_churn() {
    let mut m = SyncExpanseMap32::with_capacity(16_384, 4);
    let (mut w, mut pool) = m.split();

    for k in 0..STABLE {
        w.try_insert(k, k ^ 0xABCD_1234).expect("prefill");
    }

    std::thread::scope(|s| {
        for _ in 0..3 {
            let mut r = pool.take().expect("reader slot");
            s.spawn(move || {
                let mut ok = 0u32;
                let mut busy = 0u32;
                let mut state = 7u32;
                while ok < 20_000 {
                    let k = lcg(&mut state) % STABLE;
                    match r.try_get(k) {
                        Ok(v) => {
                            assert_eq!(v, Some(k ^ 0xABCD_1234), "stable key {k} torn");
                            ok += 1;
                        }
                        Err(Busy) => busy += 1,
                    }
                    // A stalled-forever reader is a protocol violation;
                    // busy-dominated runs still finish, but must not hang.
                    assert!(busy < 500_000_000, "reader starved");
                }
                (ok, busy)
            });
        }

        // Writer churns keys disjoint from the stable prefix.
        let mut state = 0x5EED_5EEDu32;
        for i in 0..CHURN_ROUNDS {
            let k = STABLE + (lcg(&mut state) % 8_192);
            let res = if i % 3 == 2 {
                w.try_remove(k).map(|_| ())
            } else {
                w.try_insert(k, i).map(|_| ())
            };
            match res {
                Ok(()) | Err(WriteError::ArenaFull) => {}
                Err(WriteError::ReclaimBacklog) => {
                    // Readers are live, so a backlog is transient; yield
                    // and let them quiesce.
                    std::thread::yield_now();
                }
            }
        }
    });

    // After the scope every reader is done: reclamation must succeed and
    // the stable prefix must be intact through the writer's view.
    assert!(w.try_reclaim());
    assert_eq!(w.pending_len(), 0);
    for k in 0..STABLE {
        assert_eq!(w.get(k), Some(k ^ 0xABCD_1234));
    }
}

#[test]
fn set_readers_under_churn() {
    let mut set = SyncExpanseSet32::with_capacity(16_384, 2);
    let (mut w, mut pool) = set.split();
    for k in 0..STABLE {
        w.try_insert(k * 2).expect("prefill");
    }

    std::thread::scope(|s| {
        for _ in 0..2 {
            let mut r = pool.take().expect("reader slot");
            s.spawn(move || {
                let mut ok = 0u32;
                let mut state = 3u32;
                while ok < 15_000 {
                    let k = lcg(&mut state) % (STABLE * 2);
                    if let Ok(present) = r.try_contains(k) {
                        assert_eq!(
                            present,
                            k.is_multiple_of(2) && k < STABLE * 2,
                            "stable key {k}"
                        );
                        ok += 1;
                    }
                }
            });
        }
        let mut state = 99u32;
        for i in 0..CHURN_ROUNDS {
            let k = STABLE * 2 + (lcg(&mut state) % 8_192);
            let _ = if i % 3 == 2 {
                w.try_remove(k).map(|_| ())
            } else {
                w.try_insert(k).map(|_| ())
            };
        }
    });
    assert!(w.try_reclaim());
}

/// One ordered query on the 32-bit map (#900).
#[derive(Clone, Copy, Debug)]
enum Ord32 {
    First,
    Last,
    NextAtOrAfter(u32),
    NextAfter(u32),
    PrevAtOrBefore(u32),
    PrevBefore(u32),
}

/// The answer a sequential map in `state` gives to `q`.
fn ord_expected(state: &BTreeMap<u32, u32>, q: Ord32) -> Option<(u32, u32)> {
    let pair = |o: Option<(&u32, &u32)>| o.map(|(k, v)| (*k, *v));
    match q {
        Ord32::First => pair(state.first_key_value()),
        Ord32::Last => pair(state.last_key_value()),
        Ord32::NextAtOrAfter(k) => pair(state.range(k..).next()),
        Ord32::NextAfter(k) => k.checked_add(1).and_then(|s| pair(state.range(s..).next())),
        Ord32::PrevAtOrBefore(k) => pair(state.range(..=k).next_back()),
        Ord32::PrevBefore(k) => k
            .checked_sub(1)
            .and_then(|s| pair(state.range(..=s).next_back())),
    }
}

/// G12.5 (`docs/benchmarks/concurrency/METHODOLOGY.md` §12.2): every `try_*`
/// ordered read that answers returns the correct neighbour, with its value,
/// in a state the writer committed while the read ran.
///
/// The writer logs each round's applied mutation (or none) and then
/// publishes how many rounds it has finished. A reader loads that count
/// before and after each read. Its version sample saw every round counted
/// before it started, and at most one round finished but not yet counted
/// when it ended (there is one writer), so the read's state is one of rounds
/// `t0..=t1 + 1`. After the run the log is replayed and each answer must match
/// some state in that window.
///
/// The writer runs `MIN_ROUNDS` free, then keeps churning until the readers
/// report the coverage the oracle needs (`MIN_OVERLAP` reads taken while it
/// ran, `MIN_CHURN_ANSWERS` answers that are writer keys), up to `MAX_ROUNDS`.
/// A fixed round count let a fast writer finish before slow readers had
/// anything to check.
///
/// Two properties keep that coverage off the scheduler:
///
/// - **The live floor implies the replayed floor.** The writer loads the live
///   counters *before* it publishes the round. A reader loads `t0` before it
///   counts its answer, so every answer the writer saw started before that
///   round was published: `t0 < rounds`, which is what the replay counts.
///   Loading after the publish admits answers with `t0 == rounds` (a writer
///   descheduled between the two lets readers, unopposed, produce many), and
///   the run then stops on a floor the replay does not reach.
/// - **An extension round waits for an answer.** Every read is one attempt and
///   the tree has one version, so a writer that never pauses can turn nearly
///   every read into `Busy`. Past `MIN_ROUNDS` the writer, with its bracket
///   closed, yields until some reader has answered since the previous round.
///   A read that meets no bracket answers, so the wait ends while a reader is
///   still running, and each extension round adds at least one answer.
#[test]
fn map_ordered_reads_return_a_committed_neighbour_under_writer_churn() {
    const SPAN: u32 = 4_096;
    const STRIDE: u32 = 16;
    const MIN_ROUNDS: usize = 40_000;
    const MAX_ROUNDS: usize = 400_000;
    const READERS: u32 = 3;
    const MAX_READS: usize = 40_000;
    const MIN_OVERLAP: usize = 1_000;
    const MIN_CHURN_ANSWERS: usize = 100;
    const ALL_READS: usize = READERS as usize * MAX_READS;
    let mut m = SyncExpanseMap32::with_capacity(16_384, READERS as usize);
    let (mut w, mut pool) = m.split();
    let mut initial = BTreeMap::new();
    for k in (0..SPAN).step_by(STRIDE as usize) {
        w.try_insert(k, !k).expect("prefill");
        initial.insert(k, !k);
    }
    let committed = AtomicUsize::new(0);
    let done = AtomicBool::new(false);
    let live_overlap = AtomicUsize::new(0);
    let live_churn_answers = AtomicUsize::new(0);
    let start = Barrier::new(READERS as usize + 1);

    let (log, reads, busy) = std::thread::scope(|s| {
        let handles: Vec<_> = (0..READERS)
            .map(|id| {
                let mut r = pool.take().expect("reader slot");
                let (committed, done, start) = (&committed, &done, &start);
                let (live_overlap, live_churn_answers) = (&live_overlap, &live_churn_answers);
                s.spawn(move || {
                    let mut state = 17 + id;
                    let mut out = Vec::new();
                    let mut busy = 0u64;
                    start.wait();
                    while out.len() < MAX_READS && !done.load(Ordering::SeqCst) {
                        let k = lcg(&mut state) % (SPAN + STRIDE);
                        let q = match lcg(&mut state) % 6 {
                            0 => Ord32::First,
                            1 => Ord32::Last,
                            2 => Ord32::NextAtOrAfter(k),
                            3 => Ord32::NextAfter(k),
                            4 => Ord32::PrevAtOrBefore(k),
                            _ => Ord32::PrevBefore(k),
                        };
                        let t0 = committed.load(Ordering::SeqCst);
                        let got = match q {
                            Ord32::First => r.try_first(),
                            Ord32::Last => r.try_last(),
                            Ord32::NextAtOrAfter(k) => r.try_next_at_or_after(k),
                            Ord32::NextAfter(k) => r.try_next_after(k),
                            Ord32::PrevAtOrBefore(k) => r.try_prev_at_or_before(k),
                            Ord32::PrevBefore(k) => r.try_prev_before(k),
                        };
                        let t1 = committed.load(Ordering::SeqCst);
                        match got {
                            Ok(answer) => {
                                live_overlap.fetch_add(1, Ordering::SeqCst);
                                if matches!(answer, Some((k, _)) if k % STRIDE != 0) {
                                    live_churn_answers.fetch_add(1, Ordering::SeqCst);
                                }
                                out.push((q, answer, t0, t1));
                            }
                            Err(Busy) => busy += 1,
                        }
                    }
                    (out, busy)
                })
            })
            .collect();

        // Churn keys never coincide with a prefilled multiple of STRIDE.
        start.wait();
        let mut log = Vec::with_capacity(MIN_ROUNDS);
        let mut state = 0x0BAD_5EEDu32;
        for i in 0..MAX_ROUNDS {
            let k =
                (lcg(&mut state) % (SPAN / STRIDE)) * STRIDE + 1 + lcg(&mut state) % (STRIDE - 1);
            let applied = if i % 3 == 2 {
                w.try_remove(k).ok().map(|_| (k, None))
            } else {
                let v = i as u32;
                match w.try_insert(k, v) {
                    Ok(_) => Some((k, Some(v))),
                    Err(WriteError::ReclaimBacklog) => {
                        std::thread::yield_now();
                        None
                    }
                    Err(WriteError::ArenaFull) => None,
                }
            };
            log.push(applied);
            // Loaded before the publish below: see the test's doc comment.
            let overlap = live_overlap.load(Ordering::SeqCst);
            let churn_answers = live_churn_answers.load(Ordering::SeqCst);
            committed.store(i + 1, Ordering::SeqCst);
            if i + 1 < MIN_ROUNDS {
                continue;
            }
            if overlap >= MIN_OVERLAP && churn_answers >= MIN_CHURN_ANSWERS {
                break;
            }
            // Extension round: no bracket is open here, so a running reader
            // answers. Once every reader has filled its quota none is left to
            // wait for, and the floors below decide.
            while live_overlap.load(Ordering::SeqCst) == overlap && overlap < ALL_READS {
                std::thread::yield_now();
            }
        }
        done.store(true, Ordering::SeqCst);
        let mut reads = Vec::new();
        let mut busy = 0u64;
        for h in handles {
            let (out, b) = h.join().expect("reader thread");
            reads.extend(out);
            busy += b;
        }
        (log, reads, busy)
    });

    // The oracle only checks something if reads overlapped the churn and saw
    // the writer's keys. The writer stops on live counters that bound these
    // two from below, so failing here means it ran out of rounds or readers.
    let rounds = log.len();
    let during = reads.iter().filter(|r| r.2 < rounds).count();
    let churn_answers = reads
        .iter()
        .filter(|r| r.2 < rounds && matches!(r.1, Some((k, _)) if k % STRIDE != 0))
        .count();
    assert!(
        during >= MIN_OVERLAP,
        "only {during} reads overlapped the writer in {rounds} rounds (busy {busy})"
    );
    assert!(
        churn_answers >= MIN_CHURN_ANSWERS,
        "only {churn_answers} answers were writer keys in {rounds} rounds; the oracle would check nothing"
    );

    let mut by_start: Vec<Vec<usize>> = vec![Vec::new(); rounds + 1];
    for (i, r) in reads.iter().enumerate() {
        by_start[r.2].push(i);
    }
    let mut satisfied = vec![false; reads.len()];
    let mut active: Vec<usize> = Vec::new();
    let mut state = initial;
    for t in 0..=rounds {
        active.extend(by_start[t].iter().copied());
        active.retain(|&i| {
            let (q, answer, _, t1) = reads[i];
            if ord_expected(&state, q) == answer {
                satisfied[i] = true;
                return false;
            }
            t < (t1 + 1).min(rounds)
        });
        if t < rounds
            && let Some((k, v)) = log[t]
        {
            match v {
                Some(v) => {
                    state.insert(k, v);
                }
                None => {
                    state.remove(&k);
                }
            }
        }
    }
    let wrong: Vec<_> = reads
        .iter()
        .zip(&satisfied)
        .filter(|(_, ok)| !**ok)
        .map(|(r, _)| *r)
        .take(5)
        .collect();
    assert!(
        wrong.is_empty(),
        "ordered reads that match no committed state of their window: {wrong:?}"
    );
}
