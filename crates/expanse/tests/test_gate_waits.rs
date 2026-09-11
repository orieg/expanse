//! Integration tests for gate blocked entries, wait cycles, and branch_split routing (Refs #568).
//!
//! `occ_stats` counters are process-global, so this file holds a focused set of
//! tests running under its own test binary.

#![cfg(not(miri))]
#![cfg(all(feature = "occ-stats", feature = "std", target_pointer_width = "64"))]

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::sync::{self, SyncExpanseMap, SyncExpanseSet};

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
fn contention_stat_mapping_unit_invariants() {
    assert_eq!(
        sync::contention_stat(true),
        Stat::ContentionGateClosed,
        "closed=true must map to Stat::ContentionGateClosed"
    );
    assert_eq!(
        sync::contention_stat(false),
        Stat::ContentionRetryExhausted,
        "closed=false must map to Stat::ContentionRetryExhausted"
    );
}

#[test]
fn branch_split_upgrade_structural_routing_in_sync_rs() {
    let sync_src = include_str!("../src/sync.rs");
    let lines: Vec<&str> = sync_src.lines().collect();

    let mut upgrade_call_sites = 0;
    let mut branchb_up_checks = 0;

    let test_mod_idx = lines
        .iter()
        .position(|l| l.contains("mod tests"))
        .unwrap_or(lines.len());
    for line in &lines[..test_mod_idx] {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.contains("branch_split(BranchSplitKind::Upgrade)") {
            upgrade_call_sites += 1;
        }
        if trimmed.contains("crate::mutate::BRANCHB_UP") {
            branchb_up_checks += 1;
        }
    }

    assert_eq!(
        upgrade_call_sites, 4,
        "Expected exactly 4 call sites for branch_split(BranchSplitKind::Upgrade) (pre-lock + under-lock for set & map), found {}",
        upgrade_call_sites
    );
    assert_eq!(
        branchb_up_checks, 4,
        "Expected exactly 4 BRANCHB_UP checks guarding Upgrade fallback, found {}",
        branchb_up_checks
    );
}

#[test]
fn structural_contention_stat_routing_in_sync_rs() {
    let sync_src = include_str!("../src/sync.rs");
    let lines: Vec<&str> = sync_src.lines().collect();

    let mut direct_uses = Vec::new();
    let mut call_sites = 0;
    let mut gate_closed_checks = 0;
    let mut closed_true_assignments = 0;
    let mut paired_closed_assignments = 0;

    let test_mod_idx = lines
        .iter()
        .position(|l| l.contains("mod tests"))
        .unwrap_or(lines.len());
    for (idx, line) in lines[..test_mod_idx].iter().enumerate() {
        let trimmed = line.trim();
        // Ignore comments
        if trimmed.starts_with("//") {
            continue;
        }
        if (trimmed.contains("Stat::ContentionGateClosed")
            || trimmed.contains("Stat::ContentionRetryExhausted"))
            && !lines[..idx]
                .iter()
                .rev()
                .take(15)
                .any(|l| l.contains("fn contention_stat"))
        {
            direct_uses.push((idx + 1, trimmed.to_string()));
        }
        if trimmed.contains("contention_stat(closed)") {
            call_sites += 1;
        }
        if trimmed.contains("closed = true") {
            closed_true_assignments += 1;
        }
        if trimmed.contains("if self.shared.gate.is_closed() {") {
            gate_closed_checks += 1;
            // Each `if self.shared.gate.is_closed() {` must be followed within ~4 lines by `closed = true;`
            let window_end = (idx + 5).min(test_mod_idx);
            let has_closed_assignment = lines[idx + 1..window_end]
                .iter()
                .any(|l| l.trim().contains("closed = true"));
            if has_closed_assignment {
                paired_closed_assignments += 1;
            }
        }
    }

    assert!(
        direct_uses.is_empty(),
        "Found direct use of ContentionGateClosed / ContentionRetryExhausted outside fn contention_stat: {:?}",
        direct_uses
    );
    assert_eq!(
        call_sites, 4,
        "Expected exactly 4 write loop call sites for contention_stat(closed), found {}",
        call_sites
    );
    assert_eq!(
        gate_closed_checks, 4,
        "Expected exactly 4 'if self.shared.gate.is_closed() {{' checks in write loops, found {}",
        gate_closed_checks
    );
    assert_eq!(
        paired_closed_assignments, 4,
        "Expected exactly 4 'closed = true;' assignments within 4 lines of 'is_closed()', found {}",
        paired_closed_assignments
    );
    assert_eq!(
        closed_true_assignments, 4,
        "Expected 'closed = true' to occur nowhere else (exactly 4 total assignments), found {}",
        closed_true_assignments
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

    let handle = thread::spawn(move || {
        m.insert(999_999_999, 42);
    });

    // Poll until writer thread has reached enter_writer_blocking and incremented GateBlockedEntries.
    // Removes any dependency on thread scheduling timing (AGENTS.md §2.1.5).
    let timeout = Duration::from_secs(5);
    let start = Instant::now();
    while occ_stats::snapshot()[Stat::GateBlockedEntries as usize]
        == before[Stat::GateBlockedEntries as usize]
    {
        if start.elapsed() > timeout {
            map.__test_reopen_gate();
            handle.join().ok();
            panic!(
                "map: timed out after 5s waiting for writer thread to increment GateBlockedEntries in enter_writer_blocking"
            );
        }
        thread::yield_now();
    }

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

    let s_handle = thread::spawn(move || {
        s.insert(999_999_999);
    });

    let s_start = Instant::now();
    while occ_stats::snapshot()[Stat::GateBlockedEntries as usize]
        == s_before[Stat::GateBlockedEntries as usize]
    {
        if s_start.elapsed() > timeout {
            set.__test_reopen_gate();
            s_handle.join().ok();
            panic!(
                "set: timed out after 5s waiting for writer thread to increment GateBlockedEntries in enter_writer_blocking"
            );
        }
        thread::yield_now();
    }

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
