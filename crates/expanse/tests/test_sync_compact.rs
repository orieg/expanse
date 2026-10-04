#![cfg(not(miri))]

//! Integration tests for `compact()` on `SyncExpanseMap` and `SyncExpanseSet` (Refs #1200).
//!
//! Excluded from Miri: tests spawn threads, interleave high operation counts,
//! and test concurrent read/write scaling. Miri coverage is provided by
//! `sync::miri_ub_sites`.

use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

static SUITE_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_sync_map_compact_empty() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let map = SyncExpanseMap::new();
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    map.compact();
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    map.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_set_compact_empty() {
    let set = SyncExpanseSet::new();
    assert_eq!(set.len(), 0);
    assert!(set.is_empty());
    set.compact();
    assert_eq!(set.len(), 0);
    assert!(set.is_empty());
    set.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_map_compact_identity_small() {
    let map = SyncExpanseMap::new();
    for i in 0..10 {
        assert_eq!(map.insert(i, !i), None);
    }
    assert_eq!(map.len(), 10);
    map.compact();
    assert_eq!(map.len(), 10);
    for i in 0..10 {
        assert_eq!(map.get(i), Some(!i));
    }
    map.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_set_compact_identity_small() {
    let set = SyncExpanseSet::new();
    for i in 0..10 {
        assert!(set.insert(i));
    }
    assert_eq!(set.len(), 10);
    set.compact();
    assert_eq!(set.len(), 10);
    for i in 0..10 {
        assert!(set.contains(i));
    }
    set.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_map_compact_identity_large() {
    let map = SyncExpanseMap::new();
    for i in 0..1_000 {
        let k = i * 7 + 3;
        assert_eq!(map.insert(k, !k), None);
    }
    assert_eq!(map.len(), 1_000);
    map.compact();
    assert_eq!(map.len(), 1_000);
    for i in 0..1_000 {
        let k = i * 7 + 3;
        assert_eq!(map.get(k), Some(!k));
    }
    map.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_set_compact_identity_large() {
    let set = SyncExpanseSet::new();
    for i in 0..1_000 {
        let k = i * 7 + 3;
        assert!(set.insert(k));
    }
    assert_eq!(set.len(), 1_000);
    set.compact();
    assert_eq!(set.len(), 1_000);
    for i in 0..1_000 {
        let k = i * 7 + 3;
        assert!(set.contains(k));
    }
    set.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_map_post_compact_optimistic_insert_takes_olc_path() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let map = SyncExpanseMap::new();
    // Insert past ROOT_LEAF_CAP so the root is a Tree
    for i in 0..128 {
        map.insert(i * 2, !(i * 2));
    }
    assert_eq!(map.len(), 128);
    map.compact();
    assert_eq!(map.len(), 128);

    // Compacted tree must remain prepared for concurrent OCC operation
    map.with_locked(|inner| {
        assert!(
            inner.is_occ_enabled(),
            "compacted tree allocator must be deferred to collector"
        );
        assert!(
            inner.is_engine_covers_root(),
            "compacted tree must have engine root cover active"
        );
    });

    #[cfg(feature = "occ-stats")]
    let before = expanse_trie::occ_stats::snapshot();

    // Insert new key into existing leaf must take the optimistic path without falling back
    assert_eq!(map.insert(1, !1), None);
    assert_eq!(map.get(1), Some(!1));

    #[cfg(feature = "occ-stats")]
    {
        let after = expanse_trie::occ_stats::snapshot();
        let fallbacks = after[expanse_trie::occ_stats::Stat::LockFallbacks as usize]
            - before[expanse_trie::occ_stats::Stat::LockFallbacks as usize];
        assert_eq!(
            fallbacks, 0,
            "post-compact optimistic insert must not fall back to writer lock"
        );
    }
    map.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_set_post_compact_optimistic_insert_takes_olc_path() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let set = SyncExpanseSet::new();
    for i in 0..128 {
        set.insert(i * 2);
    }
    assert_eq!(set.len(), 128);
    set.compact();
    assert_eq!(set.len(), 128);

    set.with_locked(|inner| {
        assert!(
            inner.is_occ_enabled(),
            "compacted set allocator must be deferred to collector"
        );
        assert!(
            inner.is_engine_covers_root(),
            "compacted set must have engine root cover active"
        );
    });

    #[cfg(feature = "occ-stats")]
    let before = expanse_trie::occ_stats::snapshot();

    assert!(set.insert(1));
    assert!(set.contains(1));

    #[cfg(feature = "occ-stats")]
    {
        let after = expanse_trie::occ_stats::snapshot();
        let fallbacks = after[expanse_trie::occ_stats::Stat::LockFallbacks as usize]
            - before[expanse_trie::occ_stats::Stat::LockFallbacks as usize];
        assert_eq!(
            fallbacks, 0,
            "post-compact optimistic set insert must not fall back to writer lock"
        );
    }
    set.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_map_compact_reclaims_memory() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let map = SyncExpanseMap::new();
    const TOTAL: u64 = 20_000;
    const REMOVE: u64 = 16_000;

    for i in 0..TOTAL {
        map.insert(i * 13, !i);
    }
    assert_eq!(map.len(), TOTAL);

    for i in 0..REMOVE {
        assert_eq!(map.remove(i * 13), Some(!i));
    }
    assert_eq!(map.len(), TOTAL - REMOVE);

    let used_before = map.mem_used();
    let held_before = map.mem_held();
    assert!(held_before >= used_before);

    map.compact();

    let used_after = map.mem_used();
    let held_after = map.mem_held();
    assert!(held_after >= used_after);

    assert_eq!(map.len(), TOTAL - REMOVE);
    // Compacted map used bytes must be at most drained used bytes
    assert!(
        used_after <= used_before,
        "used after {used_after} should be <= used before {used_before}"
    );

    // G-held §12.5: compute held_fresh from a fresh build with the same surviving entries
    let fresh = SyncExpanseMap::new();
    for i in REMOVE..TOTAL {
        fresh.insert(i * 13, !i);
    }
    let held_fresh = fresh.with_locked(|inner| inner.mem_held());
    let held_compacted = map.with_locked(|inner| inner.mem_held());
    let ratio = held_compacted as f64 / held_fresh as f64;
    println!(
        "SyncExpanseMap G-held: held_compacted={held_compacted}, held_fresh={held_fresh}, ratio={ratio:.4}"
    );
    assert!(ratio <= 1.10, "G-held ratio {ratio} exceeds 1.10 ceiling");

    // G-peak §12.5 (METHODOLOGY.md:470): peak = held_before + held_after <= held_before + held_fresh * 1.10
    let peak_held = held_before + held_compacted;
    let peak_ceiling = held_before as f64 + (held_fresh as f64 * 1.10);
    let (live_allocs, total_allocs) =
        map.with_locked(|inner| (inner.live_allocs(), inner.total_node_allocs()));
    eprintln!(
        "[G-PEAK MAP] drained_stride: held_before={held_before}, held_after={held_compacted}, held_fresh={held_fresh}, peak_held={peak_held}, peak_ceiling={peak_ceiling:.1}, live={live_allocs}, total={total_allocs}"
    );
    assert!(
        peak_held as f64 <= peak_ceiling + 1.0,
        "G-peak violated for sync map: peak={peak_held}, ceiling={peak_ceiling}"
    );
    assert_eq!(
        live_allocs, total_allocs,
        "G-peak census identity violated for sync map: live must equal total"
    );

    // Verify all remaining entries survived intact
    for i in REMOVE..TOTAL {
        assert_eq!(map.get(i * 13), Some(!i));
    }
    map.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_set_compact_reclaims_memory() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let set = SyncExpanseSet::new();
    const TOTAL: u64 = 20_000;
    const REMOVE: u64 = 16_000;

    for i in 0..TOTAL {
        set.insert(i * 13);
    }
    assert_eq!(set.len(), TOTAL);

    for i in 0..REMOVE {
        assert!(set.remove(i * 13));
    }
    assert_eq!(set.len(), TOTAL - REMOVE);

    let used_before = set.mem_used();
    let held_before = set.mem_held();

    set.compact();

    let used_after = set.mem_used();
    assert_eq!(set.len(), TOTAL - REMOVE);
    assert!(
        used_after <= used_before,
        "used after {used_after} should be <= used before {used_before}"
    );

    // G-held §12.5: compute held_fresh from a fresh build with the same surviving keys
    let fresh = SyncExpanseSet::new();
    for i in REMOVE..TOTAL {
        fresh.insert(i * 13);
    }
    let held_fresh = fresh.with_locked(|inner| inner.mem_held());
    let held_compacted = set.with_locked(|inner| inner.mem_held());
    let ratio = held_compacted as f64 / held_fresh as f64;
    println!(
        "SyncExpanseSet G-held: held_compacted={held_compacted}, held_fresh={held_fresh}, ratio={ratio:.4}"
    );
    assert!(ratio <= 1.10, "G-held ratio {ratio} exceeds 1.10 ceiling");

    // G-peak §12.5 (METHODOLOGY.md:470): peak = held_before + held_after <= held_before + held_fresh * 1.10
    let peak_held = held_before + held_compacted;
    let peak_ceiling = held_before as f64 + (held_fresh as f64 * 1.10);
    let (live_allocs, total_allocs) =
        set.with_locked(|inner| (inner.live_allocs(), inner.total_node_allocs()));
    eprintln!(
        "[G-PEAK SET] drained_stride: held_before={held_before}, held_after={held_compacted}, held_fresh={held_fresh}, peak_held={peak_held}, peak_ceiling={peak_ceiling:.1}, live={live_allocs}, total={total_allocs}"
    );
    assert!(
        peak_held as f64 <= peak_ceiling + 1.0,
        "G-peak violated for sync set: peak={peak_held}, ceiling={peak_ceiling}"
    );
    assert_eq!(
        live_allocs, total_allocs,
        "G-peak census identity violated for sync set: live must equal total"
    );

    for i in REMOVE..TOTAL {
        assert!(set.contains(i * 13));
    }
    set.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_map_concurrent_readers_during_compact() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let map = Arc::new(SyncExpanseMap::new());
    const COUNT: u64 = 5_000;
    for i in 0..COUNT {
        map.insert(i, !i);
    }

    let done = Arc::new(AtomicBool::new(false));
    let started = Arc::new(AtomicUsize::new(0));
    let mut reader_handles = Vec::new();

    // Spawn 4 reader threads
    for t in 0..4 {
        let m = Arc::clone(&map);
        let d = Arc::clone(&done);
        let st = Arc::clone(&started);
        reader_handles.push(thread::spawn(move || {
            let mut reads = 0usize;
            st.fetch_add(1, Ordering::SeqCst);
            loop {
                let k = (reads as u64 + t) % COUNT;
                if let Some(v) = m.get(k) {
                    assert_eq!(v, !k);
                }
                reads += 1;
                if d.load(Ordering::Relaxed) {
                    break;
                }
            }
            reads
        }));
    }

    // Wait until all readers have started reading
    while started.load(Ordering::SeqCst) < 4 {
        thread::yield_now();
    }

    // Call compact repeatedly
    for _ in 0..5 {
        map.compact();
        thread::yield_now();
    }

    done.store(true, Ordering::Relaxed);

    for handle in reader_handles {
        let reads = handle.join().expect("reader thread should not panic");
        assert!(reads > 0);
    }

    assert_eq!(map.len(), COUNT);
    for i in 0..COUNT {
        assert_eq!(map.get(i), Some(!i));
    }
    map.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_set_concurrent_readers_during_compact() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let set = Arc::new(SyncExpanseSet::new());
    const COUNT: u64 = 5_000;
    for i in 0..COUNT {
        set.insert(i);
    }

    let done = Arc::new(AtomicBool::new(false));
    let started = Arc::new(AtomicUsize::new(0));
    let mut reader_handles = Vec::new();

    for t in 0..4 {
        let s = Arc::clone(&set);
        let d = Arc::clone(&done);
        let st = Arc::clone(&started);
        reader_handles.push(thread::spawn(move || {
            let mut reads = 0usize;
            st.fetch_add(1, Ordering::SeqCst);
            loop {
                let k = (reads as u64 + t) % COUNT;
                assert!(s.contains(k));
                reads += 1;
                if d.load(Ordering::Relaxed) {
                    break;
                }
            }
            reads
        }));
    }

    // Wait until all readers have started reading
    while started.load(Ordering::SeqCst) < 4 {
        thread::yield_now();
    }

    for _ in 0..5 {
        set.compact();
        thread::yield_now();
    }

    done.store(true, Ordering::Relaxed);

    for handle in reader_handles {
        let reads = handle.join().expect("reader thread should not panic");
        assert!(reads > 0);
    }

    assert_eq!(set.len(), COUNT);
    for i in 0..COUNT {
        assert!(set.contains(i));
    }
    set.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_map_concurrent_writers_during_compact() {
    let _lock = SUITE_LOCK.lock().unwrap();
    // Model test falsifying point 3: writers executing during compact must be serialized,
    // and no writes must be dropped.
    let map = Arc::new(SyncExpanseMap::new());
    let model = Arc::new(Mutex::new(BTreeMap::<u64, u64>::new()));

    // Prefill 1,000 keys
    for i in 0..1_000 {
        map.insert(i, !i);
        model.lock().unwrap().insert(i, !i);
    }

    let done = Arc::new(AtomicBool::new(false));
    let mut writer_handles = Vec::new();

    // Spawn 2 concurrent writer threads
    for t in 0..2 {
        let m = Arc::clone(&map);
        let mdl = Arc::clone(&model);
        let d = Arc::clone(&done);
        writer_handles.push(thread::spawn(move || {
            let mut step = 0u64;
            while !d.load(Ordering::Relaxed) {
                let k = 10_000 + t * 10_000 + (step % 500);
                if step.is_multiple_of(2) {
                    let old_m = m.insert(k, step);
                    let mut lock = mdl.lock().unwrap();
                    let old_mdl = lock.insert(k, step);
                    assert_eq!(old_m, old_mdl);
                } else {
                    let old_m = m.remove(k);
                    let mut lock = mdl.lock().unwrap();
                    let old_mdl = lock.remove(&k);
                    assert_eq!(old_m, old_mdl);
                }
                step += 1;
            }
            step
        }));
    }

    // Interleave compact calls from the main thread
    for _ in 0..10 {
        map.compact();
        thread::yield_now();
    }

    done.store(true, Ordering::Relaxed);

    for handle in writer_handles {
        let ops = handle.join().expect("writer thread should not panic");
        assert!(ops > 0);
    }

    // Verify map against model
    let expected = model.lock().unwrap().clone();
    assert_eq!(map.len(), expected.len() as u64);
    for (&k, &v) in &expected {
        assert_eq!(map.get(k), Some(v), "key {k} mismatch");
    }
    map.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_map_writers_running_during_old_tree_drop() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let map = Arc::new(SyncExpanseMap::new());
    const PREFILL: u64 = 15_000;
    for i in 0..PREFILL {
        map.insert(i * 17, !i);
    }

    let started = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let mut writer_handles = Vec::new();

    for t in 0..4 {
        let m = Arc::clone(&map);
        let st = Arc::clone(&started);
        let dn = Arc::clone(&done);
        writer_handles.push(thread::spawn(move || {
            while !st.load(Ordering::Acquire) {
                thread::yield_now();
            }
            let mut writes = 0usize;
            while !dn.load(Ordering::Relaxed) {
                let k = 1_000_000 + t * 10_000 + (writes as u64 % 500);
                if writes.is_multiple_of(2) {
                    m.insert(k, writes as u64);
                } else {
                    m.remove(k);
                }
                writes += 1;
            }
            writes
        }));
    }

    started.store(true, Ordering::Release);
    for _ in 0..5 {
        map.compact();
        thread::yield_now();
    }
    done.store(true, Ordering::Release);

    for h in writer_handles {
        let w = h.join().expect("writer thread failed");
        assert!(w > 0);
    }
    map.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_set_writers_running_during_old_tree_drop() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let set = Arc::new(SyncExpanseSet::new());
    const PREFILL: u64 = 15_000;
    for i in 0..PREFILL {
        set.insert(i * 17);
    }

    let started = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let mut writer_handles = Vec::new();

    for t in 0..4 {
        let s = Arc::clone(&set);
        let st = Arc::clone(&started);
        let dn = Arc::clone(&done);
        writer_handles.push(thread::spawn(move || {
            while !st.load(Ordering::Acquire) {
                thread::yield_now();
            }
            let mut writes = 0usize;
            while !dn.load(Ordering::Relaxed) {
                let k = 1_000_000 + t * 10_000 + (writes as u64 % 500);
                if writes.is_multiple_of(2) {
                    s.insert(k);
                } else {
                    s.remove(k);
                }
                writes += 1;
            }
            writes
        }));
    }

    started.store(true, Ordering::Release);
    for _ in 0..5 {
        set.compact();
        thread::yield_now();
    }
    done.store(true, Ordering::Release);

    for h in writer_handles {
        let w = h.join().expect("writer thread failed");
        assert!(w > 0);
    }
    set.with_locked(|inner| inner.validate());
}

// ---------------------------------------------------------------------------
// G-Peak Retention Tests Across Shapes (METHODOLOGY.md §12.5 / B3 check)
// ---------------------------------------------------------------------------

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

fn check_sync_set_g_peak(name: &str, keys: &[u64], keep_indices: &[usize]) {
    let set = SyncExpanseSet::new();
    for &k in keys {
        set.insert(k);
    }

    let to_keep: std::collections::BTreeSet<u64> = keep_indices.iter().map(|&i| keys[i]).collect();
    for &k in keys {
        if !to_keep.contains(&k) {
            set.remove(k);
        }
    }

    let held_before = set.mem_held();

    let fresh = SyncExpanseSet::new();
    for &k in &to_keep {
        fresh.insert(k);
    }
    let held_fresh = fresh.with_locked(|inner| inner.mem_held());

    set.compact();

    let held_after = set.with_locked(|inner| inner.mem_held());
    let peak_held = held_before + held_after;
    let peak_ceiling = held_before as f64 + (held_fresh as f64 * 1.10);
    let (live_allocs, total_allocs) =
        set.with_locked(|inner| (inner.live_allocs(), inner.total_node_allocs()));

    eprintln!(
        "[G-PEAK SET] {name}: held_before={held_before}, held_after={held_after}, held_fresh={held_fresh}, peak_held={peak_held}, peak_ceiling={peak_ceiling:.1}, live={live_allocs}, total={total_allocs}"
    );
    assert!(
        peak_held as f64 <= peak_ceiling + 1.0,
        "{name}: G-peak violated for sync set: peak={peak_held}, ceiling={peak_ceiling}"
    );
    assert_eq!(
        live_allocs, total_allocs,
        "{name}: G-peak census identity violated for sync set: live must equal total"
    );
    set.with_locked(|inner| inner.validate());
}

fn check_sync_map_g_peak(name: &str, keys: &[u64], keep_indices: &[usize]) {
    let map = SyncExpanseMap::new();
    for &k in keys {
        map.insert(k, !k);
    }

    let to_keep: std::collections::BTreeMap<u64, u64> =
        keep_indices.iter().map(|&i| (keys[i], !keys[i])).collect();
    for &k in keys {
        if !to_keep.contains_key(&k) {
            map.remove(k);
        }
    }

    let held_before = map.mem_held();

    let fresh = SyncExpanseMap::new();
    for (&k, &v) in &to_keep {
        fresh.insert(k, v);
    }
    let held_fresh = fresh.with_locked(|inner| inner.mem_held());

    map.compact();

    let held_after = map.with_locked(|inner| inner.mem_held());
    let peak_held = held_before + held_after;
    let peak_ceiling = held_before as f64 + (held_fresh as f64 * 1.10);
    let (live_allocs, total_allocs) =
        map.with_locked(|inner| (inner.live_allocs(), inner.total_node_allocs()));

    eprintln!(
        "[G-PEAK MAP] {name}: held_before={held_before}, held_after={held_after}, held_fresh={held_fresh}, peak_held={peak_held}, peak_ceiling={peak_ceiling:.1}, live={live_allocs}, total={total_allocs}"
    );
    assert!(
        peak_held as f64 <= peak_ceiling + 1.0,
        "{name}: G-peak violated for sync map: peak={peak_held}, ceiling={peak_ceiling}"
    );
    assert_eq!(
        live_allocs, total_allocs,
        "{name}: G-peak census identity violated for sync map: live must equal total"
    );
    map.with_locked(|inner| inner.validate());
}

#[test]
fn test_sync_g_peak_retention_uniform_random() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mask = (1u64 << 28) - 1;
    let mut seen = std::collections::HashSet::with_capacity(20_000);
    let mut keys = Vec::with_capacity(20_000);
    while keys.len() < 20_000 {
        let k = rng.next() & mask;
        if seen.insert(k) {
            keys.push(k);
        }
    }
    let keep_indices: Vec<usize> = (0..keys.len()).filter(|i| i % 4 == 0).collect();
    check_sync_set_g_peak("uniform_28bit", &keys, &keep_indices);
    check_sync_map_g_peak("uniform_28bit", &keys, &keep_indices);
}

#[test]
fn test_sync_g_peak_retention_clustered() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let mut keys = Vec::with_capacity(32 * 256);
    for c in 0..32u64 {
        let prefix = (c << 16) | 0xAA00;
        for i in 0..256u64 {
            keys.push(prefix | i);
        }
    }
    let keep_indices: Vec<usize> = (0..keys.len()).filter(|i| i % 256 == 0).collect();
    check_sync_set_g_peak("clustered", &keys, &keep_indices);
    check_sync_map_g_peak("clustered", &keys, &keep_indices);
}

#[test]
fn test_sync_g_peak_retention_sequential() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let keys: Vec<u64> = (0..10_000u64).collect();
    let keep_indices: Vec<usize> = (0..keys.len()).filter(|i| i % 20 == 0).collect();
    check_sync_set_g_peak("sequential", &keys, &keep_indices);
    check_sync_map_g_peak("sequential", &keys, &keep_indices);
}

#[test]
fn test_sync_g_peak_retention_sparse() {
    let _lock = SUITE_LOCK.lock().unwrap();
    let keys: Vec<u64> = (0..2_000u64).map(|i| (i << 24) | (i & 0xFF)).collect();
    let keep_indices: Vec<usize> = (0..keys.len()).filter(|i| i % 10 == 0).collect();
    check_sync_set_g_peak("sparse", &keys, &keep_indices);
    check_sync_map_g_peak("sparse", &keys, &keep_indices);
}
