//! Memory a tree keeps after removals, against a fresh build of the keys that
//! remain: the Step 0a instrument of `docs/benchmarks/remove_retention/`.
//!
//! The remove paths step a branch down its own ladder (`BranchU` → `BranchB`
//! → `BranchL7` → `BranchL3`) and free it when it empties, but never rebuild a
//! branch subtree back into a packed leaf; only the root condenses. A tree
//! that grew past `LEAF_CAP` in an expanse and was then drained below it keeps
//! the branch shape. This example measures what that costs, per cell:
//!
//! * `R = mem_used(insert N, remove down to M) / mem_used(fresh insert of the
//!   same M keys)`;
//! * `mem_held()` after `shrink_to_fit()` on the drained tree, against the
//!   fresh build's `mem_used()` and `mem_held()`;
//! * an allocator census (`NodeAlloc::census`, read-only) of the drained tree,
//!   of the drained tree after `shrink_to_fit()`, of the fresh build and of the
//!   rebuilt tree: per slab size class, pages total, free, partly used and
//!   full, live and free blocks, and pages bucketed by live fraction;
//! * a rebuild arm: `let rebuilt = drained.clone(); drop(drained);` — the
//!   rebuilt tree's `mem_used()` / `mem_held()`, the peak `mem_held()` of the
//!   two trees together, and the process heap the clone requested on top of
//!   what was live before it (a counting global allocator; requested bytes,
//!   not the system allocator's chunk overhead). The set's `Clone` is
//!   `from_sorted_iter`; the map's is an ascending insert of every entry.
//!   Its instruction cost is the `set_rebuild_drained` / `map_rebuild_drained`
//!   Callgrind arms in `benches/instructions.rs`, not measured here.
//!
//! Both flavours (`ExpanseSet`, `ExpanseMap`), over uniform random keys at
//! several densities and keyspace widths, the census's construction-fixed
//! distributions (sequential, sparse `i << 40`, clustered runs of 256), and
//! three removal orders:
//!
//! * `shuffled` — a Fisher–Yates permutation of the keys; its first `N − M`
//!   entries are removed in that order;
//! * `sorted` — the same removed set as `shuffled`, removed in ascending key
//!   order, so the surviving set is identical and only the order differs;
//! * `range` — the `N − M` smallest keys, removed in ascending order (expiry
//!   of the oldest keys), leaving the top `M`.
//!
//! The fresh build inserts the survivors in the generator's order; a fresh
//! build's `mem_used()` does not depend on insertion order
//! (`tests/test_mem_used_order_invariant.rs`), so it is the canonical
//! denominator. Every figure is `mem_used()` / `mem_held()`, the engine's own
//! byte-exact accounting: deterministic and host-independent, so a cell has no
//! interval (AGENTS.md §8.4) and reproduces to the byte on any 64-bit host.
//! Each drained and fresh tree is also run through the structural validator and
//! its `NodeBytes` attribution is checked to sum to `mem_used()`.
//!
//! Before the grid, `model_pins` builds the fixed shapes whose byte values
//! `scripts/condense_bounds.py` pins (a 32-key `Leaf6`, the 33-key cascade, a
//! drained `BranchB` against its fresh leaf, the map's worst key-count-only
//! condense, a skip-edge fresh leaf) and asserts the engine reads the same
//! bytes, so an artifact cannot be written from a build the model no longer
//! describes.
//!
//! Run: `cargo run --release -p expanse-trie --example remove_retention [-- --json PATH] [-- --quick]`
//! `--quick` divides every population by 32 and is a smoke run only.
//! `EXPANSE_COMMIT` and `EXPANSE_RUSTC` name the commit and toolchain in the
//! artifact.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_remove_retention` |
//! | `group` | 5 |
//! | `population` | N = 3.2M removed to M = 1M at the headline cell (random @64, λ 48.8 → 15.3 keys per 2-byte expanse); random @64 also at 2M → 1M (λ 30.5 → 15.3), 4M → 1M (λ 61.0 → 15.3), 3.2M → 2M, 3.2M → 320k and 1M → 312.5k; random @62 and @56, sequential, sparse and clustered at 3.2M → 1M; the `model_pins` shapes hold at most 433 keys |
//! | `insertion_order` | generator — the population is inserted in draw order; removal order is the declared per-cell variable (`shuffled`, `sorted`, `range`) |
//! | `probes_and_reuse` | N/A (Memory) |
//! | `hit_rate` | N/A — every removal hits a present key |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | `mem_used()` / `mem_held()` accounting, `ExpanseStats::node_bytes` per form, `NodeAlloc::census` per slab class, requested heap bytes from a counting global allocator |
//! | `measured_region` | Clean: readings taken after the build, after the removals, after `shrink_to_fit()`, on a separate fresh build, and on the rebuilt tree after `clone()` and after the drained tree is dropped; the heap peak spans exactly the `clone()` call |
//! | `arm_symmetry` | The drained tree, the fresh build and the rebuilt tree hold the identical key set (asserted); `shuffled` and `sorted` remove the identical set |
//! | `statistics` | Exact byte and page counts; no interval (deterministic accounting) |
//! | `verdict` | Diagnostic census; the Step 0a gate reads the headline cell (`docs/benchmarks/remove_retention/METHODOLOGY.md`). |

use expanse_trie::alloc::{AllocCensus, CENSUS_BUCKETS};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::types::LEAF_CAP;
use expanse_trie::validate::ExpanseStats;
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Requested bytes live in the process, and the most live since the last
/// [`reset_peak`].
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;

impl Counting {
    fn grew(size: usize) {
        let now = LIVE.fetch_add(size, Ordering::Relaxed) + size;
        PEAK.fetch_max(now, Ordering::Relaxed);
    }
}

// SAFETY: every method forwards to `System` unchanged; the bookkeeping is
// atomic arithmetic and never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            Self::grew(l.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            Self::grew(l.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarded with the caller's arguments.
        let q = unsafe { System.realloc(p, l, new_size) };
        if !q.is_null() {
            if new_size >= l.size() {
                Self::grew(new_size - l.size());
            } else {
                LIVE.fetch_sub(l.size() - new_size, Ordering::Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Requested bytes live now.
fn live() -> usize {
    LIVE.load(Ordering::Relaxed)
}

/// Restarts the peak at the current live figure.
fn reset_peak() {
    PEAK.store(live(), Ordering::Relaxed);
}

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

/// The seed every committed random cell in this repository draws from.
const SEED: u64 = 0x0DDB_1A5E_5EED_0001;
/// The removal permutation's own stream, so the key draw and the removal
/// order are independent.
const SEED_PERM: u64 = 0x5EED_0DE1_E7E5_0002;

#[derive(Clone, Copy, PartialEq)]
enum Dist {
    Random(u32),
    Sequential,
    Sparse,
    Clustered,
}

impl Dist {
    fn name(self) -> String {
        match self {
            Dist::Random(b) => format!("random@{b}"),
            Dist::Sequential => "sequential".into(),
            Dist::Sparse => "sparse".into(),
            Dist::Clustered => "clustered".into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Order {
    Shuffled,
    Sorted,
    Range,
}

impl Order {
    fn name(self) -> &'static str {
        match self {
            Order::Shuffled => "shuffled",
            Order::Sorted => "sorted",
            Order::Range => "range",
        }
    }
}

/// One grid cell: (id, distribution, N, M, removal order).
type Cell = (&'static str, Dist, usize, usize, Order);

const fn c(id: &'static str, dist: Dist, n: usize, m: usize, order: Order) -> Cell {
    (id, dist, n, m, order)
}

use Dist::{Clustered, Random, Sequential, Sparse};
use Order::{Range, Shuffled, Sorted};

/// The Step 0a grid. The first row is the headline cell. Random @64 keys
/// put λ = N / 65,536 keys in each 2-byte expanse: 3.2M, 2M and 4M are
/// λ = 48.8, 30.5 and 61.0, and M = 1M is λ = 15.3.
const GRID: [Cell; 21] = [
    c("headline", Random(64), 3_200_000, 1_000_000, Shuffled),
    c("r64_sorted", Random(64), 3_200_000, 1_000_000, Sorted),
    c("r64_range", Random(64), 3_200_000, 1_000_000, Range),
    c("r64_2m_to_1m", Random(64), 2_000_000, 1_000_000, Shuffled),
    c("r64_4m_to_1m", Random(64), 4_000_000, 1_000_000, Shuffled),
    c("r64_to_2m", Random(64), 3_200_000, 2_000_000, Shuffled),
    c("r64_to_320k", Random(64), 3_200_000, 320_000, Shuffled),
    c("r64_1m_to_312k", Random(64), 1_000_000, 312_500, Shuffled),
    c("r62", Random(62), 3_200_000, 1_000_000, Shuffled),
    c("r56", Random(56), 3_200_000, 1_000_000, Shuffled),
    c("r56_sorted", Random(56), 3_200_000, 1_000_000, Sorted),
    c("r56_range", Random(56), 3_200_000, 1_000_000, Range),
    c("seq_shuffled", Sequential, 3_200_000, 1_000_000, Shuffled),
    c("seq_sorted", Sequential, 3_200_000, 1_000_000, Sorted),
    c("seq_range", Sequential, 3_200_000, 1_000_000, Range),
    c("sparse_shuffled", Sparse, 3_200_000, 1_000_000, Shuffled),
    c("sparse_sorted", Sparse, 3_200_000, 1_000_000, Sorted),
    c("sparse_range", Sparse, 3_200_000, 1_000_000, Range),
    c("clust_shuffled", Clustered, 3_200_000, 1_000_000, Shuffled),
    c("clust_sorted", Clustered, 3_200_000, 1_000_000, Sorted),
    c("clust_range", Clustered, 3_200_000, 1_000_000, Range),
];

/// `n` distinct keys in generator order.
fn keys(dist: Dist, n: usize) -> Vec<u64> {
    let mut rng = XorShift(SEED);
    match dist {
        Dist::Random(bits) => {
            let mask = if bits == 64 {
                u64::MAX
            } else {
                (1u64 << bits) - 1
            };
            let mut seen = HashSet::with_capacity(n);
            let mut out = Vec::with_capacity(n);
            while out.len() < n {
                let k = rng.next() & mask;
                if seen.insert(k) {
                    out.push(k);
                }
            }
            out
        }
        Dist::Sequential => (0..n as u64).collect(),
        Dist::Sparse => {
            assert!(n < (1 << 24), "sparse keys i << 40 must fit 64 bits");
            (0..n as u64).map(|i| i << 40).collect()
        }
        Dist::Clustered => {
            // The census generator (`keyspace_density.rs::anchor_cell`), with
            // a key that collides with an earlier run skipped, so the
            // population is exactly `n` distinct keys.
            let mut seen = HashSet::with_capacity(n);
            let mut out = Vec::with_capacity(n);
            let mut base = 0u64;
            let mut i = 0u64;
            while out.len() < n {
                if i.is_multiple_of(256) {
                    base = rng.next() & !0xFF;
                }
                let k = base + (i % 256);
                if seen.insert(k) {
                    out.push(k);
                }
                i += 1;
            }
            out
        }
    }
}

/// The keys to remove, in removal order.
fn removals(all: &[u64], m: usize, order: Order) -> Vec<u64> {
    let d = all.len() - m;
    match order {
        Order::Shuffled | Order::Sorted => {
            let mut perm: Vec<u64> = all.to_vec();
            let mut rng = XorShift(SEED_PERM);
            for i in (1..perm.len()).rev() {
                let j = (rng.next() % (i as u64 + 1)) as usize;
                perm.swap(i, j);
            }
            perm.truncate(d);
            if order == Order::Sorted {
                perm.sort_unstable();
            }
            perm
        }
        Order::Range => {
            let mut s = all.to_vec();
            s.sort_unstable();
            s.truncate(d);
            s
        }
    }
}

#[derive(Default, Clone, Copy)]
struct Reading {
    used_full: usize,
    used_drained: usize,
    held_drained: usize,
    released: usize,
    held_shrunk: usize,
    used_fresh: usize,
    held_fresh: usize,
    /// The fresh build's `mem_held()` after its own `shrink_to_fit()`.
    held_fresh_shrunk: usize,
    /// The rebuilt tree (`clone()` of the shrunk drained tree), read after
    /// the drained tree is dropped.
    used_rebuilt: usize,
    held_rebuilt: usize,
    /// The rebuilt tree's `mem_held()` after its own `shrink_to_fit()`.
    held_rebuilt_shrunk: usize,
    /// Requested heap bytes above the live figure before `clone()`, at their
    /// highest during the call: the new tree plus any scratch the clone
    /// allocates, while the drained tree is still live.
    rebuild_heap_peak_extra: usize,
}

/// The four allocator censuses of one cell.
struct Censuses {
    drained: AllocCensus,
    shrunk: AllocCensus,
    fresh: AllocCensus,
    rebuilt: AllocCensus,
}

trait Tree: Sized + Clone {
    fn new_tree() -> Self;
    fn put(&mut self, k: u64);
    fn take(&mut self, k: u64) -> bool;
    fn count(&self) -> u64;
    fn used(&self) -> usize;
    fn held(&self) -> usize;
    fn shrink(&mut self) -> usize;
    fn census(&self) -> ExpanseStats;
    fn alloc_census(&self) -> AllocCensus;
    fn check(&self);
}

impl Tree for ExpanseSet {
    fn new_tree() -> Self {
        ExpanseSet::new()
    }
    fn put(&mut self, k: u64) {
        assert!(self.insert(k), "keys are distinct");
    }
    fn take(&mut self, k: u64) -> bool {
        self.remove(k)
    }
    fn count(&self) -> u64 {
        self.len()
    }
    fn used(&self) -> usize {
        self.mem_used()
    }
    fn held(&self) -> usize {
        self.mem_held()
    }
    fn shrink(&mut self) -> usize {
        self.shrink_to_fit()
    }
    fn census(&self) -> ExpanseStats {
        self.stats()
    }
    fn alloc_census(&self) -> AllocCensus {
        Self::alloc_census(self)
    }
    fn check(&self) {
        self.validate();
    }
}

impl Tree for ExpanseMap {
    fn new_tree() -> Self {
        ExpanseMap::new()
    }
    fn put(&mut self, k: u64) {
        assert!(self.insert(k, !k).is_none(), "keys are distinct");
    }
    fn take(&mut self, k: u64) -> bool {
        self.remove(k) == Some(!k)
    }
    fn count(&self) -> u64 {
        self.len()
    }
    fn used(&self) -> usize {
        self.mem_used()
    }
    fn held(&self) -> usize {
        self.mem_held()
    }
    fn shrink(&mut self) -> usize {
        self.shrink_to_fit()
    }
    fn census(&self) -> ExpanseStats {
        self.stats()
    }
    fn alloc_census(&self) -> AllocCensus {
        Self::alloc_census(self)
    }
    fn check(&self) {
        self.validate();
    }
}

/// Asserts a census sums to the tree's own accounting.
fn checked(c: AllocCensus, used: usize, held: usize) -> AllocCensus {
    assert_eq!(c.used(), used, "census live bytes sum to mem_used()");
    assert_eq!(c.held(), held, "census parts sum to mem_held()");
    c
}

fn run<T: Tree>(all: &[u64], gone: &[u64]) -> (Reading, ExpanseStats, ExpanseStats, Censuses) {
    let m = all.len() - gone.len();
    let mut t = T::new_tree();
    for &k in all {
        t.put(k);
    }
    let used_full = t.used();
    for &k in gone {
        assert!(t.take(k), "every removal hits a present key");
    }
    assert_eq!(t.count(), m as u64);
    t.check();
    let used_drained = t.used();
    let held_drained = t.held();
    let census_drained = checked(t.alloc_census(), used_drained, held_drained);
    let released = t.shrink();
    let held_shrunk = t.held();
    let census_shrunk = checked(t.alloc_census(), used_drained, held_shrunk);
    assert_eq!(
        t.used(),
        used_drained,
        "shrink_to_fit leaves mem_used unchanged"
    );
    assert_eq!(
        held_drained - held_shrunk,
        released,
        "shrink_to_fit reports what it released"
    );
    let drained_stats = t.census();
    assert_eq!(drained_stats.node_bytes.total(), used_drained);

    let gone_set: HashSet<u64> = gone.iter().copied().collect();
    let mut f = T::new_tree();
    for &k in all.iter().filter(|k| !gone_set.contains(k)) {
        f.put(k);
    }
    assert_eq!(
        f.count(),
        m as u64,
        "the fresh build holds the surviving set"
    );
    f.check();
    let fresh_stats = f.census();
    assert_eq!(fresh_stats.node_bytes.total(), f.used());
    let (used_fresh, held_fresh) = (f.used(), f.held());
    let census_fresh = checked(f.alloc_census(), used_fresh, held_fresh);
    f.shrink();
    let held_fresh_shrunk = f.held();
    drop(f);

    // The rebuild arm: copy the (shrunk) drained tree, then free it.
    let live_before = live();
    reset_peak();
    let mut rebuilt = t.clone();
    let rebuild_heap_peak_extra = PEAK.load(Ordering::Relaxed) - live_before;
    drop(t);
    assert_eq!(rebuilt.count(), m as u64, "the rebuilt tree holds the set");
    rebuilt.check();
    let (used_rebuilt, held_rebuilt) = (rebuilt.used(), rebuilt.held());
    let census_rebuilt = checked(rebuilt.alloc_census(), used_rebuilt, held_rebuilt);
    rebuilt.shrink();
    let held_rebuilt_shrunk = rebuilt.held();
    (
        Reading {
            used_full,
            used_drained,
            held_drained,
            released,
            held_shrunk,
            used_fresh,
            held_fresh,
            held_fresh_shrunk,
            used_rebuilt,
            held_rebuilt,
            held_rebuilt_shrunk,
            rebuild_heap_peak_extra,
        },
        drained_stats,
        fresh_stats,
        Censuses {
            drained: census_drained,
            shrunk: census_shrunk,
            fresh: census_fresh,
            rebuilt: census_rebuilt,
        },
    )
}

/// One census as JSON: totals, the dense-page floor, the page histogram over
/// every slab class, and the per-class rows.
fn census_json(c: &AllocCensus) -> String {
    let pages: usize = c.slab.iter().map(|k| k.pages).sum();
    // The fewest pages the live blocks fit on, class by class.
    let dense: usize = c
        .slab
        .iter()
        .map(|k| k.live_blocks.div_ceil(k.blocks_per_page))
        .sum();
    let sum = |f: fn(&expanse_trie::alloc::SlabClassCensus) -> usize| -> usize {
        c.slab.iter().map(f).sum()
    };
    let mut hist = [0usize; CENSUS_BUCKETS];
    for k in &c.slab {
        for (h, v) in hist.iter_mut().zip(k.live_hist) {
            *h += v;
        }
    }
    let mut classes = String::new();
    for k in &c.slab {
        write!(
            classes,
            "{}{{\"bytes\": {}, \"align\": {}, \"block\": {}, \"blocks_per_page\": {}, \"pages\": {}, \
             \"pages_free\": {}, \"pages_partial\": {}, \"pages_full\": {}, \"live_blocks\": {}, \
             \"free_blocks\": {}, \"live_hist\": {:?}}}",
            if classes.is_empty() { "" } else { ", " },
            k.bytes,
            k.align,
            k.block,
            k.blocks_per_page,
            k.pages,
            k.pages_free,
            k.pages_partial,
            k.pages_full,
            k.live_blocks,
            k.free_blocks,
            k.live_hist,
        )
        .expect("write to a String");
    }
    format!(
        "{{\"slab_pages\": {pages}, \"slab_pages_dense_floor\": {dense}, \"pages_free\": {}, \
         \"pages_partial\": {}, \"pages_full\": {}, \"slab_live_bytes\": {}, \"slab_free_bytes\": {}, \
         \"slab_overhead_bytes\": {}, \"system_live_bytes\": {}, \"system_free_bytes\": {}, \
         \"live_hist\": {hist:?},\n        \"classes\": [{classes}]}}",
        sum(|k| k.pages_free),
        sum(|k| k.pages_partial),
        sum(|k| k.pages_full),
        c.slab_live_bytes(),
        c.slab_free_bytes(),
        c.slab_overhead_bytes(),
        c.system_live_bytes,
        c.system_free_bytes(),
    )
}

fn r4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

fn bytes_json(s: &ExpanseStats) -> String {
    let b = &s.node_bytes;
    let c = &s.node_counts;
    format!(
        "{{\"bytes\": {{\"immed_values\": {}, \"leaf_linear\": {}, \"leaf_bitmap\": {}, \"branch_l3\": {}, \
         \"branch_l7\": {}, \"branch_b\": {}, \"branch_u\": {}}}, \
         \"counts\": {{\"immed\": {}, \"leaf_linear\": {}, \"leaf_bitmap\": {}, \"branch_l3\": {}, \
         \"branch_l7\": {}, \"branch_b\": {}, \"branch_u\": {}}}, \"branch_depth_histogram\": {:?}}}",
        b.immed_values,
        b.leaf_linear,
        b.leaf_bitmap,
        b.branch_l3,
        b.branch_l7,
        b.branch_b,
        b.branch_u,
        c.immed,
        c.leaf_linear,
        c.leaf_bitmap,
        c.branch_l3,
        c.branch_l7,
        c.branch_b,
        c.branch_u,
        s.branch_depth_histogram,
    )
}

fn host_json() -> String {
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|v| v.trim().to_string())
        })
        .or_else(|| {
            std::process::Command::new("sysctl")
                .args(["-n", "machdep.cpu.brand_string"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into());
    let cpus = std::thread::available_parallelism().map_or(0, usize::from);
    let load = std::fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|_| "unavailable".into());
    format!(
        "{{\"cpu\": \"{cpu}\", \"logical_cpus\": {cpus}, \"os\": \"{}\", \"arch\": \"{}\", \"loadavg_at_start\": \"{load}\"}}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// Bytes below one edge of a fixed parent, measured on the engine: the
/// `mem_used()` of `filler` plus `keys` (after removing `gone`) minus that of
/// `filler` alone. The filler puts 200 top bytes under the root and 200 second
/// bytes under top byte 0, so the root and the level-7 node under top byte 0
/// are `BranchU` (fixed size) and every filler key is a single-key immediate
/// (0 bytes); the probed keys sit under their own edge of one of the two.
fn subtree_bytes<T: Tree>(keys: &[u64], gone: &[u64]) -> usize {
    let mut filler: Vec<u64> = (1u64..=200).map(|t| t << 56).collect();
    filler.extend((1u64..=200).map(|b| b << 48));
    let mut base = T::new_tree();
    for &k in &filler {
        base.put(k);
    }
    let mut t = T::new_tree();
    for &k in filler.iter().chain(keys) {
        t.put(k);
    }
    for &k in gone {
        assert!(t.take(k));
    }
    t.check();
    t.used() - base.used()
}

/// The byte values `scripts/condense_bounds.py` pins, read back from the
/// engine: (name, flavour, engine bytes, model bytes). A mismatch panics, so
/// an artifact cannot be written from a build the model no longer describes.
fn model_pins() -> Vec<(&'static str, &'static str, usize, usize)> {
    let pre6 = 0xFFu64 << 48; // the level-6 expanse (0x00, 0xFF)
    let k32: Vec<u64> = (0..32u64).map(|i| pre6 | (i << 20)).collect();
    // One key per level-5 digit, dealt round-robin over the 8 digit groups
    // (`condense_bounds.spread`).
    let k33: Vec<u64> = (0..33u64)
        .map(|i| pre6 | (((i % 8) * 32 + i / 8) << 40))
        .collect();
    // The map's worst key-count-only condense: under top byte 0xE0, a 16-key
    // child and a 1-key child left in a `BranchL3` once 16 more children are
    // drained from the cascaded branch.
    let mut worst: Vec<u64> = (0..16u64)
        .map(|j| (0xE0u64 << 56) | (1 << 48) | (j << 8))
        .collect();
    worst.push((0xE0u64 << 56) | (2 << 48));
    let drain: Vec<u64> = (3..19u64).map(|b| (0xE0u64 << 56) | (b << 48)).collect();
    let worst_all: Vec<u64> = worst.iter().chain(&drain).copied().collect();
    // 21 keys under a 5-byte prefix below the level-7 node: a skip edge, whose
    // fresh build is a leaf at the parent's child level (Leaf6), not a Leaf3.
    let pre3 = (0xFEu64 << 48) | (0xAAu64 << 40) | (0xBBu64 << 32) | (0xCCu64 << 24);
    let k21: Vec<u64> = (0..21u64)
        .map(|i| pre3 | ((i / 7) << 16) | ((i % 7) << 4))
        .collect();
    vec![
        (
            "leaf6_32",
            "set",
            subtree_bytes::<ExpanseSet>(&k32, &[]),
            192,
        ),
        (
            "leaf6_32",
            "map",
            subtree_bytes::<ExpanseMap>(&k32, &[]),
            448,
        ),
        (
            "cascade_33",
            "set",
            subtree_bytes::<ExpanseSet>(&k33, &[]),
            704,
        ),
        (
            "cascade_33",
            "map",
            subtree_bytes::<ExpanseMap>(&k33, &[]),
            704,
        ),
        (
            "drained_33_to_20",
            "set",
            subtree_bytes::<ExpanseSet>(&k33, &k33[20..]),
            512,
        ),
        (
            "drained_33_to_20",
            "map",
            subtree_bytes::<ExpanseMap>(&k33, &k33[20..]),
            512,
        ),
        (
            "fresh_20",
            "set",
            subtree_bytes::<ExpanseSet>(&k33[..20], &[]),
            144,
        ),
        (
            "fresh_20",
            "map",
            subtree_bytes::<ExpanseMap>(&k33[..20], &[]),
            336,
        ),
        (
            "worst_l3_16_1_drained",
            "map",
            subtree_bytes::<ExpanseMap>(&worst_all, &drain),
            288,
        ),
        (
            "worst_l3_16_1_fresh",
            "map",
            subtree_bytes::<ExpanseMap>(&worst, &[]),
            368,
        ),
        (
            "skip_edge_fresh_21",
            "set",
            subtree_bytes::<ExpanseSet>(&k21, &[]),
            144,
        ),
    ]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let json_path = args
        .iter()
        .position(|a| a == "--json")
        .and_then(|i| args.get(i + 1).cloned());
    let quick = args.iter().any(|a| a == "--quick");
    let scale = if quick { 32 } else { 1 };
    let host = host_json();

    println!(
        "remove retention: R = mem_used(drained to M) / mem_used(fresh M); LEAF_CAP = {LEAF_CAP}{}",
        if quick {
            " (--quick: populations / 32)"
        } else {
            ""
        }
    );
    println!(
        "{:<20} {:<3} {:>9} {:>9} {:<8} {:>9} {:>9} {:>7} {:>9} {:>7} {:>7} {:>7}",
        "cell",
        "fl",
        "N",
        "M",
        "order",
        "drained",
        "fresh",
        "R",
        "shrunk",
        "H/fresh",
        "S/HF",
        "RB/HF"
    );
    let pins = model_pins();
    let mut pins_json = String::new();
    for (name, flavor, engine, model) in &pins {
        println!("model pin {name:<24} {flavor:<3} engine {engine:>4} B  model {model:>4} B");
        assert_eq!(
            engine, model,
            "scripts/condense_bounds.py no longer describes the engine: {name} ({flavor})"
        );
        write!(
            pins_json,
            "{}{{\"name\": \"{name}\", \"flavor\": \"{flavor}\", \"engine_bytes\": {engine}, \"model_bytes\": {model}}}",
            if pins_json.is_empty() { "" } else { ", " }
        )
        .expect("write to a String");
    }
    let mut rows = String::new();
    for (id, dist, n, m, order) in GRID {
        let (n, m) = (n / scale, m / scale);
        let all = keys(dist, n);
        let gone = removals(&all, m, order);
        for flavor in ["set", "map"] {
            let (r, ds, fs, cs) = if flavor == "set" {
                run::<ExpanseSet>(&all, &gone)
            } else {
                run::<ExpanseMap>(&all, &gone)
            };
            let ratio = r.used_drained as f64 / r.used_fresh as f64;
            let held_ratio = r.held_shrunk as f64 / r.used_fresh as f64;
            let shrunk_over_held_fresh = r.held_shrunk as f64 / r.held_fresh as f64;
            let rebuilt_over_held_fresh = r.held_rebuilt as f64 / r.held_fresh as f64;
            let peak_held = r.held_shrunk + r.held_rebuilt;
            println!(
                "{id:<20} {flavor:<3} {n:>9} {m:>9} {:<8} {:>9.2} {:>9.2} {ratio:>7.3} {:>9.2} {held_ratio:>7.3} {shrunk_over_held_fresh:>7.3} {rebuilt_over_held_fresh:>7.3}",
                order.name(),
                r.used_drained as f64 / m as f64,
                r.used_fresh as f64 / m as f64,
                r.held_shrunk as f64 / m as f64,
            );
            let lam = match dist {
                Dist::Random(bits) => format!(
                    "\"lambda_n\": {}, \"lambda_m\": {}, ",
                    r4(n as f64 / (1u64 << (bits - 48)) as f64),
                    r4(m as f64 / (1u64 << (bits - 48)) as f64)
                ),
                _ => String::new(),
            };
            writeln!(
                rows,
                "    {{\"cell\": \"{id}\", \"flavor\": \"{flavor}\", \"dist\": \"{}\", \"n\": {n}, \"m\": {m}, \
                 \"removal_order\": \"{}\", {lam}\"used_full\": {}, \"used_drained\": {}, \"held_drained\": {}, \
                 \"released_by_shrink\": {}, \"held_shrunk\": {}, \"used_fresh\": {}, \"held_fresh\": {}, \
                 \"r\": {}, \"held_shrunk_over_used_fresh\": {}, \"bpk_drained\": {}, \"bpk_fresh\": {},\n      \
                 \"held_fresh_shrunk\": {}, \"used_rebuilt\": {}, \"held_rebuilt\": {}, \"held_rebuilt_shrunk\": {}, \
                 \"held_shrunk_over_held_fresh\": {}, \"held_rebuilt_over_held_fresh\": {}, \
                 \"rebuild_peak_held\": {peak_held}, \"rebuild_heap_peak_extra\": {},\n      \
                 \"drained\": {},\n      \"fresh\": {},\n      \
                 \"census\": {{\"drained\": {},\n        \"shrunk\": {},\n        \"fresh\": {},\n        \"rebuilt\": {}}}}},",
                dist.name(),
                order.name(),
                r.used_full,
                r.used_drained,
                r.held_drained,
                r.released,
                r.held_shrunk,
                r.used_fresh,
                r.held_fresh,
                r4(ratio),
                r4(held_ratio),
                r4(r.used_drained as f64 / m as f64),
                r4(r.used_fresh as f64 / m as f64),
                r.held_fresh_shrunk,
                r.used_rebuilt,
                r.held_rebuilt,
                r.held_rebuilt_shrunk,
                r4(shrunk_over_held_fresh),
                r4(rebuilt_over_held_fresh),
                r.rebuild_heap_peak_extra,
                bytes_json(&ds),
                bytes_json(&fs),
                census_json(&cs.drained),
                census_json(&cs.shrunk),
                census_json(&cs.fresh),
                census_json(&cs.rebuilt),
            )
            .expect("write to a String");
        }
    }

    if let Some(path) = json_path {
        assert!(
            !quick,
            "--quick output is a smoke run and is never written as an artifact"
        );
        let commit = std::env::var("EXPANSE_COMMIT").unwrap_or_else(|_| "unknown".into());
        let rustc = std::env::var("EXPANSE_RUSTC").unwrap_or_else(|_| "unknown".into());
        let doc = format!(
            "{{\n  \"provenance\": {{\n    \"source\": \"crates/expanse/examples/remove_retention.rs --json\",\n    \
             \"workload_id\": \"example_remove_retention\",\n    \"commit\": \"{commit}\",\n    \"rustc\": \"{rustc}\",\n    \
             \"profile\": \"release\",\n    \"host\": {host},\n    \
             \"estimators\": {{\"kind\": \"deterministic count\", \"interval\": \"none: mem_used()/mem_held() are exact byte counts of the engine's own accounting; a cell has no rounds and no interval (AGENTS.md section 8.4) and reproduces to the byte on any 64-bit host at the same commit\", \
             \"r\": \"used_drained / used_fresh: the drained tree against a fresh build of the identical surviving key set\", \
             \"held_shrunk_over_used_fresh\": \"mem_held() after shrink_to_fit() on the drained tree / the fresh build's mem_used()\", \
             \"held_shrunk_over_held_fresh\": \"mem_held() after shrink_to_fit() on the drained tree / the fresh build's mem_held() (no shrink)\", \
             \"held_rebuilt_over_held_fresh\": \"mem_held() of clone() of the shrunk drained tree, read after the drained tree is dropped, no shrink / the fresh build's mem_held() (no shrink)\", \
             \"rebuild_peak_held\": \"held_shrunk + held_rebuilt: both trees live at the end of clone(); mem_held() only falls on shrink_to_fit, clear or drop, so neither term is higher earlier in the call\", \
             \"rebuild_heap_peak_extra\": \"requested bytes from a counting global allocator, at their highest during clone(), minus the live figure before it: the new tree plus the clone's scratch; excludes the system allocator's chunk overhead; deterministic for a given toolchain\", \
             \"census\": \"NodeAlloc::census, read-only, per slab size class; live_hist buckets pages by live fraction: [0] no live block, [1..=10] deciles of live/blocks_per_page, [11] every block live; slab_pages_dense_floor is the sum over classes of ceil(live_blocks / blocks_per_page); each census is asserted to sum to mem_used() and mem_held()\", \
             \"cost\": \"not measured here; instructions come from the set_rebuild_drained / map_rebuild_drained Callgrind arms (crates/expanse/benches/instructions.rs, instruction-counts CI job)\"}},\n    \
             \"load\": \"not recorded: a byte count does not depend on host load\",\n    \
             \"generator\": \"XorShift64(0x0DDB_1A5E_5EED_0001) for keys (distinct, generator order); removal permutation Fisher-Yates from XorShift64(0x5EED_0DE1_E7E5_0002)\",\n    \
             \"leaf_cap\": {LEAF_CAP}\n  }},\n  \"model_pins\": [{pins_json}],\n  \"cells\": [\n{}\n  ]\n}}\n",
            rows.trim_end_matches(",\n")
        );
        std::fs::write(&path, doc).expect("write --json output");
        println!("\nwrote {path}");
    }
}
