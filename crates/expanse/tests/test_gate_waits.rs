//! Integration tests for gate blocked entries, wait cycles, and branch_split routing (Refs #568).
//!
//! `occ_stats` counters are process-global, so this file holds a focused set of
//! tests running under its own test binary.

#![cfg(not(miri))]
#![cfg(all(feature = "occ-stats", feature = "std", target_pointer_width = "64"))]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet};

#[test]
fn structural_no_raw_fallback_branch_split_in_sync_rs() {
    let sync_src = include_str!("../src/sync.rs");
    let lines: Vec<&str> = sync_src.lines().collect();
    let mut raw_sites = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Ignore doc-comments and code comments that merely mention the construct in prose.
        if trimmed.starts_with("//") {
            continue;
        }
        if line.contains("Fallback(FallbackCause::BranchSplit)") {
            // Only the helper fn branch_split is allowed to return Fallback(FallbackCause::BranchSplit).
            // Any other call site must route through branch_split(kind).
            let in_helper = lines[..idx]
                .iter()
                .rev()
                .take(15)
                .any(|l| l.contains("fn branch_split"));
            if !in_helper {
                raw_sites.push((idx + 1, trimmed.to_string()));
            }
        }
    }

    assert!(
        raw_sites.is_empty(),
        "Found raw Fallback(FallbackCause::BranchSplit) outside fn branch_split helper: {:?}",
        raw_sites
    );
}

#[test]
fn gate_blocked_entries_and_wait_cycles_measured() {
    // 1. SyncExpanseMap test
    let map = Arc::new(SyncExpanseMap::new());
    // Prefill past root leaf capacity so root is a tree and mutations enter Stage B (OLC)
    for i in 0..1_000u64 {
        map.insert(i * 1000 + 7, i);
    }
    assert!(map.len() >= 1_000);

    // Close gate before spawned thread attempts insert
    map.__test_close_gate();

    let before = occ_stats::snapshot();
    let m = Arc::clone(&map);
    let entered = Arc::new(AtomicBool::new(false));
    let entered_clone = Arc::clone(&entered);

    let handle = thread::spawn(move || {
        entered_clone.store(true, Ordering::Release);
        m.insert(999_999_999, 42);
    });

    // Spin until spawned thread has entered and reached the closed gate
    while !entered.load(Ordering::Acquire) {
        thread::yield_now();
    }
    thread::sleep(Duration::from_millis(15));

    // Reopen gate to unblock writer
    map.__test_reopen_gate();
    handle.join().expect("writer thread panicked");

    let after = occ_stats::snapshot();
    let d = |s: Stat| after[s as usize] - before[s as usize];

    assert!(
        d(Stat::GateBlockedEntries) >= 1,
        "map: GateBlockedEntries must be >= 1, got {}",
        d(Stat::GateBlockedEntries)
    );
    assert!(
        d(Stat::GateWaitCycles) > 0,
        "map: GateWaitCycles must be > 0, got {}",
        d(Stat::GateWaitCycles)
    );
    assert_eq!(map.get(999_999_999), Some(42));

    // 2. SyncExpanseSet test
    let set = Arc::new(SyncExpanseSet::new());
    // Prefill past root leaf capacity
    for i in 0..1_000u64 {
        set.insert(i * 1000 + 7);
    }
    assert!(set.len() >= 1_000);

    // Close gate before spawned thread attempts insert
    set.__test_close_gate();

    let s_before = occ_stats::snapshot();
    let s = Arc::clone(&set);
    let s_entered = Arc::new(AtomicBool::new(false));
    let s_entered_clone = Arc::clone(&s_entered);

    let s_handle = thread::spawn(move || {
        s_entered_clone.store(true, Ordering::Release);
        s.insert(999_999_999);
    });

    while !s_entered.load(Ordering::Acquire) {
        thread::yield_now();
    }
    thread::sleep(Duration::from_millis(15));

    set.__test_reopen_gate();
    s_handle.join().expect("writer thread panicked");

    let s_after = occ_stats::snapshot();
    let sd = |st: Stat| s_after[st as usize] - s_before[st as usize];

    assert!(
        sd(Stat::GateBlockedEntries) >= 1,
        "set: GateBlockedEntries must be >= 1, got {}",
        sd(Stat::GateBlockedEntries)
    );
    assert!(
        sd(Stat::GateWaitCycles) > 0,
        "set: GateWaitCycles must be > 0, got {}",
        sd(Stat::GateWaitCycles)
    );
    assert!(set.contains(999_999_999));
}
