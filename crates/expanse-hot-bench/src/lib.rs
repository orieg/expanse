//! HOT (Height Optimized Trie) comparison arms for Expanse, over a C++ FFI shim.
//!
//! Bench-only and detached from the root workspace; see `Cargo.toml` for why.
//! The suite's pre-registration is `docs/benchmarks/hot_comparison/METHODOLOGY.md`
//! and the tracking issue is [#660](https://github.com/orieg/expanse/issues/660).
//!
//! Two locked constraints from the Step 0 gate are enforced here rather than
//! left to each harness to remember:
//!
//! - **The suite's key domain is `u64`.** HOT tags leaves in bit 0 and recovers
//!   the payload with an arithmetic shift, so its *inline payload* is 63 bits
//!   wide. That binds only where the stored value is the key itself — Arm A's
//!   `IdentityKeyExtractor`. Arm B stores a heap pointer and handles the full
//!   64-bit domain, verified against keys spanning bit 63. The limit is
//!   therefore a **capability predicate on Arm A** ([`hot_can_inline`]),
//!   evaluated against the workload and reported as a finding about HOT — never
//!   a restriction on the workload both systems are measured over.
//! - **One allocator instrument for both arms.** [`Census`] reads counters fed
//!   by link-time interposition on the C allocator family, which captures HOT's
//!   `posix_memalign` node allocations and Rust's allocations alike.

pub mod workload;

use std::os::raw::c_void;

unsafe extern "C" {
    fn exp_census_reset();
    fn exp_census_arm(on: i32);
    fn exp_census_live() -> i64;
    fn exp_census_peak() -> i64;
    fn exp_census_allocs() -> i64;
    fn exp_census_frees() -> i64;

    fn exp_hot_pool_allocations() -> usize;

    fn exp_hot_set_new() -> *mut c_void;
    fn exp_hot_set_delete(t: *mut c_void);
    fn exp_hot_set_insert(t: *mut c_void, k: u64) -> i32;
    fn exp_hot_set_contains(t: *mut c_void, k: u64) -> i32;
    fn exp_hot_set_len(t: *mut c_void) -> usize;
    fn exp_hot_set_iterate_xor(t: *mut c_void) -> u64;
    fn exp_hot_set_scan(t: *mut c_void, lo: u64, k: usize, sink: *mut u64) -> usize;

    fn exp_hot_map_new() -> *mut c_void;
    fn exp_hot_map_delete(t: *mut c_void);
    fn exp_hot_map_insert(t: *mut c_void, k: u64, v: u64) -> i32;
    fn exp_hot_map_get(t: *mut c_void, k: u64, out: *mut u64) -> i32;
    fn exp_hot_map_len(t: *mut c_void) -> usize;
    fn exp_hot_map_iterate_xor(t: *mut c_void) -> u64;
    fn exp_hot_map_scan(t: *mut c_void, lo: u64, k: usize, sink: *mut u64) -> usize;
}

/// Width of HOT's inline value payload, in bits.
///
/// `HOTSingleThreadedChildPointer` tags leaves in bit 0 and recovers the stored
/// value with `mPointer >> 1`, so whatever HOT stores inline is 63 bits wide.
pub const HOT_INLINE_PAYLOAD_BITS: u32 = 63;

/// Whether `key` fits HOT's inline value payload.
///
/// This is a predicate on **Arm A only**, where `IdentityKeyExtractor` makes the
/// stored value the key itself. Arm B stores a heap pointer and is unaffected —
/// measured 6/6 on keys spanning bit 63, including `u64::MAX`.
///
/// It exists to be *evaluated against* a workload and reported, never to edit
/// one. An earlier revision of this crate folded every key into a 63-bit space
/// on both arms and on the Expanse side too; that doubled Expanse's effective
/// load factor (halving the keyspace is arithmetically the same as doubling the
/// population for a structure that partitions by key expanse) and moved its
/// 1M uniform-random memory cell across a density discontinuity. See §9.4 of
/// `docs/benchmarks/hot_comparison/METHODOLOGY.md`.
#[inline]
pub fn hot_can_inline(key: u64) -> bool {
    key >> HOT_INLINE_PAYLOAD_BITS == 0
}

/// What an Arm A insert did.
///
/// `NotRepresentable` is a first-class outcome rather than a hidden
/// precondition: HOT's `insert` returns `true` for keys its `lookup` will never
/// find, so an arm that cannot hold a key must say so at the call, not silently
/// shrink its population.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InlineInsert {
    /// The trie reports the key as newly present.
    Inserted,
    /// The trie reports the key as already present.
    AlreadyPresent,
    /// The key does not fit HOT's inline payload; nothing was stored.
    NotRepresentable,
}

/// Reading of the shared allocator census.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Census {
    /// Bytes currently held from the C allocator.
    pub live: i64,
    /// High-water mark of `live` since the last reset.
    pub peak: i64,
    /// Allocation calls observed while armed.
    pub allocs: i64,
    /// Free calls observed while armed.
    pub frees: i64,
}

impl Census {
    /// Zeroes the counters. Does not change whether the census is armed.
    pub fn reset() {
        // SAFETY: the shim's counters are process-global atomics with no
        // preconditions; the call cannot fail or touch caller memory.
        unsafe { exp_census_reset() }
    }

    /// Arms or disarms accounting. Only allocations made while armed are counted.
    pub fn arm(on: bool) {
        // SAFETY: as `reset` — a store to a process-global atomic.
        unsafe { exp_census_arm(i32::from(on)) }
    }

    /// Samples the counters.
    pub fn read() -> Self {
        // SAFETY: as `reset` — four relaxed loads from process-global atomics.
        unsafe {
            Self {
                live: exp_census_live(),
                peak: exp_census_peak(),
                allocs: exp_census_allocs(),
                frees: exp_census_frees(),
            }
        }
    }

    /// Runs `f` with the census armed and zeroed, returning its result and the
    /// bytes held when it finished.
    pub fn measure<T>(f: impl FnOnce() -> T) -> (T, Self) {
        Self::reset();
        Self::arm(true);
        let out = f();
        Self::arm(false);
        (out, Self::read())
    }
}

/// Cumulative allocations HOT's process-global node pool has made.
///
/// The pool is a function-local `static`, so it outlives every trie instance and
/// a dropped trie leaves reusable nodes on its free lists. A census taken after
/// any earlier HOT trie therefore undercounts — measured at 3.61 B/key warm
/// against 11.76 B/key cold on the identical workload, a 3.3x understatement,
/// while the Expanse arm is unaffected. Use [`require_cold_pool`] before a
/// census rather than relying on call order.
pub fn pool_allocations() -> usize {
    // SAFETY: reads a counter on a process-global pool; no preconditions.
    unsafe { exp_hot_pool_allocations() }
}

/// Fails loudly unless HOT's node pool is untouched in this process.
///
/// A memory census that runs on a warm pool is void (§8 of the methodology).
/// Each census cell must therefore be its own process invocation.
pub fn require_cold_pool(context: &str) {
    let n = pool_allocations();
    assert!(
        n == 0,
        "{context}: HOT's node pool is already warm ({n} prior allocations). \
         The census would undercount by reusing free-list nodes — measured 3.3x on a 100k \
         random build. Run one arm per process invocation."
    );
}

/// Outcome of the control allocation that must precede any published census.
///
/// A census whose control does not move the counter by the requested size is
/// void (§8 of the methodology), so this is checked rather than assumed.
#[derive(Copy, Clone, Debug)]
pub struct ControlResult {
    /// Bytes the counter rose by on a known-size allocation.
    pub alloc_delta: i64,
    /// Bytes still held after freeing it. Must be zero.
    pub residual: i64,
    /// Requested size of the control allocation.
    pub requested: usize,
}

impl ControlResult {
    /// Whether the instrument is trustworthy: the allocation was seen at no less
    /// than its requested size, and freeing it returned the counter to zero.
    pub fn is_valid(&self) -> bool {
        self.alloc_delta >= self.requested as i64 && self.residual == 0
    }
}

/// Validates the census against a known-size allocation.
///
/// This exists because the Step 0 gate program's free path was silently broken:
/// it observed the free happening while the byte total did not move. Every
/// census run calls this first and refuses to publish if it fails.
pub fn validate_census(requested: usize) -> ControlResult {
    Census::reset();
    Census::arm(true);
    let mut v: Vec<u8> = Vec::with_capacity(requested);
    v.push(0);
    std::hint::black_box(&v);
    let alloc_delta = Census::read().live;
    drop(v);
    let residual = Census::read().live;
    Census::arm(false);
    ControlResult {
        alloc_delta,
        residual,
        requested,
    }
}

macro_rules! trie_wrapper {
    ($name:ident, $new:ident, $del:ident, $len:ident, $iter:ident, $scan:ident) => {
        impl $name {
            /// Builds an empty trie.
            pub fn new() -> Self {
                // SAFETY: the shim allocates and returns an owned trie; the
                // pointer is non-null or the allocation would have aborted in
                // C++, and it is freed exactly once in `Drop`.
                Self(unsafe { $new() })
            }

            /// Number of entries, counted by walking the trie.
            ///
            /// Never inferred from `insert` return values: the Step 0 gate
            /// proved `insert` returns true for keys the trie cannot find, so
            /// the walk is the only trustworthy population check.
            pub fn len(&self) -> usize {
                // SAFETY: `self.0` is a live trie for the lifetime of `self`.
                unsafe { $len(self.0) }
            }

            /// Whether the trie holds no entries.
            pub fn is_empty(&self) -> bool {
                self.len() == 0
            }

            /// Full in-order traversal, folding every payload into a sink so the
            /// walk cannot be elided (§8.6).
            pub fn iterate_xor(&self) -> u64 {
                // SAFETY: as `len`.
                unsafe { $iter(self.0) }
            }

            /// Ordered scan of at most `k` entries from `lo`, returning how many
            /// were visited. The payloads are folded into a sink inside the shim.
            pub fn scan(&self, lo: u64, k: usize) -> usize {
                let mut sink = 0u64;
                // SAFETY: as `len`; `sink` is a live local for the call.
                let n = unsafe { $scan(self.0, lo, k, &raw mut sink) };
                std::hint::black_box(sink);
                n
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                // SAFETY: `self.0` was produced by the matching constructor and
                // this is the only place it is released.
                unsafe { $del(self.0) }
            }
        }
    };
}

/// HOT with `IdentityKeyExtractor<uint64_t>` — the arm paired against `ExpanseSet`.
///
/// The value *is* the key, stored inline in HOT's tagged child pointer, so
/// neither side of that pairing carries a separate payload.
pub struct HotSet(*mut c_void);

trie_wrapper!(
    HotSet,
    exp_hot_set_new,
    exp_hot_set_delete,
    exp_hot_set_len,
    exp_hot_set_iterate_xor,
    exp_hot_set_scan
);

impl HotSet {
    /// Inserts `k`.
    ///
    /// A key that does not fit HOT's inline payload returns
    /// [`InlineInsert::NotRepresentable`] and stores nothing — the arm's
    /// limitation surfaces at the call rather than as a quietly smaller trie.
    ///
    /// `Inserted` is *not* evidence the key is retrievable; HOT reports success
    /// on keys it cannot find. Check [`len`] after a build. [`len`]: Self::len
    pub fn insert(&mut self, k: u64) -> InlineInsert {
        // SAFETY: `self.0` is a live trie for the lifetime of `self`. The shim
        // returns -2 for a key wider than the inline payload and stores nothing.
        match unsafe { exp_hot_set_insert(self.0, k) } {
            1 => InlineInsert::Inserted,
            0 => InlineInsert::AlreadyPresent,
            _ => InlineInsert::NotRepresentable,
        }
    }

    /// Whether `k` is present.
    ///
    /// A key wider than the inline payload is not present by construction, and
    /// [`hot_can_inline`] says so without a lookup.
    pub fn contains(&self, k: u64) -> bool {
        // SAFETY: as `insert`.
        unsafe { exp_hot_set_contains(self.0, k) == 1 }
    }
}

/// HOT with `PairPointerKeyExtractor` — the arm paired against `ExpanseMap`.
///
/// HOT reaches its value through a heap-allocated `std::pair` per entry, which
/// is the only way it carries a value distinct from its key. That per-entry
/// allocation is part of the arm and is counted by the same census.
pub struct HotMap(*mut c_void);

trie_wrapper!(
    HotMap,
    exp_hot_map_new,
    exp_hot_map_delete,
    exp_hot_map_len,
    exp_hot_map_iterate_xor,
    exp_hot_map_scan
);

impl HotMap {
    /// Inserts `k -> v`, returning whether the trie reports it as newly present.
    ///
    /// Takes the full 64-bit domain: this arm stores a heap pointer, so
    /// [`hot_can_inline`] does not apply to it.
    pub fn insert(&mut self, k: u64, v: u64) -> bool {
        // SAFETY: `self.0` is a live trie for the lifetime of `self`.
        unsafe { exp_hot_map_insert(self.0, k, v) == 1 }
    }

    /// Reads the value for `k`.
    pub fn get(&self, k: u64) -> Option<u64> {
        let mut out = 0u64;
        // SAFETY: as `insert`; `out` is a live local for the call.
        let hit = unsafe { exp_hot_map_get(self.0, k, &raw mut out) };
        (hit == 1).then_some(out)
    }
}
