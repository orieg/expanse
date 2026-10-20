//! `SyncExpanseStrMap::mem_used_estimate` (#1586,
//! `docs/benchmarks/concurrency/METHODOLOGY.md` §43.4): equal to `mem_used`
//! on a quiet map (G3), returning while a writer holds the map's serialised
//! section (G4), and inside its stated bound beside concurrent writers (G5).

use super::*;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// How long a step that must not block may take before the test fails.
const DEADLINE: Duration = Duration::from_secs(30);

fn nf(k: &[u8]) -> &NulFreeStr {
    NulFreeStr::new(k).expect("test keys are NUL-free")
}

fn route(i: usize) -> Vec<u8> {
    format!("/api/v2/tenants/{:06}/resources/{:04}", i / 16, i % 16).into_bytes()
}

/// 8 to 16 bytes, distinct first chunks: one suffix leaf each under the root.
fn short(i: usize) -> Vec<u8> {
    let mut k = format!("s{i:07}").into_bytes();
    k.extend(core::iter::repeat_n(b'x', i % 9));
    k
}

/// Joins `h`, failing with `what` if it has not finished by the deadline.
fn join_by_deadline<T>(h: std::thread::JoinHandle<T>, what: &str) -> T {
    let deadline = Instant::now() + DEADLINE;
    while !h.is_finished() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::yield_now();
    }
    h.join().expect("the thread panicked")
}

#[track_caller]
fn assert_quiet_equal(m: &SyncExpanseStrMap) -> usize {
    let exact = m.mem_used();
    assert_eq!(
        m.mem_used_estimate(),
        exact,
        "on a quiet map the estimate is the exact figure"
    );
    exact
}

/// The exact shell-and-suffix bytes, read with every writer excluded.
fn exact_ext(m: &SyncExpanseStrMap) -> usize {
    m.with_locked(ExpanseStrMap::test_unmanaged_bytes)
}

/// G3: with nothing in flight the estimate equals `mem_used`, through
/// splits, suffix leaves, overwrites, removals to empty, exclusive sections,
/// a clear, a map shared after it was populated, and concurrent writers
/// after they join.
#[test]
fn quiet_estimate_equals_mem_used() {
    let n: usize = if cfg!(miri) { 64 } else { 2_000 };
    let m = Arc::new(SyncExpanseStrMap::new());
    assert_eq!(assert_quiet_equal(&m), 0);
    for i in 0..n {
        m.insert(nf(&route(i)), i as u64);
    }
    let routes = assert_quiet_equal(&m);
    assert!(exact_ext(&m) > 0, "the routes hold shells");
    for i in 0..n {
        m.insert(nf(&short(i)), i as u64);
    }
    assert!(assert_quiet_equal(&m) > routes);
    for i in 0..n / 2 {
        assert_eq!(m.insert(nf(&short(i)), 7), Some(i as u64));
    }
    assert_quiet_equal(&m);
    for i in 0..n {
        assert_eq!(m.remove(nf(&route(i))), Some(i as u64));
    }
    assert_quiet_equal(&m);
    m.with_exclusive(|tx| {
        for i in 0..n / 4 {
            tx.insert(nf(&route(i)), 1);
            tx.remove(nf(&short(i)));
        }
    });
    assert_quiet_equal(&m);

    // Four writers on disjoint keys: inserts that split, then removals.
    let per: usize = if cfg!(miri) { 24 } else { 500 };
    let writers: Vec<_> = (0..4usize)
        .map(|t| {
            let m = Arc::clone(&m);
            std::thread::spawn(move || {
                let base = 100_000 + t * 10_000;
                for i in 0..per {
                    m.insert(nf(&route(base + i)), i as u64);
                    m.insert(nf(&short(base + i)), i as u64);
                }
                for i in 0..per / 2 {
                    assert!(m.remove(nf(&route(base + i))).is_some());
                    assert!(m.remove(nf(&short(base + i))).is_some());
                }
            })
        })
        .collect();
    for w in writers {
        w.join().expect("writer panicked");
    }
    assert_quiet_equal(&m);
    m.clear();
    assert_eq!(assert_quiet_equal(&m), 0);

    // A map shared after it was populated: rebuilt through a deferred map.
    let mut plain = ExpanseStrMap::new();
    for i in 0..n {
        plain.insert(nf(&route(i)), i as u64);
        plain.insert(nf(&short(i)), i as u64);
    }
    let shared = SyncExpanseStrMap::from(plain);
    assert!(assert_quiet_equal(&shared) > 0);
}

/// G4: a writer holds the serialised section after inserting inside it. The
/// estimate returns while it is held and equals the exact figure the holder
/// reads; a `mem_used` caller does not return until the section ends.
#[test]
fn estimate_returns_while_a_writer_holds_the_section() {
    let n: usize = if cfg!(miri) { 48 } else { 1_000 };
    let m = Arc::new(SyncExpanseStrMap::new());
    for i in 0..n {
        m.insert(nf(&route(i)), i as u64);
    }
    let before = assert_quiet_equal(&m);
    let locked: std::sync::Mutex<Option<std::thread::JoinHandle<usize>>> =
        std::sync::Mutex::new(None);
    let (held_exact, during) = m.exclusive(|map| {
        for i in n..2 * n {
            map.insert(nf(&route(i)), i as u64);
        }
        let exact = map.mem_used();
        assert!(exact > before);
        let m2 = Arc::clone(&m);
        let est = std::thread::spawn(move || m2.mem_used_estimate());
        let during = join_by_deadline(
            est,
            "mem_used_estimate did not return while the section was held",
        );
        let m3 = Arc::clone(&m);
        let waiting = std::thread::spawn(move || m3.mem_used());
        // Long enough for an unblocked `mem_used` to have finished its walk.
        std::thread::sleep(Duration::from_millis(if cfg!(miri) { 0 } else { 50 }));
        assert!(
            cfg!(miri) || !waiting.is_finished(),
            "mem_used returned while a writer held the section"
        );
        *locked.lock().unwrap() = Some(waiting);
        (exact, during)
    });
    assert_eq!(
        during, held_exact,
        "the estimate is the held section's exact figure"
    );
    let waiting = locked.lock().unwrap().take().expect("stored");
    let after = join_by_deadline(waiting, "mem_used did not return after the section");
    assert_eq!(after, held_exact);
    assert_eq!(assert_quiet_equal(&m), held_exact);
}

/// Writer `t`'s `i`th key: a first chunk no other key shares, then a fixed
/// `TAIL`, so its insert and its removal are each exactly one suffix leaf and
/// no shell (§43.4 G5).
fn g5_key(t: usize, i: usize) -> Vec<u8> {
    let mut k = format!("{t}{i:07}").into_bytes();
    k.extend_from_slice(TAIL);
    k
}
const TAIL: &[u8] = b"-payload-tail";
const G5_WRITERS: usize = 4;

/// The lowest and highest shell-and-suffix reading a poller sees while `f`
/// runs, and how many readings it took.
fn polled_ext(m: &Arc<SyncExpanseStrMap>, f: impl FnOnce()) -> (usize, usize, usize, usize) {
    let stop = Arc::new(AtomicBool::new(false));
    let (lo, hi, polls) = (
        Arc::new(AtomicUsize::new(usize::MAX)),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
    );
    let full_hi = Arc::new(AtomicUsize::new(0));
    let poller = {
        let (m, stop, lo, hi, polls, full_hi) = (
            Arc::clone(m),
            Arc::clone(&stop),
            Arc::clone(&lo),
            Arc::clone(&hi),
            Arc::clone(&polls),
            Arc::clone(&full_hi),
        );
        std::thread::spawn(move || {
            // At least one reading before and one after the writers: the
            // loop body runs once before it tests `stop`.
            loop {
                let done = stop.load(Ordering::Acquire);
                let ext = m.ext_estimate();
                lo.fetch_min(ext, Ordering::Relaxed);
                hi.fetch_max(ext, Ordering::Relaxed);
                full_hi.fetch_max(m.mem_used_estimate(), Ordering::Relaxed);
                polls.fetch_add(1, Ordering::Relaxed);
                if done {
                    break;
                }
                std::thread::yield_now();
            }
        })
    };
    // The first reading lands before any writer starts.
    let deadline = Instant::now() + DEADLINE;
    while polls.load(Ordering::Relaxed) == 0 {
        assert!(Instant::now() < deadline, "the poller never ran");
        std::thread::yield_now();
    }
    f();
    stop.store(true, Ordering::Release);
    poller.join().expect("poller panicked");
    (
        lo.load(Ordering::Relaxed),
        hi.load(Ordering::Relaxed),
        polls.load(Ordering::Relaxed),
        full_hi.load(Ordering::Relaxed),
    )
}

/// G5: four writers insert, then remove, one suffix leaf per operation under
/// a root kept keys hold alive, while a thread polls. (a) Insert phase: every
/// shell-and-suffix reading lies in `[S_before, S_after + W * s]`. (b) Remove
/// phase: in `[S_end, S_peak]`. (c) Afterwards the estimate equals
/// `mem_used`. The whole estimate gets a sanity range only, which §43.4 does
/// not gate.
#[test]
fn estimate_polled_beside_writers_stays_in_its_bound() {
    let per: usize = if cfg!(miri) { 16 } else { 2_000 };
    let m = Arc::new(SyncExpanseStrMap::new());
    // Kept keys, more than a root leaf holds, so the root sub-map is a tree
    // and the writers take the optimistic path; none is removed.
    let kept = 2 * crate::types::ROOT_LEAF_CAP;
    for i in 0..kept {
        m.insert(nf(&format!("K{i:07}tail").into_bytes()), i as u64);
    }
    let s = crate::strmap::test_suffix_ext_bytes(TAIL.len());
    let s_before = exact_ext(&m);
    let m_before = assert_quiet_equal(&m);

    let run = |remove: bool| {
        let writers: Vec<_> = (0..G5_WRITERS)
            .map(|t| {
                let m = Arc::clone(&m);
                std::thread::spawn(move || {
                    for i in 0..per {
                        let k = g5_key(t, i);
                        if remove {
                            assert!(m.remove(nf(&k)).is_some());
                        } else {
                            assert_eq!(m.insert(nf(&k), i as u64), None);
                        }
                    }
                })
            })
            .collect();
        for w in writers {
            w.join().expect("writer panicked");
        }
    };

    // (a) Insert phase.
    let (lo, hi, polls, full_hi) = polled_ext(&m, || run(false));
    let s_after = exact_ext(&m);
    let m_after = assert_quiet_equal(&m);
    assert!(polls >= 2);
    assert_eq!(
        s_after,
        s_before + G5_WRITERS * per * s,
        "one suffix per insert"
    );
    assert!(
        lo >= s_before,
        "insert phase: {lo} below S_before {s_before}"
    );
    assert!(
        hi <= s_after + G5_WRITERS * s,
        "insert phase: {hi} above S_after {s_after} + W * s"
    );
    assert!(
        full_hi <= 2 * m_after,
        "sanity: {full_hi} above twice mem_used"
    );

    // (b) Remove phase.
    let (lo, hi, _, _) = polled_ext(&m, || run(true));
    let s_end = exact_ext(&m);
    assert_eq!(s_end, s_before, "every writer key's suffix was freed");
    assert!(lo >= s_end, "remove phase: {lo} below S_end {s_end}");
    assert!(hi <= s_after, "remove phase: {hi} above S_peak {s_after}");

    // (c) Quiet again.
    let m_end = assert_quiet_equal(&m);
    assert!(m_end >= m_before / 2, "sanity: the kept keys' bytes remain");
}
