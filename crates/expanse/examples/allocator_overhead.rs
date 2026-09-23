//! Allocator-inclusive memory census: what a process pays for a tree beyond
//! `mem_used()`.
//!
//! `mem_used()` counts the bytes of the tree's live nodes, each rounded to
//! its alignment (`alloc::accounted_size`). The process pays more, in three
//! layers this example separates. A tracking global allocator records every
//! live allocation's requested size, alignment and `malloc_usable_size`,
//! and the resident set is read before and after the build:
//!
//! 1. **engine retention**: `mem_held()` minus `mem_used()` — freed blocks
//!    the tree keeps on its per-tree freelists and the unused blocks of its
//!    slab pages, returned to the system only when the tree is dropped;
//! 2. **allocator overhead**: usable bytes plus chunk headers, minus the
//!    requested bytes. Headers are live allocations × `HEADER` (glibc: one
//!    `size_t` per in-use chunk, 8 B on 64-bit; derived from the
//!    allocator's documented chunk layout, not measured);
//! 3. **residual**: RSS delta minus (usable + headers) — fragmentation,
//!    split-off alignment padding and free chunks the allocator keeps.
//!
//! Keys are generated on the fly into a reused buffer, so the only live
//! allocations the census sees are the tree's own. Linux (glibc) is the
//! target: RSS comes from `/proc/self/statm`. On other hosts the RSS columns
//! print as `n/a`, and usable size is the platform's equivalent call.
//!
//! Run: `cargo run --release -p expanse-trie --example allocator_overhead -- [N] [shape]`
//!
//! With no shape, every shape runs in a child process of its own. RSS is a
//! per-process quantity, and a tree dropped earlier in the same process
//! leaves resident free chunks that the next build reuses, which would
//! understate every delta after the first.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_allocator_overhead` |
//! | `group` | 5 |
//! | `population` | N keys per shape (default 1,000,000; first argument) |
//! | `insertion_order` | generator draw order — each key is inserted as drawn, neither sorted nor shuffled |
//! | `probes_and_reuse` | N/A (Memory) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | `mem_used()` accounting beside allocator-reported usable bytes and RSS |
//! | `measured_region` | Clean: keys generated into a reused buffer, one tree live at a time |
//! | `arm_symmetry` | One `ExpanseMap` or `ExpanseStrMap` per shape, 8-byte values |
//! | `statistics` | Exact byte counts; RSS is a single reading per shape |
//! | `verdict` | Diagnostic census; no gate. |

use expanse_trie::map::ExpanseMap;
use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Glibc's per-chunk header on 64-bit: the in-use chunk's size field.
const HEADER: usize = 8;

static TRACKING: AtomicBool = AtomicBool::new(false);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static REQUESTED: AtomicUsize = AtomicUsize::new(0);
static USABLE: AtomicUsize = AtomicUsize::new(0);
/// `(requested size, align) -> (live count, usable bytes)`.
static HIST: Mutex<BTreeMap<(usize, usize), (isize, isize)>> = Mutex::new(BTreeMap::new());

thread_local! {
    static IN_HOOK: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

#[cfg(target_os = "linux")]
fn usable_size(p: *mut u8) -> usize {
    // SAFETY: `p` was returned by the system allocator and is live.
    unsafe { libc::malloc_usable_size(p.cast()) }
}
#[cfg(target_os = "macos")]
fn usable_size(p: *mut u8) -> usize {
    // SAFETY: `p` was returned by the system allocator and is live.
    unsafe { libc::malloc_size(p.cast::<libc::c_void>().cast_const()) }
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn usable_size(_p: *mut u8) -> usize {
    0
}

struct Tracking;

impl Tracking {
    /// Records one live allocation (`sign > 0`) or its release. `p` must be
    /// live when this runs, so a release is recorded before the free.
    fn record(p: *mut u8, layout: Layout, sign: isize) {
        // The histogram's own node allocations re-enter the global
        // allocator; the thread-local flag keeps them out of the census.
        if !TRACKING.load(Ordering::Relaxed) || IN_HOOK.with(|h| h.replace(true)) {
            return;
        }
        let u = usable_size(p);
        let counters = [(&LIVE, 1), (&REQUESTED, layout.size()), (&USABLE, u)];
        for (c, v) in counters {
            if sign > 0 {
                c.fetch_add(v, Ordering::Relaxed);
            } else {
                c.fetch_sub(v, Ordering::Relaxed);
            }
        }
        if let Ok(mut h) = HIST.lock() {
            let e = h.entry((layout.size(), layout.align())).or_insert((0, 0));
            e.0 += sign;
            e.1 += sign * u as isize;
        }
        IN_HOOK.with(|h| h.set(false));
    }
}

// SAFETY: every method forwards to `System` unchanged; the bookkeeping
// reads the returned pointer only through the allocator's usable-size query.
unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            Self::record(p, layout, 1);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            Self::record(p, layout, 1);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        Self::record(p, layout, -1);
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        Self::record(p, layout, -1);
        // SAFETY: forwarded with the caller's arguments.
        let q = unsafe { System.realloc(p, layout, new_size) };
        if q.is_null() {
            Self::record(p, layout, 1);
        } else {
            let grown = Layout::from_size_align(new_size, layout.align()).expect("realloc layout");
            Self::record(q, grown, 1);
        }
        q
    }
}

#[global_allocator]
static GLOBAL: Tracking = Tracking;

/// Resident bytes, or `None` where `/proc/self/statm` does not exist.
fn rss() -> Option<usize> {
    let s = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: usize = s.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4096)
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn trim() {
    // SAFETY: plain libc call with no pointer arguments; the return value
    // only reports whether memory was released.
    let _released = unsafe { libc::malloc_trim(0) };
}
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn trim() {}

struct SplitMix(u64);
impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

enum Tree {
    U64(ExpanseMap),
    Str(ExpanseStrMap),
}

impl Tree {
    fn mem_used(&self) -> usize {
        match self {
            Tree::U64(m) => m.mem_used(),
            Tree::Str(m) => m.mem_used(),
        }
    }

    fn mem_held(&self) -> usize {
        match self {
            Tree::U64(m) => m.mem_held(),
            Tree::Str(m) => m.mem_held(),
        }
    }

    fn shrink_to_fit(&mut self) -> usize {
        match self {
            Tree::U64(m) => m.shrink_to_fit(),
            Tree::Str(m) => m.shrink_to_fit(),
        }
    }
}

fn insert_str(m: &mut ExpanseStrMap, buf: &str, v: u64) {
    let key: &NulFreeStr = buf
        .as_bytes()
        .try_into()
        .expect("generated keys carry no NUL");
    m.insert(key, v);
}

fn build(shape: &str, n: u64) -> Tree {
    let mut rng = SplitMix(0x0DDB_1A5E_5EED_0001);
    match shape {
        "sequential" => {
            let mut m = ExpanseMap::new();
            for i in 0..n {
                m.insert(i, i);
            }
            Tree::U64(m)
        }
        // Microsecond timestamps from a 2026 epoch, one event per 1..2,000 us.
        "timestamp" => {
            let mut m = ExpanseMap::new();
            let mut t: u64 = 1_780_000_000_000_000;
            for i in 0..n {
                t += 1 + rng.next() % 2_000;
                m.insert(t, i);
            }
            Tree::U64(m)
        }
        "random" => {
            let mut m = ExpanseMap::new();
            for i in 0..n {
                m.insert(rng.next(), i);
            }
            Tree::U64(m)
        }
        // `tNNN:orders:<i>`, the downstream report's prefix shape.
        "prefix" => {
            let mut m = ExpanseStrMap::new();
            let mut buf = String::with_capacity(48);
            for i in 0..n {
                buf.clear();
                write!(buf, "t{:03}:orders:{}", rng.next() % 1_000, i).expect("String write");
                insert_str(&mut m, &buf, i);
            }
            Tree::Str(m)
        }
        // 36-byte UUIDv4 text.
        "uuid" => {
            let mut m = ExpanseStrMap::new();
            let mut buf = String::with_capacity(48);
            for i in 0..n {
                let (a, b) = (rng.next(), rng.next());
                buf.clear();
                write!(
                    buf,
                    "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
                    a >> 32,
                    (a >> 16) & 0xFFFF,
                    a & 0xFFF,
                    0x8000 | ((b >> 48) & 0x3FFF),
                    b & 0xFFFF_FFFF_FFFF
                )
                .expect("String write");
                insert_str(&mut m, &buf, i);
            }
            Tree::Str(m)
        }
        _ => unreachable!("unknown shape {shape}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: u64 = args
        .get(1)
        .map(|a| a.parse().expect("N must be an integer"))
        .unwrap_or(1_000_000);
    let Some(shape) = args.get(2) else {
        println!("allocator census, N = {n} keys per shape, 8-byte values (bytes/key)");
        println!(
            "{:<11} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>10} {:>9} {:>9}",
            "shape",
            "mem_used",
            "mem_held",
            "request",
            "usable",
            "+headers",
            "RSS",
            "trimmed",
            "allocs/key",
            "held/shr",
            "RSS/shr"
        );
        let mut detail = String::new();
        for shape in SHAPES {
            let out = std::process::Command::new(&args[0])
                .args([n.to_string(), shape.to_string()])
                .output()
                .expect("spawn the census child");
            assert!(
                out.status.success(),
                "census child for {shape} failed: {out:?}"
            );
            let text = String::from_utf8(out.stdout).expect("utf-8 child output");
            let (row, rest) = text.split_once('\n').expect("child prints a row first");
            println!("{row}");
            detail.push_str(rest);
        }
        print!("{detail}");
        println!(
            "\nmem_held: mem_used plus freed blocks and slab slack the tree keeps; request: sum of \
             live Layout sizes; usable: allocator usable size; +headers: usable + \
             {HEADER} B per live chunk (glibc 64-bit, derived); RSS / trimmed: resident delta \
             after the build, and after malloc_trim(0), one process per shape (Linux only); \
             held/shr, RSS/shr: mem_held and trimmed RSS after shrink_to_fit()."
        );
        return;
    };
    run_shape(shape, n);
}

const SHAPES: [&str; 5] = ["sequential", "timestamp", "random", "prefix", "uuid"];

/// Builds one shape and prints its row, then its size-class detail.
fn run_shape(shape: &str, n: u64) {
    let mut detail = String::new();
    {
        trim();
        HIST.lock().expect("histogram lock").clear();
        let rss0 = rss();
        TRACKING.store(true, Ordering::SeqCst);
        let mut tree = build(shape, n);
        TRACKING.store(false, Ordering::SeqCst);
        let rss1 = rss();
        trim();
        let rss2 = rss();
        let per = |b: usize| b as f64 / n as f64;
        let live = LIVE.load(Ordering::SeqCst);
        let req = REQUESTED.load(Ordering::SeqCst);
        let usable = USABLE.load(Ordering::SeqCst);
        let held = tree.mem_held();
        tree.shrink_to_fit();
        trim();
        let rss3 = rss();
        let held_after = tree.mem_held();
        let rss_col = |r: Option<usize>| match (rss0, r) {
            (Some(a), Some(b)) => format!("{:.2}", per(b.saturating_sub(a))),
            _ => "n/a".to_string(),
        };
        println!(
            "{:<11} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>9} {:>9} {:>10.4} {:>9.2} {:>9}",
            shape,
            per(tree.mem_used()),
            per(held),
            per(req),
            per(usable),
            per(usable + live * HEADER),
            rss_col(rss1),
            rss_col(rss2),
            live as f64 / n as f64,
            per(held_after),
            rss_col(rss3)
        );
        // The request sizes carrying the most allocator overhead.
        let hist = HIST.lock().expect("histogram lock");
        let mut rows: Vec<(isize, usize, usize, isize, isize)> = hist
            .iter()
            .filter(|(_, (c, _))| *c > 0)
            .map(|(&(sz, al), &(c, u))| (u + c * HEADER as isize - (sz as isize) * c, sz, al, c, u))
            .collect();
        drop(hist);
        rows.sort_unstable_by_key(|r| std::cmp::Reverse(r.0));
        let total_over = (usable + live * HEADER).saturating_sub(req).max(1) as f64;
        writeln!(
            detail,
            "\n{shape}: request sizes by overhead (usable + headers - requested)"
        )
        .expect("write");
        writeln!(
            detail,
            "  {:>7} {:>5} {:>11} {:>9} {:>11} {:>7}",
            "request", "align", "live", "usable/ea", "overhead/k", "share"
        )
        .expect("write");
        for (over, sz, al, c, u) in rows.into_iter().take(8) {
            writeln!(
                detail,
                "  {:>7} {:>5} {:>11} {:>9.1} {:>11.3} {:>6.1}%",
                sz,
                al,
                c,
                u as f64 / c as f64,
                over as f64 / n as f64,
                100.0 * over as f64 / total_over
            )
            .expect("write");
        }
        drop(tree);
    }
    print!("{detail}");
}
