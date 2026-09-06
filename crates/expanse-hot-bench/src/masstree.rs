//! Masstree — the comparison arm of #661 (`docs/benchmarks/masstree_comparison/`).
//!
//! Compiled only under the `masstree` feature. One table type serves both
//! pairings of METHODOLOGY §4: [`Masstree::insert`] / [`Masstree::get`] take
//! `u64` keys and pass them as 8-byte big-endian strings, so Masstree's
//! byte-lexicographic order is numeric order and the whole key is one ikey
//! slice; [`Masstree::str_insert`] / [`Masstree::str_get`] take byte strings.
//!
//! **Every operation takes a thread handle.** Masstree's reclamation is
//! per-thread (§3.2): a [`MtThread`] is a slot the shim owns, bracketed by
//! [`MtThread::enter`] and [`MtThread::exit`], and the Masstree side of every
//! timed loop calls [`MtThread::quiesce`] every [`QUIESCE_EVERY`] operations —
//! `mttest`'s own cadence, billed to Masstree.
//!
//! **The 255-byte predicate is Masstree's contract, not a harness choice**
//! (§3.4). [`masstree_can_key`] is evaluated over a workload and reported; a
//! key it rejects must never reach the string entry points, which is why they
//! panic on the shim's refusal code rather than returning a quiet miss.
//!
//! The concurrent wrappers of the HOT suite exposed `&self` operations because
//! ROWEX's per-thread state lives in TBB thread-locals; here the thread state
//! is the explicit handle, so `&self` plus a handle is the whole contract and
//! the table is `Send + Sync`.

use std::os::raw::c_void;

/// Operations between two `quiesce` calls on the Masstree side of a timed
/// loop — `kvtest.hh` quiesces every `1 << 6` operations.
pub const QUIESCE_EVERY: u64 = 64;

/// Thread slots the shim owns. The concurrent arm uses at most 16 for its
/// workers (the P-core pin) plus a few for verification.
pub const MT_SLOTS: u32 = 64;

/// Masstree's `MASSTREE_MAXKEYLEN` at the shipped default. Read back from the
/// header through [`masstree_max_key_len`]; the validation gate asserts the
/// two agree.
pub const MASSTREE_MAX_KEY_LEN: usize = 255;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
/// Masstree's own node census, from `json_stats` (§3.3, engine-instrument column).
pub struct MtStats {
    /// Keys the walk counted.
    pub size: u64,
    /// Leaves across every layer.
    pub leaves: u64,
    /// Internodes across every layer.
    pub internodes: u64,
    /// Layers below the first (one per shared 8-byte slice that needed one).
    pub layers: u64,
    /// Capacity of the key-suffix bags, internal and external.
    pub ksuf_capacity: u64,
    /// Bytes leaves were allocated beyond their minimum for an internal bag.
    pub overridden_ksuf_capacity: u64,
    /// Pool-rounded `sizeof(leaf)`.
    pub leaf_bytes: u64,
    /// Pool-rounded `sizeof(internode)`.
    pub internode_bytes: u64,
    /// `leaves × leaf_bytes + internodes × internode_bytes + ksuf_capacity +
    /// overridden_ksuf_capacity` — `masstree_envelope.structural_bytes`.
    pub structural_bytes: u64,
}

unsafe extern "C" {
    fn exp_mt_max_key_len() -> usize;
    fn exp_mt_slab_bytes() -> usize;
    fn exp_mt_leaf_bytes() -> usize;
    fn exp_mt_internode_bytes() -> usize;

    fn exp_mt_thread(slot: u32) -> *mut c_void;
    fn exp_mt_thread_enter(ti: *mut c_void);
    fn exp_mt_thread_exit(ti: *mut c_void);
    fn exp_mt_quiesce(ti: *mut c_void);
    fn exp_mt_settle_step(ti: *mut c_void);

    fn exp_mt_new(ti: *mut c_void) -> *mut c_void;
    fn exp_mt_delete(t: *mut c_void, ti: *mut c_void);

    fn exp_mt_insert(t: *mut c_void, ti: *mut c_void, k: u64, v: u64) -> i32;
    fn exp_mt_get(t: *mut c_void, ti: *mut c_void, k: u64, out: *mut u64) -> i32;
    fn exp_mt_len(t: *mut c_void, ti: *mut c_void) -> usize;
    fn exp_mt_iterate_xor(t: *mut c_void, ti: *mut c_void) -> u64;
    fn exp_mt_scan(t: *mut c_void, ti: *mut c_void, lo: u64, k: usize, sink: *mut u64) -> usize;

    fn exp_mt_str_insert(t: *mut c_void, ti: *mut c_void, k: *const u8, len: usize, v: u64) -> i32;
    fn exp_mt_str_get(
        t: *mut c_void,
        ti: *mut c_void,
        k: *const u8,
        len: usize,
        out: *mut u64,
    ) -> i32;
    fn exp_mt_str_scan(
        t: *mut c_void,
        ti: *mut c_void,
        lo: *const u8,
        lo_len: usize,
        k: usize,
        sink: *mut u64,
    ) -> usize;

    fn exp_mt_stats(t: *mut c_void, ti: *mut c_void, out: *mut MtStats);

    // `concurrent = false` twins (§10.3).
    fn exp_mts_new(ti: *mut c_void) -> *mut c_void;
    fn exp_mts_delete(t: *mut c_void, ti: *mut c_void);

    fn exp_mts_insert(t: *mut c_void, ti: *mut c_void, k: u64, v: u64) -> i32;
    fn exp_mts_get(t: *mut c_void, ti: *mut c_void, k: u64, out: *mut u64) -> i32;
    fn exp_mts_len(t: *mut c_void, ti: *mut c_void) -> usize;
    fn exp_mts_iterate_xor(t: *mut c_void, ti: *mut c_void) -> u64;
    fn exp_mts_scan(t: *mut c_void, ti: *mut c_void, lo: u64, k: usize, sink: *mut u64) -> usize;

    fn exp_mts_str_insert(t: *mut c_void, ti: *mut c_void, k: *const u8, len: usize, v: u64)
    -> i32;
    fn exp_mts_str_get(
        t: *mut c_void,
        ti: *mut c_void,
        k: *const u8,
        len: usize,
        out: *mut u64,
    ) -> i32;
    fn exp_mts_str_scan(
        t: *mut c_void,
        ti: *mut c_void,
        lo: *const u8,
        lo_len: usize,
        k: usize,
        sink: *mut u64,
    ) -> usize;

    fn exp_mts_stats(t: *mut c_void, ti: *mut c_void, out: *mut MtStats);
}

/// `MASSTREE_MAXKEYLEN`, read from the header through the shim.
pub fn masstree_max_key_len() -> usize {
    // SAFETY: returns a compile-time constant; no preconditions.
    unsafe { exp_mt_max_key_len() }
}

/// The pool slab size — the census quantum of §3.3 — as the shim reports it.
pub fn masstree_slab_bytes() -> usize {
    // SAFETY: returns a constant; no preconditions.
    unsafe { exp_mt_slab_bytes() }
}

/// Pool-rounded `sizeof(leaf<15>)` and `sizeof(internode<15>)`.
pub fn masstree_node_bytes() -> (usize, usize) {
    // SAFETY: both return constants; no preconditions.
    unsafe { (exp_mt_leaf_bytes(), exp_mt_internode_bytes()) }
}

/// Whether a key of `len` bytes is inside Masstree's contract (§3.4).
///
/// A predicate on **Masstree**, evaluated against a workload and reported —
/// never used to trim a workload.
#[inline]
pub fn masstree_can_key(len: usize) -> bool {
    len <= MASSTREE_MAX_KEY_LEN
}

/// A Masstree thread slot: the `threadinfo` every operation takes.
///
/// Slots are created lazily by the shim under a mutex and never freed. A slot
/// belongs to one OS thread at a time; the harnesses hand slot `w` to writer
/// `w` and slot `16 + r` to reader `r`, and use a fresh slot per census cell
/// (§3.6).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MtThread(*mut c_void);

// SAFETY: the handle is an opaque pointer to a `threadinfo` the shim owns for
// the life of the process; moving it between threads is how a Rust thread
// takes its slot. Using one slot from two threads at once is the caller's
// contract, documented on the type.
unsafe impl Send for MtThread {}
// SAFETY: as above — the handle itself is immutable; the shim serialises slot
// creation and every operation through a handle is one Masstree declares safe
// for the thread holding it.
unsafe impl Sync for MtThread {}

impl MtThread {
    /// The slot with index `slot` (`< MT_SLOTS`), created on first use.
    pub fn slot(slot: u32) -> Self {
        assert!(
            slot < MT_SLOTS,
            "thread slot {slot} out of range ({MT_SLOTS} slots)"
        );
        // SAFETY: the shim bounds-checks and serialises creation; the returned
        // pointer is non-null (the shim aborts on allocation failure).
        Self(unsafe { exp_mt_thread(slot) })
    }

    /// `rcu_start`: the thread is active from here until [`exit`](Self::exit).
    pub fn enter(self) {
        // SAFETY: `self.0` is a live threadinfo owned by the shim.
        unsafe { exp_mt_thread_enter(self.0) }
    }

    /// `rcu_stop`: a finished thread must not pin the active epoch.
    pub fn exit(self) {
        // SAFETY: as `enter`.
        unsafe { exp_mt_thread_exit(self.0) }
    }

    /// One census reclamation step (§10.4): advance the global epoch by one and
    /// quiesce this slot, freeing what the build deferred through RCU. The
    /// memory pillar repeats it until the census sees no further frees.
    pub fn settle_step(self) {
        // SAFETY: as `enter`.
        unsafe { exp_mt_settle_step(self.0) }
    }

    /// `mttest`'s `rcu_quiesce`: advance the global epoch from the clock if it
    /// moved, then reclaim this slot's limbo. Called every [`QUIESCE_EVERY`]
    /// operations inside Masstree's timed loops (§3.2).
    #[inline]
    pub fn quiesce(self) {
        // SAFETY: as `enter`.
        unsafe { exp_mt_quiesce(self.0) }
    }
}

/// Which configuration of Masstree's template a table is (§10.3).
///
/// `nodeparams::concurrent` selects the node-version type: `Single` uses
/// `singlethreaded_nodeversion` (no fences, no spin locks) and is the twin of
/// `ExpanseMap` / `ExpanseStrMap`; `Concurrent` uses the fenced `nodeversion`
/// and is the twin of `SyncExpanseMap` / `SyncExpanseStrMap`. The same split
/// HOT ships as `HOTSingleThreaded` and `HOTRowex`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Table {
    /// `concurrent = false`.
    Single,
    /// `concurrent = true`.
    Concurrent,
}

impl Table {
    /// The name used in result rows.
    pub fn name(self) -> &'static str {
        match self {
            Table::Single => "single",
            Table::Concurrent => "concurrent",
        }
    }

    /// Parses a result-row name.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "single" => Some(Table::Single),
            "concurrent" => Some(Table::Concurrent),
            _ => None,
        }
    }
}

/// What a string insert did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StrInsert {
    /// Newly present.
    Inserted,
    /// Already present; the value was replaced.
    Replaced,
    /// Longer than `MASSTREE_MAXKEYLEN`; nothing was stored (§3.4).
    NotRepresentable,
}

/// A Masstree table: `basic_table<nodeparams<15, 15>>` with `uint64_t` values,
/// in either [`Table`] configuration.
///
/// A [`Table::Concurrent`] table is concurrent by Masstree's own protocol:
/// `insert`, `get` and `str_*` may be called from any number of threads at
/// once, each with its own [`MtThread`]. A [`Table::Single`] table must only
/// ever be driven from one thread; the concurrent harness never constructs one.
/// `len`, `iterate_xor`, the scans and `stats` walk the structure and are
/// quiescent-only — called after every writer has joined.
pub struct Masstree {
    t: *mut c_void,
    owner: MtThread,
    table: Table,
}

/// Calls the `exp_mt_*` or `exp_mts_*` entry point for the table's configuration.
macro_rules! dispatch {
    ($self:ident, $conc:ident, $single:ident, ($($arg:expr),*)) => {
        match $self.table {
            Table::Concurrent => $conc($self.t, $($arg),*),
            Table::Single => $single($self.t, $($arg),*),
        }
    };
}

// SAFETY: Masstree's table is designed for concurrent use through per-thread
// `threadinfo`s (per-node locks for writers, version-validated readers); the
// wrapper exposes exactly those operations, each requiring a handle, and the
// walks are documented quiescent-only. The raw pointer is owned and freed once.
unsafe impl Send for Masstree {}
// SAFETY: as above — shared access maps onto operations Masstree declares
// concurrent, and nothing else is reachable through `&Masstree`.
unsafe impl Sync for Masstree {}

impl Masstree {
    /// Builds an empty table, allocating its root leaf from `owner`'s pool.
    ///
    /// In a census cell this call sits **inside** the armed window and `owner`
    /// is a fresh slot (§3.3, §3.6).
    pub fn new(owner: MtThread, table: Table) -> Self {
        // SAFETY: `owner` is a live slot; the shim returns an owned table,
        // freed exactly once in `Drop`.
        let t = unsafe {
            match table {
                Table::Concurrent => exp_mt_new(owner.0),
                Table::Single => exp_mts_new(owner.0),
            }
        };
        Self { t, owner, table }
    }

    /// Which configuration this table is.
    pub fn table(&self) -> Table {
        self.table
    }

    /// Concurrent insert of `k -> v`; `true` if newly present, `false` if the
    /// value was replaced — `ExpanseMap::insert` semantics.
    #[inline]
    pub fn insert(&self, ti: MtThread, k: u64, v: u64) -> bool {
        // SAFETY: `self.t` is a live table and `ti` a live slot held by the
        // calling thread; the call is one Masstree declares concurrent.
        unsafe { dispatch!(self, exp_mt_insert, exp_mts_insert, (ti.0, k, v)) == 1 }
    }

    /// Concurrent lookup that fetches the stored value.
    #[inline]
    pub fn get(&self, ti: MtThread, k: u64) -> Option<u64> {
        let mut out = 0u64;
        // SAFETY: as `insert`; `out` is a live local for the call.
        let hit = unsafe { dispatch!(self, exp_mt_get, exp_mts_get, (ti.0, k, &raw mut out)) };
        (hit == 1).then_some(out)
    }

    /// Population by walking from the empty key — quiescent-only, never
    /// inferred from insert return values.
    pub fn len(&self, ti: MtThread) -> usize {
        // SAFETY: `self.t` is live; the caller guarantees no concurrent writer.
        unsafe { dispatch!(self, exp_mt_len, exp_mts_len, (ti.0)) }
    }

    /// Whether the table holds no entries (quiescent-only).
    pub fn is_empty(&self, ti: MtThread) -> bool {
        self.len(ti) == 0
    }

    /// Full in-order traversal, folding every value into a sink (quiescent-only).
    pub fn iterate_xor(&self, ti: MtThread) -> u64 {
        // SAFETY: as `len`.
        unsafe { dispatch!(self, exp_mt_iterate_xor, exp_mts_iterate_xor, (ti.0)) }
    }

    /// Ordered scan of at most `k` entries from `lo` (inclusive); returns how
    /// many were visited and the folded values.
    pub fn scan(&self, ti: MtThread, lo: u64, k: usize) -> (usize, u64) {
        let mut sink = 0u64;
        // SAFETY: as `len`; `sink` is a live local for the call.
        let n = unsafe {
            dispatch!(
                self,
                exp_mt_scan,
                exp_mts_scan,
                (ti.0, lo, k, &raw mut sink)
            )
        };
        (n, sink)
    }

    /// Concurrent string insert. A key the §3.4 predicate rejects returns
    /// [`StrInsert::NotRepresentable`] and stores nothing.
    #[inline]
    pub fn str_insert(&self, ti: MtThread, k: &[u8], v: u64) -> StrInsert {
        // SAFETY: as `insert`; `k` is a live slice for the call and Masstree
        // copies the bytes it keeps.
        match unsafe {
            dispatch!(
                self,
                exp_mt_str_insert,
                exp_mts_str_insert,
                (ti.0, k.as_ptr(), k.len(), v)
            )
        } {
            1 => StrInsert::Inserted,
            0 => StrInsert::Replaced,
            _ => StrInsert::NotRepresentable,
        }
    }

    /// Concurrent string lookup that fetches the stored value.
    ///
    /// Panics if `k` is beyond the predicate: such a key reaching this side is
    /// a harness bug (§9), not a miss.
    #[inline]
    pub fn str_get(&self, ti: MtThread, k: &[u8]) -> Option<u64> {
        let mut out = 0u64;
        // SAFETY: as `str_insert`; `out` is a live local for the call.
        match unsafe {
            dispatch!(
                self,
                exp_mt_str_get,
                exp_mts_str_get,
                (ti.0, k.as_ptr(), k.len(), &raw mut out)
            )
        } {
            1 => Some(out),
            0 => None,
            _ => panic!(
                "a {}-byte key reached the Masstree side; the harness must withhold the column (§3.4)",
                k.len()
            ),
        }
    }

    /// Ordered string scan of at most `k` entries from `lo` (inclusive).
    /// Panics if `lo` is beyond the predicate, as [`str_get`](Self::str_get).
    pub fn str_scan(&self, ti: MtThread, lo: &[u8], k: usize) -> (usize, u64) {
        let mut sink = 0u64;
        // SAFETY: as `len`; `lo` and `sink` are live for the call.
        let n = unsafe {
            dispatch!(
                self,
                exp_mt_str_scan,
                exp_mts_str_scan,
                (ti.0, lo.as_ptr(), lo.len(), k, &raw mut sink)
            )
        };
        assert!(
            n != usize::MAX,
            "a {}-byte scan start reached the Masstree side; the harness must withhold the column (§3.4)",
            lo.len()
        );
        (n, sink)
    }

    /// Masstree's own node census (quiescent-only). Allocates through
    /// `operator new`, so census cells call it with the counters disarmed.
    pub fn stats(&self, ti: MtThread) -> MtStats {
        let mut out = MtStats::default();
        // SAFETY: as `len`; `out` is a live, correctly laid out `#[repr(C)]`
        // local matching the shim's struct field for field.
        unsafe { dispatch!(self, exp_mt_stats, exp_mts_stats, (ti.0, &raw mut out)) };
        out
    }
}

impl Drop for Masstree {
    fn drop(&mut self) {
        // SAFETY: `self.t` was produced by `exp_mt_new` and is released here
        // only; `owner` is a live slot. Census cells `forget` the table
        // instead (§3.6), so this never runs in one.
        unsafe { dispatch!(self, exp_mt_delete, exp_mts_delete, (self.owner.0)) }
    }
}
