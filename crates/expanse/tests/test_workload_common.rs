//! Pins the key sequences of the bench and example workload helpers.
//!
//! The benches and examples under `crates/expanse/{benches,examples}` each
//! carried a private copy of the same XorShift64 step, and the copies differ
//! in how they seed it: a raw seed, a zero guard with a per-file fallback, a
//! forced-odd seed, or an extra output mix. Every published figure those
//! harnesses produced was drawn from one of those seed paths, so sharing the
//! helpers must not move a single key. The pins below were taken from the
//! per-file copies before they were shared (`legacy` below), and each case
//! checks one seed path against them.
#![cfg(not(miri))]
#![allow(missing_docs)]

use std::collections::HashSet;

/// Verbatim copies of the per-file helpers as they stood before sharing.
mod legacy {
    /// `benches/compare.rs` (also the raw copy in most other harnesses).
    pub mod compare {
        use std::collections::HashSet;

        pub struct XorShift(pub u64);
        impl XorShift {
            pub fn next(&mut self) -> u64 {
                let mut x = self.0;
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                self.0 = x;
                x
            }
        }

        pub struct KeyGen<'a> {
            dist: &'a str,
            rng: XorShift,
            i: u64,
            base: u64,
        }

        impl<'a> KeyGen<'a> {
            pub fn new(dist: &'a str, seed: u64) -> Self {
                Self::with_offset(dist, seed, 0)
            }

            pub fn with_offset(dist: &'a str, seed: u64, offset: u64) -> Self {
                Self {
                    dist,
                    rng: XorShift(seed),
                    i: offset,
                    base: 0,
                }
            }

            pub fn next(&mut self) -> u64 {
                let k = match self.dist {
                    "sequential" => self.i,
                    "random" => self.rng.next(),
                    "clustered" => {
                        if self.i.is_multiple_of(256) {
                            self.base = self.rng.next() & !0xFF;
                        }
                        self.base + (self.i % 256)
                    }
                    "sparse" => self.i << 40,
                    _ => unreachable!(),
                };
                self.i += 1;
                k
            }
        }

        pub fn keys(dist: &str, n: usize) -> Vec<u64> {
            let mut g = KeyGen::new(dist, 0x0DDB_1A5E_5EED_0001);
            (0..n).map(|_| g.next()).collect()
        }

        pub fn generate_miss_keys(
            dist: &str,
            present: &HashSet<u64>,
            n: usize,
            seed: u64,
        ) -> Vec<u64> {
            let mut g = KeyGen::with_offset(dist, seed, present.len() as u64);
            let mut seen: HashSet<u64> = HashSet::with_capacity(n * 2);
            let mut out = Vec::with_capacity(n);
            let budget = n.saturating_mul(64).saturating_add(1024);
            for _ in 0..budget {
                if out.len() == n {
                    return out;
                }
                let c = g.next();
                if !present.contains(&c) && seen.insert(c) {
                    out.push(c);
                }
            }
            panic!("{dist}: could not draw {n} distinct absent keys within budget");
        }
    }

    /// `examples/bench_lookup_compare.rs`: zero-guarded seed.
    pub mod lookup_compare {
        use std::collections::HashSet;

        pub struct XorShift(u64);
        impl XorShift {
            pub fn new(seed: u64) -> Self {
                Self(if seed == 0 { 0x5CA1_AB1E_0001 } else { seed })
            }
            pub fn next(&mut self) -> u64 {
                let mut x = self.0;
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                self.0 = x;
                x
            }
        }

        pub struct KeyGen<'a> {
            dist: &'a str,
            rng: XorShift,
            i: u64,
            base: u64,
        }

        impl<'a> KeyGen<'a> {
            pub fn new(dist: &'a str, seed: u64) -> Self {
                Self::with_offset(dist, seed, 0)
            }

            pub fn with_offset(dist: &'a str, seed: u64, offset: u64) -> Self {
                Self {
                    dist,
                    rng: XorShift::new(seed),
                    i: offset,
                    base: 0,
                }
            }

            pub fn next(&mut self) -> u64 {
                let k = match self.dist {
                    "sequential" => self.i,
                    "random" => self.rng.next(),
                    "clustered" => {
                        if self.i.is_multiple_of(256) {
                            self.base = self.rng.next() & !0xFF;
                        }
                        self.base + (self.i % 256)
                    }
                    "sparse" => self.i << 40,
                    other => panic!("unknown distribution '{other}'"),
                };
                self.i += 1;
                k
            }
        }

        pub fn generate_keys(dist: &str, n: usize, seed: u64) -> Vec<u64> {
            let mut g = KeyGen::new(dist, seed);
            (0..n).map(|_| g.next()).collect()
        }

        pub fn generate_miss_keys(
            dist: &str,
            present: &HashSet<u64>,
            n: usize,
            seed: u64,
        ) -> Vec<u64> {
            let mut g = KeyGen::with_offset(dist, seed, present.len() as u64);
            let mut seen: HashSet<u64> = HashSet::with_capacity(n * 2);
            let mut out = Vec::with_capacity(n);
            let budget = n.saturating_mul(64).saturating_add(1024);
            for _ in 0..budget {
                if out.len() == n {
                    return out;
                }
                let c = g.next();
                if !present.contains(&c) && seen.insert(c) {
                    out.push(c);
                }
            }
            panic!("{dist}: could not draw {n} distinct absent keys within budget");
        }

        pub fn median(mut v: Vec<f64>) -> f64 {
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        }
    }

    /// `benches/comparative.rs`: u32 keys, its own distributions.
    pub mod comparative {
        use super::compare::XorShift;
        use std::collections::HashSet;

        pub struct KeyGen<'a> {
            dist: &'a str,
            rng: XorShift,
            i: u64,
            base: u32,
        }

        impl<'a> KeyGen<'a> {
            pub fn new(dist: &'a str, seed: u64) -> Self {
                Self::with_offset(dist, seed, 0)
            }

            pub fn with_offset(dist: &'a str, seed: u64, offset: u64) -> Self {
                Self {
                    dist,
                    rng: XorShift(seed),
                    i: offset,
                    base: 0,
                }
            }

            pub fn next(&mut self) -> u32 {
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

        pub fn keys(dist: &str, n: usize) -> Vec<u32> {
            let mut g = KeyGen::new(dist, 0x0DDB_1A5E_5EED_0001);
            (0..n).map(|_| g.next()).collect()
        }

        pub fn miss_keys(dist: &str, present: &HashSet<u32>, n: usize, seed: u64) -> Vec<u32> {
            let offset = match dist {
                "sparse" | "dense" => present.len() as u64,
                _ => 0,
            };
            let mut g = KeyGen::with_offset(dist, seed, offset);
            let mut seen: HashSet<u32> = HashSet::with_capacity(n * 2);
            let mut out = Vec::with_capacity(n);
            let budget = n.saturating_mul(64).saturating_add(1024);
            for _ in 0..budget {
                if out.len() == n {
                    return out;
                }
                let c = g.next();
                if !present.contains(&c) && seen.insert(c) {
                    out.push(c);
                }
            }
            panic!("{dist}: could not draw {n} distinct absent keys within budget");
        }
    }

    /// `benches/leaf_cap_cascaded{,_wallclock}.rs`: 63-bit masked draws.
    pub mod cascaded {
        use super::compare::XorShift;
        use std::collections::HashSet;

        pub const SEED: u64 = 0x0DDB_1A5E_5EED_0001;
        pub const MISS_SEED: u64 = 0x51ED_0FF5_C0FF_EE01;
        const BITS: u32 = 63;
        const MASK: u64 = (1u64 << BITS) - 1;

        pub fn keys(n: usize) -> Vec<u64> {
            let mut rng = XorShift(SEED);
            (0..n).map(|_| rng.next() & MASK).collect()
        }

        pub fn miss_keys(present: &HashSet<u64>, n: usize) -> Vec<u64> {
            let mut rng = XorShift(MISS_SEED);
            let mut seen: HashSet<u64> = HashSet::with_capacity(n * 2);
            let mut out = Vec::with_capacity(n);
            let budget = n.saturating_mul(64).saturating_add(1024);
            for _ in 0..budget {
                if out.len() == n {
                    return out;
                }
                let c = rng.next() & MASK;
                if !present.contains(&c) && seen.insert(c) {
                    out.push(c);
                }
            }
            panic!("could not draw {n} distinct absent keys within budget");
        }
    }

    /// The zero-guarded copies (`benches/hashbrown_*.rs`), one fallback each.
    pub struct Guarded(u64);
    impl Guarded {
        pub fn new(seed: u64, fallback: u64) -> Self {
            Self(if seed == 0 { fallback } else { seed })
        }
        pub fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    /// `benches/reclaim_stall.rs` / `benches/strmap_poll_stall.rs`: `seed | 1`.
    pub struct Odd(u64);
    impl Odd {
        pub fn new(seed: u64) -> Self {
            Self(seed | 1)
        }
        pub fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    /// `benches/leaf_layout_timing.rs`: the unit-interval output mix.
    pub fn unit(rng: &mut compare::XorShift) -> f64 {
        (rng.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// `examples/bytes_per_key_32.rs`: the truncating output.
    pub fn next_u32(rng: &mut compare::XorShift) -> u32 {
        let mut x = rng.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        rng.0 = x;
        x as u32
    }

    /// `benches/art_common/mod.rs` and `benches/zset_common/mod.rs`.
    pub fn median_mid(mut v: Vec<f64>) -> f64 {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = v.len();
        if n == 0 {
            0.0
        } else if n % 2 == 1 {
            v[n / 2]
        } else {
            (v[n / 2 - 1] + v[n / 2]) / 2.0
        }
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
// Pins, taken from the `legacy` copies above.
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

#[test]
fn legacy_xorshift_seed_paths_match_pins() {
    let mut r = legacy::compare::XorShift(SEED);
    assert_eq!(draws(|| r.next(), 4), RAW);
    let mut r = legacy::compare::XorShift(0);
    assert_eq!(
        draws(|| r.next(), 2),
        [0, 0],
        "the raw path has no zero guard"
    );
    for (fb, want) in FALLBACKS.iter().zip(GUARDED_ZERO) {
        let mut g = legacy::Guarded::new(0, *fb);
        assert_eq!(draws(|| g.next(), 2), want, "fallback {fb:#x}");
        let mut g = legacy::Guarded::new(SEED, *fb);
        assert_eq!(
            draws(|| g.next(), 4),
            RAW,
            "a non-zero seed ignores {fb:#x}"
        );
    }
    let mut o = legacy::Odd::new(ODD_EVEN_SEED);
    assert_eq!(draws(|| o.next(), 3), ODD);
    let mut o = legacy::Odd::new(0);
    assert_eq!(draws(|| o.next(), 2), ODD_ZERO);
    let mut r = legacy::compare::XorShift(SEED);
    assert_eq!(draws(|| legacy::unit(&mut r).to_bits(), 3), UNIT_BITS);
    let mut r = legacy::compare::XorShift(SEED);
    assert_eq!(draws(|| legacy::next_u32(&mut r), 3), U32);
}

#[test]
fn legacy_keygen_and_misses_match_pins() {
    for (d, dist) in DISTS.iter().enumerate() {
        let k = legacy::compare::keys(dist, 260);
        assert_eq!(k[..3], KEYS_HEAD[d], "{dist}");
        assert_eq!(k[255..258], KEYS_255[d], "{dist}");
        assert_eq!(
            legacy::lookup_compare::generate_keys(dist, 260, SEED),
            k,
            "{dist}"
        );
        assert_eq!(
            legacy::lookup_compare::generate_keys(dist, 3, 0),
            GUARDED_KEYS_ZERO[d],
            "{dist}"
        );
        let present: HashSet<u64> = legacy::compare::keys(dist, 1000).into_iter().collect();
        assert_eq!(
            legacy::compare::generate_miss_keys(dist, &present, 4, MISS_SEED),
            MISSES[d],
            "{dist}"
        );
        assert_eq!(
            legacy::lookup_compare::generate_miss_keys(dist, &present, 4, MISS_SEED),
            MISSES[d],
            "{dist}: a non-zero seed takes the raw path"
        );
    }
    for (d, dist) in CDISTS.iter().enumerate() {
        let k = legacy::comparative::keys(dist, 260);
        assert_eq!([&k[..3], &k[255..258]].concat(), CKEYS[d], "{dist}");
        let present: HashSet<u32> = legacy::comparative::keys(dist, 1000).into_iter().collect();
        assert_eq!(
            legacy::comparative::miss_keys(dist, &present, 4, MISS_SEED),
            CMISSES[d],
            "{dist}"
        );
    }
    let k = legacy::cascaded::keys(1000);
    assert_eq!(k[..3], CASCADED_KEYS);
    let present: HashSet<u64> = k.into_iter().collect();
    assert_eq!(legacy::cascaded::miss_keys(&present, 4), CASCADED_MISSES);
}

#[test]
fn legacy_medians_match_pins() {
    assert_eq!(legacy::median_mid(vec![3.0, 1.0, 2.0]), 2.0);
    assert_eq!(legacy::median_mid(vec![4.0, 1.0, 3.0, 2.0]), 2.5);
    assert_eq!(legacy::median_mid(vec![]), 0.0);
    assert_eq!(legacy::lookup_compare::median(vec![3.0, 1.0, 2.0]), 2.0);
    assert_eq!(
        legacy::lookup_compare::median(vec![4.0, 1.0, 3.0, 2.0]),
        3.0
    );
}
