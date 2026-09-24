//! Memory left behind after an `ExpanseStrMap` is drained group by group.
//!
//! Reproduces a downstream pattern: `G` groups of `R` 17-byte keys (a 5-byte
//! group prefix, `t/`, a 10-byte id; both numbers in a 7-bit big-endian
//! encoding offset by one, so every byte is in `0x01..=0x80`). For each group
//! in ascending order the group's keys are scanned in order, optionally
//! copied into a `Vec`, and removed in ascending order. Beside the string map
//! an `ExpanseMap` may hold one entry per group, rewritten in place while the
//! groups are drained.
//!
//! Four variants separate the candidate causes of the retained memory:
//!
//! | variant | second map | scan buffers |
//! |---|---|---|
//! | `alone` | no | no |
//! | `alone+scan` | no | yes |
//! | `both` | yes | no |
//! | `both+scan` | yes | yes |
//!
//! Each variant runs in a child process of its own, since resident memory is
//! a per-process quantity. Columns, all read after the drain:
//!
//! - `live`: bytes live through the global allocator, from a counting
//!   `#[global_allocator]` (requested sizes);
//! - `used` / `held`: the string map's `mem_used()` / `mem_held()`;
//! - `inuse` / `free`: glibc `mallinfo2()` `uordblks` / `fordblks` (glibc only);
//! - `RSS`: resident delta over the reading before the first insert, after
//!   `malloc_trim(0)` (Linux only).
//!
//! Every figure is also printed at four points: all keys present, drained,
//! drained then `shrink_to_fit()`, and the string map dropped. All are bytes
//! per group.
//!
//! Run: `cargo run --release -p expanse-trie --example strmap_drain_retention -- [G] [R] [ids]`
//! where `ids` is `seq` (ascending within and across groups, the default) or
//! `random` (drawn per key, then sorted within the group).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_strmap_drain_retention` |
//! | `group` | 5 |
//! | `population` | G × R keys (default G = 1,000, R = 1,000) |
//! | `insertion_order` | sorted — groups ascending, keys ascending within a group |
//! | `probes_and_reuse` | N/A (Memory) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | `mem_used()` / `mem_held()` beside counted live heap bytes, `mallinfo2` and RSS |
//! | `measured_region` | Clean: one variant per process; readings after `malloc_trim(0)` |
//! | `arm_symmetry` | Same keys, values and order in every variant; only the second map and the scan buffers differ |
//! | `statistics` | Exact byte counts; RSS is a single reading per point |
//! | `verdict` | Diagnostic census; no gate. |

use expanse_trie::map::ExpanseMap;
use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

static LIVE: AtomicIsize = AtomicIsize::new(0);

struct Counting;

// SAFETY: every method forwards to `System` unchanged; the bookkeeping is one
// atomic add and never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size() as isize, Ordering::Relaxed);
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarded with the caller's arguments.
        let q = unsafe { System.realloc(p, l, new_size) };
        if !q.is_null() {
            LIVE.fetch_add(new_size as isize - l.size() as isize, Ordering::Relaxed);
        }
        q
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Resident bytes, or `None` where `/proc/self/statm` does not exist.
fn rss() -> Option<usize> {
    let s = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: usize = s.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4096)
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn trim() {
    // SAFETY: plain libc call with no pointer arguments.
    let _released = unsafe { libc::malloc_trim(0) };
}
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn trim() {}

/// glibc `(uordblks, fordblks)`: bytes in in-use chunks, bytes in free chunks.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn mallinfo() -> Option<(usize, usize)> {
    // SAFETY: plain libc call returning a struct by value.
    let m = unsafe { libc::mallinfo2() };
    Some((m.uordblks, m.fordblks))
}
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn mallinfo() -> Option<(usize, usize)> {
    None
}

/// `n` in `width` bytes, 7 bits per byte, big-endian, each byte offset by one.
fn enc7(out: &mut Vec<u8>, n: u64, width: u32) {
    for i in (0..width).rev() {
        out.push(((n >> (7 * i)) & 0x7F) as u8 + 1);
    }
}

fn key(buf: &mut Vec<u8>, group: u64, id: u64) {
    buf.clear();
    enc7(buf, group, 5);
    buf.extend_from_slice(b"t/");
    enc7(buf, id, 10);
}

fn nul_free(b: &[u8]) -> &NulFreeStr {
    NulFreeStr::new(b).expect("encoded keys carry no NUL")
}

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

/// The ids of every group, sorted within the group.
fn ids(g: u64, r: u64, random: bool) -> Vec<Vec<u64>> {
    let mut rng = SplitMix(0x5EED_D0A1_4E7E_0001);
    (0..g)
        .map(|gi| {
            let mut v: Vec<u64> = (0..r)
                .map(|ri| {
                    if random {
                        // 70 bits of encoding; stay below 2^63.
                        rng.next() >> 1
                    } else {
                        gi * r + ri
                    }
                })
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Reading {
    live: isize,
    used: usize,
    held: usize,
    inuse: Option<usize>,
    free: Option<usize>,
    rss: Option<usize>,
}

fn read(m: Option<&ExpanseStrMap>) -> Reading {
    trim();
    let mi = mallinfo();
    Reading {
        live: LIVE.load(Ordering::SeqCst),
        used: m.map_or(0, ExpanseStrMap::mem_used),
        held: m.map_or(0, ExpanseStrMap::mem_held),
        inuse: mi.map(|x| x.0),
        free: mi.map(|x| x.1),
        rss: rss(),
    }
}

fn run_variant(variant: &str, g: u64, r: u64, random: bool) {
    let (second, scan) = match variant {
        "alone" => (false, false),
        "alone+scan" => (false, true),
        "both" => (true, false),
        "both+scan" => (true, true),
        other => panic!("unknown variant {other}"),
    };
    let all_ids = ids(g, r, random);
    let base = read(None);

    let mut sm = ExpanseStrMap::new();
    let mut groups = second.then(ExpanseMap::new);
    let mut buf = Vec::with_capacity(17);
    for (gi, gids) in all_ids.iter().enumerate() {
        if let Some(m) = groups.as_mut() {
            m.insert(gi as u64, 0);
        }
        for &id in gids {
            key(&mut buf, gi as u64, id);
            sm.insert(nul_free(&buf), id);
        }
    }
    let full = read(Some(&sm));

    for (gi, gids) in all_ids.iter().enumerate() {
        let mut prefix = Vec::with_capacity(7);
        enc7(&mut prefix, gi as u64, 5);
        prefix.extend_from_slice(b"t/");
        if scan {
            let mut copies: Vec<(Vec<u8>, u64)> = Vec::new();
            {
                let mut c = sm.cursor_at_or_after(nul_free(&prefix));
                while let Some((k, v)) = c.next() {
                    if !k.starts_with(&prefix) {
                        break;
                    }
                    // SAFETY: the slot is live until the next mutation, and
                    // none happens while the cursor is held.
                    copies.push((k.to_vec(), unsafe { *v.as_ptr() }));
                }
            }
            assert_eq!(copies.len(), gids.len(), "scan found the whole group");
            for (k, _) in &copies {
                assert!(sm.remove(nul_free(k)).is_some());
            }
        } else {
            let mut n = 0usize;
            {
                let mut c = sm.cursor_at_or_after(nul_free(&prefix));
                while let Some((k, _)) = c.next() {
                    if !k.starts_with(&prefix) {
                        break;
                    }
                    n += 1;
                }
            }
            assert_eq!(n, gids.len(), "scan found the whole group");
            for &id in gids {
                key(&mut buf, gi as u64, id);
                assert!(sm.remove(nul_free(&buf)).is_some());
            }
        }
        if let Some(m) = groups.as_mut() {
            m.insert(gi as u64, 1 + gi as u64);
        }
    }
    assert!(sm.is_empty());
    let drained = read(Some(&sm));
    let released = sm.shrink_to_fit();
    let shrunk = read(Some(&sm));
    drop(sm);
    let dropped = read(None);
    let second_held = groups.as_ref().map_or(0, ExpanseMap::mem_held);

    let per = |b: isize| b as f64 / g as f64;
    let opt = |a: Option<usize>, b: Option<usize>| match (a, b) {
        (Some(a), Some(b)) => format!("{:.1}", per(b as isize - a as isize)),
        _ => "n/a".into(),
    };
    for (point, x) in [
        ("full", full),
        ("drained", drained),
        ("shrunk", shrunk),
        ("dropped", dropped),
    ] {
        println!(
            "{:<11} {:<8} {:>10.1} {:>10.1} {:>10.1} {:>10} {:>10} {:>10}",
            variant,
            point,
            per(x.live - base.live),
            per(x.used as isize),
            per(x.held as isize),
            opt(base.inuse, x.inuse),
            opt(base.free, x.free),
            opt(base.rss, x.rss),
        );
    }
    println!(
        "{variant:<11} shrink_to_fit released {released} B; second map mem_held {second_held} B"
    );
}

const VARIANTS: [&str; 4] = ["alone", "alone+scan", "both", "both+scan"];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let g: u64 = args.get(1).map_or(1_000, |a| a.parse().expect("G"));
    let r: u64 = args.get(2).map_or(1_000, |a| a.parse().expect("R"));
    let mode = args.get(3).map_or("seq", String::as_str);
    assert!(matches!(mode, "seq" | "random"), "ids: seq or random");
    if let Some(v) = args.get(4) {
        run_variant(v, g, r, mode == "random");
        return;
    }
    println!(
        "G = {g} groups, R = {r} keys per group, ids = {mode}; bytes per group over the pre-insert reading"
    );
    println!(
        "{:<11} {:<8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "variant", "point", "live", "used", "held", "inuse", "free", "RSS"
    );
    for v in VARIANTS {
        let out = std::process::Command::new(&args[0])
            .args([
                g.to_string(),
                r.to_string(),
                mode.to_string(),
                v.to_string(),
            ])
            .output()
            .expect("spawn the variant child");
        assert!(out.status.success(), "variant {v} failed: {out:?}");
        print!("{}", String::from_utf8(out.stdout).expect("utf-8"));
    }
}
