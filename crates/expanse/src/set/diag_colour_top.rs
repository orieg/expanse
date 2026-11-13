//! #1682 diagnostic (test-only, `#[ignore]`d): an upper bound on what
//! skipping a set's top node would save on MariaDB OQGraph's colour-map tape.
//!
//! The colour tape's vertex ids are below 2^32, so every key's level-8 digit
//! is 0 and every lookup takes the top node's digit-0 child. Variant `child`
//! enters [`get::test_set`] at that child edge with seven undecoded bytes;
//! variant `root` calls [`ExpanseSet::contains`], which matches the root and
//! enters at the top edge with eight. Both variants re-locate the child edge
//! after every set and unset, so the only difference between them is the
//! entry point of the test calls. The saving measured is therefore an upper
//! bound for any change that removes the top node's step: it also removes the
//! root match, which such a change would keep.
//!
//! One process runs one variant. The driver runs the two variants in
//! interleaved pairs under `perf stat -e cycles` and differences their
//! counts; the build work (tape, arrays) is identical in both, so it cancels.
//!
//! ```text
//! DIAG_COLOUR_VARIANT=root|child DIAG_COLOUR_N=100000 DIAG_COLOUR_REPS=20 \
//!   cargo test --release -p expanse-trie --lib set::diag_colour_top -- --ignored --exact \
//!   set::diag_colour_top::colour_top_skip --nocapture
//! ```

#[allow(dead_code, clippy::all, missing_docs)]
#[path = "../../../expanse-capi/benches/colour_map_common/mod.rs"]
mod tape;

use super::{ExpanseSet, Root};
use crate::get;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge};
use core::hint::black_box;

/// The top node's child edge for level-8 digit 0, when the root is a tree
/// whose top node is a branch at level 8 holding that digit.
fn top_child(set: &ExpanseSet) -> Option<*const Edge> {
    let Root::Tree { top, .. } = &set.root else {
        return None;
    };
    let ptr = top.node_ptr();
    // SAFETY: (all arms) a pointer-tagged top edge references a live branch
    // of its tagged type (the module contract of `get`); only digit 0's
    // slot is read.
    unsafe {
        match top.tag_byte() {
            0x01 => {
                let b = &*ptr.cast::<BranchL3>();
                let n = b.hdr.num as usize;
                (b.hdr.level == 8)
                    .then(|| (0..n.min(3)).find(|&i| b.hdr.digits[i] == 0))
                    .flatten()
                    .map(|i| b.edges.as_ptr().add(i))
            }
            0x02 => {
                let b = &*ptr.cast::<BranchL7>();
                (b.hdr.level == 8)
                    .then(|| b.hdr.find(0))
                    .flatten()
                    .map(|i| b.edges.as_ptr().add(i))
            }
            0x03 => {
                let b = &*ptr.cast::<BranchB>();
                if b.level != 8 {
                    return None;
                }
                let (sub, slot) = b.bitmap.test_and_subexpanse_rank_with_sub(0)?;
                Some((*b.subarrays.as_ptr().add(sub)).add(slot).cast_const())
            }
            0x04 => {
                let b = &*ptr.cast::<BranchU>();
                Some(b.edges.as_ptr())
            }
            _ => None,
        }
    }
}

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

#[test]
#[ignore = "#1682 diagnostic: run under perf stat by the driver, never in the suite"]
fn colour_top_skip() {
    let variant = env("DIAG_COLOUR_VARIANT", "root");
    let child_entry = match variant.as_str() {
        "root" => false,
        "child" => true,
        other => panic!("DIAG_COLOUR_VARIANT={other:?} (accepted: root, child)"),
    };
    let n: u64 = env("DIAG_COLOUR_N", "100000")
        .parse()
        .expect("DIAG_COLOUR_N");
    let reps: usize = env("DIAG_COLOUR_REPS", "20")
        .parse()
        .expect("DIAG_COLOUR_REPS");
    let t = tape::tape(n);
    let (mut sink, mut via_child, mut via_root) = (0u64, 0u64, 0u64);
    for _ in 0..reps {
        let mut sets = [ExpanseSet::new(), ExpanseSet::new()];
        let mut child: [Option<*const Edge>; 2] = [None, None];
        for &op in black_box(&t) {
            let (op, a, v) = tape::decode(op);
            match op {
                tape::OP_TEST => {
                    let hit = match child[a] {
                        Some(e) if child_entry => {
                            via_child += 1;
                            // SAFETY: re-located after the array's last
                            // change, so it is a live edge of the current
                            // tree covering level 7, and the key's level-8
                            // digit is 0.
                            unsafe { get::test_set(&*e, v, 7) }
                        }
                        _ => {
                            via_root += 1;
                            sets[a].contains(v)
                        }
                    };
                    sink = sink.wrapping_add(u64::from(hit));
                }
                tape::OP_SET | tape::OP_UNSET => {
                    let changed = if op == tape::OP_SET {
                        sets[a].insert(v)
                    } else {
                        sets[a].remove(v)
                    };
                    sink = sink.wrapping_add(u64::from(changed));
                    child[a] = top_child(&sets[a]);
                }
                _ => sets[a] = ExpanseSet::new(),
            }
        }
    }
    // The driver checks that both variants print the same sink, and that
    // `child` sent at least 99% of its tests through the child edge.
    println!(
        "DIAG_COLOUR variant={variant} n={n} reps={reps} calls={} tests={} \
         via_child={via_child} via_root={via_root} sink={}",
        t.len() * reps,
        tape::census(&t)[0] * reps,
        black_box(sink)
    );
}
