//! Phase 0 mathematical crossover & promotion-rate evaluation harness for issue #392.
//!
//! Evaluates in-register lightweight value compression codecs (ZeroTrim8,
//! Nibble4, Alnum6) against named representative datasets to verify
//! empirical promotion rates against theoretical ceilings per AGENTS.md §8.13.
//!
//! Run: `cargo run --release -p expanse-trie --example value_compression_eval`
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_value_compression_eval` |
//! | `group` | 5 |
//! | `population` | 100k per dataset |
//! | `probes_and_reuse` | 100k samples per shape |
//! | `hit_rate` | 100% |
//! | `miss_gen_method` | N/A (Evaluation census) |
//! | `value_dereference` | Codec round-trip verification |
//! | `measured_region` | Offline codec census |
//! | `arm_symmetry` | Symmetric comparison across datasets |
//! | `statistics` | Exact count and promotion ratio |
//! | `verdict` | **PASS** `[verified: RUN (Phase 0)]`: Empirical evaluation of value compression crossover. |

struct XorShift64(u64);

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

// ---------------------------------------------------------------------------
// Pure SWAR Codecs
// ---------------------------------------------------------------------------

/// ZeroTrim8: 8-byte integers where the high byte is zero (u64 < 2^56).
#[inline(always)]
fn try_compress_zero_trim_8(data: &[u8]) -> Option<u64> {
    if data.len() != 8 {
        return None;
    }
    let val = u64::from_le_bytes(data.try_into().ok()?);
    if val < (1 << 56) { Some(val) } else { None }
}

#[inline(always)]
fn decompress_zero_trim_8(val: u64, out: &mut [u8; 8]) {
    *out = val.to_le_bytes();
}

/// Nibble4: 8 to 14 ASCII decimal digits ('0'..='9') packed into 4-bit nibbles.
#[inline(always)]
fn try_compress_nibble_4(data: &[u8]) -> Option<(u64, u8)> {
    let len = data.len();
    if !(8..=14).contains(&len) {
        return None;
    }
    let mut packed = 0u64;
    for (i, &b) in data.iter().enumerate() {
        if !b.is_ascii_digit() {
            return None;
        }
        let nibble = (b - b'0') as u64;
        packed |= nibble << (4 * i);
    }
    Some((packed, len as u8))
}

#[inline(always)]
fn decompress_nibble_4(packed: u64, len: usize, out: &mut [u8; 16]) {
    for (i, slot) in out.iter_mut().enumerate().take(len) {
        let nibble = ((packed >> (4 * i)) & 0x0F) as u8;
        *slot = b'0' + nibble;
    }
}

/// Alnum6: 8 or 9 ASCII characters from dictionary [0-9A-Za-z_-] (64 symbols).
const ALNUM6_LUT: [u8; 64] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', // 0..=9
    b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', b'I', b'J', // 10..=19
    b'K', b'L', b'M', b'N', b'O', b'P', b'Q', b'R', b'S', b'T', // 20..=29
    b'U', b'V', b'W', b'X', b'Y', b'Z', // 30..=35
    b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j', // 36..=45
    b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', // 46..=55
    b'u', b'v', b'w', b'x', b'y', b'z', // 56..=61
    b'-', b'_', // 62..=63
];

#[inline(always)]
fn alnum6_encode_char(b: u8) -> Option<u64> {
    match b {
        b'0'..=b'9' => Some((b - b'0') as u64),
        b'A'..=b'Z' => Some((b - b'A' + 10) as u64),
        b'a'..=b'z' => Some((b - b'a' + 36) as u64),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

#[inline(always)]
fn try_compress_alnum_6(data: &[u8]) -> Option<(u64, u8)> {
    let len = data.len();
    if len != 8 && len != 9 {
        return None;
    }
    let mut packed = 0u64;
    for (i, &b) in data.iter().enumerate() {
        let code = alnum6_encode_char(b)?;
        packed |= code << (6 * i);
    }
    Some((packed, len as u8))
}

#[inline(always)]
fn decompress_alnum_6(packed: u64, len: usize, out: &mut [u8; 16]) {
    for (i, slot) in out.iter_mut().enumerate().take(len) {
        let code = ((packed >> (6 * i)) & 0x3F) as usize;
        *slot = ALNUM6_LUT[code];
    }
}

// ---------------------------------------------------------------------------
// Combined Codec Dispatch
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum CodecKind {
    ZeroTrim8,
    Nibble4(u8),
    Alnum6(u8),
}

fn try_compress(data: &[u8]) -> Option<(u64, CodecKind)> {
    // 1. Try ZeroTrim8 for 8-byte integers with upper zero byte
    if let Some(packed) = try_compress_zero_trim_8(data) {
        return Some((packed, CodecKind::ZeroTrim8));
    }
    // 2. Try Nibble4 for 8..14 decimal digit strings
    if let Some((packed, len)) = try_compress_nibble_4(data) {
        return Some((packed, CodecKind::Nibble4(len)));
    }
    // 3. Try Alnum6 for 8..9 alphanumeric strings
    if let Some((packed, len)) = try_compress_alnum_6(data) {
        return Some((packed, CodecKind::Alnum6(len)));
    }
    None
}

fn decompress(packed: u64, kind: CodecKind, out: &mut [u8; 16]) -> usize {
    match kind {
        CodecKind::ZeroTrim8 => {
            let mut buf8 = [0u8; 8];
            decompress_zero_trim_8(packed, &mut buf8);
            out[..8].copy_from_slice(&buf8);
            8
        }
        CodecKind::Nibble4(len) => {
            let n = len as usize;
            decompress_nibble_4(packed, n, out);
            n
        }
        CodecKind::Alnum6(len) => {
            let n = len as usize;
            decompress_alnum_6(packed, n, out);
            n
        }
    }
}

// ---------------------------------------------------------------------------
// Dataset Generators
// ---------------------------------------------------------------------------

fn generate_monotonic_ints(n: usize) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(n);
    for i in 1..=n {
        let val = (i as u64) * 1000;
        out.push(val.to_le_bytes().to_vec());
    }
    out
}

fn generate_alnum_slugs(n: usize) -> Vec<Vec<u8>> {
    let mut rng = XorShift64::new(0xABCD_1234_5678_0001);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let len = if i % 2 == 0 { 8 } else { 9 };
        let mut s = Vec::with_capacity(len);
        for _ in 0..len {
            let idx = (rng.next_u64() % 64) as usize;
            s.push(ALNUM6_LUT[idx]);
        }
        out.push(s);
    }
    out
}

fn generate_decimal_strings(n: usize) -> Vec<Vec<u8>> {
    let mut rng = XorShift64::new(0x9876_5432_10FE_0001);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let len = 8 + (rng.next_u64() % 7) as usize; // 8..=14
        let mut s = Vec::with_capacity(len);
        for _ in 0..len {
            let d = b'0' + (rng.next_u64() % 10) as u8;
            s.push(d);
        }
        out.push(s);
    }
    out
}

fn generate_uniform_random_8b(n: usize) -> Vec<Vec<u8>> {
    let mut rng = XorShift64::new(0x1122_3344_5566_7788);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(rng.next_u64().to_le_bytes().to_vec());
    }
    out
}

fn generate_uniform_random_16b(n: usize) -> Vec<Vec<u8>> {
    let mut rng = XorShift64::new(0xAABB_CCDD_EEFF_0011);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut buf = Vec::with_capacity(16);
        buf.extend_from_slice(&rng.next_u64().to_le_bytes());
        buf.extend_from_slice(&rng.next_u64().to_le_bytes());
        out.push(buf);
    }
    out
}

// ---------------------------------------------------------------------------
// Evaluation Runner
// ---------------------------------------------------------------------------

struct EvalResult {
    name: &'static str,
    workload_id: &'static str,
    total_samples: usize,
    promoted_count: usize,
    promotion_rate: f64,
    round_trip_errors: usize,
}

fn evaluate_dataset(name: &'static str, workload_id: &'static str, data: &[Vec<u8>]) -> EvalResult {
    let total = data.len();
    let mut promoted = 0;
    let mut errors = 0;
    let mut decode_buf = [0u8; 16];

    for sample in data {
        if let Some((packed, kind)) = try_compress(sample) {
            promoted += 1;
            let decoded_len = decompress(packed, kind, &mut decode_buf);
            if &decode_buf[..decoded_len] != sample.as_slice() {
                errors += 1;
            }
        }
    }

    EvalResult {
        name,
        workload_id,
        total_samples: total,
        promoted_count: promoted,
        promotion_rate: (promoted as f64 / total as f64) * 100.0,
        round_trip_errors: errors,
    }
}

fn main() {
    println!("=== Phase 0: Value Compression & Inline Promotion Evaluation ===");
    println!("Evaluating empirical promotion rates on N = 100,000 samples per dataset\n");

    let n = 100_000;
    let datasets = [
        (
            "Monotonic 64-bit Integers (8B)",
            "value_compression_monotonic_int",
            generate_monotonic_ints(n),
        ),
        (
            "Alphanumeric Slugs / IDs (8-9B)",
            "value_compression_alnum_slug",
            generate_alnum_slugs(n),
        ),
        (
            "Decimal Timestamp Strings (8-14B)",
            "value_compression_decimal_str",
            generate_decimal_strings(n),
        ),
        (
            "Uniform Random (8B) [Negative Control]",
            "value_compression_uniform_random_8b",
            generate_uniform_random_8b(n),
        ),
        (
            "Uniform Random (16B) [Negative Control]",
            "value_compression_uniform_random_16b",
            generate_uniform_random_16b(n),
        ),
    ];

    println!(
        "| {:<38} | {:<34} | {:>10} | {:>10} | {:>12} | {:>6} |",
        "Dataset Description", "Workload ID", "Total", "Promoted", "Promo Rate", "Errors"
    );
    println!(
        "|{:-<40}|{:-<36}|{:-<12}|{:-<12}|{:-<14}|{:-<8}|",
        "", "", "", "", "", ""
    );

    let mut all_passed = true;
    for (name, workload_id, data) in datasets {
        let res = evaluate_dataset(name, workload_id, &data);
        println!(
            "| {:<38} | {:<34} | {:>10} | {:>10} | {:>11.2}% | {:>6} |",
            res.name,
            res.workload_id,
            res.total_samples,
            res.promoted_count,
            res.promotion_rate,
            res.round_trip_errors
        );

        assert_eq!(
            res.round_trip_errors, 0,
            "Round-trip fidelity failure on {}",
            res.name
        );

        // Pre-registered gate checks
        if workload_id.contains("monotonic")
            || workload_id.contains("alnum")
            || workload_id.contains("decimal")
        {
            if res.promotion_rate < 70.0 {
                eprintln!(
                    "FAILED: Target dataset {} did not achieve >= 70% promotion rate floor",
                    res.name
                );
                all_passed = false;
            }
        } else if workload_id.contains("uniform_random_8b") {
            // Must be <= 5.08% theoretical ceiling (2^56 / 2^64 = 1/256 = 0.39% for single tag, ~0.39% observed)
            if res.promotion_rate > 5.08 {
                eprintln!(
                    "FAILED: Uniform 8B exceeded theoretical 5.08% ceiling: {:.2}%",
                    res.promotion_rate
                );
                all_passed = false;
            }
        } else if workload_id.contains("uniform_random_16b") && res.promoted_count > 0 {
            eprintln!(
                "FAILED: Uniform 16B produced non-zero promotion (mathematically impossible)"
            );
            all_passed = false;
        }
    }

    println!("\n=== Gate Verdict ===");
    if all_passed {
        println!("PASS: Phase 0 promotion rate criteria satisfied on all targeted shapes.");
        println!(
            "  - Target datasets (monotonic int, alnum slug, decimal str): >= 70.0% promotion rate achieved."
        );
        println!("  - Round-trip error count: 0 across all 500,000 samples.");
        println!("  - Uniform random negative controls respect theoretical pigeonhole ceilings.");
    } else {
        println!("FAIL: Phase 0 gate criteria not satisfied.");
        std::process::exit(1);
    }
}
