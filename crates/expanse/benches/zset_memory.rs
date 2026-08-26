//! Pillar 4: Memory footprint per sorted-set member.
//!
//! Deterministic, load-immune allocator accounting via a counting `GlobalAlloc`
//! wrapper (the same technique as the bytes/key memory budget): live heap bytes
//! are sampled before and after each structure is built, and divided by the
//! member count. Reported for both engines with a sub-structure breakdown:
//!
//! * Expanse ZSET = `order` map (composite `(score,member)` keys) + `members`
//!   map (`member -> score`).
//! * SkipList + Dict = span skip list + `hashbrown` dict.
//!
//! Modeling notes (see METHODOLOGY.md): the skip list is arena-backed with a
//! per-node boxed level array, which *understates* a production pointer-chasing
//! skip list's per-node allocator overhead; and members are `u32` inline rather
//! than Redis's heap `sds` strings. Both choices make the baseline conservative
//! — a memory win for Expanse here is a floor, not a ceiling.

#[path = "zset_common/mod.rs"]
mod zset_common;

use expanse_trie::map::ExpanseMap;
use hashbrown::HashMap;
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use zset_common::{ExpanseZSet, SkiplistZSet, XorShift64, composite, shuffled_members};

struct TrackingAlloc;
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards every operation to the System allocator, recording the live
// byte balance around it. The recorded size is exactly the requested layout.
unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: delegating directly to the System allocator.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: delegating directly to the System allocator.
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static GLOBAL: TrackingAlloc = TrackingAlloc;

#[inline]
fn live() -> usize {
    LIVE_BYTES.load(Ordering::SeqCst)
}

/// Measure bytes/member for both engines and their sub-structures over a fixed
/// (members, scores) build. Returns a JSON object.
fn measure(label: &str, members: &[u32], scores: &[u32]) -> serde_json::Value {
    let pop = members.len();
    let bpk = |bytes: usize| bytes as f64 / pop as f64;

    // --- Expanse: whole ZSET ---
    let before = live();
    let mut exp = ExpanseZSet::new();
    for i in 0..pop {
        exp.zadd(members[i], scores[i]);
    }
    let exp_total = live().saturating_sub(before);
    drop(exp);

    // --- Expanse: order map alone (composite keys) ---
    let before = live();
    let mut order = ExpanseMap::new();
    for i in 0..pop {
        order.insert(composite(scores[i], members[i]), members[i] as u64);
    }
    let exp_order = live().saturating_sub(before);
    drop(order);

    // --- Expanse: members map alone ---
    let before = live();
    let mut mem = ExpanseMap::new();
    for i in 0..pop {
        mem.insert(members[i] as u64, scores[i] as u64);
    }
    let exp_members = live().saturating_sub(before);
    drop(mem);

    // --- SkipList + Dict: whole ZSET ---
    let before = live();
    let mut sl = SkiplistZSet::new(0xC0FF_EE12_3456_789A);
    for i in 0..pop {
        sl.zadd(members[i], scores[i]);
    }
    let sl_total = live().saturating_sub(before);
    drop(sl);

    // --- Dict alone (member -> score) ---
    let before = live();
    let mut dict: HashMap<u32, u32> = HashMap::new();
    for i in 0..pop {
        dict.insert(members[i], scores[i]);
    }
    let sl_dict = live().saturating_sub(before);
    drop(dict);

    let sl_list = sl_total.saturating_sub(sl_dict);

    json!({
        "label": label,
        "population": pop,
        "expanse_bytes_per_member": bpk(exp_total),
        "expanse_order_bytes_per_member": bpk(exp_order),
        "expanse_members_bytes_per_member": bpk(exp_members),
        "skiplist_bytes_per_member": bpk(sl_total),
        "skiplist_list_bytes_per_member": bpk(sl_list),
        "skiplist_dict_bytes_per_member": bpk(sl_dict),
        "ratio_skiplist_over_expanse": if exp_total > 0 { sl_total as f64 / exp_total as f64 } else { 0.0 },
        "winner": if exp_total <= sl_total { "expanse" } else { "skiplist" },
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    zset_common::validate();

    let populations: Vec<u32> = if quick {
        vec![10_000, 100_000]
    } else {
        vec![100_000, 1_000_000]
    };
    let scale = 1_000_000u32;

    let mut out = Vec::new();
    for &pop in &populations {
        let mut rng = XorShift64::new(0x4D3D_0741_1111_2222 ^ pop as u64);
        let members = shuffled_members(pop, &mut rng);
        let random_scores: Vec<u32> = (0..pop).map(|_| rng.below(scale)).collect();
        // Sequential scores: score == member (a fully ordered leaderboard).
        let seq_scores: Vec<u32> = members.clone();

        out.push(json!({
            "population": pop,
            "random_scores": measure("random_scores", &members, &random_scores),
            "sequential_scores": measure("sequential_scores", &members, &seq_scores),
        }));
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("{out:#?}");
    }
}
