//! The small form of a `JudyL` array: one exact-size block.
//!
//! A `JudyL` array of up to [`SMALL_MAX`] keys is a single allocation,
//! `[pop: u32, cap: u32][keys: u64 × cap][values: u64 × cap]`, keys sorted
//! ascending. It is the root-level linear leaf of the engine kept outside
//! the engine, so that an array this small holds neither the boxed
//! `ExpanseMap` handle nor a slab page per leaf size class. `compat.rs`
//! promotes the array to an `ExpanseMap` on the insert that would make
//! `SMALL_MAX + 1` keys; a promoted array does not come back.
//!
//! The capacity climbs [`LADDER`] and never shrinks: a block is freed when
//! its last key is deleted or the array is freed.
//!
//! Value slots live in the block, so a slot pointer stays valid until the
//! next call that inserts into or deletes from the array, which is the
//! `JudyL` contract.

#[cfg(not(feature = "std"))]
use crate::core_alloc::alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc};
use core::ptr::{self, NonNull};
#[cfg(feature = "std")]
use std::alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc};

/// Most keys the small form holds: the engine's root-leaf capacity.
pub(crate) const SMALL_MAX: usize = 31;

/// Capacities a block grows through. The last is one above [`SMALL_MAX`],
/// so a block never needs a capacity past it.
const LADDER: [u32; 8] = [1, 2, 4, 8, 12, 16, 24, 32];

#[repr(C, align(8))]
struct Header {
    pop: u32,
    cap: u32,
}

const _: () = assert!(size_of::<Header>() == 8 && LADDER[7] as usize > SMALL_MAX);

/// Bytes of a block of `cap` entries.
const fn block_bytes(cap: usize) -> usize {
    size_of::<Header>() + 16 * cap
}

fn layout(cap: usize) -> Layout {
    // SAFETY: `cap` is at most 32, so the size is at most 520 bytes, and the
    // alignment is 8.
    unsafe { Layout::from_size_align_unchecked(block_bytes(cap), align_of::<Header>()) }
}

/// A live small block.
#[derive(Clone, Copy)]
pub(crate) struct Small(NonNull<Header>);

/// What [`Small::insert`] did.
pub(crate) enum Inserted {
    /// The key's value slot, in the block now at `block`.
    Slot { block: Small, slot: *mut u64 },
    /// The key is absent and the block already holds [`SMALL_MAX`] keys.
    Full,
}

/// What [`Small::remove`] did.
pub(crate) enum Removed {
    /// The key was absent.
    Absent,
    /// The key was removed and the block lives on.
    Kept,
    /// The key was the last one; the block has been freed.
    Emptied,
}

impl Small {
    /// The block at `p`, or `None` for null.
    ///
    /// # Safety
    ///
    /// `p` is null or a block produced by [`Small::insert`] and not freed.
    pub(crate) unsafe fn from_ptr(p: *mut u8) -> Option<Self> {
        NonNull::new(p.cast()).map(Self)
    }

    /// The block's address, as stored in the array word.
    pub(crate) fn as_ptr(self) -> *mut u8 {
        self.0.as_ptr().cast()
    }

    /// Keys held.
    pub(crate) fn pop(self) -> usize {
        // SAFETY: the block is live (type invariant).
        unsafe { (*self.0.as_ptr()).pop as usize }
    }

    fn cap(self) -> usize {
        // SAFETY: the block is live (type invariant).
        unsafe { (*self.0.as_ptr()).cap as usize }
    }

    /// Bytes the block holds from the allocator.
    pub(crate) fn bytes(self) -> usize {
        block_bytes(self.cap())
    }

    fn keys_ptr(self) -> *mut u64 {
        // SAFETY: the keys start one header past the block's start, inside
        // the allocation; the block is 8-aligned.
        unsafe { self.0.as_ptr().add(1).cast() }
    }

    fn vals_ptr(self) -> *mut u64 {
        // SAFETY: the values start `cap` words past the keys, inside the
        // allocation.
        unsafe { self.keys_ptr().add(self.cap()) }
    }

    /// The `i`th key in ascending order; `i < pop`.
    fn key(self, i: usize) -> u64 {
        debug_assert!(i < self.pop());
        // SAFETY: `i < pop <= cap`, and the first `pop` keys are initialised.
        unsafe { *self.keys_ptr().add(i) }
    }

    /// The `i`th value slot; `i < pop`.
    fn slot(self, i: usize) -> *mut u64 {
        debug_assert!(i < self.pop());
        // SAFETY: `i < pop <= cap`, inside the allocation.
        unsafe { self.vals_ptr().add(i) }
    }

    /// Keys strictly below `k`: the position `k` holds or would take.
    fn rank(self, k: u64) -> usize {
        let (mut lo, mut hi) = (0, self.pop());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.key(mid) < k {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// The position of `k`, if present.
    fn find(self, k: u64) -> Option<usize> {
        let i = self.rank(k);
        (i < self.pop() && self.key(i) == k).then_some(i)
    }

    fn entry(self, i: usize) -> (u64, *mut u64) {
        (self.key(i), self.slot(i))
    }

    /// The `i`th entry in ascending key order, for `i < pop`.
    pub(crate) fn nth(self, i: usize) -> Option<(u64, *mut u64)> {
        (i < self.pop()).then(|| self.entry(i))
    }

    /// The value slot of `k`, or null.
    pub(crate) fn get(self, k: u64) -> *mut u64 {
        self.find(k).map_or(ptr::null_mut(), |i| self.slot(i))
    }

    pub(crate) fn contains_key(self, k: u64) -> bool {
        self.find(k).is_some()
    }

    /// Keys in `a..=b`; 0 for a reversed range.
    pub(crate) fn count_range(self, a: u64, b: u64) -> u64 {
        if a > b {
            return 0;
        }
        let below_a = self.rank(a);
        let upto_b = match b.checked_add(1) {
            Some(next) => self.rank(next),
            None => self.pop(),
        };
        (upto_b - below_a) as u64
    }

    /// The entry with the smallest key `>= k`.
    pub(crate) fn next_at_or_after(self, k: u64) -> Option<(u64, *mut u64)> {
        self.nth(self.rank(k))
    }

    /// The entry with the smallest key `> k`.
    pub(crate) fn next_after(self, k: u64) -> Option<(u64, *mut u64)> {
        self.next_at_or_after(k.checked_add(1)?)
    }

    /// The entry with the largest key `< k`.
    pub(crate) fn prev_before(self, k: u64) -> Option<(u64, *mut u64)> {
        self.rank(k).checked_sub(1).map(|i| self.entry(i))
    }

    /// The entry with the largest key `<= k`.
    pub(crate) fn prev_at_or_before(self, k: u64) -> Option<(u64, *mut u64)> {
        match k.checked_add(1) {
            Some(next) => self.prev_before(next),
            None => self.pop().checked_sub(1).map(|i| self.entry(i)),
        }
    }

    /// Inserts `k` with a zero value if absent and returns its slot. `this`
    /// is the array's block, or `None` for an empty array.
    ///
    /// # Safety
    ///
    /// The caller owns the block exclusively and stores the returned block
    /// address in place of the one passed: growth can move the block.
    pub(crate) unsafe fn insert(this: Option<Self>, k: u64) -> Inserted {
        let (mut block, pos) = match this {
            Some(b) => {
                let pos = b.rank(k);
                if pos < b.pop() && b.key(pos) == k {
                    return Inserted::Slot {
                        block: b,
                        slot: b.slot(pos),
                    };
                }
                if b.pop() == SMALL_MAX {
                    return Inserted::Full;
                }
                (b, pos)
            }
            None => {
                let l = layout(LADDER[0] as usize);
                // SAFETY: `l` has a non-zero size.
                let Some(p) = NonNull::new(unsafe { alloc(l) }.cast::<Header>()) else {
                    handle_alloc_error(l)
                };
                // SAFETY: `p` is a fresh allocation of at least one header.
                unsafe {
                    p.as_ptr().write(Header {
                        pop: 0,
                        cap: LADDER[0],
                    });
                }
                (Self(p), 0)
            }
        };
        let pop = block.pop();
        if pop == block.cap() {
            // SAFETY: the block is exclusively owned and `pop < SMALL_MAX`.
            block = unsafe { block.grow() };
        }
        let (keys, vals) = (block.keys_ptr(), block.vals_ptr());
        // SAFETY: `pos <= pop < cap`, so both shifted ranges and the written
        // entry lie inside the block; `ptr::copy` allows the overlap.
        unsafe {
            ptr::copy(keys.add(pos), keys.add(pos + 1), pop - pos);
            ptr::copy(vals.add(pos), vals.add(pos + 1), pop - pos);
            keys.add(pos).write(k);
            vals.add(pos).write(0);
            (*block.0.as_ptr()).pop = pop as u32 + 1;
        }
        Inserted::Slot {
            block,
            slot: block.slot(pos),
        }
    }

    /// Moves the block to the next capacity of [`LADDER`].
    ///
    /// # Safety
    ///
    /// The caller owns the block exclusively, and its capacity is below the
    /// ladder's last.
    unsafe fn grow(self) -> Self {
        let (pop, old) = (self.pop(), self.cap());
        let new = LADDER
            .iter()
            .map(|&c| c as usize)
            .find(|&c| c > old)
            .expect("a block below the ladder's last capacity");
        // SAFETY: the block was allocated with `layout(old)`, and the new
        // size is non-zero and at most 520 bytes.
        let p = unsafe { realloc(self.as_ptr(), layout(old), block_bytes(new)) };
        let Some(p) = NonNull::new(p.cast::<Header>()) else {
            handle_alloc_error(layout(new))
        };
        let grown = Self(p);
        let keys = grown.keys_ptr();
        // SAFETY: the values sat `old` words past the keys and move to `new`
        // words past them; both ranges lie inside the grown block, and
        // `ptr::copy` allows the overlap. The header is rewritten last.
        unsafe {
            ptr::copy(keys.add(old), keys.add(new), pop);
            (*p.as_ptr()).cap = new as u32;
        }
        grown
    }

    /// Removes `k` if present; frees the block when it was the last key.
    ///
    /// # Safety
    ///
    /// The caller owns the block exclusively and nulls its array word on
    /// [`Removed::Emptied`].
    pub(crate) unsafe fn remove(self, k: u64) -> Removed {
        let Some(pos) = self.find(k) else {
            return Removed::Absent;
        };
        let pop = self.pop();
        if pop == 1 {
            // SAFETY: exclusively owned, and not used again.
            unsafe { self.free() };
            return Removed::Emptied;
        }
        let (keys, vals) = (self.keys_ptr(), self.vals_ptr());
        // SAFETY: `pos < pop <= cap`, so both shifted ranges lie inside the
        // block; `ptr::copy` allows the overlap.
        unsafe {
            ptr::copy(keys.add(pos + 1), keys.add(pos), pop - pos - 1);
            ptr::copy(vals.add(pos + 1), vals.add(pos), pop - pos - 1);
            (*self.0.as_ptr()).pop = pop as u32 - 1;
        }
        Removed::Kept
    }

    /// Frees the block and returns the bytes it held.
    ///
    /// # Safety
    ///
    /// The caller owns the block exclusively and does not use it again.
    pub(crate) unsafe fn free(self) -> usize {
        let cap = self.cap();
        // SAFETY: the block was allocated with `layout(cap)`.
        unsafe { dealloc(self.as_ptr(), layout(cap)) };
        block_bytes(cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frees the block under test on every exit, a panic included.
    struct Guard(Option<Small>);
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Some(b) = self.0.take() {
                // SAFETY: the guard owns the block.
                unsafe { b.free() };
            }
        }
    }

    fn put(g: &mut Guard, k: u64) -> *mut u64 {
        // SAFETY: the guard owns the block and stores the moved address.
        match unsafe { Small::insert(g.0, k) } {
            Inserted::Slot { block, slot } => {
                g.0 = Some(block);
                slot
            }
            Inserted::Full => panic!("block full at {k}"),
        }
    }

    #[test]
    fn block_bytes_follow_the_ladder() {
        let mut g = Guard(None);
        let mut seen = [0usize; SMALL_MAX + 1];
        for i in 0..SMALL_MAX {
            put(&mut g, i as u64 * 7);
            seen[i + 1] = g.0.unwrap().bytes();
        }
        assert_eq!(
            [seen[1], seen[2], seen[10], seen[20], seen[31]],
            [24, 40, 200, 392, 520]
        );
    }

    #[test]
    fn insert_search_and_remove_agree_with_a_sorted_model() {
        let mut g = Guard(None);
        let mut model = std::collections::BTreeMap::new();
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..SMALL_MAX {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            // A narrow key space, so neighbours and duplicates both occur.
            let k = match x % 5 {
                0 => 0,
                1 => u64::MAX,
                _ => x % 64,
            };
            let slot = put(&mut g, k);
            // SAFETY: the slot is live until the next insert or remove.
            unsafe {
                if let std::collections::btree_map::Entry::Vacant(e) = model.entry(k) {
                    assert_eq!(*slot, 0, "a new slot is zero");
                    *slot = !k;
                    e.insert(!k);
                }
                assert_eq!(*slot, !k);
            }
        }
        let b = g.0.unwrap();
        assert_eq!(b.pop(), model.len());
        // A full block refuses a new key and still finds a present one.
        while g.0.unwrap().pop() < SMALL_MAX {
            let k = 1_000 + g.0.unwrap().pop() as u64;
            let slot = put(&mut g, k);
            // SAFETY: as above.
            unsafe { *slot = !k };
            model.insert(k, !k);
        }
        let b = g.0.unwrap();
        // SAFETY: the guard owns the block.
        let refused = unsafe { Small::insert(Some(b), 999_999) };
        assert!(matches!(refused, Inserted::Full));
        for probe in (0..70).chain([999, 1_000, 1_031, u64::MAX - 1, u64::MAX]) {
            let read = |e: Option<(u64, *mut u64)>| {
                // SAFETY: the slot is live; nothing mutates the block here.
                e.map(|(k, s)| (k, unsafe { *s }))
            };
            let copied = |e: Option<(&u64, &u64)>| e.map(|(k, v)| (*k, *v));
            assert_eq!(b.contains_key(probe), model.contains_key(&probe));
            assert_eq!(
                read(b.next_at_or_after(probe)),
                copied(model.range(probe..).next())
            );
            assert_eq!(
                read(b.prev_at_or_before(probe)),
                copied(model.range(..=probe).next_back())
            );
            assert_eq!(
                read(b.prev_before(probe)),
                copied(model.range(..probe).next_back())
            );
            let after = probe
                .checked_add(1)
                .and_then(|n| copied(model.range(n..).next()));
            assert_eq!(read(b.next_after(probe)), after);
            assert_eq!(
                b.count_range(probe, u64::MAX),
                model.range(probe..).count() as u64
            );
            assert_eq!(
                b.count_range(3, probe),
                if probe < 3 {
                    0
                } else {
                    model.range(3..=probe).count() as u64
                }
            );
        }
        for (i, (k, v)) in model.iter().enumerate() {
            // SAFETY: the slot is live; nothing mutates the block here.
            assert_eq!(b.nth(i).map(|(k, s)| (k, unsafe { *s })), Some((*k, *v)));
        }
        assert!(b.nth(model.len()).is_none());
        // Remove in an order that is neither ascending nor descending.
        let mut order: Vec<u64> = model.keys().copied().collect();
        let third = order.len() / 3;
        order.rotate_left(third);
        for (n, k) in order.iter().enumerate() {
            let b = g.0.unwrap();
            // SAFETY: the guard owns the block.
            let absent = unsafe { b.remove(k ^ 0x4000_0000) };
            assert!(matches!(absent, Removed::Absent));
            // SAFETY: the guard owns the block and drops a freed one.
            match unsafe { b.remove(*k) } {
                Removed::Kept => assert!(n + 1 < order.len()),
                Removed::Emptied => {
                    assert_eq!(n + 1, order.len());
                    g.0 = None;
                }
                Removed::Absent => panic!("{k} was present"),
            }
            model.remove(k);
            if let Some(b) = g.0 {
                assert_eq!(b.pop(), model.len());
                for (k, v) in &model {
                    // SAFETY: the slot is live until the next remove.
                    assert_eq!(unsafe { *b.get(*k) }, *v);
                }
            }
        }
        assert!(g.0.is_none());
    }
}
