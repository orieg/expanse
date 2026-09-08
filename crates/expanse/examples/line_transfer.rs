//! Cross-core cache-line transfer cost, one core pair at a time (#568).
//!
//! Two threads, each pinned to one named CPU, hand a single cache line
//! back and forth: A stores an odd turn number, B waits for it and stores
//! the even reply, A waits for the reply. Every hand-over moves the line
//! from one core's cache to the other's, so `elapsed / (2 × round trips)`
//! is the cost of one one-way transfer between that pair. Two waiting
//! modes: `spin` (the waiter polls, as an OCC reader does on an odd
//! version) and `park` (the waiter sleeps in the kernel and is unparked —
//! the wake path a mutex waiter takes). A third mode, `pause`, times a
//! single thread's `spin_loop()` iterations on `cpu_a`, so a spin *count*
//! from `occ_stats` (`sample_spins`) converts to time on this host.
//!
//! The number is a host property, not an engine result: it is the
//! `t_line` term in the contention bound `scripts/olc_bounds.py` evaluates
//! before any multi-writer code is written (#568 PR 4), and it is what
//! separates "a line moved" from "a thread was woken" in the writer's
//! per-insert cost. `scripts/line_transfer_matrix.py` runs every unordered
//! pair of physical performance cores and records the matrix with
//! provenance; this binary is one cell of it.
//!
//! Linux only: it sets its own affinity with `sched_setaffinity`, which is
//! why the driver is exempt from the suite pin (it *is* a pin).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `line_transfer_pair` |
//! | `group` | 2 |
//! | `population` | one 128-byte-aligned `AtomicU64`; no keys |
//! | `insertion_order` | n/a — no data structure is built |
//! | `probes_and_reuse` | `round_trips` (argument, default 200,000) hand-overs per direction, after a 10% warm-up of the same shape |
//! | `hit_rate` | n/a |
//! | `miss_gen_method` | n/a |
//! | `value_dereference` | the turn number is loaded with `Acquire` and compared on every wait; the final value is `black_box`ed |
//! | `measured_region` | from both threads passing a barrier to A observing the last reply; thread spawn, affinity calls and warm-up are outside |
//! | `arm_symmetry` | both threads run the same loop with roles swapped; `spin` and `park` differ only in how the waiter waits; `pause` has one thread and no transfer; the pair (a, b) is measured once per invocation and the driver covers every unordered pair of physical P-cores |
//! | `statistics` | one point per invocation; the driver repeats and takes BCa 95% intervals (§8.4) |
//! | `verdict` | pending measurement |

#![allow(missing_docs)]

use std::hint::black_box;
use std::sync::Barrier;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// One cache line (two on hosts with 128-byte adjacent-line prefetch).
#[repr(align(128))]
struct Line(AtomicU64);

#[cfg(target_os = "linux")]
fn pin_to(cpu: usize) {
    // SAFETY: `cpu_set_t` is plain data; zeroed is a valid empty set, and
    // the libc macros only touch bits inside it. `sched_setaffinity(0, ..)`
    // acts on the calling thread.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut set);
        libc::CPU_SET(cpu, &mut set);
        let rc = libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set);
        assert!(
            rc == 0,
            "sched_setaffinity({cpu}) failed: {}",
            std::io::Error::last_os_error()
        );
    }
}

#[cfg(not(target_os = "linux"))]
fn pin_to(_cpu: usize) {
    eprintln!(
        "line_transfer sets thread affinity with sched_setaffinity; Linux only. No number was produced."
    );
    std::process::exit(2);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Spin,
    Park,
    /// One thread on `cpu_a`, no peer: `iterations` calls of `spin_loop()`.
    Pause,
}

fn usage() -> ! {
    eprintln!("usage: line_transfer <cpu_a> <cpu_b> <spin|park|pause> [round_trips]");
    eprintln!(
        "  pause: one thread on cpu_a, round_trips iterations of spin_loop(); cpu_b is ignored"
    );
    std::process::exit(2);
}

/// Waits until `line` holds `want`. `spin` polls; `park` sleeps and relies
/// on the peer's `unpark` (a parked thread may wake spuriously, hence the
/// re-check loop).
#[inline(always)]
fn wait_for(line: &Line, want: u64, mode: Mode) {
    while line.0.load(Ordering::Acquire) != want {
        match mode {
            Mode::Spin => std::hint::spin_loop(),
            Mode::Park => std::thread::park(),
            Mode::Pause => unreachable!("pause mode never waits on the line"),
        }
    }
}

/// `trips` round trips between the two pinned threads; returns the
/// elapsed seconds of the measured region.
fn run(cpu_a: usize, cpu_b: usize, mode: Mode, trips: u64) -> f64 {
    let line = Line(AtomicU64::new(0));
    let start = Barrier::new(2);
    // The caller's thread is A; B needs A's handle to unpark it in `park`
    // mode, and A needs B's once it is spawned. `unpark` on a thread that
    // is not parked leaves a token, so the order of store and unpark is
    // free of lost wake-ups.
    let a_handle = std::thread::current();
    std::thread::scope(|sc| {
        let b = std::thread::Builder::new()
            .name(format!("peer-{cpu_b}"))
            .spawn_scoped(sc, || {
                pin_to(cpu_b);
                start.wait();
                for i in 0..trips {
                    wait_for(&line, 2 * i + 1, mode);
                    line.0.store(2 * i + 2, Ordering::Release);
                    if mode == Mode::Park {
                        a_handle.unpark();
                    }
                }
            })
            .expect("spawn peer thread");
        let b_handle = b.thread().clone();
        pin_to(cpu_a);
        start.wait();
        let t0 = Instant::now();
        for i in 0..trips {
            line.0.store(2 * i + 1, Ordering::Release);
            if mode == Mode::Park {
                b_handle.unpark();
            }
            wait_for(&line, 2 * i + 2, mode);
        }
        let secs = t0.elapsed().as_secs_f64();
        black_box(line.0.load(Ordering::Relaxed));
        b.join().expect("peer thread panicked");
        secs
    })
}

/// `n` iterations of `spin_loop()` on the current thread, with a load on a
/// private line in each so the loop has the shape of the reader's wait.
fn pause_iterations(n: u64) -> f64 {
    let line = Line(AtomicU64::new(1));
    let t0 = Instant::now();
    let mut i = 0u64;
    while i < n {
        if line.0.load(Ordering::Acquire) == 0 {
            break;
        }
        std::hint::spin_loop();
        i += 1;
    }
    black_box(i);
    t0.elapsed().as_secs_f64()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 || a.len() > 5 {
        usage();
    }
    let cpu_a: usize = a[1].parse().unwrap_or_else(|_| usage());
    let cpu_b: usize = a[2].parse().unwrap_or_else(|_| usage());
    let mode = match a[3].as_str() {
        "spin" => Mode::Spin,
        "park" => Mode::Park,
        "pause" => Mode::Pause,
        _ => usage(),
    };
    let trips: u64 = a
        .get(4)
        .map_or(200_000, |s| s.parse().unwrap_or_else(|_| usage()));
    assert!(trips > 0);
    if mode == Mode::Pause {
        pin_to(cpu_a);
        let _ = pause_iterations((trips / 10).max(1_000));
        let secs = pause_iterations(trips);
        println!(
            "{{\"workload_id\":\"line_transfer_pair\",\"cpu_a\":{cpu_a},\"cpu_b\":null,\
             \"mode\":\"pause\",\"round_trips\":{trips},\"elapsed_s\":{secs:.6},\
             \"ns_per_transfer\":{:.2}}}",
            secs * 1e9 / trips as f64
        );
        return;
    }
    assert!(cpu_a != cpu_b, "a pair needs two different CPUs");
    // Warm-up of the same shape, discarded: first-touch of the line, the
    // barrier, and (for `park`) the first futex wake are not the transfer.
    let _ = run(cpu_a, cpu_b, mode, (trips / 10).max(1_000));
    let secs = run(cpu_a, cpu_b, mode, trips);
    let ns_per_transfer = secs * 1e9 / (2.0 * trips as f64);
    println!(
        "{{\"workload_id\":\"line_transfer_pair\",\"cpu_a\":{cpu_a},\"cpu_b\":{cpu_b},\
         \"mode\":\"{}\",\"round_trips\":{trips},\"elapsed_s\":{secs:.6},\
         \"ns_per_transfer\":{ns_per_transfer:.2}}}",
        match mode {
            Mode::Spin => "spin",
            Mode::Park => "park",
            Mode::Pause => unreachable!("pause returned above"),
        }
    );
}
