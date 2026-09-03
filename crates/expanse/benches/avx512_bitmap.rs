//! AVX-512 `Bitmap256` cardinality kernel vs the shipped scalar kernel, swept
//! across cache residency.
//!
//! [`Bitmap256::count_and`] is the inner kernel of the `algebra.rs` intersection
//! walk behind the `search_boolean` suite: at a pair of aligned bitmap leaves it
//! is four `AND`s and four `popcnt`s. A 256-bit bitmap is one `ymm`, and two of
//! them are one `zmm`, so `avx512_vpopcntdq` can retire the whole cardinality in
//! a single `vpopcntq`. `docs/HARDWARE.md` §6 carried that as an open
//! "Moderate, benchmark-gated" opportunity; this harness is the benchmark.
//!
//! **Four arms, and the second one is why.**
//!
//! | Arm | What it is |
//! |---|---|
//! | `scalar_swar` | `count_and` reached from a feature-less caller — what `algebra.rs` ships today, so `count_ones` lowers to the ~12-instruction SWAR sequence on the baseline x86-64 target |
//! | `scalar_popcnt` | the same kernel reached from a `#[target_feature(enable = "popcnt")]` clone, the way `get::walk` reaches it — hardware `popcnt`, no new ISA requirement |
//! | `v256` | `vpand` + `vpopcntq` over one bitmap pair per `ymm` |
//! | `v512` | the same over two pairs per `zmm` |
//!
//! `scalar_popcnt` exists because without it the vector arms get credited with a
//! win that is really popcnt-vs-SWAR (§8.3: a baseline must be the production
//! configuration, not a strawman). The gap between the first two arms is a
//! portable finding in its own right, and unlike the vector arms it is
//! measurable by the existing Callgrind gate.
//!
//! **What this measures, and what it does not.** Every arm here walks a flat,
//! contiguous array of bitmap pairs. That is a *kernel ceiling*, not an engine
//! measurement: the real walk reaches its bitmap leaves by chasing `Edge`
//! pointers through a slab arena, so it pays load latency this harness does not.
//! The `dram_chased` regime is the closest analogue — a dependent random walk
//! over a DRAM-resident buffer — and it is deliberately the arm to read first.
//! No figure here licenses a claim about `intersection_len` throughput.
//!
//! **This suite can never run under Callgrind.** Valgrind implements no AVX-512
//! (KDE bug 383010, on hold since 2019). Under it, `is_x86_feature_detected!`
//! reports every `avx512*` feature as false, so a dispatched kernel silently
//! measures its scalar fallback, and an unconditional one dies with SIGILL on
//! the EVEX prefix. `examples/avx512_probe.rs` demonstrates both. That is why
//! this suite is declared `wallclock` in `.github/bench-suites.json`, gates
//! nothing, and is not reachable from any Callgrind lane.
//!
//! On a host without `avx512vpopcntdq` only the scalar arm runs, and the harness
//! says so on stderr rather than reporting a quietly shortened sweep (§8.1).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `avx512_bitmap_count_and` |
//! | `group` | 5 |
//! | `population` | 256 to 4,194,304 bitmap pairs (16 KiB to 256 MiB) |
//! | `probes_and_reuse` | Whole buffer traversed per iteration; buffer reused across arms |
//! | `hit_rate` | N/A (cardinality kernel, every pair is visited) |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | Every `count_and` result accumulated and `black_box`ed |
//! | `measured_region` | Clean — buffers and permutation built in setup, outside `iter` |
//! | `arm_symmetry` | All four arms read the identical buffers in the identical order, with unaligned loads and an accumulator reduced once outside the loop; `scalar_popcnt` is the production-configuration baseline the vector arms are rated against |
//! | `statistics` | Criterion sampling; BCa 95% intervals via `scripts/bench_baseline.py --harvest` |
//! | `verdict` | **REPORT-ONLY** `[verified: RUN (Zen 5 AVX-512 host)]`: ceiling probe; gates nothing. |

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use expanse_trie::bits::Bitmap256;
use std::hint::black_box;

/// One cache-residency regime: how many bitmap *pairs* the sweep walks.
///
/// A pair is two `Bitmap256` = 64 B of traffic, so `pairs * 64` is the working
/// set. Sized against the reference AVX-512 host (Zen 5: 48 KiB L1d, 1 MiB L2,
/// 32 MiB L3 per CCD); the point is the shape of the curve across the hierarchy,
/// not that each label lands exactly inside that level on every part.
struct Regime {
    name: &'static str,
    pairs: usize,
    /// Visit indices in a dependent pseudo-random order rather than linearly.
    chased: bool,
}

const REGIMES: &[Regime] = &[
    Regime { name: "l1", pairs: 256, chased: false },
    Regime { name: "l2", pairs: 8_192, chased: false },
    Regime { name: "l3", pairs: 262_144, chased: false },
    Regime { name: "dram", pairs: 4_194_304, chased: false },
    Regime { name: "dram_chased", pairs: 4_194_304, chased: true },
];

fn xorshift(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

/// Bitmap leaves exist because a subexpanse is *dense*, so the words are filled
/// from a PRNG (~50% set) rather than left sparse. Cardinality cost is
/// data-independent on all three arms, but the density keeps the fixture honest
/// about what a real leaf holds.
fn build(n: usize, seed: u64) -> Vec<Bitmap256> {
    let mut s = seed;
    (0..n)
        .map(|_| Bitmap256 {
            words: [
                xorshift(&mut s),
                xorshift(&mut s),
                xorshift(&mut s),
                xorshift(&mut s),
            ],
        })
        .collect()
}

/// A permutation with a single cycle, so the walk cannot be prefetched ahead:
/// each index is only known once the previous one has been loaded.
fn build_chase(n: usize, seed: u64) -> Vec<u32> {
    let mut order: Vec<u32> = (0..n as u32).collect();
    let mut s = seed;
    for i in (1..n).rev() {
        let j = (xorshift(&mut s) % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }
    // next[order[k]] = order[k+1] — one Hamiltonian cycle over the buffer.
    let mut next = vec![0u32; n];
    for k in 0..n {
        next[order[k] as usize] = order[(k + 1) % n];
    }
    next
}

/// The shipped kernel: `Bitmap256::count_and`, four `AND`s and four `popcnt`s.
fn scalar_and_count(a: &[Bitmap256], b: &[Bitmap256]) -> u64 {
    let mut acc = 0u64;
    for i in 0..a.len() {
        acc += a[i].count_and(&b[i]) as u64;
    }
    acc
}

fn scalar_and_count_chased(a: &[Bitmap256], b: &[Bitmap256], next: &[u32]) -> u64 {
    let (mut acc, mut i) = (0u64, 0usize);
    for _ in 0..a.len() {
        acc += a[i].count_and(&b[i]) as u64;
        i = next[i] as usize;
    }
    acc
}

/// The same kernel reached the way `get::walk` reaches it: from inside a
/// `#[target_feature(enable = "popcnt")]` clone, so the `#[inline(always)]`
/// `count_and` adopts the feature and its `count_ones` calls lower to hardware
/// `popcnt` instead of the ~12-instruction SWAR sequence the baseline x86-64
/// target otherwise emits (`bits.rs`, `popcnt_rt`).
///
/// `algebra.rs` has no such clone today — its `count_and` call site sits in a
/// feature-less function — so `scalar_swar` is what the intersection walk
/// actually ships and this arm is what it *could* ship, portably, with no new
/// ISA requirement. Keeping both is the point: without it, the vector arms
/// would be credited with a win that is really popcnt-vs-SWAR.
///
/// # Safety
///
/// Caller must have verified `popcnt`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
unsafe fn scalar_popcnt_and_count(a: &[Bitmap256], b: &[Bitmap256]) -> u64 {
    let mut acc = 0u64;
    for i in 0..a.len() {
        acc += a[i].count_and(&b[i]) as u64;
    }
    acc
}

/// # Safety
///
/// Caller must have verified `popcnt`; `next` must be a permutation of
/// `0..a.len()`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
unsafe fn scalar_popcnt_and_count_chased(a: &[Bitmap256], b: &[Bitmap256], next: &[u32]) -> u64 {
    let (mut acc, mut i) = (0u64, 0usize);
    for _ in 0..a.len() {
        acc += a[i].count_and(&b[i]) as u64;
        i = next[i] as usize;
    }
    acc
}

// `incompatible_msrv`: see `examples/avx512_probe.rs` — AVX-512 intrinsics are
// stable since 1.89, the crate floor is 1.88, and this module is behind the
// off-by-default `avx512` feature so no default build compiles it.
#[cfg(all(target_arch = "x86_64", feature = "avx512"))]
#[allow(clippy::incompatible_msrv)]
mod vector {
    use super::Bitmap256;

    /// One bitmap pair per `ymm`: `vpand` + `vpopcntq`, accumulated in-register
    /// and reduced once after the loop.
    ///
    /// # Safety
    ///
    /// Caller must have verified `avx2`, `avx512vl` and `avx512vpopcntdq`.
    /// `a` and `b` must be the same length.
    #[target_feature(enable = "avx2,avx512vl,avx512vpopcntdq")]
    pub unsafe fn v256_and_count(a: &[Bitmap256], b: &[Bitmap256]) -> u64 {
        use core::arch::x86_64::*;
        // SAFETY: `_mm256_setzero_si256` touches no memory; the loop below reads
        // exactly 32 B from each of `a[i]` and `b[i]`, which is `size_of::<Bitmap256>()`.
        // Loads are unaligned because arena-resident leaves carry no 32 B guarantee.
        unsafe {
            let mut acc = _mm256_setzero_si256();
            for i in 0..a.len() {
                let x = _mm256_loadu_si256(a[i].words.as_ptr().cast());
                let y = _mm256_loadu_si256(b[i].words.as_ptr().cast());
                acc = _mm256_add_epi64(acc, _mm256_popcnt_epi64(_mm256_and_si256(x, y)));
            }
            let mut out = [0u64; 4];
            _mm256_storeu_si256(out.as_mut_ptr().cast(), acc);
            out[0] + out[1] + out[2] + out[3]
        }
    }

    /// Two bitmap pairs per `zmm` — the full Zen 5 width. Only reachable on the
    /// contiguous regimes: the chased walk learns `i` one step at a time and
    /// cannot present two independent pairs to one vector.
    ///
    /// # Safety
    ///
    /// Caller must have verified `avx512f` and `avx512vpopcntdq`.
    /// `a` and `b` must be the same length.
    #[target_feature(enable = "avx512f,avx512vpopcntdq")]
    pub unsafe fn v512_and_count(a: &[Bitmap256], b: &[Bitmap256]) -> u64 {
        use core::arch::x86_64::*;
        // SAFETY: the loop reads 64 B per step from each slice, bounded by
        // `n = len & !1`, so the final read ends exactly at the slice end when
        // `len` is even and one element short of it when odd. The odd tail is
        // finished scalar-side below.
        unsafe {
            let mut acc = _mm512_setzero_si512();
            let n = a.len() & !1;
            let mut i = 0;
            while i < n {
                let x = _mm512_loadu_si512(a[i].words.as_ptr().cast());
                let y = _mm512_loadu_si512(b[i].words.as_ptr().cast());
                acc = _mm512_add_epi64(acc, _mm512_popcnt_epi64(_mm512_and_si512(x, y)));
                i += 2;
            }
            let mut tail = 0u64;
            for j in n..a.len() {
                tail += a[j].count_and(&b[j]) as u64;
            }
            _mm512_reduce_add_epi64(acc) as u64 + tail
        }
    }

    /// # Safety
    ///
    /// Caller must have verified `avx2`, `avx512vl` and `avx512vpopcntdq`.
    /// `next` must be a permutation of `0..a.len()`.
    #[target_feature(enable = "avx2,avx512vl,avx512vpopcntdq")]
    pub unsafe fn v256_and_count_chased(
        a: &[Bitmap256],
        b: &[Bitmap256],
        next: &[u32],
    ) -> u64 {
        use core::arch::x86_64::*;
        // SAFETY: `i` is always an in-bounds index because `next` is a
        // permutation of `0..a.len()`; each step reads 32 B from each slice.
        unsafe {
            let mut acc = _mm256_setzero_si256();
            let mut i = 0usize;
            for _ in 0..a.len() {
                let x = _mm256_loadu_si256(a[i].words.as_ptr().cast());
                let y = _mm256_loadu_si256(b[i].words.as_ptr().cast());
                acc = _mm256_add_epi64(acc, _mm256_popcnt_epi64(_mm256_and_si256(x, y)));
                i = next[i] as usize;
            }
            let mut out = [0u64; 4];
            _mm256_storeu_si256(out.as_mut_ptr().cast(), acc);
            out[0] + out[1] + out[2] + out[3]
        }
    }
}

/// Whether the AVX-512 arms can run at all on this host.
///
/// Answered once, printed once. Under Valgrind this is always false — see the
/// module header and `examples/avx512_probe.rs`.
fn avx512_available() -> bool {
    #[cfg(all(target_arch = "x86_64", feature = "avx512"))]
    {
        std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("avx512f")
            && std::arch::is_x86_feature_detected!("avx512vl")
            && std::arch::is_x86_feature_detected!("avx512vpopcntdq")
    }
    #[cfg(not(all(target_arch = "x86_64", feature = "avx512")))]
    {
        false
    }
}

/// Whether the vector arms were compiled in at all, as distinct from being
/// compiled in and then found unsupported at runtime. The two produce the same
/// shortened report and must not be reported as the same thing.
const ARMS_COMPILED: bool = cfg!(all(target_arch = "x86_64", feature = "avx512"));

fn bench(c: &mut Criterion) {
    let have = avx512_available();
    #[cfg(target_arch = "x86_64")]
    let have_popcnt = std::arch::is_x86_feature_detected!("popcnt");
    #[cfg(target_arch = "x86_64")]
    if !have_popcnt {
        eprintln!(
            "avx512_bitmap: ⚠️  no hardware `popcnt` on this host (or masked) — the \
             `scalar_popcnt` arm is ABSENT from this report, so `scalar_swar` is the \
             only scalar reference and any vector ratio against it conflates \
             popcnt-vs-SWAR with the vector win."
        );
    }
    if have {
        eprintln!("avx512_bitmap: avx512vpopcntdq present — scalar + v256 + v512 arms active");
    } else if ARMS_COMPILED {
        // §8.1: a shortened sweep must announce itself, not read as a full one.
        eprintln!(
            "avx512_bitmap: ⚠️  vector arms COMPILED IN but avx512vpopcntdq is not \
             available on this host (absent, or masked by Valgrind) — ONLY the \
             scalar arm ran. The vector arms are absent from this report, not \
             measured-as-equal."
        );
    } else {
        eprintln!(
            "avx512_bitmap: ⚠️  vector arms NOT COMPILED IN — build with \
             `--features avx512` to measure them (that raises the effective MSRV \
             to 1.89; see crates/expanse/Cargo.toml). ONLY the scalar arm ran."
        );
    }

    let mut group = c.benchmark_group("avx512_bitmap");
    // The big regimes stream 256 MiB per iteration; the default 100 samples
    // would run for minutes without narrowing the interval.
    group.sample_size(20);

    for r in REGIMES {
        let a = build(r.pairs, 0x1234_5678_9abc_def0);
        let b = build(r.pairs, 0x0fed_cba9_8765_4321);
        let next = if r.chased {
            build_chase(r.pairs, 0xdead_beef_cafe_f00d)
        } else {
            Vec::new()
        };

        // Parity: every active arm must agree with the shipped kernel before any
        // of them is timed (docs/TESTING.md SIMD/intrinsic parity rule).
        #[cfg(target_arch = "x86_64")]
        let expect = if r.chased {
            scalar_and_count_chased(&a, &b, &next)
        } else {
            scalar_and_count(&a, &b)
        };
        #[cfg(target_arch = "x86_64")]
        if have_popcnt {
            // SAFETY: `have_popcnt` verified the feature; `next` is a
            // permutation of `0..a.len()` by construction.
            let got = unsafe {
                if r.chased {
                    scalar_popcnt_and_count_chased(&a, &b, &next)
                } else {
                    scalar_popcnt_and_count(&a, &b)
                }
            };
            assert_eq!(expect, got, "scalar_popcnt disagrees with scalar_swar");
        }
        #[cfg(all(target_arch = "x86_64", feature = "avx512"))]
        if have {
            if r.chased {
                // SAFETY: `have` verified avx2 + avx512vl + avx512vpopcntdq above,
                // and `next` is a permutation of `0..a.len()` by construction.
                let got = unsafe { vector::v256_and_count_chased(&a, &b, &next) };
                assert_eq!(expect, got, "v256 chased disagrees with Bitmap256::count_and");
            } else {
                // SAFETY: features verified above; `a` and `b` have equal length.
                let got = unsafe { vector::v256_and_count(&a, &b) };
                assert_eq!(expect, got, "v256 disagrees with Bitmap256::count_and");
                // SAFETY: features verified above; `a` and `b` have equal length.
                let got = unsafe { vector::v512_and_count(&a, &b) };
                assert_eq!(expect, got, "v512 disagrees with Bitmap256::count_and");
            }
        }

        group.bench_with_input(BenchmarkId::new("scalar_swar", r.name), r, |bencher, r| {
            bencher.iter(|| {
                if r.chased {
                    black_box(scalar_and_count_chased(
                        black_box(&a),
                        black_box(&b),
                        black_box(&next),
                    ))
                } else {
                    black_box(scalar_and_count(black_box(&a), black_box(&b)))
                }
            });
        });

        #[cfg(target_arch = "x86_64")]
        if have_popcnt {
            group.bench_with_input(BenchmarkId::new("scalar_popcnt", r.name), r, |bencher, r| {
                bencher.iter(|| {
                    // SAFETY: `have_popcnt` verified the feature for this process.
                    unsafe {
                        if r.chased {
                            black_box(scalar_popcnt_and_count_chased(
                                black_box(&a),
                                black_box(&b),
                                black_box(&next),
                            ))
                        } else {
                            black_box(scalar_popcnt_and_count(black_box(&a), black_box(&b)))
                        }
                    }
                });
            });
        }

        #[cfg(all(target_arch = "x86_64", feature = "avx512"))]
        if have {
            group.bench_with_input(BenchmarkId::new("v256", r.name), r, |bencher, r| {
                bencher.iter(|| {
                    // SAFETY: `have` verified avx2 + avx512vl + avx512vpopcntdq
                    // for the lifetime of this process.
                    unsafe {
                        if r.chased {
                            black_box(vector::v256_and_count_chased(
                                black_box(&a),
                                black_box(&b),
                                black_box(&next),
                            ))
                        } else {
                            black_box(vector::v256_and_count(black_box(&a), black_box(&b)))
                        }
                    }
                });
            });

            // The chased walk is serially dependent: there is no second
            // independent pair to fill the upper half of the `zmm`, so a 512-bit
            // arm there would be the 256-bit arm with extra shuffling. Reporting
            // it would invite a comparison that the data shape forbids.
            if !r.chased {
                group.bench_with_input(BenchmarkId::new("v512", r.name), r, |bencher, _| {
                    bencher.iter(|| {
                        // SAFETY: `have` verified avx512f + avx512vpopcntdq.
                        unsafe { black_box(vector::v512_and_count(black_box(&a), black_box(&b))) }
                    });
                });
            }
        }
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
