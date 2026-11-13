//! Integration tests for the concurrency ablation arms (Hypothesis D). Arms
//! (a), (b) and (c) are all production defaults now, each with an inverse
//! feature that restores the state it replaced (AGENTS.md §2.7):
//! - Arm (a): per-writer-slot allocator accounting counters, the default since
//!   #568/#930; `ablation-unsharded-alloc` restores the shared counters
//! - Arm (b): striped epoch garbage bins and sharded retained bytes, the
//!   default since #568
//! - Arm (c): per-stripe collector freelists, the default since #568;
//!   `ablation-unstriped-freelist` restores the single shared array
//!
//! These check that each build stays correct under real threads,
//! and CI also runs them under ASan. Which stripe a thread lands on is
//! pinned by the `ablation_` unit tests in `alloc::tests` and `occ::tests`,
//! which can choose it.

// Kept as its own attribute, and in this exact form: the nightly Miri shard
// census (`scripts/check_miri_shards.py`) matches `^#!\[cfg\(not\(miri\)\)\]`
// literally rather than parsing nested `cfg(all(..))`.
#![cfg(not(miri))]
#![cfg(feature = "std")]

use expanse_trie::occ::Collector;
use expanse_trie::sync::SyncExpanseMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// Writers on disjoint key ranges insert and remove through the real
/// concurrent map while a reader looks keys up, so node retirement, epoch
/// advances and reclamation into the collector freelists all run with the
/// enabled ablations in place (with `occ-stats` on, one run counted tens of
/// thousands of covered fallbacks and retirements and thousands of advances).
/// Afterwards every surviving key must be present with its value and every
/// removed key absent.
#[test]
fn ablated_map_stays_correct_under_concurrent_writers() {
    const WRITERS: u64 = 4;
    const PER_WRITER: u64 = 20_000;
    // Scatters each writer's keys across the keyspace, so inserts split and
    // grow nodes and the covered fallback (the only path that advances the
    // epoch) runs.
    fn key(w: u64, i: u64) -> u64 {
        (w * PER_WRITER + i).wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }

    let map = Arc::new(SyncExpanseMap::new());
    let done = Arc::new(AtomicBool::new(false));
    let reader = {
        let (map, done) = (Arc::clone(&map), Arc::clone(&done));
        thread::spawn(move || {
            let mut i = 0u64;
            while !done.load(Ordering::Relaxed) {
                if let Some(v) = map.get(key(i % WRITERS, i % PER_WRITER)) {
                    assert_eq!(v, i % PER_WRITER, "a reader saw a foreign value");
                }
                i = i.wrapping_add(7919);
            }
        })
    };
    let writers: Vec<_> = (0..WRITERS)
        .map(|w| {
            let map = Arc::clone(&map);
            thread::spawn(move || {
                for i in 0..PER_WRITER {
                    map.insert(key(w, i), i);
                }
                for i in (0..PER_WRITER).step_by(2) {
                    assert_eq!(map.remove(key(w, i)), Some(i));
                }
            })
        })
        .collect();
    for h in writers {
        h.join().unwrap();
    }
    done.store(true, Ordering::Relaxed);
    reader.join().unwrap();

    assert_eq!(map.len(), WRITERS * PER_WRITER / 2);
    for w in 0..WRITERS {
        for i in 0..PER_WRITER {
            let want = if i % 2 == 1 { Some(i) } else { None };
            assert_eq!(map.get(key(w, i)), want, "writer {w} key {i}");
        }
    }
}

/// Arm (a)'s property: on a shared tree, whose allocator is deferred to the
/// collector, the per-writer-slot accounting shards sum to the exact total
/// when allocations happen on several threads, and net to zero when other
/// threads free them. It runs through `SyncExpanseMap`, the only safe way to
/// reach one allocator from several threads (`NodeAlloc` is `Send` and not
/// `Sync`), and the only place the shards exist.
///
/// - After the inserts, `mem_used` equals a plain map's built from the same
///   keys: the node census is fixed by the key set, not by the order or the
///   thread the keys came from (`tests/test_mem_used_order_invariant.rs`).
/// - Each thread then removes the range a different thread inserted, so every
///   node is freed (retired) on a different writer slot than allocated it,
///   and `mem_used` must come back to exactly 0.
///
/// Under `ablation-unsharded-alloc` there are no shards and the same
/// assertions check the shared inline counters instead.
#[test]
fn test_deferred_accounting_shards_sum_across_threads() {
    const WRITERS: u64 = 4;
    const PER: u64 = 5_000;
    fn key(w: u64, i: u64) -> u64 {
        (i << 8) | w
    }
    let map = Arc::new(SyncExpanseMap::new());
    let inserts: Vec<_> = (0..WRITERS)
        .map(|w| {
            let m = Arc::clone(&map);
            thread::spawn(move || {
                for i in 0..PER {
                    m.insert(key(w, i), i);
                }
            })
        })
        .collect();
    for h in inserts {
        h.join().expect("insert thread panicked");
    }

    let mut plain = expanse_trie::ExpanseMap::new();
    for w in 0..WRITERS {
        for i in 0..PER {
            plain.insert(key(w, i), i);
        }
    }
    assert_eq!(
        map.mem_used(),
        plain.mem_used(),
        "the shards do not sum to the plain map's accounting for the same keys"
    );

    let removes: Vec<_> = (0..WRITERS)
        .map(|w| {
            let m = Arc::clone(&map);
            // A range this thread did not insert.
            let other = (w + 1) % WRITERS;
            thread::spawn(move || {
                for i in 0..PER {
                    assert_eq!(m.remove(key(other, i)), Some(i));
                }
            })
        })
        .collect();
    for h in removes {
        h.join().expect("remove thread panicked");
    }
    assert_eq!(
        map.mem_used(),
        0,
        "frees on other writer slots do not net the allocations to zero"
    );
}

#[test]
fn test_striped_epoch_bins_multi_writer() {
    let collector = Arc::new(Collector::new());
    let num_writers = 4;
    let items_per_writer = 200;
    let done = Arc::new(AtomicBool::new(false));

    // Reader thread that continuously pins/unpins
    let r_collector = Arc::clone(&collector);
    let r_done = Arc::clone(&done);
    let reader_handle = thread::spawn(move || {
        let reader = r_collector.register();
        while !r_done.load(Ordering::Relaxed) {
            let _pin = reader.pin();
            thread::yield_now();
        }
    });

    let mut writer_handles = Vec::new();
    for _ in 0..num_writers {
        let c = Arc::clone(&collector);
        writer_handles.push(thread::spawn(move || {
            for _ in 0..items_per_writer {
                let layout = std::alloc::Layout::from_size_align(64, 16).unwrap();
                // SAFETY: layout has non-zero size (64) and valid alignment (16).
                let raw = unsafe { std::alloc::alloc_zeroed(layout) };
                let ptr = core::ptr::NonNull::new(raw).unwrap();
                // SAFETY: `ptr` is a fresh `(64, 16)` global allocation that
                // was never published and is not used again.
                unsafe { c.retire(ptr, 64, 16) };
                c.try_advance();
            }
        }));
    }

    for h in writer_handles {
        h.join().unwrap();
    }

    // Stop reader and let it unpin
    done.store(true, Ordering::Release);
    reader_handle.join().unwrap();

    // Advance epochs to allow all retired garbage to become freeable
    for _ in 0..10 {
        collector.try_advance();
    }

    // All retired items have passed their grace period and been reclaimed
    assert_eq!(collector.retained_bytes(), 0);
}

#[test]
fn test_striped_epoch_single_thread_lifecycle() {
    let collector = Arc::new(Collector::new());
    let reader = collector.register();
    let pin = reader.pin();

    let layout = std::alloc::Layout::from_size_align(64, 16).unwrap();
    // SAFETY: non-zero size (64) and valid alignment (16).
    let raw = unsafe { std::alloc::alloc_zeroed(layout) };
    let ptr = core::ptr::NonNull::new(raw).unwrap();
    // SAFETY: `ptr` is a fresh `(64, 16)` global allocation that was never
    // published and is not used again.
    unsafe { collector.retire(ptr, 64, 16) };
    assert_eq!(collector.retained_bytes(), 64);

    // Advance attempts under pin must not reclaim the block
    collector.try_advance();
    collector.try_advance();
    assert_eq!(collector.retained_bytes(), 64);

    drop(pin);
    collector.try_advance();
    collector.try_advance();
    assert_eq!(collector.retained_bytes(), 0);
}
