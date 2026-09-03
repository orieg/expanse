//! Prints what an AVX-512 kernel would see at runtime — nothing else.
//!
//! Companion to `popcnt_probe.rs`, which asked the same question of `popcnt`
//! (issue #1): does CPUID detection survive Valgrind? For AVX-512 the answer is
//! no, and it fails in two different ways depending on how the kernel is
//! reached. Both matter to this repo, because Callgrind is the primary
//! regression instrument and Valgrind implements no AVX-512 at all (KDE bug
//! 383010, opened 2017, enabling branch put on hold 2019).
//!
//! 1. **Runtime-dispatched** (`is_x86_feature_detected!`): Valgrind masks the
//!    `avx512*` CPUID bits, so dispatch silently selects the scalar fallback.
//!    The `instruction-counts` job would report an AVX-512 kernel as "no
//!    change" — a degradation that renders as success (AGENTS.md §8.1).
//! 2. **Unconditional** (a `-C target-cpu=x86-64-v4` build): the EVEX prefix is
//!    not decoded, and Valgrind raises SIGILL.
//!
//! Run bare for (1), which is safe everywhere. Pass `--force` for (2), which is
//! *expected* to die under Valgrind and is therefore never wired into CI:
//!
//! ```text
//! cargo run --example avx512_probe                    # detection only
//! valgrind --tool=callgrind cargo-built-binary         # shows the masking
//! cargo run --example avx512_probe -- --force          # SIGILLs under Valgrind
//! ```
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_avx512_probe` |
//! | `group` | 5 |
//! | `population` | 1 probe |
//! | `probes_and_reuse` | Single CPUID query, optional single instruction |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | CPUID probe |
//! | `measured_region` | Clean — untimed |
//! | `arm_symmetry` | Diagnostic check |
//! | `statistics` | Boolean status |
//! | `verdict` | **PASS** `[verified: RUN (Zen 5 AVX-512 host, native and under Callgrind)]`: CPUID masking and SIGILL both reproduced. |

fn main() {
    #[cfg(target_arch = "x86_64")]
    {
        let feats = [
            ("avx2", std::arch::is_x86_feature_detected!("avx2")),
            ("avx512f", std::arch::is_x86_feature_detected!("avx512f")),
            ("avx512vl", std::arch::is_x86_feature_detected!("avx512vl")),
            ("avx512bw", std::arch::is_x86_feature_detected!("avx512bw")),
            ("avx512dq", std::arch::is_x86_feature_detected!("avx512dq")),
            (
                "avx512vpopcntdq",
                std::arch::is_x86_feature_detected!("avx512vpopcntdq"),
            ),
        ];
        for (name, present) in feats {
            println!("detect {name:<18} {present}");
        }
        let dispatched = if feats[5].1 { "AVX512" } else { "scalar-fallback" };
        println!("dispatch_taken     {dispatched}");

        let forced = std::env::args().any(|a| a == "--force");
        #[cfg(feature = "avx512")]
        if forced {
            println!("forcing unconditional AVX-512 execution (expect SIGILL under Valgrind) ...");
            // SAFETY: gated on the caller passing `--force`, which documents that
            // they accept a SIGILL on a host or emulator without AVX-512. On real
            // AVX-512 hardware this is a plain `vpopcntq` on a constant.
            let r = unsafe { force_avx512() };
            println!("forced_avx512_ok   {r}");
        }
        if !forced {
            println!("forced_avx512      skipped (pass --force to attempt it)");
        } else if !cfg!(feature = "avx512") {
            // §8.1: --force asked for something this build cannot do. Say so.
            println!(
                "forced_avx512      UNAVAILABLE — built without `--features avx512`, \
                 so this binary contains no AVX-512 instruction to attempt"
            );
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    println!("detect avx512*            n/a (not x86_64)");
}

/// Executes `vpopcntq` on a `zmm` register regardless of what CPUID reported.
///
/// # Safety
///
/// The caller must accept that this traps with SIGILL on any host or emulator
/// without `avx512f` + `avx512vpopcntdq` — including every Valgrind tool.
// `incompatible_msrv`: AVX-512 intrinsics are stable only since Rust 1.89 and the
// crate floor is 1.88. That floor is preserved because this whole item is behind
// the off-by-default `avx512` feature, so no default build — and in particular
// not the `Core / MSRV 1.88 Build` job's `cargo check --workspace --all-targets`
// — ever compiles it.
#[cfg(all(target_arch = "x86_64", feature = "avx512"))]
#[allow(clippy::incompatible_msrv)]
#[target_feature(enable = "avx512f,avx512vpopcntdq")]
unsafe fn force_avx512() -> u64 {
    use core::arch::x86_64::*;
    use std::hint::black_box;
    // Operates entirely on register state built from a constant and touches no
    // memory, so the body holds no unsafe operation; legality of *reaching* it is
    // the caller's precondition, documented above.
    let v = _mm512_set1_epi64(black_box(0x0F0F_0F0F_0F0F_0F0Fu64 as i64));
    _mm512_reduce_add_epi64(_mm512_popcnt_epi64(v)) as u64
}
