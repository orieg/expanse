//! Phase 6b: `ExpanseMap`, the public map-flavor tree (compat: JudyL).
//!
//! Root organization mirrors `ExpanseSet`: populations up to
//! [`crate::set::ROOT_LEAF_CAP`] live in a root leaf — here parallel
//! sorted-key and value arrays in one allocation — before a level-8 trie
//! exists. The tree condenses back into a root
//! leaf when its population falls one below the promotion boundary.

use crate::alloc::NodeAlloc;
use crate::get;
use crate::mutate;
use crate::mutate_map;
use crate::node::Edge;
use crate::set::ROOT_LEAF_CAP;
use crate::sync::RootSnapshot;
use crate::types::Key;
use crate::validate::ExpanseStats;
use core::ptr::NonNull;

#[derive(Clone, Copy)]
enum Root {
    Empty,
    /// One allocation: `pop` sorted keys, then `pop` values.
    Leaf {
        ptr: NonNull<u8>,
        pop: usize,
    },
    /// A level-8 trie; `pop` is the total population (the JPM role).
    Tree {
        top: Edge,
        pop: u64,
    },
}

/// The map engine core: root organization plus every walk and mutation,
/// with **no owned allocator and no owned insert-path cache** — both are
/// passed in per call (issue #363 Step A).
///
/// [`ExpanseMap`] wraps one core with its own [`NodeAlloc`] and path
/// cache, forwarding through `#[inline(always)]` shims so the compiled
/// public paths are identical to the pre-split layout. `ExpanseStrMap`
/// embeds a bare core per sub-trie node and passes the **one allocator
/// shared across the whole string map**, which is what shrinks a
/// `StrNode` from ~700 bytes (embedded allocator + path cache) to the
/// size of this struct.
///
/// A core frees nothing on drop — it cannot, without its allocator — so
/// every owner must route teardown through [`Self::clear`] (or the
/// pathless twin) with the allocator that produced the core's nodes.
pub(crate) struct MapCore {
    root: Root,
}

/// A sparse, dynamic map from `u64` keys to `u64` values (compat: JudyL).
///
/// Adaptive expanse-partitioned trie: memory stays near-proportional to
/// population across sequential, random, clustered, and sparse key
/// distributions, and lookups run in at most eight digit steps.
pub struct ExpanseMap {
    core: MapCore,
    alloc: NodeAlloc,
    path: core::cell::UnsafeCell<crate::mutate_map::InsertPathMap>,
}

// SAFETY: as for `ExpanseSet` — exclusive ownership of all reachable
// allocations; not `Sync`, shared access goes through `SyncExpanseMap`.
unsafe impl Send for ExpanseMap {}

/// Allocation size of a root leaf holding `pop` entries: a class-sized
/// key area followed by a class-sized value area. Class-sizing (as the
/// trie's linear leaves already do) means consecutive inserts and
/// deletes shift in place instead of reallocating on every operation —
/// without it, every map a C caller keeps under 32 entries paid a
/// malloc, a full copy and a free per insert (issue #1).
fn leaf_size(pop: usize) -> usize {
    16 * crate::leaf::cap_class(pop)
}

/// Offset of the value area inside a root leaf of `pop` entries. Keyed
/// to the capacity class, not the population, so it does not move when
/// the population changes within a class.
///
/// **This is the single definition of the root-leaf layout.** It is
/// `pub(crate)` because the concurrent read path in `sync` must use the
/// same rule: when this was duplicated there, the two copies drifted the
/// moment capacity classes arrived and readers returned a neighbouring
/// key's value (caught by `sync::tests::concurrent_readers_under_churn`
/// on aarch64). Anything that needs the value area asks here.
pub(crate) fn leaf_values_offset(pop: usize) -> usize {
    8 * crate::leaf::cap_class(pop)
}

impl MapCore {
    /// An empty core.
    pub(crate) const fn new() -> Self {
        Self { root: Root::Empty }
    }

    /// Number of keys in the map.
    #[inline(always)]
    #[must_use]
    pub(crate) fn len(&self) -> u64 {
        match &self.root {
            Root::Empty => 0,
            Root::Leaf { pop, .. } => *pop as u64,
            Root::Tree { pop, .. } => *pop,
        }
    }

    /// True when no keys are present.
    #[inline(always)]
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn leaf_parts(ptr: NonNull<u8>, pop: usize) -> (&'static [u64], *mut u64) {
        // SAFETY: the root leaf holds `pop` keys in a class-sized area,
        // then `pop` values in a second class-sized area, both 8-aligned
        // (allocations are cache-line aligned). The lifetime is scoped by
        // callers to the borrow of self.
        unsafe {
            (
                core::slice::from_raw_parts(ptr.as_ptr().cast::<u64>(), pop),
                ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>(),
            )
        }
    }

    /// Returns the value stored for `key`.
    #[inline(always)]
    #[must_use]
    pub(crate) fn get(&self, key: Key) -> Option<u64> {
        match &self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let pop = *pop;
                let kptr = ptr.as_ptr().cast::<u64>();
                let at = if pop <= 4 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if pop >= 1 && *kptr == key {
                            0
                        } else if pop >= 2 && *kptr.add(1) == key {
                            1
                        } else if pop >= 3 && *kptr.add(2) == key {
                            2
                        } else if pop >= 4 && *kptr.add(3) == key {
                            3
                        } else {
                            return None;
                        }
                    }
                } else if pop <= 8 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if *kptr == key {
                            0
                        } else if *kptr.add(1) == key {
                            1
                        } else if *kptr.add(2) == key {
                            2
                        } else if *kptr.add(3) == key {
                            3
                        } else if pop >= 5 && *kptr.add(4) == key {
                            4
                        } else if pop >= 6 && *kptr.add(5) == key {
                            5
                        } else if pop >= 7 && *kptr.add(6) == key {
                            6
                        } else if pop >= 8 && *kptr.add(7) == key {
                            7
                        } else {
                            return None;
                        }
                    }
                } else {
                    // SAFETY: root leaf holds `pop` keys.
                    let keys = unsafe { core::slice::from_raw_parts(kptr, pop) };
                    keys.binary_search(&key).ok()?
                };
                // SAFETY: root leaf values live at values offset.
                let vptr = unsafe { ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>() };
                // SAFETY: `at < pop` values live behind the keys.
                Some(unsafe { *vptr.add(at) })
            }
            // SAFETY: the trie is maintained by the map mutation engine
            // and satisfies the lookup contract.
            Root::Tree { top, .. } => unsafe { get::get_map(top, key, 8) },
        }
    }

    /// Look up a batch of `keys` simultaneously, writing values into `out`.
    ///
    /// When the root is a multi-level digital trie, `get_batch` interleaves key descents
    /// across CPU Line Fill Buffers in chunks of 8 keys and issues software prefetch hints
    /// on branch nodes to overlap DRAM memory latency.
    #[inline]
    pub(crate) fn get_batch(&self, keys: &[Key], out: &mut [Option<u64>]) {
        assert_eq!(
            keys.len(),
            out.len(),
            "keys and out slices must have equal length"
        );
        if keys.is_empty() {
            return;
        }
        match &self.root {
            Root::Empty => {
                out.fill(None);
            }
            Root::Leaf { .. } => {
                for (k, o) in keys.iter().zip(out.iter_mut()) {
                    *o = self.get(*k);
                }
            }
            Root::Tree { top, .. } => {
                // SAFETY: tree satisfies lookup invariants.
                unsafe {
                    get::get_map_batch(top, keys, out, 8);
                }
            }
        }
    }

    /// Look up a batch of `keys`, writing found values into `out_values` and presence flags
    /// into `out_found` (when `Some`). Returns the count of found keys.
    #[inline]
    pub(crate) fn get_batch_into(
        &self,
        keys: &[Key],
        out_values: &mut [u64],
        mut out_found: Option<&mut [bool]>,
    ) -> usize {
        assert_eq!(
            keys.len(),
            out_values.len(),
            "keys and out_values must have equal length"
        );
        if let Some(ref found) = out_found {
            assert_eq!(
                keys.len(),
                found.len(),
                "keys and out_found must have equal length"
            );
        }
        if keys.is_empty() {
            return 0;
        }

        let mut found_count = 0;
        let mut tmp_opts = [None; 8];
        let mut offset = 0;

        for (k_chunk, v_chunk) in keys.chunks(8).zip(out_values.chunks_mut(8)) {
            let chunk_len = k_chunk.len();
            let opt_sub = &mut tmp_opts[..chunk_len];
            self.get_batch(k_chunk, opt_sub);
            for (idx, opt) in opt_sub.iter().enumerate() {
                let is_hit = opt.is_some();
                if let Some(val) = *opt {
                    v_chunk[idx] = val;
                    found_count += 1;
                }
                if let Some(ref mut found_slice) = out_found {
                    found_slice[offset + idx] = is_hit;
                }
            }
            offset += chunk_len;
        }

        found_count
    }

    /// Returns a pointer to `key`'s value slot in the leaf or root leaf, or `None`
    /// if absent.
    #[inline(always)]
    #[must_use]
    pub(crate) fn get_slot_ptr(&self, key: Key) -> Option<core::ptr::NonNull<u64>> {
        match &self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let pop = *pop;
                let kptr = ptr.as_ptr().cast::<u64>();
                let at = if pop <= 4 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if pop >= 1 && *kptr == key {
                            0
                        } else if pop >= 2 && *kptr.add(1) == key {
                            1
                        } else if pop >= 3 && *kptr.add(2) == key {
                            2
                        } else if pop >= 4 && *kptr.add(3) == key {
                            3
                        } else {
                            return None;
                        }
                    }
                } else if pop <= 8 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if *kptr == key {
                            0
                        } else if *kptr.add(1) == key {
                            1
                        } else if *kptr.add(2) == key {
                            2
                        } else if *kptr.add(3) == key {
                            3
                        } else if pop >= 5 && *kptr.add(4) == key {
                            4
                        } else if pop >= 6 && *kptr.add(5) == key {
                            5
                        } else if pop >= 7 && *kptr.add(6) == key {
                            6
                        } else if pop >= 8 && *kptr.add(7) == key {
                            7
                        } else {
                            return None;
                        }
                    }
                } else {
                    // SAFETY: root leaf holds `pop` keys.
                    let keys = unsafe { core::slice::from_raw_parts(kptr, pop) };
                    keys.binary_search(&key).ok()?
                };
                // SAFETY: root leaf values live at values offset.
                let vptr = unsafe { ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>() };
                // SAFETY: `at < pop` values live behind the keys.
                core::ptr::NonNull::new(unsafe { vptr.add(at) })
            }
            Root::Tree { top, .. } => {
                // SAFETY: top is live valid tree root pointer.
                unsafe { crate::get::locate_slot((&raw const *top).cast_mut(), key, 8) }
            }
        }
    }

    /// Returns a **writable pointer to `key`'s value slot**, or `None` if
    /// the key is absent — the compat layer's `JudyLGet`/`JudyLIns` return
    /// convention. The pointer stays valid until the next structural
    /// mutation of the map (the classic JudyL contract); reading or
    /// writing through it after an `insert`/`remove`/`clear` is undefined.
    #[inline(always)]
    #[must_use]
    pub(crate) fn get_value_slot(
        &mut self,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<core::ptr::NonNull<u64>> {
        match &mut self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let pop = *pop;
                let kptr = ptr.as_ptr().cast::<u64>();
                let at = if pop <= 4 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if pop >= 1 && *kptr == key {
                            0
                        } else if pop >= 2 && *kptr.add(1) == key {
                            1
                        } else if pop >= 3 && *kptr.add(2) == key {
                            2
                        } else if pop >= 4 && *kptr.add(3) == key {
                            3
                        } else {
                            return None;
                        }
                    }
                } else if pop <= 8 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if *kptr == key {
                            0
                        } else if *kptr.add(1) == key {
                            1
                        } else if *kptr.add(2) == key {
                            2
                        } else if *kptr.add(3) == key {
                            3
                        } else if pop >= 5 && *kptr.add(4) == key {
                            4
                        } else if pop >= 6 && *kptr.add(5) == key {
                            5
                        } else if pop >= 7 && *kptr.add(6) == key {
                            6
                        } else if pop >= 8 && *kptr.add(7) == key {
                            7
                        } else {
                            return None;
                        }
                    }
                } else {
                    // SAFETY: root leaf holds `pop` keys.
                    let keys = unsafe { core::slice::from_raw_parts(kptr, pop) };
                    keys.binary_search(&key).ok()?
                };
                // SAFETY: root leaf values live at values offset.
                let vptr = unsafe { ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>() };
                // SAFETY: `at < pop` values live behind the keys.
                core::ptr::NonNull::new(unsafe { vptr.add(at) })
            }
            // SAFETY: trie maintained/owned by this map's engine; the
            // raw walk derives the slot from node pointers only.
            Root::Tree { top, .. } => {
                let prefix = key >> 8;
                if path.prefix == prefix {
                    if !path.leaf.is_null() {
                        let d = (key & 0xFF) as u8;
                        // SAFETY: path holds valid live LeafBitmapL pointer.
                        let node = unsafe { &*path.leaf };
                        let sub = (d >> 5) as usize;
                        if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
                            // SAFETY: sub < 8 accesses valid subarray; rank is in bounds.
                            let slot = unsafe { (*node.values.as_ptr().add(sub)).add(rank) };
                            return core::ptr::NonNull::new(slot);
                        }
                        return None;
                    } else if !path.leaf1.is_null() {
                        let d = (key & 0xFF) as u8;
                        let cur_pop = path.terminal_pop as usize;
                        let base = path.leaf1;
                        // SAFETY: base points to a live Leaf1 allocation; map_keys_offset is in-bounds.
                        let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(cur_pop)) };
                        // SAFETY: keys_ptr holds cur_pop 1-byte keys.
                        let slot =
                            unsafe { crate::leaf::search_fixed::<1>(keys_ptr, cur_pop, d as u64) };
                        if let Some(slot) = slot {
                            // SAFETY: slot < cur_pop values live behind keys.
                            return core::ptr::NonNull::new(unsafe {
                                base.cast::<u64>().add(slot)
                            });
                        }
                        return None;
                    }
                }
                // SAFETY: top is live valid tree root pointer.
                unsafe { crate::get::locate_slot(&raw mut *top, key, 8) }
            }
        }
    }

    /// Inserts `key` with value 0 if absent — the existing value is kept
    /// untouched — and returns a **writable pointer to its value slot**:
    /// the compat `JudyLIns` contract, in one tree walk. The pointer stays
    /// valid until the next structural mutation.
    #[inline(always)]
    pub(crate) fn ins_slot(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> core::ptr::NonNull<u64> {
        match &mut self.root {
            Root::Tree { top, pop } => {
                let prefix = key >> 8;
                if path.prefix == prefix {
                    if !path.leaf.is_null() {
                        let d = (key & 0xFF) as u8;
                        // SAFETY: path holds valid live LeafBitmapL pointer.
                        let node = unsafe { &mut *path.leaf };
                        let sub = (d >> 5) as usize;
                        if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
                            // SAFETY: value subarray holds subexpanse_count values.
                            let slot = unsafe { node.values[sub].add(rank) };
                            return core::ptr::NonNull::new(slot).expect("slot");
                        }
                        let rank = node.bitmap.subexpanse_rank(d) as usize;
                        let old_n = node.bitmap.subexpanse_count(sub) as usize;
                        if old_n > 0
                            && crate::leaf::cap_class(old_n + 1) == crate::leaf::cap_class(old_n)
                        {
                            // Fast path: spare class capacity — shift in place.
                            // SAFETY: the subarray holds cap_class(old_n) slots.
                            unsafe {
                                let arr = node.values[sub];
                                core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                                arr.add(rank).write(0);
                            }
                        } else {
                            let new = alloc
                                .alloc_bytes(crate::mutate::sub_vals_size(old_n + 1))
                                .cast::<u64>();
                            // SAFETY: copy old_n values around the inserted rank.
                            unsafe {
                                if old_n > 0 {
                                    let old = node.values[sub];
                                    new.as_ptr().copy_from_nonoverlapping(old, rank);
                                    new.as_ptr()
                                        .add(rank + 1)
                                        .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                                    alloc.free_bytes(
                                        core::ptr::NonNull::new(old.cast()).expect("values"),
                                        crate::mutate::sub_vals_size(old_n),
                                    );
                                }
                                new.as_ptr().add(rank).write(0);
                            }
                            node.values[sub] = new.as_ptr();
                        }
                        node.bitmap.set(d);
                        path.pending_pop += 1;
                        path.terminal_pop += 1;
                        *pop += 1;
                        // SAFETY: keep terminal edge pop0 up to date.
                        unsafe {
                            (*path.edges[0]).set_pop0(1, (path.terminal_pop - 1) as u64);
                        }
                        // SAFETY: freshly inserted slot.
                        let slot = unsafe { node.values[sub].add(rank) };
                        return core::ptr::NonNull::new(slot).expect("slot");
                    } else if !path.leaf1.is_null() {
                        let d = (key & 0xFF) as u8;
                        let cur_pop = path.terminal_pop as usize;
                        let base = path.leaf1;
                        // SAFETY: base points to a live Leaf1 allocation; map_keys_offset is in-bounds.
                        let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(cur_pop)) };
                        // SAFETY: cur_pop >= 1 when leaf1 is active, so cur_pop - 1 is in bounds.
                        let last = unsafe { *keys_ptr.add(cur_pop - 1) };
                        if d > last {
                            if cur_pop < crate::mutate::LEAF1_CAP
                                && crate::leaf::cap_class(cur_pop + 1)
                                    == crate::leaf::cap_class(cur_pop)
                            {
                                // SAFETY: spare class capacity in the live Leaf1 allocation.
                                unsafe {
                                    *keys_ptr.add(cur_pop) = d;
                                    let vals = base.cast::<u64>();
                                    vals.add(cur_pop).write(0);
                                    (*path.edges[0]).set_pop0(1, cur_pop as u64);
                                }
                                path.terminal_pop += 1;
                                path.pending_pop += 1;
                                *pop += 1;
                                // SAFETY: freshly written slot in live value area.
                                let slot = unsafe { base.cast::<u64>().add(cur_pop) };
                                return core::ptr::NonNull::new(slot).expect("slot");
                            }
                        } else if d == last {
                            // SAFETY: cur_pop - 1 is the existing slot for `last`.
                            let slot = unsafe { base.cast::<u64>().add(cur_pop - 1) };
                            return core::ptr::NonNull::new(slot).expect("slot");
                        }
                    }
                }
                path.clear();
                // SAFETY: trie maintained/owned by this map's engine.
                let (prev, slot) =
                    unsafe { mutate_map::map_insert_path_dyn::<true>(alloc, top, key, 0, 8, path) };
                if prev.is_none() {
                    *pop += 1;
                }
                // SAFETY: map_insert always returns a valid, non-null slot pointer.
                unsafe { core::ptr::NonNull::new_unchecked(slot) }
            }
            Root::Empty => {
                let ptr = alloc.alloc_bytes(leaf_size(1));
                // SAFETY: fresh allocation: key slot then value slot.
                unsafe {
                    ptr.as_ptr().cast::<u64>().write(key);
                    let vptr = ptr.as_ptr().add(leaf_values_offset(1)).cast::<u64>();
                    vptr.write(0);
                    self.root = Root::Leaf { ptr, pop: 1 };
                    core::ptr::NonNull::new(vptr).expect("slot")
                }
            }
            Root::Leaf { ptr, pop } => {
                let (ptr_val, pop_val) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr_val, pop_val);
                let (hit, at) = if pop_val > 0 {
                    let last = keys[pop_val - 1];
                    if key > last {
                        (false, pop_val)
                    } else if key == last {
                        (true, pop_val - 1)
                    } else if pop_val <= 4 {
                        let k0 = keys[0];
                        if key < k0 {
                            (false, 0)
                        } else if key == k0 {
                            (true, 0)
                        } else if pop_val == 2 {
                            (false, 1)
                        } else {
                            let k1 = keys[1];
                            if key < k1 {
                                (false, 1)
                            } else if key == k1 {
                                (true, 1)
                            } else if pop_val == 3 {
                                (false, 2)
                            } else {
                                let k2 = keys[2];
                                if key < k2 {
                                    (false, 2)
                                } else if key == k2 {
                                    (true, 2)
                                } else {
                                    (false, 3)
                                }
                            }
                        }
                    } else {
                        match keys.binary_search(&key) {
                            Ok(pos) => (true, pos),
                            Err(pos) => (false, pos),
                        }
                    }
                } else {
                    (false, 0)
                };
                if hit {
                    // SAFETY: vals points to live values array in the root leaf.
                    let slot = unsafe { vals.add(at) };
                    return core::ptr::NonNull::new(slot).expect("slot");
                }
                if pop_val < ROOT_LEAF_CAP {
                    if leaf_size(pop_val + 1) == leaf_size(pop_val) {
                        // Spare class capacity: shift in place, no realloc.
                        // SAFETY: same class, areas keep offsets, slot is in bounds.
                        unsafe {
                            let base = ptr_val.as_ptr().cast::<u64>();
                            core::ptr::copy(base.add(at), base.add(at + 1), pop_val - at);
                            base.add(at).write(key);
                            let v = ptr_val
                                .as_ptr()
                                .add(leaf_values_offset(pop_val))
                                .cast::<u64>();
                            core::ptr::copy(v.add(at), v.add(at + 1), pop_val - at);
                            let slot = v.add(at);
                            slot.write(0);
                            self.root = Root::Leaf {
                                ptr: ptr_val,
                                pop: pop_val + 1,
                            };
                            core::ptr::NonNull::new(slot).expect("slot")
                        }
                    } else {
                        let new = alloc.alloc_bytes(leaf_size(pop_val + 1));
                        // SAFETY: copy keys and values around insertion point into new leaf.
                        unsafe {
                            let nk = new.as_ptr().cast::<u64>();
                            nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                            nk.add(at).write(key);
                            nk.add(at + 1)
                                .copy_from_nonoverlapping(keys.as_ptr().add(at), pop_val - at);
                            let nv = new
                                .as_ptr()
                                .add(leaf_values_offset(pop_val + 1))
                                .cast::<u64>();
                            nv.copy_from_nonoverlapping(vals, at);
                            let slot = nv.add(at);
                            slot.write(0);
                            nv.add(at + 1)
                                .copy_from_nonoverlapping(vals.add(at), pop_val - at);
                            alloc.free_bytes(ptr_val, leaf_size(pop_val));
                            self.root = Root::Leaf {
                                ptr: new,
                                pop: pop_val + 1,
                            };
                            core::ptr::NonNull::new(slot).expect("slot")
                        }
                    }
                } else {
                    self.insert(alloc, key, 0, path);
                    self.get_value_slot(key, path).expect("just-ensured key")
                }
            }
        }
    }

    /// Phase 7 (occ): by-value root snapshot for the validated concurrent
    /// read walk (see `ExpanseSet::occ_root`).
    #[inline(always)]
    pub(crate) fn occ_snapshot(&self) -> RootSnapshot {
        match self.root {
            Root::Empty => RootSnapshot::Empty,
            Root::Leaf { ptr, pop } => RootSnapshot::Leaf {
                ptr: ptr.as_ptr(),
                pop,
            },
            Root::Tree { top, pop } => RootSnapshot::Tree { top, pop },
        }
    }

    /// Membership test.
    #[inline(always)]
    #[must_use]
    pub(crate) fn contains_key(&self, key: Key) -> bool {
        self.get(key).is_some()
    }

    /// Inserts `key → val`; returns the replaced value if the key was
    /// already present.
    #[inline(always)]
    pub(crate) fn insert(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        match &mut self.root {
            Root::Empty => {
                let ptr = alloc.alloc_bytes(leaf_size(1));
                // SAFETY: fresh allocation: key slot then value slot.
                unsafe {
                    ptr.as_ptr().cast::<u64>().write(key);
                    ptr.as_ptr().cast::<u64>().add(1).write(val);
                }
                self.root = Root::Leaf { ptr, pop: 1 };
                None
            }
            Root::Leaf { ptr, pop } => {
                let (ptr, pop) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr, pop);
                let (hit, at) = if pop > 0 {
                    let last = keys[pop - 1];
                    if key > last {
                        (false, pop)
                    } else if key == last {
                        (true, pop - 1)
                    } else {
                        match keys.binary_search(&key) {
                            Ok(pos) => (true, pos),
                            Err(pos) => (false, pos),
                        }
                    }
                } else {
                    (false, 0)
                };
                if hit {
                    // SAFETY: in-place value swap.
                    unsafe {
                        let slot = vals.add(at);
                        let old = *slot;
                        slot.write(val);
                        return Some(old);
                    }
                }
                if pop < ROOT_LEAF_CAP {
                    if leaf_size(pop + 1) == leaf_size(pop) {
                        // Spare class capacity: shift both areas in
                        // place, no allocation and no copy of the
                        // whole leaf.
                        // SAFETY: same class, so the areas keep their
                        // offsets and the extra slot is in bounds.
                        unsafe {
                            let base = ptr.as_ptr().cast::<u64>();
                            core::ptr::copy(base.add(at), base.add(at + 1), pop - at);
                            base.add(at).write(key);
                            let v = ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>();
                            core::ptr::copy(v.add(at), v.add(at + 1), pop - at);
                            v.add(at).write(val);
                        }
                        self.root = Root::Leaf { ptr, pop: pop + 1 };
                        return None;
                    }
                    let new = alloc.alloc_bytes(leaf_size(pop + 1));
                    // SAFETY: copy keys and values around the insertion
                    // point into the fresh (pop + 1)-entry leaf.
                    unsafe {
                        let nk = new.as_ptr().cast::<u64>();
                        nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                        nk.add(at).write(key);
                        nk.add(at + 1)
                            .copy_from_nonoverlapping(keys.as_ptr().add(at), pop - at);
                        let nv = new.as_ptr().add(leaf_values_offset(pop + 1)).cast::<u64>();
                        nv.copy_from_nonoverlapping(vals, at);
                        nv.add(at).write(val);
                        nv.add(at + 1)
                            .copy_from_nonoverlapping(vals.add(at), pop - at);
                        alloc.free_bytes(ptr, leaf_size(pop));
                    }
                    self.root = Root::Leaf {
                        ptr: new,
                        pop: pop + 1,
                    };
                    None
                } else {
                    // Root leaf overflow: build the level-8 trie.
                    let mut top = Edge::NULL;
                    for (at, &k) in keys.iter().enumerate() {
                        // SAFETY: trie built and owned by `alloc`;
                        // values read in-bounds.
                        let prev = unsafe {
                            mutate_map::map_insert_dyn::<false>(
                                alloc,
                                &mut top,
                                k,
                                *vals.add(at),
                                8,
                            )
                        };
                        debug_assert!(prev.0.is_none());
                    }
                    // SAFETY: same trie; populate path for subsequent sequential/clustered inserts.
                    let prev = unsafe {
                        mutate_map::map_insert_path_dyn::<false>(alloc, &mut top, key, val, 8, path)
                    };
                    debug_assert!(prev.0.is_none());
                    // SAFETY: old root leaf no longer referenced.
                    unsafe { alloc.free_bytes(ptr, leaf_size(pop)) };
                    self.root = Root::Tree {
                        top,
                        pop: pop as u64 + 1,
                    };
                    None
                }
            }
            Root::Tree { top, pop } => {
                let prefix = key >> 8;
                if path.prefix == prefix {
                    if !path.leaf.is_null() {
                        let d = (key & 0xFF) as u8;
                        // SAFETY: path holds valid live LeafBitmapL pointer.
                        let node = unsafe { &mut *path.leaf };
                        let sub = (d >> 5) as usize;
                        if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
                            // SAFETY: value subarray holds subexpanse_count values; in-place swap.
                            unsafe {
                                let slot = node.values[sub].add(rank);
                                let old = *slot;
                                slot.write(val);
                                return Some(old);
                            }
                        }
                        let rank = node.bitmap.subexpanse_rank(d) as usize;
                        let old_n = node.bitmap.subexpanse_count(sub) as usize;
                        if old_n > 0
                            && crate::leaf::cap_class(old_n + 1) == crate::leaf::cap_class(old_n)
                        {
                            // Fast path: spare class capacity — shift in place.
                            // SAFETY: the subarray holds cap_class(old_n) slots.
                            unsafe {
                                let arr = node.values[sub];
                                core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                                arr.add(rank).write(val);
                            }
                        } else {
                            let new = alloc
                                .alloc_bytes(crate::mutate::sub_vals_size(old_n + 1))
                                .cast::<u64>();
                            // SAFETY: copy old_n values around the inserted rank.
                            unsafe {
                                if old_n > 0 {
                                    let old = node.values[sub];
                                    new.as_ptr().copy_from_nonoverlapping(old, rank);
                                    new.as_ptr()
                                        .add(rank + 1)
                                        .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                                    alloc.free_bytes(
                                        core::ptr::NonNull::new(old.cast()).expect("values"),
                                        crate::mutate::sub_vals_size(old_n),
                                    );
                                }
                                new.as_ptr().add(rank).write(val);
                            }
                            node.values[sub] = new.as_ptr();
                        }
                        node.bitmap.set(d);
                        path.pending_pop += 1;
                        path.terminal_pop += 1;
                        *pop += 1;
                        // SAFETY: keep terminal edge pop0 up to date.
                        unsafe {
                            (*path.edges[0]).set_pop0(1, (path.terminal_pop - 1) as u64);
                        }
                        return None;
                    } else if !path.leaf1.is_null() {
                        let d = (key & 0xFF) as u8;
                        let cur_pop = path.terminal_pop as usize;
                        let base = path.leaf1;
                        // SAFETY: base points to a live Leaf1 allocation; map_keys_offset is in-bounds.
                        let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(cur_pop)) };
                        // SAFETY: cur_pop >= 1 when leaf1 is active, so cur_pop - 1 is in bounds.
                        let last = unsafe { *keys_ptr.add(cur_pop - 1) };
                        if d > last {
                            if cur_pop < crate::mutate::LEAF1_CAP
                                && crate::leaf::cap_class(cur_pop + 1)
                                    == crate::leaf::cap_class(cur_pop)
                            {
                                // SAFETY: spare class capacity in the live Leaf1 allocation.
                                unsafe {
                                    *keys_ptr.add(cur_pop) = d;
                                    let vals = base.cast::<u64>();
                                    vals.add(cur_pop).write(val);
                                    (*path.edges[0]).set_pop0(1, cur_pop as u64);
                                }
                                path.terminal_pop += 1;
                                path.pending_pop += 1;
                                *pop += 1;
                                return None;
                            }
                        } else if d == last {
                            // SAFETY: cur_pop - 1 is the existing slot for `last`.
                            unsafe {
                                let vals = base.cast::<u64>();
                                let slot = vals.add(cur_pop - 1);
                                let old = *slot;
                                slot.write(val);
                                return Some(old);
                            }
                        }
                    }
                }
                path.clear();
                // SAFETY: trie maintained/owned by this map's engine.
                let prev = unsafe {
                    mutate_map::map_insert_path_dyn::<false>(alloc, top, key, val, 8, path)
                }
                .0;
                if prev.is_none() {
                    *pop += 1;
                }
                prev
            }
        }
    }

    /// Removes `key`; returns its value if it was present.
    #[inline(always)]
    pub(crate) fn remove(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        path.clear();
        match &mut self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let (ptr, pop) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr, pop);
                let at = keys.binary_search(&key).ok()?;
                // SAFETY: in-bounds value read.
                let old = unsafe { *vals.add(at) };
                if pop == 1 {
                    // SAFETY: last entry removed; free the leaf.
                    unsafe { alloc.free_bytes(ptr, leaf_size(1)) };
                    self.root = Root::Empty;
                } else if crate::leaf::cap_class(pop - 1) == crate::leaf::cap_class(pop) {
                    // Fast path: capacity class unchanged — shift surviving entries in-place.
                    // SAFETY: in-place shift inside class-sized buffer.
                    unsafe {
                        let nk = ptr.as_ptr().cast::<u64>();
                        core::ptr::copy(nk.add(at + 1), nk.add(at), pop - 1 - at);
                        core::ptr::copy(vals.add(at + 1), vals.add(at), pop - 1 - at);
                    }
                    self.root = Root::Leaf { ptr, pop: pop - 1 };
                } else {
                    let new = alloc.alloc_bytes(leaf_size(pop - 1));
                    // SAFETY: copy the surviving keys/values into the
                    // smaller leaf.
                    unsafe {
                        let nk = new.as_ptr().cast::<u64>();
                        nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                        nk.add(at)
                            .copy_from_nonoverlapping(keys.as_ptr().add(at + 1), pop - 1 - at);
                        let nv = new.as_ptr().add(leaf_values_offset(pop - 1)).cast::<u64>();
                        nv.copy_from_nonoverlapping(vals, at);
                        nv.add(at)
                            .copy_from_nonoverlapping(vals.add(at + 1), pop - 1 - at);
                        alloc.free_bytes(ptr, leaf_size(pop));
                    }
                    self.root = Root::Leaf {
                        ptr: new,
                        pop: pop - 1,
                    };
                }
                Some(old)
            }
            Root::Tree { top, pop } => {
                // SAFETY: trie maintained/owned by this map's engine.
                let old = unsafe { mutate_map::map_remove_dyn(alloc, top, key, 8) };
                if old.is_some() {
                    *pop -= 1;
                    if *pop == 0 {
                        debug_assert!(top.is_null());
                        self.root = Root::Empty;
                    } else if *pop < ROOT_LEAF_CAP as u64 {
                        // Hysteresis twin of the root-leaf promotion.
                        self.condense_to_root_leaf(alloc, path);
                    }
                }
                old
            }
        }
    }

    /// Removes every entry (frees through `alloc`, which must be the
    /// allocator that produced this core's nodes).
    ///
    /// The caller's accounting invariant (`bytes_in_use == 0` after a
    /// lone map clears) lives with the allocator's owner — a shared
    /// allocator still carries its other cores' bytes here.
    #[inline(always)]
    pub(crate) fn clear(&mut self, alloc: &NodeAlloc, path: &mut crate::mutate_map::InsertPathMap) {
        path.clear();
        match &mut self.root {
            Root::Empty => {}
            Root::Leaf { ptr, pop } => {
                // SAFETY: freeing the root leaf exactly once.
                unsafe { alloc.free_bytes(*ptr, leaf_size(*pop)) };
            }
            Root::Tree { top, .. } => {
                // SAFETY: freeing the whole owned trie exactly once.
                unsafe { mutate::free_subtree::<true>(alloc, top) };
            }
        }
        self.root = Root::Empty;
    }

    /// Defensive trie structure validator that does not panic.
    ///
    /// The owner must flush its insert-path cache first (pending
    /// population deltas would otherwise disagree with the tree).
    ///
    /// Returns `Ok(())` if the trie invariants are fully met, or `Err(reason)`
    /// indicating what structural corruption was detected.
    pub(crate) fn validate_defensive(&self) -> Result<(), String> {
        match &self.root {
            Root::Empty => Ok(()),
            Root::Leaf { ptr, pop } => {
                if *pop < 1 || *pop > ROOT_LEAF_CAP {
                    return Err(format!("root leaf pop {pop} out of range"));
                }
                let (keys, _) = Self::leaf_parts(*ptr, *pop);
                if !keys.windows(2).all(|w| w[0] < w[1]) {
                    return Err("root leaf keys unsorted".into());
                }
                Ok(())
            }
            Root::Tree { top, pop } => {
                if top.is_null() {
                    return Err("tree root with null top".into());
                }
                let mut stats = ExpanseStats::default();
                let counted =
                    crate::validate::expanse_validate_and_stats::<true>(top, 8, &mut stats, 0)?;
                if counted != *pop {
                    return Err(format!(
                        "total population {pop} disagrees with tree {counted}"
                    ));
                }
                Ok(())
            }
        }
    }

    /// Gathers structural statistics of the trie (owner flushes first).
    #[must_use]
    pub(crate) fn stats(&self) -> ExpanseStats {
        let mut stats = ExpanseStats::default();
        match &self.root {
            Root::Empty => {}
            Root::Leaf { pop, .. } => {
                stats.depth_histogram[0] = 1;
                stats.leaf_pop_histogram[*pop] = 1;
                stats.node_counts.leaf_linear = 1;
            }
            Root::Tree { top, .. } => {
                let _ = crate::validate::expanse_validate_and_stats::<true>(top, 8, &mut stats, 0);
            }
        }
        stats
    }
}

impl MapCore {
    fn leaf_entry(&self, at: usize) -> (u64, u64) {
        let Root::Leaf { ptr, pop } = &self.root else {
            unreachable!("leaf_entry outside root-leaf state")
        };
        let (keys, vals) = Self::leaf_parts(*ptr, *pop);
        // SAFETY: at < pop values live behind the keys.
        (keys[at], unsafe { *vals.add(at) })
    }

    /// Smallest entry in the map.
    #[inline(always)]
    #[must_use]
    pub(crate) fn first(&self) -> Option<(u64, u64)> {
        self.next_at_or_after(0)
    }

    /// Largest entry in the map.
    #[inline(always)]
    #[must_use]
    pub(crate) fn last(&self) -> Option<(u64, u64)> {
        self.prev_at_or_before(u64::MAX)
    }

    /// Smallest entry with key `>= key` (compat: `JudyLFirst`).
    #[inline(always)]
    #[must_use]
    pub(crate) fn next_at_or_after(&self, key: Key) -> Option<(u64, u64)> {
        match &self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let (keys, _) = Self::leaf_parts(*ptr, *pop);
                let at = keys.partition_point(|&k| k < key);
                (at < *pop).then(|| self.leaf_entry(at))
            }
            // SAFETY: trie maintained/owned by this map's engine.
            Root::Tree { top, .. } => unsafe { crate::nav::next::<true>(top, key, 8) },
        }
    }

    /// Smallest entry with key `> key` (compat: `JudyLNext`).
    #[inline(always)]
    #[must_use]
    pub(crate) fn next_after(&self, key: Key) -> Option<(u64, u64)> {
        self.next_at_or_after(key.checked_add(1)?)
    }

    /// Largest entry with key `<= key` (compat: `JudyLLast`).
    #[inline(always)]
    #[must_use]
    pub(crate) fn prev_at_or_before(&self, key: Key) -> Option<(u64, u64)> {
        match &self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let (keys, _) = Self::leaf_parts(*ptr, *pop);
                let at = keys.partition_point(|&k| k <= key).checked_sub(1)?;
                Some(self.leaf_entry(at))
            }
            // SAFETY: trie maintained/owned by this map's engine.
            Root::Tree { top, .. } => unsafe { crate::nav::prev::<true>(top, key, 8) },
        }
    }

    /// Largest entry with key `< key` (compat: `JudyLPrev`).
    #[inline(always)]
    #[must_use]
    pub(crate) fn prev_before(&self, key: Key) -> Option<(u64, u64)> {
        self.prev_at_or_before(key.checked_sub(1)?)
    }

    /// Number of keys strictly below `key` (rank; owner flushes first).
    #[inline(always)]
    #[must_use]
    pub(crate) fn count_below(&self, key: Key) -> u64 {
        match &self.root {
            Root::Empty => 0,
            Root::Leaf { ptr, pop } => {
                let (keys, _) = Self::leaf_parts(*ptr, *pop);
                keys.partition_point(|&k| k < key) as u64
            }
            // SAFETY: trie maintained/owned by this map's engine.
            Root::Tree { top, .. } => unsafe { crate::nav::count_below::<true>(top, key, 8) },
        }
    }

    /// Number of keys in the inclusive range (compat: `JudyLCount`;
    /// owner flushes first).
    #[inline(always)]
    #[must_use]
    pub(crate) fn count_range(&self, range: core::ops::RangeInclusive<u64>) -> u64 {
        let (a, b) = (*range.start(), *range.end());
        if a > b {
            return 0;
        }
        self.count_below(b) + u64::from(self.contains_key(b)) - self.count_below(a)
    }

    /// The entry with `n` keys below it — 0-based select (compat:
    /// `JudyLByCount`, which is 1-based; owner flushes first).
    #[inline(always)]
    #[must_use]
    pub(crate) fn by_count(&self, n: u64) -> Option<(u64, u64)> {
        if n >= self.len() {
            return None;
        }
        match &self.root {
            Root::Empty => None,
            Root::Leaf { .. } => Some(self.leaf_entry(n as usize)),
            // SAFETY: trie maintained/owned by this map's engine; n is
            // below the population.
            Root::Tree { top, .. } => Some(unsafe { crate::nav::by_count::<true>(top, n, 8) }),
        }
    }

    /// Builds a forward (ascending) raw cursor over all entries.
    fn iter_fwd_raw(&self) -> crate::iter::RawIter<true> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { ptr, pop } => {
                let keys_ptr = ptr.as_ptr().cast::<u64>();
                // SAFETY: root leaf holds pop values starting at leaf_values_offset(pop).
                let vals_ptr = unsafe { keys_ptr.add(leaf_values_offset(*pop) / 8) };
                crate::iter::RawIter::from_root_leaf(keys_ptr, vals_ptr, *pop)
            }
            // SAFETY: tree maintained by map engine per invariants.
            Root::Tree { top, .. } => unsafe { crate::iter::RawIter::from_tree(top) },
        }
    }

    /// Builds a forward (ascending) raw cursor over entries with key `>= start`.
    fn range_fwd_raw(&self, start: Key) -> crate::iter::RawIter<true> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { ptr, pop } => {
                let (keys, values) = Self::leaf_parts(*ptr, *pop);
                // SAFETY: root leaf contains valid keys and values arrays of length pop.
                unsafe {
                    crate::iter::RawIter::from_root_leaf_range(keys.as_ptr(), values, *pop, start)
                }
            }
            // SAFETY: tree maintained by map engine per invariants.
            Root::Tree { top, .. } => unsafe { crate::iter::RawIter::from_tree_range(top, start) },
        }
    }

    /// The trie root edge for cursor seeks, or `Edge::NULL` when the root is a
    /// flat leaf / empty (cursor seeks then stay leaf-local).
    #[inline]
    fn cursor_top(&self) -> Edge {
        match &self.root {
            Root::Tree { top, .. } => *top,
            _ => Edge::NULL,
        }
    }

    /// Creates a stateful forward [`MapCursor`](crate::cursor::MapCursor) for
    /// monotone skip-scans, positioned before the first entry.
    ///
    /// Unlike the stateless [`next_at_or_after`](Self::next_at_or_after), which
    /// re-descends from the root on every call, the cursor keeps its descent
    /// path and re-descends only from the deepest ancestor whose expanse still
    /// covers the next target (issue #340; docs/ALGORITHMS.md §3.5).
    #[inline(always)]
    #[must_use]
    pub(crate) fn cursor(&self) -> crate::cursor::MapCursor<'_> {
        crate::cursor::MapCursor::new(self.iter_fwd_raw(), self.cursor_top())
    }

    /// Creates a [`MapCursor`](crate::cursor::MapCursor) positioned at the
    /// smallest key `>= start`.
    #[inline(always)]
    #[must_use]
    pub(crate) fn cursor_from(&self, start: Key) -> crate::cursor::MapCursor<'_> {
        crate::cursor::MapCursor::new(self.range_fwd_raw(start), self.cursor_top())
    }

    /// Ascending iterator over `(key, value)` entries.
    #[inline(always)]
    #[must_use]
    pub(crate) fn iter(&self) -> MapIter<'_> {
        MapIter {
            _map: core::marker::PhantomData,
            raw: self.iter_fwd_raw(),
        }
    }

    /// Returns an iterator over entries in the inclusive range `[start, end]`.
    #[inline(always)]
    #[must_use]
    pub(crate) fn range(&self, range: core::ops::RangeInclusive<Key>) -> MapRange<'_> {
        let (start, end) = (*range.start(), *range.end());
        let raw = if start > end {
            crate::iter::RawIter::new()
        } else {
            self.range_fwd_raw(start)
        };
        MapRange {
            _map: core::marker::PhantomData,
            raw,
            end,
        }
    }

    /// Builds a reverse (descending) raw cursor over all entries.
    fn iter_rev_raw(&self) -> crate::iter::RawIter<true> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { ptr, pop } => {
                let keys_ptr = ptr.as_ptr().cast::<u64>();
                // SAFETY: root leaf holds pop values starting at leaf_values_offset(pop).
                let vals_ptr = unsafe { keys_ptr.add(leaf_values_offset(*pop) / 8) };
                crate::iter::RawIter::from_root_leaf_rev(keys_ptr, vals_ptr, *pop)
            }
            // SAFETY: tree maintained by map engine per invariants.
            Root::Tree { top, .. } => unsafe { crate::iter::RawIter::from_tree_rev(top) },
        }
    }

    /// Builds a reverse (descending) raw cursor over entries with key `<= end`.
    fn range_rev_raw(&self, end: Key) -> crate::iter::RawIter<true> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { ptr, pop } => {
                let (keys, values) = Self::leaf_parts(*ptr, *pop);
                // SAFETY: root leaf contains valid keys and values arrays of length pop.
                unsafe {
                    crate::iter::RawIter::from_root_leaf_range_rev(keys.as_ptr(), values, *pop, end)
                }
            }
            // SAFETY: tree maintained by map engine per invariants.
            Root::Tree { top, .. } => unsafe {
                crate::iter::RawIter::from_tree_range_rev(top, end)
            },
        }
    }

    /// Descending (double-ended) iterator over `(key, value)` entries.
    #[inline(always)]
    #[must_use]
    pub(crate) fn iter_rev(&self) -> MapIterRev<'_> {
        MapIterRev {
            map: self,
            raw: self.iter_rev_raw(),
            front: None,
            lo: 0,
            hi: Key::MAX,
            done: self.is_empty(),
        }
    }

    /// Returns a descending (double-ended) iterator over entries in the
    /// inclusive range `[start, end]`.
    #[inline(always)]
    #[must_use]
    pub(crate) fn range_rev(&self, range: core::ops::RangeInclusive<Key>) -> MapRangeRev<'_> {
        let (start, end) = (*range.start(), *range.end());
        let (raw, done) = if start > end {
            (crate::iter::RawIter::new(), true)
        } else {
            (self.range_rev_raw(end), false)
        };
        MapRangeRev {
            map: self,
            raw,
            front: None,
            lo: start,
            hi: end,
            done,
        }
    }

    /// Returns an iterator over entries in `range` where the hot metadata word
    /// (bits 63..32 of the 64-bit value slot) satisfies the predicate.
    #[inline(always)]
    pub(crate) fn range_filtered<'a, P>(
        &'a self,
        range: core::ops::RangeInclusive<Key>,
        mut predicate: P,
    ) -> impl Iterator<Item = (Key, u64)> + 'a
    where
        P: FnMut(Key, u32) -> bool + 'a,
    {
        self.range(range).filter(move |&(k, v)| {
            let meta = (v >> 32) as u32;
            predicate(k, meta)
        })
    }

    /// Scans entries in `range`, evaluating `predicate(key, hot_meta)` directly on
    /// the raw value slot before invoking `callback(key, raw_val)`.
    #[inline(always)]
    pub(crate) fn scan_filtered<P, F>(
        &self,
        range: core::ops::RangeInclusive<Key>,
        mut predicate: P,
        mut callback: F,
    ) where
        P: FnMut(Key, u32) -> bool,
        F: FnMut(Key, u64) -> bool,
    {
        for (k, v) in self.range(range) {
            let meta = (v >> 32) as u32;
            if predicate(k, meta) && !callback(k, v) {
                break;
            }
        }
    }
}

/// Ascending entry iterator over an [`ExpanseMap`].
///
/// Forward-only by design: the deterministic-Callgrind zero-regression gate
/// (`AGENTS.md` §5) forbids adding any per-element work to this hot path, so
/// reverse iteration lives on the dedicated [`MapIterRev`] / [`MapRangeRev`]
/// types (`iter_rev` / `range_rev`), which are themselves double-ended.
pub struct MapIter<'a> {
    _map: core::marker::PhantomData<&'a MapCore>,
    raw: crate::iter::RawIter<true>,
}

impl Iterator for MapIter<'_> {
    type Item = (u64, u64);

    #[inline(always)]
    fn next(&mut self) -> Option<(u64, u64)> {
        self.raw.next()
    }
}

/// Ascending entry iterator over a key range in an [`ExpanseMap`].
pub struct MapRange<'a> {
    _map: core::marker::PhantomData<&'a MapCore>,
    raw: crate::iter::RawIter<true>,
    end: Key,
}

impl Iterator for MapRange<'_> {
    type Item = (Key, u64);

    #[inline(always)]
    fn next(&mut self) -> Option<(Key, u64)> {
        let (k, v) = self.raw.next()?;
        if k > self.end {
            return None;
        }
        Some((k, v))
    }
}

/// Double-ended entry iterator over an [`ExpanseMap`], descending by default.
///
/// `next` streams descending from the largest key; `next_back` streams
/// ascending from the smallest. The two ends share an inclusive `[lo, hi]`
/// window so interleaved calls never cross: `next` lowers `hi`, `next_back`
/// raises `lo`, and each stops once the window closes. The ascending cursor is
/// built lazily, so a pure-descending walk never pays for it.
pub struct MapIterRev<'a> {
    map: &'a MapCore,
    raw: crate::iter::RawIter<true>,
    front: Option<crate::iter::RawIter<true>>,
    lo: Key,
    hi: Key,
    done: bool,
}

impl Iterator for MapIterRev<'_> {
    type Item = (Key, u64);

    #[inline]
    fn next(&mut self) -> Option<(Key, u64)> {
        if self.done {
            return None;
        }
        let (k, v) = self.raw.next_back()?;
        if k < self.lo {
            self.done = true;
            return None;
        }
        if k == self.lo {
            self.done = true;
        } else {
            self.hi = k - 1;
        }
        Some((k, v))
    }
}

impl DoubleEndedIterator for MapIterRev<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<(Key, u64)> {
        if self.done {
            return None;
        }
        if self.front.is_none() {
            self.front = Some(self.map.iter_fwd_raw());
        }
        let (k, v) = self.front.as_mut().unwrap().next()?;
        if k > self.hi {
            self.done = true;
            return None;
        }
        if k == self.hi {
            self.done = true;
        } else {
            self.lo = k + 1;
        }
        Some((k, v))
    }
}

/// Double-ended entry iterator over a key range in an [`ExpanseMap`],
/// descending by default. See [`MapIterRev`] for the shared-window discipline.
pub struct MapRangeRev<'a> {
    map: &'a MapCore,
    raw: crate::iter::RawIter<true>,
    front: Option<crate::iter::RawIter<true>>,
    lo: Key,
    hi: Key,
    done: bool,
}

impl Iterator for MapRangeRev<'_> {
    type Item = (Key, u64);

    #[inline]
    fn next(&mut self) -> Option<(Key, u64)> {
        if self.done {
            return None;
        }
        let (k, v) = self.raw.next_back()?;
        if k < self.lo {
            self.done = true;
            return None;
        }
        if k == self.lo {
            self.done = true;
        } else {
            self.hi = k - 1;
        }
        Some((k, v))
    }
}

impl DoubleEndedIterator for MapRangeRev<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<(Key, u64)> {
        if self.done {
            return None;
        }
        if self.front.is_none() {
            // At first `next_back`, `lo` still equals the original range start.
            self.front = Some(self.map.range_fwd_raw(self.lo));
        }
        let (k, v) = self.front.as_mut().unwrap().next()?;
        if k > self.hi {
            self.done = true;
            return None;
        }
        if k == self.hi {
            self.done = true;
        } else {
            self.lo = k + 1;
        }
        Some((k, v))
    }
}

impl<'a> IntoIterator for &'a ExpanseMap {
    type Item = (u64, u64);
    type IntoIter = MapIter<'a>;

    fn into_iter(self) -> MapIter<'a> {
        self.iter()
    }
}

impl FromIterator<(Key, u64)> for ExpanseMap {
    fn from_iter<I: IntoIterator<Item = (Key, u64)>>(iter: I) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

impl Extend<(Key, u64)> for ExpanseMap {
    fn extend<I: IntoIterator<Item = (Key, u64)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl MapCore {
    /// Rebuilds the flat root leaf (parallel key/value arrays) from a
    /// small tree — the shrink twin of the promotion.
    #[inline(always)]
    fn condense_to_root_leaf(
        &mut self,
        alloc: &NodeAlloc,
        path: &mut crate::mutate_map::InsertPathMap,
    ) {
        let Root::Tree { top, pop } = &mut self.root else {
            unreachable!("condense outside tree state")
        };
        let n = *pop as usize;
        debug_assert!((1..ROOT_LEAF_CAP).contains(&n));
        let new = alloc.alloc_bytes(leaf_size(n));
        let mut written = 0usize;
        let mut from = Some(0u64);
        // SAFETY: engine-maintained trie per this type's invariants.
        while let Some((k, v)) = from.and_then(|f| unsafe { crate::nav::next::<true>(top, f, 8) }) {
            debug_assert!(written < n);
            // SAFETY: in-bounds writes: keys then values.
            unsafe {
                new.as_ptr().cast::<u64>().add(written).write(k);
                new.as_ptr()
                    .add(leaf_values_offset(n))
                    .cast::<u64>()
                    .add(written)
                    .write(v);
            }
            written += 1;
            from = k.checked_add(1);
        }
        debug_assert_eq!(written, n);
        path.clear();
        // SAFETY: whole trie owned by this map; freed exactly once.
        unsafe { mutate::free_subtree::<true>(alloc, top) };
        self.root = Root::Leaf { ptr: new, pop: n };
    }
}

/// Pathless twins for owners without a persistent insert-path cache
/// (`ExpanseStrMap`'s sub-tries, issue #363 Step A). Each hands the
/// engine a fresh empty path: the sequential-bypass check can never hit
/// (`prefix == u64::MAX` matches no `key >> 8`), so after inlining the
/// bypass bookkeeping is dead and folds away.
impl MapCore {
    #[inline(always)]
    pub(crate) fn insert_pathless(&mut self, alloc: &NodeAlloc, key: Key, val: u64) -> Option<u64> {
        self.insert(
            alloc,
            key,
            val,
            &mut crate::mutate_map::InsertPathMap::empty(),
        )
    }

    #[inline(always)]
    pub(crate) fn remove_pathless(&mut self, alloc: &NodeAlloc, key: Key) -> Option<u64> {
        self.remove(alloc, key, &mut crate::mutate_map::InsertPathMap::empty())
    }

    #[inline(always)]
    pub(crate) fn ins_slot_pathless(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
    ) -> core::ptr::NonNull<u64> {
        self.ins_slot(alloc, key, &mut crate::mutate_map::InsertPathMap::empty())
    }

    #[inline(always)]
    pub(crate) fn value_slot_pathless(&mut self, key: Key) -> Option<core::ptr::NonNull<u64>> {
        self.get_value_slot(key, &mut crate::mutate_map::InsertPathMap::empty())
    }

    #[inline(always)]
    pub(crate) fn clear_pathless(&mut self, alloc: &NodeAlloc) {
        self.clear(alloc, &mut crate::mutate_map::InsertPathMap::empty());
    }
}

/// The public surface: forwards to `MapCore` with this map's own
/// allocator and insert-path cache. The core's hot entry points are
/// `#[inline(always)]`, so each forwarder compiles to exactly the
/// pre-split method body (issue #363 Step A's zero-regression
/// requirement on the single-threaded `JudyL*` paths).
impl ExpanseMap {
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: MapCore::new(),
            alloc: NodeAlloc::new(),
            path: core::cell::UnsafeCell::new(crate::mutate_map::InsertPathMap::empty()),
        }
    }

    #[inline(always)]
    fn flush_path(&self) {
        // SAFETY: path is an internal cursor whose state is flushed through UnsafeCell.
        unsafe {
            (*self.path.get()).flush();
        }
    }

    /// Number of keys in the map.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.core.len()
    }

    /// True when no keys are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.core.is_empty()
    }

    /// Heap bytes currently used by the map's nodes and leaves.
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.alloc.bytes_in_use()
    }

    /// Cumulative node/leaf allocations made by this container since it
    /// was created (diagnostics; see `tests/no_heap_churn.rs`, which
    /// subtracts these from the process-wide count to isolate
    /// incidental scratch allocation).
    #[must_use]
    pub fn total_node_allocs(&self) -> usize {
        self.alloc.total_allocs()
    }

    /// Returns the value stored for `key`.
    #[inline(always)]
    #[must_use]
    pub fn get(&self, key: Key) -> Option<u64> {
        self.core.get(key)
    }

    /// Look up a batch of `keys` simultaneously, writing values into `out`.
    ///
    /// When the root is a multi-level digital trie, `get_batch` interleaves key descents
    /// across CPU Line Fill Buffers in chunks of 8 keys and issues software prefetch hints
    /// on branch nodes to overlap DRAM memory latency.
    #[inline]
    pub fn get_batch(&self, keys: &[Key], out: &mut [Option<u64>]) {
        self.core.get_batch(keys, out);
    }

    /// Look up a batch of `keys`, writing found values into `out_values` and presence flags
    /// into `out_found` (when `Some`). Returns the count of found keys.
    #[inline]
    pub fn get_batch_into(
        &self,
        keys: &[Key],
        out_values: &mut [u64],
        out_found: Option<&mut [bool]>,
    ) -> usize {
        self.core.get_batch_into(keys, out_values, out_found)
    }

    /// Returns a pointer to `key`'s value slot in the leaf or root leaf, or `None`
    /// if absent.
    #[inline(always)]
    #[must_use]
    pub fn get_slot_ptr(&self, key: Key) -> Option<core::ptr::NonNull<u64>> {
        self.core.get_slot_ptr(key)
    }

    /// Returns a **writable pointer to `key`'s value slot**, or `None` if
    /// the key is absent — the compat layer's `JudyLGet`/`JudyLIns` return
    /// convention. The pointer stays valid until the next structural
    /// mutation of the map (the classic JudyL contract); reading or
    /// writing through it after an `insert`/`remove`/`clear` is undefined.
    #[inline(always)]
    #[must_use]
    pub fn get_value_slot(&mut self, key: Key) -> Option<core::ptr::NonNull<u64>> {
        self.core.get_value_slot(key, self.path.get_mut())
    }

    /// Inserts `key` with value 0 if absent — the existing value is kept
    /// untouched — and returns a **writable pointer to its value slot**:
    /// the compat `JudyLIns` contract, in one tree walk. The pointer stays
    /// valid until the next structural mutation.
    #[inline(always)]
    pub fn ins_slot(&mut self, key: Key) -> core::ptr::NonNull<u64> {
        self.core.ins_slot(&self.alloc, key, self.path.get_mut())
    }

    /// Phase 7 (occ): by-value root snapshot + allocation handle for
    /// the validated concurrent read walk (see `ExpanseSet::occ_root`).
    pub(crate) fn occ_root(&self) -> (RootSnapshot, &NodeAlloc) {
        (self.core.occ_snapshot(), &self.alloc)
    }

    /// Membership test.
    #[must_use]
    pub fn contains_key(&self, key: Key) -> bool {
        self.core.contains_key(key)
    }

    /// Inserts `key → val`; returns the replaced value if the key was
    /// already present.
    pub fn insert(&mut self, key: Key, val: u64) -> Option<u64> {
        self.core.insert(&self.alloc, key, val, self.path.get_mut())
    }

    /// Removes `key`; returns its value if it was present.
    pub fn remove(&mut self, key: Key) -> Option<u64> {
        self.core.remove(&self.alloc, key, self.path.get_mut())
    }

    /// Removes every entry.
    pub fn clear(&mut self) {
        self.core.clear(&self.alloc, self.path.get_mut());
        debug_assert_eq!(self.alloc.bytes_in_use(), 0);
    }

    /// Walks the whole structure, panicking on any violated invariant
    /// (`docs/TESTING.md`, "Structural invariant validator").
    pub fn validate(&self) {
        if let Err(err) = self.validate_defensive() {
            panic!("{err}");
        }
    }

    /// Defensive trie structure validator that does not panic.
    ///
    /// Returns `Ok(())` if the trie invariants are fully met, or `Err(reason)`
    /// indicating what structural corruption was detected.
    pub fn validate_defensive(&self) -> Result<(), String> {
        self.flush_path();
        self.core.validate_defensive()
    }

    /// Gathers structural statistics of the trie.
    #[must_use]
    pub fn stats(&self) -> ExpanseStats {
        self.flush_path();
        self.core.stats()
    }

    /// Smallest entry in the map.
    #[must_use]
    pub fn first(&self) -> Option<(u64, u64)> {
        self.core.first()
    }

    /// Largest entry in the map.
    #[must_use]
    pub fn last(&self) -> Option<(u64, u64)> {
        self.core.last()
    }

    /// Smallest entry with key `>= key` (compat: `JudyLFirst`).
    #[must_use]
    pub fn next_at_or_after(&self, key: Key) -> Option<(u64, u64)> {
        self.core.next_at_or_after(key)
    }

    /// Smallest entry with key `> key` (compat: `JudyLNext`).
    #[must_use]
    pub fn next_after(&self, key: Key) -> Option<(u64, u64)> {
        self.core.next_after(key)
    }

    /// Largest entry with key `<= key` (compat: `JudyLLast`).
    #[must_use]
    pub fn prev_at_or_before(&self, key: Key) -> Option<(u64, u64)> {
        self.core.prev_at_or_before(key)
    }

    /// Largest entry with key `< key` (compat: `JudyLPrev`).
    #[must_use]
    pub fn prev_before(&self, key: Key) -> Option<(u64, u64)> {
        self.core.prev_before(key)
    }

    /// Number of keys strictly below `key` (rank).
    #[must_use]
    pub fn count_below(&self, key: Key) -> u64 {
        self.flush_path();
        self.core.count_below(key)
    }

    /// Number of keys in the inclusive range (compat: `JudyLCount`).
    #[must_use]
    pub fn count_range(&self, range: core::ops::RangeInclusive<u64>) -> u64 {
        self.flush_path();
        self.core.count_range(range)
    }

    /// The entry with `n` keys below it — 0-based select (compat:
    /// `JudyLByCount`, which is 1-based).
    #[must_use]
    pub fn by_count(&self, n: u64) -> Option<(u64, u64)> {
        self.flush_path();
        self.core.by_count(n)
    }

    /// Creates a stateful forward [`MapCursor`](crate::cursor::MapCursor) for
    /// monotone skip-scans, positioned before the first entry.
    ///
    /// Unlike the stateless [`next_at_or_after`](Self::next_at_or_after), which
    /// re-descends from the root on every call, the cursor keeps its descent
    /// path and re-descends only from the deepest ancestor whose expanse still
    /// covers the next target (issue #340; docs/ALGORITHMS.md §3.5).
    #[must_use]
    pub fn cursor(&self) -> crate::cursor::MapCursor<'_> {
        self.core.cursor()
    }

    /// Creates a [`MapCursor`](crate::cursor::MapCursor) positioned at the
    /// smallest key `>= start`.
    #[must_use]
    pub fn cursor_from(&self, start: Key) -> crate::cursor::MapCursor<'_> {
        self.core.cursor_from(start)
    }

    /// Ascending iterator over `(key, value)` entries.
    #[must_use]
    pub fn iter(&self) -> MapIter<'_> {
        self.core.iter()
    }

    /// Returns an iterator over entries in the inclusive range `[start, end]`.
    #[must_use]
    pub fn range(&self, range: core::ops::RangeInclusive<Key>) -> MapRange<'_> {
        self.core.range(range)
    }

    /// Descending (double-ended) iterator over `(key, value)` entries.
    #[must_use]
    pub fn iter_rev(&self) -> MapIterRev<'_> {
        self.core.iter_rev()
    }

    /// Returns a descending (double-ended) iterator over entries in the
    /// inclusive range `[start, end]`.
    #[must_use]
    pub fn range_rev(&self, range: core::ops::RangeInclusive<Key>) -> MapRangeRev<'_> {
        self.core.range_rev(range)
    }

    /// Returns an iterator over entries in `range` where the hot metadata word
    /// (bits 63..32 of the 64-bit value slot) satisfies the predicate.
    pub fn range_filtered<'a, P>(
        &'a self,
        range: core::ops::RangeInclusive<Key>,
        predicate: P,
    ) -> impl Iterator<Item = (Key, u64)> + 'a
    where
        P: FnMut(Key, u32) -> bool + 'a,
    {
        self.core.range_filtered(range, predicate)
    }

    /// Scans entries in `range`, evaluating `predicate(key, hot_meta)` directly on
    /// the raw value slot before invoking `callback(key, raw_val)`.
    pub fn scan_filtered<P, F>(
        &self,
        range: core::ops::RangeInclusive<Key>,
        predicate: P,
        callback: F,
    ) where
        P: FnMut(Key, u32) -> bool,
        F: FnMut(Key, u64) -> bool,
    {
        self.core.scan_filtered(range, predicate, callback);
    }
}

impl Default for ExpanseMap {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExpanseMap {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Regression for the fuzz crash `crash-7048e639` (ASan overflow):
    /// a 1-byte-remainder linear leaf with pop 9..=12 has a
    /// cap_class-derived key area of only 12 bytes, which the 16-byte
    /// vectorized search kernel must not be gated into. Exercises every
    /// pop in the formerly-misgated range through get/insert probes.
    #[test]
    fn kb1_leaf_pop_9_to_12_lookups() {
        for pop in 9usize..=12 {
            let mut m = ExpanseMap::new();
            for i in 0..pop as u64 {
                m.insert(i * 3, i + 100);
            }
            for i in 0..pop as u64 {
                assert_eq!(m.get(i * 3), Some(i + 100), "pop={pop} key={}", i * 3);
                assert_eq!(m.get(i * 3 + 1), None, "pop={pop} miss={}", i * 3 + 1);
            }
            assert_eq!(m.get(u64::MAX), None);
        }
    }

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

    #[cfg(miri)]
    const OPS: usize = 250;
    #[cfg(not(miri))]
    const OPS: usize = 6000;

    fn model_run(seed: u64, gen_key: impl Fn(&mut XorShift) -> u64) {
        let mut rng = XorShift(seed);
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        for op in 0..OPS {
            let key = gen_key(&mut rng);
            match rng.next() % 4 {
                0 | 3 => {
                    let val = rng.next();
                    assert_eq!(map.insert(key, val), model.insert(key, val), "ins {key:#x}");
                }
                1 => assert_eq!(map.remove(key), model.remove(&key), "rem {key:#x}"),
                _ => assert_eq!(map.get(key), model.get(&key).copied(), "get {key:#x}"),
            }
            assert_eq!(map.len(), model.len() as u64);
            if op % 64 == 0 {
                map.validate();
            }
        }
        map.validate();
        for (&k, &v) in &model {
            assert_eq!(map.get(k), Some(v), "model entry {k:#x}");
        }
        let keys: Vec<u64> = model.keys().copied().collect();
        for k in keys {
            assert_eq!(map.remove(k), model.remove(&k));
        }
        map.validate();
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn model_sequential() {
        model_run(0x1A, |rng| rng.next() % 4096);
    }

    #[test]
    fn model_random_full_width() {
        model_run(0x2B, |rng| rng.next());
    }

    #[test]
    fn model_clustered() {
        let bases = [
            0u64,
            0xDEAD_BEEF_0000,
            0xFFFF_FFFF_FFFF_FF00,
            0x1234_5678_9ABC_0000,
        ];
        model_run(0x3C, move |rng| {
            let base = bases[(rng.next() % 4) as usize];
            base.wrapping_add(rng.next() % 512)
        });
    }

    #[test]
    fn model_sparse_high_bytes() {
        model_run(0x4D, |rng| (rng.next() % 4096) << 48);
    }

    #[test]
    fn ladder_full_climb_and_descend() {
        // 0..=255 with distinct values: immed → linear map leaf → bitmap
        // map leaf on the way up (no full expanse for maps), every
        // hysteresis step down, values intact throughout.
        let stride = if cfg!(miri) { 32 } else { 1 };
        let mut map = ExpanseMap::new();
        for k in 0u64..=255 {
            assert_eq!(map.insert(k, k * 7 + 1), None);
            if k % stride == 0 {
                map.validate();
            }
        }
        assert_eq!(map.len(), 256);
        for k in 0u64..=255 {
            assert_eq!(map.get(k), Some(k * 7 + 1), "value of {k}");
        }
        // Replacement must preserve structure and return the old value.
        assert_eq!(map.insert(100, 9999), Some(701));
        assert_eq!(map.get(100), Some(9999));
        map.validate();
        for k in (0u64..=255).rev() {
            let expected = if k == 100 { 9999 } else { k * 7 + 1 };
            assert_eq!(map.remove(k), Some(expected), "remove {k}");
            if k % stride == 0 {
                map.validate();
            }
        }
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn wide_keys_drain_leaf_to_null() {
        // 7-byte remainders have map_immed_max == 1: leaves must drain to
        // null, never to an empty immediate.
        let mut map = ExpanseMap::new();
        let keys: Vec<u64> = (0..40u64)
            .map(|i| i.wrapping_mul(0x0000_FF12_3456_789B) & ((1 << 56) - 1))
            .collect();
        for (i, &k) in keys.iter().enumerate() {
            assert_eq!(map.insert(k, i as u64), None);
        }
        map.validate();
        for (i, &k) in keys.iter().enumerate() {
            assert_eq!(map.remove(k), Some(i as u64));
        }
        map.validate();
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn wide_fanout_reaches_uncompressed_branch() {
        let mut map = ExpanseMap::new();
        for hi in 0u64..256 {
            for lo in 0u64..16 {
                let k = (hi << 8) | lo;
                assert_eq!(map.insert(k, !k), None);
            }
        }
        map.validate();
        assert_eq!(map.len(), 4096);
        for hi in 8u64..256 {
            for lo in 0u64..16 {
                let k = (hi << 8) | lo;
                assert_eq!(map.remove(k), Some(!k));
            }
            if hi % 64 == 0 {
                map.validate();
            }
        }
        map.validate();
        assert_eq!(map.len(), 8 * 16);
        map.clear();
        assert_eq!(map.mem_used(), 0);
    }

    fn nav_differential(map: &ExpanseMap, model: &BTreeMap<u64, u64>, probes: &[u64]) {
        let pair = |o: Option<(&u64, &u64)>| o.map(|(k, v)| (*k, *v));
        assert_eq!(map.first(), pair(model.first_key_value()));
        assert_eq!(map.last(), pair(model.last_key_value()));
        assert!(
            map.iter().eq(model.iter().map(|(k, v)| (*k, *v))),
            "iterator order/values"
        );
        for &k in probes {
            assert_eq!(
                map.next_at_or_after(k),
                pair(model.range(k..).next()),
                "next>={k:#x}"
            );
            assert_eq!(
                map.prev_at_or_before(k),
                pair(model.range(..=k).next_back()),
                "prev<={k:#x}"
            );
            if k < u64::MAX {
                assert_eq!(map.next_after(k), pair(model.range(k + 1..).next()));
            }
            if k > 0 {
                assert_eq!(map.prev_before(k), pair(model.range(..k).next_back()));
            }
            assert_eq!(map.count_below(k), model.range(..k).count() as u64);
        }
        for n in 0..model.len().min(64) as u64 {
            assert_eq!(map.by_count(n), pair(model.iter().nth(n as usize)));
        }
        assert_eq!(map.by_count(model.len() as u64), None);
        for pr in probes.chunks(2) {
            if let [a, b] = pr {
                let (a, b) = (*a.min(b), *a.max(b));
                assert_eq!(map.count_range(a..=b), model.range(a..=b).count() as u64);
            }
        }
    }

    #[test]
    fn navigation_matches_model() {
        let n_rand = if cfg!(miri) { 30 } else { 1500 };
        let mut rng = XorShift(0x5EED_BA5E_D00D_F00D);
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        for k in 0u64..=255 {
            map.insert(k, !k);
            model.insert(k, !k);
        }
        for _ in 0..n_rand {
            let k = match rng.next() % 3 {
                0 => rng.next(),
                1 => 0x77_0000_0000 + (rng.next() % 300),
                _ => (rng.next() % 2048) << 48,
            };
            let v = rng.next();
            map.insert(k, v);
            model.insert(k, v);
        }
        for k in [0u64, u64::MAX] {
            map.insert(k, k ^ 0x5A5A);
            model.insert(k, k ^ 0x5A5A);
        }
        map.validate();
        let mut probes: Vec<u64> = if cfg!(miri) {
            model.keys().copied().step_by(16).collect()
        } else {
            model.keys().copied().collect()
        };
        for &k in model.keys().take(if cfg!(miri) { 10 } else { 200 }) {
            probes.push(k.wrapping_add(1));
            probes.push(k.wrapping_sub(1));
        }
        for _ in 0..if cfg!(miri) { 10 } else { 400 } {
            probes.push(rng.next());
        }
        nav_differential(&map, &model, &probes);
        map.clear();
    }

    #[test]
    fn navigation_root_leaf_and_empty() {
        let map = ExpanseMap::new();
        assert_eq!(map.first(), None);
        assert_eq!(map.by_count(0), None);
        assert_eq!(map.count_range(0..=u64::MAX), 0);
        assert!(map.iter().next().is_none());

        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        for (i, k) in [5u64, 100, 7, 0, u64::MAX, 1 << 40].into_iter().enumerate() {
            map.insert(k, i as u64 * 11);
            model.insert(k, i as u64 * 11);
        }
        let probes: Vec<u64> = (0..64u64).map(|i| i * 0x0404_0404_0404).collect();
        nav_differential(&map, &model, &probes);
    }

    #[test]
    fn value_slots_are_writable_and_stable_between_mutations() {
        // Covers get::locate_slot under Miri: slots over every terminal
        // form (immediates single/multi, linear leaves, bitmap leaves,
        // root leaf), written through and read back via the public API.
        let mut map = ExpanseMap::new();
        // Root-leaf state.
        map.insert(3, 30);
        map.insert(9, 90);
        let slot = map.get_value_slot(9).unwrap();
        // SAFETY: slot valid until the next mutation; none intervenes.
        unsafe { slot.as_ptr().write(91) };
        assert_eq!(map.get(9), Some(91));
        assert!(map.get_value_slot(4).is_none());

        // Tree state across the ladder: dense byte run (bitmap leaf),
        // clustered (leaves), sparse-high (immediates down deep chains).
        // Miri interprets every walk; keep the corpus small there.
        let (dense, clustered, sparse) = if cfg!(miri) {
            (64, 64, 16)
        } else {
            (256, 640, 64)
        };
        let mut keys = Vec::new();
        for k in 0u64..dense {
            map.insert(k, k + 1);
            keys.push(k);
        }
        for k in (0u64..clustered).map(|i| 0x30_0000 + i * 3) {
            map.insert(k, k + 1);
            keys.push(k);
        }
        for k in (1u64..sparse).map(|i| i << 52) {
            map.insert(k, k + 1);
            keys.push(k);
        }
        for &k in &keys {
            let slot = map.get_value_slot(k).expect("present key has a slot");
            // SAFETY: valid slot; read then write before any mutation.
            unsafe {
                assert_eq!(*slot.as_ptr(), k + 1, "slot reads the value {k:#x}");
                slot.as_ptr().write(!k);
            }
        }
        for &k in &keys {
            assert_eq!(map.get(k), Some(!k), "written value visible {k:#x}");
        }
        map.validate();
        map.clear();
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn model_prefix_runs() {
        model_run(0x6F, |rng| {
            let base = rng.next() & !0xFF;
            base | (rng.next() % 256)
        });
    }

    #[test]
    fn narrow_pointer_lifecycle_with_values() {
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        let base = 0x1122_3344_5566_0000u64;
        for i in 0..256u64 {
            assert_eq!(map.insert(base | i, i * 3 + 1), None);
            model.insert(base | i, i * 3 + 1);
        }
        map.validate();
        assert!(
            map.mem_used() <= 2600,
            "cluster should collapse to one skip edge + values, used {}",
            map.mem_used()
        );
        for i in 0..256u64 {
            assert_eq!(map.get(base | i), Some(i * 3 + 1), "value {i}");
        }
        assert_eq!(map.get(base ^ (1 << 32)), None);
        assert_eq!(map.count_range(base..=base | 0xFF), 256);
        assert_eq!(map.by_count(7), Some((base | 7, 22)));

        // Value slots must locate through the narrow pointer too.
        let slot = map.get_value_slot(base | 9).unwrap();
        // SAFETY: slot valid until the next mutation.
        unsafe { slot.as_ptr().write(9999) };
        model.insert(base | 9, 9999);
        assert_eq!(map.get(base | 9), Some(9999));

        // Divergence splits, then drain through the conversions.
        for div in [1u64 << 16, 1 << 32, 1 << 48] {
            let k = base ^ div;
            assert_eq!(map.insert(k, !k), None, "diverging insert {k:#x}");
            model.insert(k, !k);
            map.validate();
        }
        assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
        for i in (1..256u64).rev() {
            assert_eq!(map.remove(base | i), model.remove(&(base | i)), "rm {i}");
            if i % 32 == 0 {
                map.validate();
            }
        }
        assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
        map.clear();
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn branch_skip_clusters() {
        // Map mirror of the set-flavor cluster test: divergence-level
        // branch placement, navigation, slots, and drain through the
        // downgrade ladder — all across a skipping branch.
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        let bases = [0x1122_3344_5566_0000u64, 0x99AA_BBCC_DDEE_0000u64];
        let count = if cfg!(miri) { 64u64 } else { 512u64 };
        for &base in &bases {
            for i in 0..count {
                assert_eq!(map.insert(base | i, !(base | i)), None);
                model.insert(base | i, !(base | i));
            }
            map.validate();
        }
        // Values dominate map memory (8 bytes/key); the skip keeps the
        // structural overhead to one branch + two bitmap leaves per
        // cluster instead of a per-level chain.
        let per_key = map.mem_used() as f64 / model.len() as f64;
        assert!(
            per_key <= 14.0,
            "structural overhead should collapse, {per_key:.2} B/key"
        );
        assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
        assert_eq!(
            map.next_at_or_after(bases[0] | count),
            Some((bases[1], !bases[1]))
        );
        assert_eq!(
            map.prev_before(bases[1]),
            Some((bases[0] | (count - 1), !(bases[0] | (count - 1))))
        );
        assert_eq!(map.count_range(bases[0]..=bases[0] | (count - 1)), count);
        // Value slots resolve through the skipping branch.
        let slot = map.get_value_slot(bases[0] | (count / 2)).unwrap();
        // SAFETY: slot valid until next mutation.
        unsafe { slot.as_ptr().write(42) };
        assert_eq!(map.get(bases[0] | (count / 2)), Some(42));
        model.insert(bases[0] | (count / 2), 42);
        // Diverge inside the skipped span, then drain one cluster.
        let split = bases[0] ^ (0x31 << 24);
        assert_eq!(map.insert(split, 7), None);
        model.insert(split, 7);
        map.validate();
        for i in (0..count).rev() {
            assert_eq!(map.remove(bases[0] | i), model.remove(&(bases[0] | i)));
            if i % 16 == 0 {
                map.validate();
                assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
            }
        }
        map.clear();
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn root_condenses_and_regrows() {
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        for k in 0u64..120 {
            map.insert(k << 24, !k);
            model.insert(k << 24, !k);
        }
        let stride = if cfg!(miri) { 10 } else { 1 };
        for k in (20u64..120).rev() {
            assert_eq!(map.remove(k << 24), model.remove(&(k << 24)), "rm {k}");
            if k % stride == 0 {
                map.validate();
                assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))), "at {k}");
            }
        }
        // Values survive condensation; slots still work.
        let slot = map.get_value_slot(5 << 24).unwrap();
        // SAFETY: slot valid until next mutation.
        unsafe { slot.as_ptr().write(777) };
        assert_eq!(map.get(5 << 24), Some(777));
        for k in 300u64..360 {
            map.insert(k << 40, k);
            model.insert(k << 40, k);
        }
        model.insert(5 << 24, 777);
        map.validate();
        assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
        map.clear();
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    #[should_panic(expected = "branch pop0 disagrees with subtree")]
    fn negative_control_validator_must_fire() {
        let mut map = ExpanseMap::new();
        for k in 0u64..200 {
            map.insert(k * 977, k);
        }
        let Root::Tree { top, .. } = &mut map.core.root else {
            panic!("expected a tree root");
        };
        // SAFETY: the top is a live BranchL3 (one distinct top digit in
        // this key set); we only rewrite the first child edge's aux bytes.
        unsafe {
            let b = &mut *top.node_ptr().cast::<crate::node::BranchL3>();
            let child = &mut b.edges[0];
            let pop0 = child.pop0(7);
            child.set_pop0(7, pop0 + 1);
        }
        map.validate();
    }

    #[test]
    fn test_deferred_ancestor_pop_clustered_and_boundary_flush() {
        let mut map = ExpanseMap::new();
        for cluster in 0..10u64 {
            let prefix = (cluster + 1) << 16;
            for i in 0..200u64 {
                assert_eq!(map.insert(prefix | i, i * 10), None);
            }
        }
        assert_eq!(map.len(), 2000);
        for cluster in 0..10u64 {
            let prefix = (cluster + 1) << 16;
            for i in 0..200u64 {
                assert_eq!(map.get(prefix | i), Some(i * 10));
            }
        }
        map.validate();
        assert_eq!(map.len(), 2000);
    }

    #[test]
    fn test_sequential_linear_leaf_cursor_bypass_and_upgrade() {
        let mut map = ExpanseMap::new();
        for i in 0..1000u64 {
            assert_eq!(map.insert(i, i * 3), None);
            assert_eq!(map.get(i), Some(i * 3));
            assert_eq!(map.len(), i + 1);
        }
        map.validate();
        for i in 0..1000u64 {
            assert_eq!(map.get(i), Some(i * 3));
        }
        for i in 0..1000u64 {
            assert_eq!(map.remove(i), Some(i * 3));
            assert_eq!(map.get(i), None);
        }
        map.validate();
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn test_from_iterator_and_extend() {
        let entries: Vec<(u64, u64)> = (0..500u64).map(|i| (i * 7, i * 13)).collect();
        let map: ExpanseMap = entries.iter().copied().collect();
        assert_eq!(map.len(), 500);
        map.validate();
        for &(k, v) in &entries {
            assert_eq!(map.get(k), Some(v));
        }

        let mut extended = ExpanseMap::new();
        extended.extend(entries.iter().copied());
        assert_eq!(extended.len(), 500);
        extended.validate();
        for &(k, v) in &entries {
            assert_eq!(extended.get(k), Some(v));
        }
    }

    #[test]
    #[allow(clippy::reversed_empty_ranges)]
    fn test_map_range_cursor_parity_with_btreemap() {
        use std::collections::BTreeMap;
        let mut expanse = ExpanseMap::new();
        let mut btree = BTreeMap::new();

        // 1. Root leaf population (small)
        for i in [10u64, 25, 30, 42, 50, 75, 99, 120] {
            expanse.insert(i, i * 2);
            btree.insert(i, i * 2);
        }

        let queries = [
            0..=0,
            0..=10,
            10..=10,
            11..=24,
            25..=50,
            50..=120,
            100..=200,
            150..=200,
            0..=u64::MAX,
            50..=10, // inverted range
        ];

        for r in &queries {
            let exp_res: Vec<_> = expanse.range(r.clone()).collect();
            let bt_res: Vec<_> = if r.start() <= r.end() {
                btree.range(r.clone()).map(|(&k, &v)| (k, v)).collect()
            } else {
                vec![]
            };
            assert_eq!(exp_res, bt_res, "Root leaf range mismatch for {:?}", r);
        }

        // 2. Large multi-level trie population across multiple key distributions
        let mut large_exp = ExpanseMap::new();
        let mut large_bt = BTreeMap::new();

        // Dense cluster 1
        for i in 1000..2000u64 {
            large_exp.insert(i, i ^ 0xAA);
            large_bt.insert(i, i ^ 0xAA);
        }
        // Sparse cluster 2
        for i in (100_000..200_000u64).step_by(128) {
            large_exp.insert(i, i ^ 0xBB);
            large_bt.insert(i, i ^ 0xBB);
        }
        // Wide 64-bit keys
        for i in 0..500u64 {
            let k = (i + 1).wrapping_mul(0x0102_0304_0506_0708);
            large_exp.insert(k, i);
            large_bt.insert(k, i);
        }

        let trie_queries = [
            0..=500,
            1000..=1050,
            1500..=2500,
            99_000..=150_000,
            150_000..=250_000,
            0x0102_0304_0506_0708..=0x0502_0304_0506_0708,
            0..=u64::MAX,
            u64::MAX..=u64::MAX,
            2000..=1000,
        ];

        for r in &trie_queries {
            let exp_res: Vec<_> = large_exp.range(r.clone()).collect();
            let bt_res: Vec<_> = if r.start() <= r.end() {
                large_bt.range(r.clone()).map(|(&k, &v)| (k, v)).collect()
            } else {
                vec![]
            };
            assert_eq!(exp_res, bt_res, "Large trie range mismatch for {:?}", r);
        }
    }

    #[test]
    fn test_get_batch_against_single_get() {
        let mut map = ExpanseMap::new();
        let mut all_keys = Vec::new();

        // 1. Empty map batch test
        let mut out = [Some(999); 8];
        map.get_batch(&[1, 2, 3, 4, 5, 6, 7, 8], &mut out);
        assert_eq!(out, [None; 8]);

        // 2. Populate diverse keys: sequential, sparse, wide 64-bit
        for i in 0..10_000u64 {
            let k = if i % 3 == 0 {
                i
            } else if i % 3 == 1 {
                i * 1000
            } else {
                i.wrapping_mul(0x1122_3344_5566_7788)
            };
            let v = k ^ 0xDEAD_BEEF;
            map.insert(k, v);
            all_keys.push(k);
        }

        // Test various batch sizes: 0, 1, 7, 8, 9, 16, 100, full
        let test_batch_sizes = [0, 1, 2, 7, 8, 9, 15, 16, 64, 100, 1000];
        for &size in &test_batch_sizes {
            if size > all_keys.len() {
                continue;
            }
            let query_keys: Vec<u64> = all_keys[..size]
                .iter()
                .enumerate()
                .map(|(idx, &k)| if idx % 4 == 0 { k + 1 } else { k })
                .collect();

            let mut batch_out = vec![None; size];
            map.get_batch(&query_keys, &mut batch_out);

            let mut values_out = vec![0u64; size];
            let mut found_out = vec![false; size];
            let found_count =
                map.get_batch_into(&query_keys, &mut values_out, Some(&mut found_out));

            let mut expected_found = 0;
            for (idx, &k) in query_keys.iter().enumerate() {
                let single = map.get(k);
                assert_eq!(batch_out[idx], single, "Mismatch for key {}", k);
                assert_eq!(found_out[idx], single.is_some());
                if let Some(val) = single {
                    assert_eq!(values_out[idx], val);
                    expected_found += 1;
                }
            }
            assert_eq!(found_count, expected_found);
        }
    }
}
