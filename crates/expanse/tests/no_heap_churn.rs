//! Allocation-count regression guards (issue #1 item 2).
//!
//! Deterministic and load-immune: a counting global allocator records
//! every `alloc`/`dealloc` the process makes, so "this path does not
//! touch the heap" becomes a checkable property rather than a claim to
//! be re-litigated with a profiler. Its own test binary, since a global
//! allocator is process-wide.
//!
//! The engine deliberately *does* allocate for node and leaf storage —
//! that is what `NodeAlloc` accounts for. What these tests pin is the
//! absence of **incidental** allocation: scratch buffers in the mutation
//! path, which used to cost a malloc/free per insert into an immediate
//! edge (the most common terminal form for sparse and random keys).

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

// Per-thread, not process-wide: the test harness runs tests on parallel
// threads, and a shared counter makes each test measure its neighbours'
// allocations (which is exactly how the first draft of this file
// produced four-digit phantom counts). Const-initialized so touching the
// TLS slot cannot itself allocate, and `try_with` so an allocation
// during thread teardown is ignored rather than panicking.
thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

fn allocs_now() -> usize {
    ALLOCS.try_with(Cell::get).unwrap_or(0)
}

struct Counting;

// SAFETY: every method forwards to the system allocator unchanged; the
// counter is a thread-local cell and affects no allocation behaviour.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        bump();
        // SAFETY: forwarded contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded contract.
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        bump();
        // SAFETY: forwarded contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Allocations made **on this thread** during `f`.
fn allocations_during(f: impl FnOnce()) -> usize {
    let before = allocs_now();
    f();
    allocs_now() - before
}

/// Allocations during `f` that were **not** the engine taking node or
/// leaf storage — i.e. incidental scratch. Node storage is the
/// structure doing its job and is accounted separately by `NodeAlloc`;
/// scratch is pure overhead.
macro_rules! scratch_allocations_during {
    ($container:expr, $body:block) => {{
        let nodes_before = $container.total_node_allocs();
        let before = allocs_now();
        $body;
        let total = allocs_now() - before;
        let nodes = $container.total_node_allocs() - nodes_before;
        assert!(
            total >= nodes,
            "node allocations ({nodes}) exceed the process count ({total})"
        );
        total - nodes
    }};
}

/// Repeated inserts into an already-populated immediate edge must not
/// allocate at all: the key set is read into a stack buffer, updated and
/// written back in place.
#[test]
fn immediate_updates_do_not_allocate() {
    let mut map = ExpanseMap::new();
    // Two keys under one deep expanse: an immediate edge that stays an
    // immediate across every operation below.
    let base = 0x1122_3344_5566_0000u64;
    map.insert(base, 1);
    map.insert(base + 1, 2);

    // Overwrites: pure value writes, no structural change at all — so
    // not even node storage may move here.
    let n = allocations_during(|| {
        for i in 0..1000u64 {
            map.insert(base, i);
            map.insert(base + 1, i);
        }
    });
    assert_eq!(n, 0, "value overwrites allocated {n} times");

    // Insert-then-remove at the same immediate, repeatedly: the form is
    // rewritten in place each way. Node storage may still be released
    // and re-taken by NodeAlloc, but nothing else may allocate — this is
    // the loop that previously paid a malloc + free per iteration for a
    // scratch `Vec`.
    let before_nodes = map.mem_used();
    let n = scratch_allocations_during!(map, {
        for _ in 0..1000 {
            map.insert(base + 2, 7);
            map.remove(base + 2);
        }
    });
    assert_eq!(map.mem_used(), before_nodes, "node accounting drifted");
    assert_eq!(
        n, 0,
        "immediate insert/remove churn used {n} scratch allocations"
    );
}

/// Set flavor: same property.
#[test]
fn set_immediate_updates_do_not_allocate() {
    let mut set = ExpanseSet::new();
    let base = 0xAABB_CCDD_EE00_0000u64;
    for i in 0..3u64 {
        set.insert(base + i);
    }
    let n = scratch_allocations_during!(set, {
        for _ in 0..1000 {
            // Already present: pure lookups through the insert path.
            set.insert(base);
            set.insert(base + 1);
            // Churn one key in and out of the same immediate.
            set.insert(base + 3);
            set.remove(base + 3);
        }
    });
    assert_eq!(n, 0, "set immediate churn used {n} scratch allocations");
}

/// Lookups must never allocate, on any distribution or form.
#[test]
fn lookups_do_not_allocate() {
    let mut map = ExpanseMap::new();
    let mut rng = 0x5EEDu64;
    let mut keys = Vec::new();
    for _ in 0..20_000 {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        keys.push(rng);
        map.insert(rng, !rng);
    }
    // Also cover dense and clustered regions (bitmap leaves, full
    // expanses, narrow pointers).
    for i in 0..512u64 {
        map.insert(i, i);
        map.insert(0x7777_0000_0000 + i, i);
        keys.push(i);
        keys.push(0x7777_0000_0000 + i);
    }

    let n = allocations_during(|| {
        let mut sink = 0u64;
        for &k in &keys {
            sink ^= map.get(k).unwrap_or(0);
            sink ^= u64::from(map.contains_key(k ^ 1));
        }
        std::hint::black_box(sink);
    });
    assert_eq!(n, 0, "lookups allocated {n} times");
}

/// Ordered navigation and rank/select must not allocate either — they
/// back the compat layer's `First`/`Next`/`Count`/`ByCount`, which C
/// callers invoke in tight loops.
#[test]
fn navigation_does_not_allocate() {
    let mut set = ExpanseSet::new();
    for i in 0..5_000u64 {
        set.insert(i * 37);
    }
    let first = allocations_during(|| {
        std::hint::black_box(set.first());
    });
    let step = allocations_during(|| {
        std::hint::black_box(set.next_after(37));
    });
    let rank = allocations_during(|| {
        std::hint::black_box(set.count_range(0..=u64::MAX));
    });
    let select = allocations_during(|| {
        std::hint::black_box(set.by_count(2_500));
    });
    let iter = allocations_during(|| {
        std::hint::black_box(set.iter().count());
    });
    let sweep = allocations_during(|| {
        let mut k = set.first();
        let mut count = 0u64;
        while let Some(cur) = k {
            count += 1;
            k = set.next_after(cur);
        }
        assert_eq!(count, 5_000);
    });
    println!("first={first} step={step} rank={rank} select={select} iter={iter} sweep={sweep}");
    assert_eq!(
        (first, step, rank, select, iter, sweep),
        (0, 0, 0, 0, 0, 0),
        "navigation allocated"
    );
}
