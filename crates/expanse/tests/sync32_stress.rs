//! Threaded stress for the 32-bit single-writer/many-reader wrapper: one
//! writer churns while readers hammer the optimistic path. Excluded under
//! Miri — the validated racy reads are the documented seqlock trade the
//! `sync32` module docs cover, and are exercised under ASan/TSan in CI
//! instead.
#![cfg(not(miri))]

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
