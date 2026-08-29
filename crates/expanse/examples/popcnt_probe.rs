//! Prints whether runtime `popcnt` detection succeeds — nothing else.
//!
//! Exists to answer one pre-registered question (issue #1): does
//! `is_x86_feature_detected!("popcnt")` still return true **under
//! valgrind**? Valgrind is known to mask some CPUID bits; if it masks
//! this one, the `instruction-counts` job measures the SWAR fallback and
//! the dispatch reads as "no effect" — a trap that must be visible in
//! the job log, not discovered by head-scratching. CI runs this probe
//! natively and under valgrind and prints both answers side by side.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_popcnt_probe` |
//! | `group` | 5 |
//! | `population` | 1 probe |
//! | `probes_and_reuse` | Single instruction |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | CPUID probe |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Diagnostic check |
//! | `statistics` | Boolean status |
//! | `verdict` | **PASS** `[verified: RUN (CI instruction-counts)]`: CPUID popcnt verification. |

fn main() {
    #[cfg(target_arch = "x86_64")]
    println!(
        "popcnt detected: {}",
        std::arch::is_x86_feature_detected!("popcnt")
    );
    #[cfg(not(target_arch = "x86_64"))]
    println!("popcnt detected: n/a (not x86_64; count_ones is native)");
}
