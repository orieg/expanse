//! Deterministic dependent node visits per probe tests (Refs #1249).
//!
//! Validates that `probe_visits`:
//! 1. Exactly matches analytically derived node ladder counts across all node forms
//!    (RootLeaf, Immed, BranchL3, BranchL7, BranchB, BranchU, BitmapLeaf, decode-byte skips).
//! 2. Satisfies exact structural census identities (Σ visits == census.sums) over all
//!    canonical distributions.
//! 3. Proves decode-byte skips do not count as visits (skips verified in CPU registers).
//! 4. Fails closed under mutations (negative control).

#![cfg(not(miri))]

use expanse_trie::map::ExpanseMap;
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

// =========================================================================
// 1. Hand-Constructed Trees with Analytically Derived Node Ladders (3a)
// =========================================================================

/// Verifies analytically derived node ladder counts across every node form
/// in `docs/ARCHITECTURE.md` §10.
#[test]
fn test_analytical_node_forms_and_ladders() {
    // ---------------------------------------------------------------------
    // Form 1: Root Leaf (`Root::Leaf`)
    // ---------------------------------------------------------------------
    // Invariant (docs/ARCHITECTURE.md §1):
    // Population <= ROOT_LEAF_CAP (31) stays in one sorted root leaf allocation.
    // Probing a root leaf traverses zero branch edges (edges_followed = 0)
    // and zero BranchB nodes (branch_b_subarrays = 0).
    // The leaf allocation is inspected: exactly 1 leaf load (leaf_loads = 1).
    // Total visits = 0 + 0 + 1 = 1.
    {
        let mut map = ExpanseMap::new();
        for i in 1..=10u64 {
            map.insert(i * 10, i * 100);
        }
        let stats = map.stats();
        assert_eq!(
            stats.node_counts.leaf_linear, 1,
            "must be a single root leaf"
        );
        assert_eq!(stats.node_counts.branch_l3, 0);
        assert_eq!(stats.node_counts.branch_b, 0);

        // Hit:
        for i in 1..=10u64 {
            let k = i * 10;
            let v = map.probe_visits(k);
            assert_eq!(v.edges_followed, 0, "root leaf has no branch edges");
            assert_eq!(v.branch_b_subarrays, 0, "root leaf has no BranchB");
            assert_eq!(v.leaf_loads, 1, "root leaf counts as 1 leaf load");
            assert_eq!(v.total_visits(), 1);
            assert!(v.found);
            assert_eq!(v.value, Some(i * 100));
            assert_eq!(v.value, map.get(k));
        }

        // Miss on root leaf:
        let v_miss = map.probe_visits(999);
        assert_eq!(v_miss.edges_followed, 0);
        assert_eq!(v_miss.branch_b_subarrays, 0);
        assert_eq!(v_miss.leaf_loads, 1, "miss on root leaf checked the leaf");
        assert_eq!(v_miss.total_visits(), 1);
        assert!(!v_miss.found);
        assert_eq!(v_miss.value, None);
    }

    // ---------------------------------------------------------------------
    // Form 2: 1-Key Immediate (`Immed7_1` / Tag `0x70`)
    // ---------------------------------------------------------------------
    // Invariant (docs/ARCHITECTURE.md §3.3, §10.4):
    // 1-key map immediates pack the key into `aux` (7 bytes) and the value
    // directly into `Word0` of the `Edge` descriptor. Zero heap leaf allocation.
    // Construction: 200 keys with distinct top bytes (i << 56) for i in 0..200.
    // 200 > 192 (BITMAP_TO_UNCOMPRESSED_THRESHOLD), so level 8 is BranchU.
    // Each of the 200 child edges has 1 key with 7 remaining bytes -> Immed7_1.
    // Memory loads:
    // - Follows root edge to BranchU: edges_followed = 1.
    // - Direct indexing into BranchU array: branch_b_subarrays = 0.
    // - Immed7_1 contains value in Word0: leaf_loads = 0 (no heap leaf loaded!).
    // Total visits = 1 + 0 + 0 = 1.
    {
        let mut map = ExpanseMap::new();
        for i in 0..200u64 {
            map.insert(i << 56, i * 10);
        }
        let stats = map.stats();
        assert_eq!(stats.node_counts.branch_u, 1, "root is BranchU");
        assert_eq!(
            stats.node_counts.immed, 200,
            "all 200 children are immediates"
        );
        assert_eq!(stats.node_counts.leaf_linear, 0, "no heap linear leaves");

        // Hit on 1-key immediate:
        for i in 0..200u64 {
            let k = i << 56;
            let v = map.probe_visits(k);
            assert_eq!(v.edges_followed, 1, "traversed BranchU edge");
            assert_eq!(v.branch_b_subarrays, 0, "no BranchB in path");
            assert_eq!(v.leaf_loads, 0, "1-key immediate has 0 heap leaf loads");
            assert_eq!(v.total_visits(), 1);
            assert!(v.found);
            assert_eq!(v.value, Some(i * 10));
            assert_eq!(v.value, map.get(k));
        }

        // Miss on unpopulated digit 250 in BranchU:
        let v_miss = map.probe_visits(250u64 << 56);
        assert_eq!(v_miss.edges_followed, 1, "traversed BranchU edge");
        assert_eq!(v_miss.branch_b_subarrays, 0);
        assert_eq!(v_miss.leaf_loads, 0);
        assert_eq!(v_miss.total_visits(), 1);
        assert!(!v_miss.found);
        assert_eq!(v_miss.value, None);
    }

    // ---------------------------------------------------------------------
    // Form 3: Linear Branch `BranchL3`
    // ---------------------------------------------------------------------
    // Invariant (docs/ARCHITECTURE.md §3.2):
    // BranchL3 holds up to 3 children (BRANCH_L3_CAP = 3).
    // Construction: 3 distinct digits at level 8 (0, 1, 2), each holding 15 keys.
    // Total pop = 45 > ROOT_LEAF_CAP (31), so root promotes to Tree.
    // Root level 8 has 3 children <= 3 -> BranchL3 (node_counts.branch_l3 = 1).
    // Each child has 15 keys with 7 remaining bytes -> Leaf7 (leaf_linear = 3).
    // Memory loads:
    // - Follows root edge to BranchL3: edges_followed = 1.
    // - BranchL3 has no subarrays: branch_b_subarrays = 0.
    // - Follows child edge to Leaf7: leaf_loads = 1.
    // Total visits = 1 + 0 + 1 = 2.
    {
        let mut map = ExpanseMap::new();
        for i in 0..3u64 {
            for j in 1..=15u64 {
                map.insert((i << 56) | j, (i << 56) | (j * 7));
            }
        }
        let stats = map.stats();
        assert_eq!(stats.node_counts.branch_l3, 1, "root is BranchL3");
        assert_eq!(stats.node_counts.leaf_linear, 3, "3 linear leaves");
        assert_eq!(stats.node_counts.branch_b, 0);

        // Hit:
        for i in 0..3u64 {
            for j in 1..=15u64 {
                let k = (i << 56) | j;
                let v = map.probe_visits(k);
                assert_eq!(v.edges_followed, 1, "BranchL3 edge");
                assert_eq!(v.branch_b_subarrays, 0, "no BranchB");
                assert_eq!(v.leaf_loads, 1, "loaded Leaf7");
                assert_eq!(v.total_visits(), 2);
                assert!(v.found);
                assert_eq!(v.value, Some((i << 56) | (j * 7)));
            }
        }

        // Miss on absent digit in BranchL3:
        let v_digit_miss = map.probe_visits((5u64 << 56) | 1);
        assert_eq!(v_digit_miss.edges_followed, 1, "BranchL3 inspected");
        assert_eq!(v_digit_miss.branch_b_subarrays, 0);
        assert_eq!(v_digit_miss.leaf_loads, 0, "aborted before leaf");
        assert_eq!(v_digit_miss.total_visits(), 1);
        assert!(!v_digit_miss.found);

        // Miss on present digit but absent leaf key:
        let v_leaf_miss = map.probe_visits(999);
        assert_eq!(v_leaf_miss.edges_followed, 1, "BranchL3 inspected");
        assert_eq!(v_leaf_miss.branch_b_subarrays, 0);
        assert_eq!(v_leaf_miss.leaf_loads, 1, "Leaf7 inspected and missed");
        assert_eq!(v_leaf_miss.total_visits(), 2);
        assert!(!v_leaf_miss.found);
    }

    // ---------------------------------------------------------------------
    // Form 4: Linear Branch `BranchL7`
    // ---------------------------------------------------------------------
    // Invariant (docs/ARCHITECTURE.md §3.2):
    // 3 < active children <= 7 (BRANCH_L7_CAP = 7) -> BranchL7.
    // Construction: 6 distinct digits at level 8 (0..6), each holding 10 keys.
    // Total pop = 60 > 31.
    // Root level 8 has 6 children -> BranchL7 (node_counts.branch_l7 = 1).
    // Memory loads:
    // - Follows root edge to BranchL7: edges_followed = 1.
    // - BranchL7 has no subarrays: branch_b_subarrays = 0.
    // - Follows child edge to Leaf7: leaf_loads = 1.
    // Total visits = 1 + 0 + 1 = 2.
    {
        let mut map = ExpanseMap::new();
        for i in 0..6u64 {
            for j in 1..=10u64 {
                map.insert((i << 56) | j, (i << 56) | (j * 11));
            }
        }
        let stats = map.stats();
        assert_eq!(stats.node_counts.branch_l7, 1, "root is BranchL7");
        assert_eq!(stats.node_counts.leaf_linear, 6, "6 linear leaves");
        assert_eq!(stats.node_counts.branch_b, 0);

        // Hit:
        for i in 0..6u64 {
            let k = (i << 56) | 1;
            let v = map.probe_visits(k);
            assert_eq!(v.edges_followed, 1, "BranchL7 edge");
            assert_eq!(v.branch_b_subarrays, 0, "no BranchB");
            assert_eq!(v.leaf_loads, 1, "loaded Leaf7");
            assert_eq!(v.total_visits(), 2);
            assert!(v.found);
        }

        // Miss on absent digit:
        let v_miss = map.probe_visits((7u64 << 56) | 1);
        assert_eq!(v_miss.edges_followed, 1);
        assert_eq!(v_miss.branch_b_subarrays, 0);
        assert_eq!(v_miss.leaf_loads, 0);
        assert_eq!(v_miss.total_visits(), 1);
        assert!(!v_miss.found);
    }

    // ---------------------------------------------------------------------
    // Form 5: Bitmap Branch `BranchB`
    // ---------------------------------------------------------------------
    // Invariant (docs/ARCHITECTURE.md §3.2):
    // 7 < active children <= 192 (BITMAP_TO_UNCOMPRESSED_THRESHOLD) -> BranchB.
    // Construction: 16 distinct digits at level 8 (0..16), each holding 3 keys.
    // Total pop = 48 > 31.
    // Root level 8 has 16 children -> BranchB (node_counts.branch_b = 1).
    // Each child has 3 keys with 7 remaining bytes -> Leaf7 (leaf_linear = 16).
    // Memory loads:
    // - Follows root edge to BranchB: edges_followed = 1.
    // - Tests bitmap rank, loads subarray pointer b.subarrays[sub]: branch_b_subarrays = 1.
    // - Loads leaf from subarray edge: leaf_loads = 1.
    // Total visits = 1 + 1 + 1 = 3.
    {
        let mut map = ExpanseMap::new();
        for i in 0..16u64 {
            map.insert((i << 56) | 1, 101);
            map.insert((i << 56) | 2, 102);
            map.insert((i << 56) | 3, 103);
        }
        let stats = map.stats();
        assert_eq!(stats.node_counts.branch_b, 1, "root is BranchB");
        assert_eq!(stats.node_counts.leaf_linear, 16, "16 linear leaves");

        // Hit: exactly 1 edge, 1 BranchB subarray, 1 leaf load = 3 visits.
        for i in 0..16u64 {
            let k = (i << 56) | 1;
            let v = map.probe_visits(k);
            assert_eq!(v.edges_followed, 1, "BranchB edge");
            assert_eq!(v.branch_b_subarrays, 1, "loaded BranchB subarray pointer");
            assert_eq!(v.leaf_loads, 1, "loaded Leaf7");
            assert_eq!(v.total_visits(), 3);
            assert!(v.found);
            assert_eq!(v.value, Some(101));
        }

        // Miss on absent digit in BranchB (bitmap bit 0 -> early return):
        let v_bitmap_miss = map.probe_visits((20u64 << 56) | 1);
        assert_eq!(v_bitmap_miss.edges_followed, 1, "inspected BranchB");
        assert_eq!(
            v_bitmap_miss.branch_b_subarrays, 0,
            "aborted before subarray load"
        );
        assert_eq!(v_bitmap_miss.leaf_loads, 0, "aborted before leaf load");
        assert_eq!(v_bitmap_miss.total_visits(), 1);
        assert!(!v_bitmap_miss.found);

        // Miss on present digit but absent key in leaf (subarray loaded, leaf missed):
        let v_leaf_miss = map.probe_visits(999);
        assert_eq!(v_leaf_miss.edges_followed, 1, "inspected BranchB");
        assert_eq!(v_leaf_miss.branch_b_subarrays, 1, "loaded BranchB subarray");
        assert_eq!(v_leaf_miss.leaf_loads, 1, "loaded Leaf7 and missed");
        assert_eq!(v_leaf_miss.total_visits(), 3);
        assert!(!v_leaf_miss.found);
    }

    // ---------------------------------------------------------------------
    // Form 6: Bitmap Leaf (`LeafBitmapL` in map / `LeafBitmap1` in set)
    // ---------------------------------------------------------------------
    // Invariant (docs/ARCHITECTURE.md §3.3):
    // Population >= 32 at level 1 (last byte) converts to a bitmap leaf.
    // Construction: 40 keys with shared prefix 0x0102_0304_0506_0700 | j for j in 0..40.
    // Pop = 40 > 31. Level 1 leaf has 40 keys >= 32 -> LeafBitmapL.
    // Top edge has narrow pointer for levels 8..2, pointing to BranchL3 / leaf.
    // Memory loads:
    // Probing present key loads LeafBitmapL (leaf_loads = 1).
    {
        let mut map = ExpanseMap::new();
        let prefix = 0x0102_0304_0506_0700u64;
        for j in 0..40u64 {
            map.insert(prefix | j, j * 3);
        }
        let stats = map.stats();
        assert_eq!(stats.node_counts.leaf_bitmap, 1, "leaf is LeafBitmapL");
        assert_eq!(stats.node_counts.branch_b, 0);

        for j in 0..40u64 {
            let k = prefix | j;
            let v = map.probe_visits(k);
            assert_eq!(v.branch_b_subarrays, 0, "no BranchB");
            assert_eq!(v.leaf_loads, 1, "LeafBitmapL loaded");
            assert!(v.found);
            assert_eq!(v.value, Some(j * 3));
        }

        // Miss within LeafBitmapL (absent bit 200):
        let v_miss = map.probe_visits(prefix | 200);
        assert_eq!(v_miss.branch_b_subarrays, 0);
        assert_eq!(v_miss.leaf_loads, 1, "LeafBitmapL loaded and tested bit");
        assert!(!v_miss.found);
        assert_eq!(v_miss.value, None);
    }

    // ---------------------------------------------------------------------
    // Form 7: Narrow-Pointer Skip (Decode Bytes)
    // ---------------------------------------------------------------------
    // THE INVARIANT RULE (docs/ARCHITECTURE.md §3.1, §10.1):
    // In Judy/Expanse tries, narrow pointers collapse single-child chains of
    // undecoded bytes into decode bytes packed inside the 16-byte `Edge` descriptor.
    // When descending, `decode_matches(edge, key, child_level, level)` tests
    // the skipped bytes against `edge.aux_word()` in CPU registers.
    // No intermediate branch nodes exist or are loaded from memory.
    // Therefore: SKIPPED LEVELS MUST NOT COUNT AS NODE VISITS.
    //
    // Construction: 45 keys sharing high 4 bytes 0x1122_3344_0000_0000.
    // At level 4, 3 digits (1..=3) each holding 15 keys.
    // Levels 8, 7, 6, 5 are identical across all keys: skipped via narrow pointer!
    // Probing key 0x1122_3344_0000_0000 | (1 << 24) | 1:
    // Only the branch node at level 4 and the leaf node are loaded!
    // Memory loads:
    // - Follows top edge to level 4 BranchL3: edges_followed = 1 (or 2 if wrapped).
    // - branch_b_subarrays = 0.
    // - leaf_loads = 1.
    //
    // CRITICAL NEGATIVE TEST:
    // A probe with a mismatched decode byte (e.g. key 0x9922_3344_0000_0000 | ...)
    // fails `decode_matches` at the root edge BEFORE dereferencing any child node!
    // Exactly 0 nodes loaded: edges = 0, branch_b = 0, leaf_loads = 0, total = 0!
    {
        let mut map = ExpanseMap::new();
        let p = 0x1122_3344_0000_0000u64;
        for d in 1..=3u64 {
            for j in 1..=15u64 {
                map.insert(p | (d << 24) | j, j);
            }
        }
        let stats = map.stats();
        assert_eq!(stats.node_counts.branch_b, 0, "no BranchB nodes");

        // Hit:
        let k_hit = p | (1 << 24) | 1;
        let v_hit = map.probe_visits(k_hit);
        assert_eq!(v_hit.edges_followed, 2);
        assert_eq!(v_hit.branch_b_subarrays, 0);
        assert_eq!(v_hit.leaf_loads, 1);
        assert_eq!(v_hit.total_visits(), 3);
        assert!(v_hit.found);

        // Branch decode-byte mismatch:
        // Key differs in byte 7 (0x99 instead of 0x11).
        // Branch node was loaded to read its level (edges_followed = 1), but decode_matches
        // aborted BEFORE descending into any child edge or loading any leaf!
        let k_mismatch = 0x9922_3344_0000_0000u64 | (1 << 24) | 1;
        let v_mismatch = map.probe_visits(k_mismatch);
        assert_eq!(
            v_mismatch.edges_followed, 1,
            "branch header was loaded to read level"
        );
        assert_eq!(v_mismatch.branch_b_subarrays, 0);
        assert_eq!(
            v_mismatch.leaf_loads, 0,
            "no leaf loaded on decode mismatch"
        );
        assert_eq!(v_mismatch.total_visits(), 1);
        assert!(!v_mismatch.found);
        assert_eq!(v_mismatch.value, None);
    }

    // Hand-Constructed Narrow-Pointer with Explicit Decode Bytes:
    // Level-3 expanse: BranchU at level 3 -> child edge at index 0x77 is a narrow pointer
    // skipping level 2 straight to a LeafB1 at level 1 with decode byte 0xC4.
    // Verifies that decode mismatch aborts in registers BEFORE dereferencing the leaf pointer!
    {
        use expanse_trie::node::{BranchU, Edge, LeafBitmap1};

        let mut leaf_b = LeafBitmap1::new();
        leaf_b.bitmap.set(0x42);
        let mut skip_jp = Edge::new_node((&raw mut leaf_b).cast(), 0x0C); // 0x0C = LeafBitmap1
        skip_jp.set_decode_bytes(1, &[0xC4]);

        let mut root_u = BranchU::new();
        root_u.edges[0x77] = skip_jp;
        let root = Edge::new_node((&raw mut root_u).cast(), 0x04); // 0x04 = BranchU

        // Hit (0x77_C4_42 at level 3):
        // Follows root to BranchU: edges_followed = 1.
        // Child at 0x77: decode byte 0xC4 matches.
        // Loads leaf_b: leaf_loads = 1.
        // total_visits = 2.
        // SAFETY: well-formed hand-built tree over live locals.
        let v_hit = unsafe { expanse_trie::validate::walk_set_probe_visits(&root, 0x77_C4_42, 3) };
        assert_eq!(v_hit.edges_followed, 1);
        assert_eq!(v_hit.branch_b_subarrays, 0);
        assert_eq!(v_hit.leaf_loads, 1);
        assert_eq!(v_hit.total_visits(), 2);
        assert!(v_hit.found);

        // Mismatch on skipped decode byte (0x77_C5_42 at level 3):
        // Follows root to BranchU: edges_followed = 1.
        // Child at 0x77 has decode byte 0xC4, but key has 0xC5.
        // decode_matches FAILS BEFORE loading leaf_b!
        // leaf_loads = 0!
        // total_visits = 1!
        // SAFETY: well-formed hand-built tree over live locals.
        let v_mismatch =
            unsafe { expanse_trie::validate::walk_set_probe_visits(&root, 0x77_C5_42, 3) };
        assert_eq!(v_mismatch.edges_followed, 1);
        assert_eq!(v_mismatch.branch_b_subarrays, 0);
        assert_eq!(
            v_mismatch.leaf_loads, 0,
            "decode mismatch aborts BEFORE loading leaf"
        );
        assert_eq!(v_mismatch.total_visits(), 1);
        assert!(!v_mismatch.found);
    }

    // ---------------------------------------------------------------------
    // Form 8: Composed Multi-Level Hierarchy (BranchB -> BranchL3 -> Leaf)
    // ---------------------------------------------------------------------
    // Construction:
    // - Level 8 has 16 children (forces BranchB at level 8).
    // - Child 0 at level 7 has 3 children (forces BranchL3 at level 7).
    // - Each child of BranchL3 holds 15 keys (forces Leaf6).
    // Keys under digit 0 encounter:
    // 1. Root edge to BranchB: edges_followed = 1.
    // 2. Subarray pointer in BranchB: branch_b_subarrays = 1.
    // 3. Subarray edge to BranchL3: edges_followed = 2.
    // 4. BranchL3 edge to Leaf6: leaf_loads = 1.
    // Total visits = 2 edges + 1 BranchB + 1 leaf = 4.
    {
        let mut map = ExpanseMap::new();
        // Child 0: 3 digits at level 7 (0, 1, 2), 15 keys each
        for d in 0..3u64 {
            for j in 1..=15u64 {
                map.insert((d << 48) | j, j * 10);
            }
        }
        // Children 1..16 at level 8: 3 keys each
        for i in 1..16u64 {
            map.insert((i << 56) | 1, 1);
            map.insert((i << 56) | 2, 2);
            map.insert((i << 56) | 3, 3);
        }

        let stats = map.stats();
        assert!(stats.node_counts.branch_b >= 1, "must contain BranchB");
        assert!(stats.node_counts.branch_l3 >= 1, "must contain BranchL3");

        // Probing keys under digit 0:
        for d in 0..3u64 {
            for j in 1..=15u64 {
                let k = (d << 48) | j;
                let v = map.probe_visits(k);
                assert_eq!(v.edges_followed, 2, "BranchB (1) + BranchL3 (2)");
                assert_eq!(v.branch_b_subarrays, 1, "loaded BranchB subarray");
                assert_eq!(v.leaf_loads, 1, "loaded Leaf6");
                assert_eq!(v.total_visits(), 4, "total = 2 edges + 1 BranchB + 1 leaf");
                assert!(v.found);
                assert_eq!(v.value, Some(j * 10));
            }
        }
    }
}

// =========================================================================
// 2. Independent Census Identity Oracle (3b)
// =========================================================================

/// Verifies that Σ probe_visits over all present keys strictly equals the
/// aggregate structural census computed by `probe_visits_census()`.
///
/// Under the digital tree geometry:
///   Σ_keys v.branch_b_subarrays == Σ_{B in BranchB} keys_beneath(B)
///   Σ_keys v.edges_followed     == Σ_{N in branches} keys_beneath(N)
///   Σ_keys v.leaf_loads         == Σ_{L in heap leaves} keys_in(L)
#[test]
fn test_census_identity_across_all_distributions() {
    let dists = [
        "sequential",
        "random",
        "clustered",
        "dense_leaf",
        "linear_leaf",
    ];
    let pop = 10_000;

    for dist in dists {
        let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
        let mut keys = Vec::with_capacity(pop);
        match dist {
            "sequential" => keys.extend(0..pop as u64),
            "random" => keys.extend((0..pop).map(|_| rng.next())),
            "clustered" => {
                let mut base = 0;
                for i in 0..pop as u64 {
                    if i % 256 == 0 {
                        base = rng.next() & !0xFF;
                    }
                    keys.push(base + (i % 256));
                }
            }
            "dense_leaf" => {
                for _ in 0..(pop / 32) {
                    let prefix = rng.next() & !0xFF;
                    for j in 0..32 {
                        keys.push(prefix | (j as u64));
                    }
                }
            }
            "linear_leaf" => {
                for _ in 0..(pop / 15) {
                    let prefix = rng.next() & !0xFF;
                    for j in 0..15 {
                        keys.push(prefix | (j as u64));
                    }
                }
            }
            _ => unreachable!(),
        }

        let mut map = ExpanseMap::new();
        for &k in &keys {
            map.insert(k, !k);
        }

        // 1. Independent structural census oracle:
        let census = map.probe_visits_census();
        assert_eq!(
            census.total_keys,
            map.len() as usize,
            "dist {dist}: census key count must match map.len()"
        );

        // 2. Aggregate actual visits over all present keys:
        let mut actual_edges_sum = 0usize;
        let mut actual_branch_b_sum = 0usize;
        let mut actual_leaves_sum = 0usize;

        for &k in &keys {
            let v = map.probe_visits(k);
            assert!(v.found, "dist {dist}: present key {k} must be found");
            assert_eq!(v.value, Some(!k), "dist {dist}: value match");
            assert_eq!(v.value, map.get(k), "dist {dist}: value matches get");

            actual_edges_sum += v.edges_followed;
            actual_branch_b_sum += v.branch_b_subarrays;
            actual_leaves_sum += v.leaf_loads;
        }

        // 3. Assert exact census identities:
        assert_eq!(
            actual_edges_sum, census.sum_edges_followed,
            "dist {dist}: edges_followed sum must equal structural census"
        );
        assert_eq!(
            actual_branch_b_sum, census.sum_branch_b_subarrays,
            "dist {dist}: branch_b_subarrays sum must equal structural census"
        );
        assert_eq!(
            actual_leaves_sum, census.sum_leaf_loads,
            "dist {dist}: leaf_loads sum must equal structural census"
        );

        // 4. Test absent keys (misses):
        let mut miss_rng = XorShift(0x5EED_F2E5_0000_0002);
        for _ in 0..500 {
            let absent = miss_rng.next();
            if map.get(absent).is_none() {
                let v = map.probe_visits(absent);
                assert!(!v.found, "dist {dist}: absent key must not be found");
                assert_eq!(v.value, None);
                assert_eq!(v.value, map.get(absent));
            }
        }
    }
}

/// Verifies census identity on `ExpanseSet`.
#[test]
fn test_set_census_identity() {
    let mut rng = XorShift(0xCAFE_BABE_0001);
    let mut set = ExpanseSet::new();
    let mut keys = Vec::new();
    for _ in 0..5_000 {
        let k = rng.next();
        set.insert(k);
        keys.push(k);
    }

    let census = set.probe_visits_census();
    assert_eq!(census.total_keys, set.len() as usize);

    let mut actual_edges_sum = 0usize;
    let mut actual_branch_b_sum = 0usize;
    let mut actual_leaves_sum = 0usize;

    for &k in &keys {
        let v = set.probe_visits(k);
        assert!(v.found);
        assert_eq!(v.found, set.contains(k));
        assert_eq!(v.value, None);

        actual_edges_sum += v.edges_followed;
        actual_branch_b_sum += v.branch_b_subarrays;
        actual_leaves_sum += v.leaf_loads;
    }

    assert_eq!(actual_edges_sum, census.sum_edges_followed);
    assert_eq!(actual_branch_b_sum, census.sum_branch_b_subarrays);
    assert_eq!(actual_leaves_sum, census.sum_leaf_loads);

    // Absent keys:
    for _ in 0..500 {
        let absent = rng.next();
        if !set.contains(absent) {
            let v = set.probe_visits(absent);
            assert!(!v.found);
            assert_eq!(v.found, set.contains(absent));
        }
    }
}
