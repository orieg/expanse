//! Why keyspace width behaves as a population multiplier, and where the cliff is.
//!
//! This probe settled a question the HOT suite got wrong. Clearing the top bit
//! of a key halves the keyspace, and for a structure that partitions by key
//! *expanse* the only parameter that matters is occupancy per expanse — so
//! narrowing the domain by one bit is arithmetically the same as doubling the
//! population. The sweep shows it directly: the 63-bit column at N reproduces
//! the 64-bit column at 2N, and the 62-bit column at 4N, to two decimals.
//!
//! It also locates a density cliff whose position halves per bit removed. That
//! matters beyond this suite: the committed `memory-budget` ceiling in
//! `crates/expanse/examples/bytes_per_key.rs` is calibrated at a single
//! population that sits just before the 64-bit cliff.
//!
//! Deterministic accounting only (`mem_used()`), no wall-clock (§8.4).
use expanse_hot_bench::Census;
use expanse_trie::set::ExpanseSet;

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

fn build(n: usize, mask: u64) -> (usize, usize) {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut s = ExpanseSet::new();
    for _ in 0..n {
        s.insert(rng.next() & mask);
    }
    (s.len() as usize, s.mem_used())
}

/// Sweeps the non-random distributions across the same N range.
///
/// §9.5 of the suite methodology derives, from the generators, that these three
/// have construction-fixed expanse occupancy and so do not move with density —
/// `sparse` in particular puts the top two key bytes at `i >> 8`, giving exactly
/// 256 keys per 2-byte expanse at any N. This is the measurement that checks the
/// derivation rather than trusting it.
fn sweep_shapes() {
    println!("\nDistribution anchors across N (set flavor, mem_used B/key)");
    println!(
        "{:>10} {:>12} {:>12} {:>12}",
        "N", "sequential", "clustered", "sparse i<<40"
    );
    for n in [100_000usize, 400_000, 1_000_000, 2_000_000] {
        let mut row = Vec::new();
        for dist in ["sequential", "clustered", "sparse"] {
            let mut s = ExpanseSet::new();
            let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
            let mut base = 0u64;
            for i in 0..n as u64 {
                let k = match dist {
                    "sequential" => i,
                    "sparse" => i << 40,
                    _ => {
                        if i % 256 == 0 {
                            base = rng.next() & !0xFF;
                        }
                        base + (i % 256)
                    }
                };
                s.insert(k);
            }
            row.push(s.mem_used() as f64 / s.len() as f64);
        }
        println!("{n:>10} {:>12.2} {:>12.2} {:>12.2}", row[0], row[1], row[2]);
    }
}

fn main() {
    // Forces the shim object to be linked. build.rs emits `-Wl,--wrap=...` for
    // every binary in this crate, but the wrappers live in the shim, so a
    // binary that references nothing from it fails to link with an undefined
    // `__wrap_malloc`. Touching the census pulls it in.
    Census::reset();
    println!(
        "{:>10} {:>10} {:>10} {:>10} {:>8}",
        "N", "64-bit B/k", "63-bit B/k", "62-bit B/k", "63/64"
    );
    for n in [
        100_000usize,
        200_000,
        400_000,
        600_000,
        800_000,
        900_000,
        1_000_000,
        1_200_000,
        2_000_000,
    ] {
        let (n64, m64) = build(n, u64::MAX);
        let (n63, m63) = build(n, (1u64 << 63) - 1);
        let (n62, m62) = build(n, (1u64 << 62) - 1);
        let b64 = m64 as f64 / n64 as f64;
        let b63 = m63 as f64 / n63 as f64;
        let b62 = m62 as f64 / n62 as f64;
        println!(
            "{n:>10} {b64:>10.2} {b63:>10.2} {b62:>10.2} {:>8.3}",
            b63 / b64
        );
    }
    sweep_shapes();
}
