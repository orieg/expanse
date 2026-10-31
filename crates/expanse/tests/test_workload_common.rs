//! Pins the key sequences of the shared bench and example workload helpers
//! (`benches/workload_common/mod.rs`).
//!
//! The benches and examples under `crates/expanse/{benches,examples}` each
//! carried a private copy of the same XorShift64 step, and the copies differ
//! in how they seed it: a raw seed, a zero guard with a per-file fallback, a
//! forced-odd seed, or an extra output mix. Every published figure those
//! harnesses produced was drawn from one of those seed paths, so sharing the
//! helpers must not move a single key. The pins below were taken from the
//! per-file copies before they were shared, and each case checks one seed
//! path of the shared module against them.
#![cfg(not(miri))]
#![allow(missing_docs)]

use std::collections::HashSet;

#[path = "../benches/workload_common/mod.rs"]
mod workload_common;

use workload_common::{
    KeyGen, XorShift, absent, keys, median, median_upper, miss_keys, miss_keys_from,
};

/// `benches/comparative.rs`'s u32 generator. It stays local to that bench
/// (its distributions are its own); it is repeated here so the shared
/// [`absent`] rejection loop is checked on the draw stream it serves there.
struct ComparativeKeyGen<'a> {
    dist: &'a str,
    rng: XorShift,
    i: u64,
    base: u32,
}

impl<'a> ComparativeKeyGen<'a> {
    fn with_offset(dist: &'a str, seed: u64, offset: u64) -> Self {
        Self {
            dist,
            rng: XorShift(seed),
            i: offset,
            base: 0,
        }
    }

    fn next(&mut self) -> u32 {
        let i = self.i;
        let k = match self.dist {
            "sparse" => (i as u32).wrapping_mul(1000),
            "clustered" => {
                if i.is_multiple_of(256) {
                    self.base = (self.rng.next() as u32) & !0xFF;
                }
                self.base + (i % 256) as u32
            }
            "dense" => i as u32,
            _ => unreachable!(),
        };
        self.i += 1;
        k
    }
}

const SEED: u64 = 0x0DDB_1A5E_5EED_0001;
const MISS_SEED: u64 = 0x51ED_0FF5_C0FF_EE01;
const DISTS: [&str; 4] = ["sequential", "random", "clustered", "sparse"];
/// The per-file zero-guard fallbacks: `hashbrown_container_dists`,
/// `hashbrown_memory_alloc`, `hashbrown_native_suite`,
/// `hashbrown_tail_latency`, `hashbrown_ycsb`, `bench_lookup_compare`.
const FALLBACKS: [u64; 6] = [
    0x89AB_CDEF_0123_4567,
    0xFEED_BEEF_1234_5678,
    0xDEAD_BEEF_CAFE_BABE,
    0x4D5E_6F70_8192_A3B4,
    0x1A2B_3C4D_5E6F_7081,
    0x5CA1_AB1E_0001,
];

// ---------------------------------------------------------------------------
// Pins, taken from the per-file copies before they were shared.
// ---------------------------------------------------------------------------

/// Raw seed: the first four draws from `SEED`.
const RAW: [u64; 4] = [
    0x8e0c_0201_0d92_fa41,
    0x79b8_d30a_eb95_6ff5,
    0x19d7_50aa_8633_186a,
    0xfafd_fd84_7540_24da,
];

/// Zero guard: the first two draws from seed 0, per fallback in `FALLBACKS`.
const GUARDED_ZERO: [[u64; 2]; 6] = [
    [0xf2d7_ff29_8b06_ba2d, 0x5a83_3622_f6c9_9c19],
    [0x3183_ab85_8a62_a0d4, 0xbd82_6775_ecee_d095],
    [0x27dc_5c1b_2d04_284b, 0xa296_3972_7aeb_52db],
    [0xbec9_bf23_87a9_ebf3, 0xa0ad_bb0d_5e6a_24e4],
    [0xd806_b090_ed5f_ae20, 0x06ca_5934_5c52_997c],
    [0x89a0_9e81_d74a_1c41, 0x1104_cef7_3599_b839],
];

/// Forced-odd seed: from an even seed, and from 0.
const ODD_EVEN_SEED: u64 = 0x1234_5678_9ABC_DEF0;
const ODD: [u64; 3] = [
    0xfe80_0d65_2978_3b0c,
    0x1ce7_8591_aab1_887a,
    0xdac1_32b5_7f5d_b5ea,
];
const ODD_ZERO: [u64; 2] = [0x4082_2041, 0x1000_4106_0c01_1441];

/// Unit-interval mix (as `f64` bits) and the truncating `u32` output, from `SEED`.
const UNIT_BITS: [u64; 3] = [
    0x3fe1_c180_4021_b25f,
    0x3fde_6e34_c2ba_e55a,
    0x3fb9_d750_aa86_3318,
];
const U32: [u32; 3] = [0x0d92_fa41, 0xeb95_6ff5, 0x8633_186a];

/// `KeyGen` (u64) per distribution in `DISTS`: keys `0..3` and `255..258` of a
/// 260-key population.
const KEYS_HEAD: [[u64; 3]; 4] = [
    [0, 1, 2],
    [
        0x8e0c_0201_0d92_fa41,
        0x79b8_d30a_eb95_6ff5,
        0x19d7_50aa_8633_186a,
    ],
    [
        0x8e0c_0201_0d92_fa00,
        0x8e0c_0201_0d92_fa01,
        0x8e0c_0201_0d92_fa02,
    ],
    [0, 1 << 40, 2 << 40],
];
const KEYS_255: [[u64; 3]; 4] = [
    [0xff, 0x100, 0x101],
    [
        0xe355_ad1e_b910_07c5,
        0x7bca_17fe_2d8f_768a,
        0x4a91_005a_4916_8ae7,
    ],
    [
        0x8e0c_0201_0d92_faff,
        0x79b8_d30a_eb95_6f00,
        0x79b8_d30a_eb95_6f01,
    ],
    [0xff << 40, 0x100 << 40, 0x101 << 40],
];
/// Four misses against the first 1,000 keys, from `MISS_SEED`.
const MISSES: [[u64; 4]; 4] = [
    [0x3e8, 0x3e9, 0x3ea, 0x3eb],
    [
        0xd0f8_420e_8a7f_b19d,
        0x19c7_1d0e_edc8_89be,
        0xe1ff_2b3b_fd5c_b72d,
        0xcce3_f2e5_8168_6503,
    ],
    [0xe8, 0xe9, 0xea, 0xeb],
    [0x3e8 << 40, 0x3e9 << 40, 0x3ea << 40, 0x3eb << 40],
];
/// The zero-guarded `KeyGen` from seed 0 (falls back to `0x5CA1_AB1E_0001`).
const GUARDED_KEYS_ZERO: [[u64; 3]; 4] = [
    [0, 1, 2],
    [
        0x89a0_9e81_d74a_1c41,
        0x1104_cef7_3599_b839,
        0xb1e2_8923_c089_a509,
    ],
    [
        0x89a0_9e81_d74a_1c00,
        0x89a0_9e81_d74a_1c01,
        0x89a0_9e81_d74a_1c02,
    ],
    [0, 1 << 40, 2 << 40],
];

/// `comparative.rs` (u32) per distribution in `CDISTS`: keys `0..3` and
/// `255..258`, then four misses against the first 1,000 keys.
const CDISTS: [&str; 3] = ["sparse", "clustered", "dense"];
const CKEYS: [[u32; 6]; 3] = [
    [0, 0x3e8, 0x7d0, 0x3e418, 0x3e800, 0x3ebe8],
    [
        0x0d92_fa00,
        0x0d92_fa01,
        0x0d92_fa02,
        0x0d92_faff,
        0xeb95_6f00,
        0xeb95_6f01,
    ],
    [0, 1, 2, 0xff, 0x100, 0x101],
];
const CMISSES: [[u32; 4]; 3] = [
    [0xf4240, 0xf4628, 0xf4a10, 0xf4df8],
    [0x8a7f_b100, 0x8a7f_b101, 0x8a7f_b102, 0x8a7f_b103],
    [0x3e8, 0x3e9, 0x3ea, 0x3eb],
];

/// `leaf_cap_cascaded{,_wallclock}.rs`: three 63-bit keys, then four misses
/// against the first 1,000.
const CASCADED_KEYS: [u64; 3] = [
    0x0e0c_0201_0d92_fa41,
    0x79b8_d30a_eb95_6ff5,
    0x19d7_50aa_8633_186a,
];
const CASCADED_MISSES: [u64; 4] = [
    0x50f8_420e_8a7f_b19d,
    0x19c7_1d0e_edc8_89be,
    0x61ff_2b3b_fd5c_b72d,
    0x4ce3_f2e5_8168_6503,
];

fn draws<T>(mut next: impl FnMut() -> T, n: usize) -> Vec<T> {
    (0..n).map(|_| next()).collect()
}

/// The three seed paths and the two output mixes.
#[test]
fn xorshift_seed_paths_match_pins() {
    let mut r = XorShift(SEED);
    assert_eq!(draws(|| r.next(), 4), RAW);
    let mut r = XorShift(0);
    assert_eq!(
        draws(|| r.next(), 2),
        [0, 0],
        "the raw path has no zero guard"
    );
    for (fb, want) in FALLBACKS.iter().zip(GUARDED_ZERO) {
        let mut g = XorShift::nonzero_or(0, *fb);
        assert_eq!(draws(|| g.next(), 2), want, "fallback {fb:#x}");
        let mut g = XorShift::nonzero_or(SEED, *fb);
        assert_eq!(
            draws(|| g.next(), 4),
            RAW,
            "a non-zero seed ignores {fb:#x}"
        );
    }
    let mut o = XorShift::odd(ODD_EVEN_SEED);
    assert_eq!(draws(|| o.next(), 3), ODD);
    let mut o = XorShift::odd(0);
    assert_eq!(draws(|| o.next(), 2), ODD_ZERO);
    let mut r = XorShift(SEED);
    assert_eq!(draws(|| r.unit().to_bits(), 3), UNIT_BITS);
    let mut r = XorShift(SEED);
    assert_eq!(draws(|| r.next_u32(), 3), U32);
}

/// `KeyGen` on the raw path (`compare.rs`) and on the zero-guarded path
/// (`bench_lookup_compare.rs`), and the misses each draws.
#[test]
fn keygen_and_misses_match_pins() {
    for (d, dist) in DISTS.iter().enumerate() {
        let k = keys(dist, 260);
        assert_eq!(k[..3], KEYS_HEAD[d], "{dist}");
        assert_eq!(k[255..258], KEYS_255[d], "{dist}");
        let mut g = KeyGen::from_rng(dist, XorShift::nonzero_or(SEED, 0x5CA1_AB1E_0001), 0);
        assert_eq!(
            draws(|| g.next(), 260),
            k,
            "{dist}: a non-zero seed takes the raw path"
        );
        let mut g = KeyGen::from_rng(dist, XorShift::nonzero_or(0, 0x5CA1_AB1E_0001), 0);
        assert_eq!(draws(|| g.next(), 3), GUARDED_KEYS_ZERO[d], "{dist}");
        let present: HashSet<u64> = keys(dist, 1000).into_iter().collect();
        assert_eq!(miss_keys(dist, &present, 4, MISS_SEED), MISSES[d], "{dist}");
        let mut g = KeyGen::from_rng(
            dist,
            XorShift::nonzero_or(MISS_SEED, 0x5CA1_AB1E_0001),
            present.len() as u64,
        );
        assert_eq!(miss_keys_from(&mut g, &present, 4), MISSES[d], "{dist}");
    }
}

/// The shared rejection loop on `comparative.rs`'s u32 stream and on the
/// 63-bit masked stream of `leaf_cap_cascaded{,_wallclock}.rs`.
#[test]
fn absent_matches_pins() {
    for (d, dist) in CDISTS.iter().enumerate() {
        let mut g = ComparativeKeyGen::with_offset(dist, SEED, 0);
        let k = draws(|| g.next(), 1000);
        assert_eq!([&k[..3], &k[255..258]].concat(), CKEYS[d], "{dist}");
        let present: HashSet<u32> = k.into_iter().collect();
        let offset = match *dist {
            "sparse" | "dense" => present.len() as u64,
            _ => 0,
        };
        let mut g = ComparativeKeyGen::with_offset(dist, MISS_SEED, offset);
        assert_eq!(
            absent(&present, 4, || g.next()),
            Some(CMISSES[d].to_vec()),
            "{dist}"
        );
    }
    const MASK: u64 = (1u64 << 63) - 1;
    let mut rng = XorShift(SEED);
    let k = draws(|| rng.next() & MASK, 1000);
    assert_eq!(k[..3], CASCADED_KEYS);
    let present: HashSet<u64> = k.into_iter().collect();
    let mut rng = XorShift(MISS_SEED);
    assert_eq!(
        absent(&present, 4, || rng.next() & MASK),
        Some(CASCADED_MISSES.to_vec())
    );
}

/// An exhausted budget is reported, never padded: a stream with one absent
/// value cannot yield two.
#[test]
fn absent_reports_an_exhausted_budget() {
    let present: HashSet<u64> = (0..10).collect();
    let mut i = 0u64;
    let got = absent(&present, 2, || {
        i += 1;
        i % 11
    });
    assert_eq!(got, None);
}

#[test]
fn medians_match_pins() {
    assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
    assert_eq!(median(vec![4.0, 1.0, 3.0, 2.0]), 2.5);
    assert_eq!(median(vec![]), 0.0);
    assert_eq!(median_upper(vec![3.0, 1.0, 2.0]), 2.0);
    assert_eq!(median_upper(vec![4.0, 1.0, 3.0, 2.0]), 3.0);
}
