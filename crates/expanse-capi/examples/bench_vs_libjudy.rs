//! Head-to-head comparison of libexpanse against a dlopen'd **stock
//! libjudy** through the identical C surface, per `docs/BENCHMARKING.md`:
//! interleaved A/B arms alternated per round so machine drift hits both
//! sides and cancels in the paired ratio (the php-judy issue-#87 lesson).
//!
//! Reports per distribution × population: insert build time, random `get`
//! probe time, and `MemUsed` bytes/key (the memory numbers are allocator
//! accounting — deterministic; the timing numbers are indicative unless
//! run on a quiet host under the system-load protocol).
//!
//! Run (needs stock libjudy: `libjudy-dev` on Linux, `brew install judy`
//! on macOS):
//! `cargo run --release -p expanse-capi --example bench_vs_libjudy`

#[cfg(not(unix))]
fn main() {
    eprintln!("bench_vs_libjudy needs a dlopen platform (Linux/macOS) with stock libjudy");
}

#[cfg(unix)]
use core::ffi::{c_int, c_void};
#[cfg(unix)]
use core::ptr::null_mut;
#[cfg(unix)]
use std::time::Instant;

#[cfg(unix)]
use expanse as ours;

#[cfg(unix)]
type Word = usize;
#[cfg(unix)]
type PErr = *mut ours::JError;
#[cfg(unix)]
type FIns = unsafe extern "C" fn(*mut *mut c_void, Word, PErr) -> *mut c_void;
#[cfg(unix)]
type FGet = unsafe extern "C" fn(*const c_void, Word, PErr) -> *mut c_void;
#[cfg(unix)]
type FFree = unsafe extern "C" fn(*mut *mut c_void, PErr) -> Word;
#[cfg(unix)]
type FMem = unsafe extern "C" fn(*const c_void) -> Word;

#[cfg(unix)]
unsafe extern "C" {
    fn dlopen(filename: *const u8, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
}

#[cfg(unix)]
struct Stock {
    ins: FIns,
    get: FGet,
    free: FFree,
    mem: FMem,
}

#[cfg(unix)]
impl Stock {
    fn load() -> Self {
        let names: [&core::ffi::CStr; 4] = [
            c"libJudy.so.1",
            c"libJudy.so",
            c"/opt/homebrew/opt/judy/lib/libJudy.dylib",
            c"libJudy.dylib",
        ];
        let mut handle = null_mut();
        for n in names {
            // SAFETY: valid NUL-terminated names.
            handle = unsafe { dlopen(n.to_bytes_with_nul().as_ptr(), 2) };
            if !handle.is_null() {
                break;
            }
        }
        assert!(!handle.is_null(), "stock libjudy not found");
        let sym = |name: &core::ffi::CStr| {
            // SAFETY: valid handle and name; caller types match the ABI.
            let p = unsafe { dlsym(handle, name.to_bytes_with_nul().as_ptr()) };
            assert!(!p.is_null(), "missing {name:?}");
            p
        };
        // SAFETY: resolved symbols transmuted to their C signatures.
        unsafe {
            Self {
                ins: core::mem::transmute::<*mut c_void, FIns>(sym(c"JudyLIns")),
                get: core::mem::transmute::<*mut c_void, FGet>(sym(c"JudyLGet")),
                free: core::mem::transmute::<*mut c_void, FFree>(sym(c"JudyLFreeArray")),
                mem: core::mem::transmute::<*mut c_void, FMem>(sym(c"JudyLMemUsed")),
            }
        }
    }
}

#[cfg(unix)]
struct XorShift(u64);
#[cfg(unix)]
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

#[cfg(unix)]
fn keys(dist: &str, n: usize) -> Vec<Word> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(n);
    match dist {
        "sequential" => out.extend((0..n as u64).map(|k| k as Word)),
        "random" => out.extend((0..n).map(|_| rng.next() as Word)),
        "clustered" => {
            let mut base = 0u64;
            for i in 0..n as u64 {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                out.push((base + (i % 256)) as Word);
            }
        }
        _ => unreachable!(),
    }
    out
}

#[cfg(unix)]
struct Arm {
    ins: FIns,
    get: FGet,
    free: FFree,
    mem: FMem,
}

#[cfg(unix)]
/// One measured pass: build, probe, memory, teardown.
fn run_arm(arm: &Arm, ks: &[Word], probes: &[Word]) -> (f64, f64, f64) {
    let mut a: *mut c_void = null_mut();
    // SAFETY: both arms driven strictly per the C contract.
    unsafe {
        let t0 = Instant::now();
        for &k in ks {
            let slot = (arm.ins)(&raw mut a, k, null_mut()).cast::<Word>();
            slot.write(!k);
        }
        let build_ns = t0.elapsed().as_nanos() as f64 / ks.len() as f64;

        let t0 = Instant::now();
        let mut acc = 0usize;
        for _ in 0..8 {
            for &p in probes {
                let slot = (arm.get)(a, p, null_mut());
                acc = acc.wrapping_add(slot as usize);
            }
        }
        std::hint::black_box(acc);
        let get_ns = t0.elapsed().as_nanos() as f64 / (8 * probes.len()) as f64;

        let bytes_per_key = (arm.mem)(a) as f64 / ks.len() as f64;
        (arm.free)(&raw mut a, null_mut());
        (build_ns, get_ns, bytes_per_key)
    }
}

#[cfg(unix)]
fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

#[cfg(unix)]
fn main() {
    let stock = Stock::load();
    let ours_arm = Arm {
        ins: ours::JudyLIns,
        get: ours::JudyLGet,
        free: ours::JudyLFreeArray,
        mem: ours::JudyLMemUsed,
    };
    let stock_arm = Arm {
        ins: stock.ins,
        get: stock.get,
        free: stock.free,
        mem: stock.mem,
    };

    const ROUNDS: usize = 5;
    println!(
        "libexpanse vs stock libjudy (JudyL surface), interleaved A/B, median of {ROUNDS} rounds"
    );
    println!("ratio < 1.0 means libexpanse is faster / smaller\n");
    println!(
        "{:<11} {:>9} | {:>9} {:>9} {:>6} | {:>8} {:>8} {:>6} | {:>7} {:>7} {:>6}",
        "dist",
        "pop",
        "ins ns/o",
        "ins ns/j",
        "ratio",
        "get ns/o",
        "get ns/j",
        "ratio",
        "B/k o",
        "B/k j",
        "ratio"
    );
    for dist in ["sequential", "random", "clustered"] {
        for pop in [100_000usize, 1_000_000] {
            let ks = keys(dist, pop);
            let mut rng = XorShift(0xBEEF_CAFE_1234_5678);
            let probes: Vec<Word> = (0..4096)
                .map(|_| ks[(rng.next() as usize) % ks.len()])
                .collect();
            let (mut ob, mut og, mut om) = (vec![], vec![], vec![]);
            let (mut sb, mut sg, mut sm) = (vec![], vec![], vec![]);
            for round in 0..ROUNDS {
                // Alternate arm order per round so drift cancels.
                let order: [(&Arm, bool); 2] = if round % 2 == 0 {
                    [(&ours_arm, true), (&stock_arm, false)]
                } else {
                    [(&stock_arm, false), (&ours_arm, true)]
                };
                for (arm, is_ours) in order {
                    let (b, g, m) = run_arm(arm, &ks, &probes);
                    if is_ours {
                        ob.push(b);
                        og.push(g);
                        om.push(m);
                    } else {
                        sb.push(b);
                        sg.push(g);
                        sm.push(m);
                    }
                }
            }
            let (ob, og, om) = (median(ob), median(og), median(om));
            let (sb, sg, sm) = (median(sb), median(sg), median(sm));
            println!(
                "{:<11} {:>9} | {:>9.1} {:>9.1} {:>6.2} | {:>8.1} {:>8.1} {:>6.2} | {:>7.2} {:>7.2} {:>6.2}",
                dist,
                pop,
                ob,
                sb,
                ob / sb,
                og,
                sg,
                og / sg,
                om,
                sm,
                om / sm
            );
        }
    }
    println!("\n(o = ours/libexpanse, j = stock libjudy; timing indicative unless quiet-host)");
}
