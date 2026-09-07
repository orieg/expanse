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
        if total >= nodes { total - nodes } else { 0 }
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

/// Small arrays — the ones a C caller keeps under the root-leaf cap —
/// must not allocate on every insert. Before capacity classes reached the
/// root leaf, each insert here cost a malloc, a full copy and a free.
#[test]
fn small_arrays_do_not_allocate_per_insert() {
    let mut map = ExpanseMap::new();
    let n = allocations_during(|| {
        for k in 0..30u64 {
            map.insert(k * 7, k);
        }
    });
    // Class-sized growth: 1, 2, 4, 8, 12, 16, 20, 24, 28, 32 slots — an
    // allocation only when the class changes, not once per insert.
    assert!(n <= 12, "30 inserts into a small map allocated {n} times");
    assert_eq!(map.len(), 30);
    for k in 0..30u64 {
        assert_eq!(map.get(k * 7), Some(k));
    }

    let mut set = ExpanseSet::new();
    let n = allocations_during(|| {
        for k in 0..30u64 {
            set.insert(k * 7);
        }
    });
    assert!(n <= 12, "30 inserts into a small set allocated {n} times");
    assert_eq!(set.len(), 30);
}

/// The linear-leaf → bitmap-leaf conversion (level-1 overflow, the
/// dense insert path) is bounded scratch: the value regrouping used to
/// bucket through a `[Vec<u64>; 8]` — eight scratch allocations per
/// conversion — for values the sort order had already grouped. What
/// remains is the single materialization buffer per conversion
/// (`read_map_leaf`, sized with headroom so it no longer pays a growth
/// reallocation; replacing it with a stack buffer was measured TWICE
/// and regressed both times — see build_bitmap_leaf_map's scope note).
#[test]
fn bitmap_leaf_conversion_scratch_is_bounded() {
    let mut map = ExpanseMap::new();
    // One 256-key expanse, filled past LEAF1_CAP (25) so the linear
    // level-1 leaf converts to a bitmap leaf, then grown further so the
    // bitmap leaf's value subarrays keep extending.
    let base = 0x0102_0304_0500u64;
    let n = scratch_allocations_during!(map, {
        for i in 0..200u64 {
            map.insert(base + i, i);
        }
    });
    // 2 = one root-conversion buffer + one leaf materialization at the
    // bitmap conversion. Was 11+ before: 8 subexpanse buckets + growth.
    assert!(n <= 2, "dense build used {n} scratch allocations (bound 2)");
    for i in 0..200u64 {
        assert_eq!(map.get(base + i), Some(i));
    }
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

/// 32-bit sequential ingest: allocations per insert.
///
/// Before #615 an insert into an already-populated bitmap subexpanse
/// reallocated and copied the whole subarray — one malloc plus two
/// memcpys plus one retirement **per key**. Applying `trie32::cap_class`
/// to subarray growth, as the 64-bit engine already did for its leaves,
/// makes the growths that stay inside a capacity class shift in place.
///
/// This pins the resulting rate so it cannot silently regress. The floor
/// is the number of *structural* allocations the trie genuinely needs
/// (nodes, class crossings); the ceiling is set just above the measured
/// rate. It is a ratio, not a wall-clock figure, so it is deterministic
/// and load-immune (AGENTS.md §8.4: hard assertions belong on
/// deterministic counters only).
#[test]
fn map32_sequential_ingest_allocation_rate() {
    use expanse_trie::map32::ExpanseMap32;

    const N: u32 = 2_000;
    let mut map = ExpanseMap32::new();
    // Warm the container itself out of the measured window.
    map.insert(1, 1);
    map.remove(1);

    let n = allocations_during(|| {
        for i in 0..N {
            map.insert(1_700_000_000 + i, i);
        }
    });
    assert_eq!(map.len(), N as usize);

    let per_insert = n as f64 / f64::from(N);
    println!("map32 sequential ingest: {n} allocations over {N} inserts ({per_insert:.3}/insert)");
    assert!(
        per_insert < 0.45,
        "32-bit sequential ingest cost {per_insert:.3} allocations/insert \
         (0.875 before #615, 0.364 after); bitmap-subarray growth is \
         reallocating per key again"
    );
}

/// Set flavour of the same ingest: `BranchB32` child subarrays are the
/// only cap-classed store a set exercises (a set bitmap leaf is a bare
/// 256-bit mask with no subarray at all).
#[test]
fn set32_sequential_ingest_allocation_rate() {
    use expanse_trie::set32::ExpanseSet32;

    const N: u32 = 10_000;
    let mut set = ExpanseSet32::new();
    set.insert(1);
    set.remove(1);

    let n = allocations_during(|| {
        for i in 0..N {
            set.insert(1_700_000_000 + i);
        }
    });
    assert_eq!(set.len(), N as usize);

    let per_insert = n as f64 / f64::from(N);
    println!("set32 sequential ingest: {n} allocations over {N} inserts ({per_insert:.4}/insert)");
    assert!(
        per_insert < 0.09,
        "32-bit set ingest cost {per_insert:.4} allocations/insert \
         (0.0782 before #615, 0.0763 after)"
    );
}

/// A monotonic 32-bit fill must allocate *less* per key than a scattered one.
///
/// [#615](https://github.com/orieg/expanse/issues/615) listed "node promotion
/// churn during a monotonic fill, where a leaf crosses capacity classes
/// repeatedly as the population grows" as a candidate for the 32-bit
/// per-insert constant. It is the opposite way round, and this pins that:
/// ascending keys share prefixes, so they fill few leaves deeply and cross
/// far fewer capacity classes in total than keys scattered over the space.
///
/// Measured at the time of writing over 10,000 keys — allocations per key:
/// monotonic 0.352, clustered 0.575, stride-64 0.580, uniform random 0.983.
/// The assertion is the ordering, not those values, so capacity-class tuning
/// is free to move them; what may not happen silently is a monotonic fill
/// becoming the allocation-heavy shape.
#[test]
fn monotonic_fill_allocates_less_than_scattered() {
    use expanse_trie::map32::ExpanseMap32;
    const N: u32 = 10_000;

    let fill = |keys: Vec<u32>| -> usize {
        let mut map = ExpanseMap32::new();
        let n = allocations_during(|| {
            for &k in &keys {
                map.insert(k, k ^ 0x55);
            }
        });
        assert_eq!(map.len(), keys.len(), "fill lost keys");
        // Dropped, not leaked: `dealloc` does not bump the counter, so the
        // teardown cannot reach the number, and a leaked 10k-key map fails
        // LeakSanitizer in the ASan job (AGENTS.md §5, RAII fixture hygiene).
        n
    };

    let monotonic = fill((0..N).collect());
    let clustered = fill((0..N).map(|i| (i / 8) * 4096 + (i % 8)).collect());
    let random = fill({
        let mut x: u64 = 0x0DDB_1A5E_5EED_0001;
        (0..N)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u32
            })
            .collect()
    });

    assert!(
        monotonic < clustered && clustered < random,
        "allocation count should rise with key scatter, got monotonic {monotonic}, \
         clustered {clustered}, random {random}"
    );
    assert!(
        monotonic * 2 < random,
        "a monotonic fill should allocate far less than a scattered one, got \
         {monotonic} vs {random} over {N} keys"
    );
}

/// A string key that does not resolve inside its terminal chunk costs
/// **one** allocation for its leaf, not two (#723).
///
/// `ExpanseStrMap`'s leaf used to be `{ suffix: Box<[u8]>, value: u64 }`: a
/// shell allocation plus a separate byte buffer, for every key whose
/// remainder does not fit in a terminal chunk. That is the whole of the
/// memory finding — 69.17 B/key for a 12-byte key against Masstree's 33.91
/// on the allocator instrument — and it is also a second dependent load on
/// the lookup path, since reaching the bytes to compare meant chasing the
/// shell's fat pointer into another allocation.
///
/// **The count is the gate, not the timing.** A leaf that goes back to two
/// allocations fails here on any machine, under any load, at no cost —
/// where a wall-clock regression needs the reference host and an interval
/// (AGENTS.md §8.4).
///
/// Measured as a difference between two arms that share their first eight
/// key bytes and differ only in whether a leaf is produced at all, so node
/// storage — the structure doing its job — cancels instead of being
/// estimated. The difference is exactly one allocation per leaf, at every
/// population; before #723 it was exactly two.
#[test]
fn strmap_suffix_leaf_costs_one_allocation() {
    use expanse_trie::strmap::ExpanseStrMap;

    // Four base-32 digits offset into printable ASCII: injective over this
    // range and NUL-free. (`i.to_be_bytes().map(|b| b | 0x40)` is neither —
    // it collapses 0 and 64 onto the same key, which is how the first draft
    // of this test silently measured 543 keys instead of 544.)
    let digits = |i: u32| {
        let mut k = Vec::with_capacity(12);
        for d in 0..4 {
            k.push(0x40u8 + ((i >> (5 * d)) & 31) as u8);
        }
        k
    };
    // 12 bytes: the first chunk is 8 non-NUL bytes, so the key does not
    // terminate there and its 4-byte remainder becomes a suffix leaf. This
    // is the `short` shape the comparison suites measure.
    let with_leaf = |i: u32| {
        let mut k = digits(i);
        k.extend_from_slice(b"abcd");
        k.extend_from_slice(b"tail");
        k
    };
    // 7 bytes: the same leading digits, but the chunk now contains the
    // terminating NUL, so the value lands in the node itself and no leaf is
    // allocated. Same key count, same node population, no leaves.
    let no_leaf = |i: u32| {
        let mut k = digits(i);
        k.extend_from_slice(b"abc");
        k
    };

    // More than one population: one allocation per leaf is a slope, and a
    // slope needs more than one point. Node storage cancels in the difference
    // at each of them. `no_heap_churn` is a nightly Miri shard
    // (`.github/workflows/nightly.yml`), and the interpreter pays for every
    // one of these inserts, so it takes the small pair — the invariant holds
    // at any population (docs/CI.md §5, as `test_mem_used_order_invariant`
    // does for the same reason).
    let populations: &[u32] = if cfg!(miri) {
        &[64, 256]
    } else {
        &[256, 1024, 4096]
    };
    for &n in populations {
        let leafy: Vec<Vec<u8>> = (0..n).map(with_leaf).collect();
        let flat: Vec<Vec<u8>> = (0..n).map(no_leaf).collect();

        let mut a = ExpanseStrMap::new();
        let with = allocations_during(|| {
            for (i, k) in leafy.iter().enumerate() {
                a.insert(k, i as u64);
            }
        });
        // Read back inside the same map so a shape that quietly stopped
        // producing leaves cannot pass by allocating nothing.
        for (i, k) in leafy.iter().enumerate() {
            assert_eq!(a.get(k), Some(i as u64), "leaf key {i} lost at n={n}");
        }
        // Dropped rather than leaked: a retained map fails LeakSanitizer in
        // the ASan job (AGENTS.md §5).
        drop(a);

        let mut b = ExpanseStrMap::new();
        let without = allocations_during(|| {
            for (i, k) in flat.iter().enumerate() {
                b.insert(k, i as u64);
            }
        });
        assert_eq!(b.len(), u64::from(n), "terminal arm lost keys at n={n}");
        drop(b);

        let per_leaf = with - without;
        assert_eq!(
            per_leaf, n as usize,
            "at n={n}, {n} suffix leaves cost {per_leaf} allocations \
             ({with} with leaves, {without} without) — one each is the \
             post-#723 shape, two each was the `Box<StrSuffix>` shell plus \
             its `Box<[u8]>` byte buffer"
        );
    }
}

/// A `k`-element ordered scan of `ExpanseStrMap` allocates nothing per
/// element (#722).
///
/// The shipped `next_at_or_after` / `next_after` pair is a *positional*
/// surface: each step is a fresh root descent that returns a freshly
/// allocated key, so a scan costs `k` descents and allocates with `k`. Two
/// comparison suites measured the consequence — HOT won 72 of 72 string scan
/// cells and Masstree won every one, down to 0.036× — and both attributed it
/// to the surface rather than to the trie. `cursor` descends once and reuses
/// one key buffer.
///
/// **The invariant is a slope, so it is measured as one.** The cursor's key
/// buffer and path stack do grow — a handful of reallocations while they
/// reach the corpus's longest key and deepest path — so "zero allocations" is
/// false as a total and true as a rate. Scanning ten times as many elements
/// must therefore cost *exactly the same* number of allocations. That is the
/// property, it holds on any machine under any load, and it is what a timing
/// on the reference host cannot establish (AGENTS.md §8.4).
#[test]
fn strmap_cursor_scan_does_not_allocate_per_element() {
    use expanse_trie::strmap::ExpanseStrMap;

    // Distinct first chunks, so most keys land as suffix leaves, plus enough
    // length variation to make the key buffer grow before it settles.
    let key = |i: u32| {
        let mut k = Vec::with_capacity(20);
        for d in 0..4 {
            k.push(0x40u8 + ((i >> (5 * d)) & 31) as u8);
        }
        k.extend_from_slice(b"abcd");
        k.extend_from_slice(&b"tail-padding-of-varying-length"[..(i as usize % 17) + 1]);
        k
    };

    let n: u32 = if cfg!(miri) { 400 } else { 20_000 };
    let mut m = ExpanseStrMap::new();
    for i in 0..n {
        m.insert(&key(i), u64::from(i));
    }

    // Scan `k` elements from the start and report the allocations it cost.
    let scan = |k: usize, m: &mut ExpanseStrMap| {
        let mut seen = 0usize;
        let mut sink = 0u64;
        let n = allocations_during(|| {
            let mut c = m.cursor();
            while let Some((key, slot)) = c.next() {
                // Consume both halves so neither can be optimized away, and
                // touch the key bytes so a cursor that handed out a stale
                // buffer would fail the length check below.
                // SAFETY: the cursor holds the map borrowed for its lifetime,
                // so the slot it just returned is a live value word and no
                // structural mutation can run between here and the read.
                let value = unsafe { *slot.as_ptr() };
                sink ^= u64::from(key[0]) ^ value;
                seen += 1;
                if seen == k {
                    break;
                }
            }
        });
        assert_eq!(seen, k, "scan stopped early");
        std::hint::black_box(sink);
        n
    };

    let small = (n / 10) as usize;
    let large = n as usize;
    let a = scan(small, &mut m);
    let b = scan(large, &mut m);

    assert_eq!(
        a, b,
        "a {large}-element scan cost {b} allocations against {a} for {small}: \
         the cursor is allocating per element, which is the #722 defect"
    );

    // And the positional surface, for contrast: it must scale with k, which
    // is why this test exists. Kept as a live comparison rather than a
    // remembered number so it cannot quietly stop being true.
    let positional = |k: usize, m: &mut ExpanseStrMap| {
        let first = m.first().expect("non-empty").0;
        allocations_during(|| {
            let mut cur = Some(first.clone());
            for _ in 0..k {
                match cur.take() {
                    Some(key) => cur = m.next_after(&key).map(|(k, _)| k),
                    None => break,
                }
            }
        })
    };
    let p_small = positional(small, &mut m);
    let p_large = positional(large, &mut m);
    assert!(
        p_large > p_small * 5,
        "the positional surface was expected to allocate with k ({p_small} for \
         {small}, {p_large} for {large}); if it no longer does, this test is \
         measuring the wrong thing"
    );
}
