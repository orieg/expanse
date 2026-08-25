//! High-performance stack-based in-order iterator for Expanse tries.
//!
//! Replaces per-element `next_at_or_after` re-descents with an amortized
//! stack walk, streaming contiguous leaf elements in O(1) time per key.

use crate::bits::Bitmap256;
use crate::leaf;
use crate::mutate::decode_value;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::types::{EdgeTag, EdgeType, Key};

/// State of an active branch node on the traversal stack.
#[derive(Clone, Copy)]
enum BranchKind {
    L3 {
        ptr: *const BranchL3,
        num: u8,
        digits: [u8; 8],
        idx: u8,
    },
    L7 {
        ptr: *const BranchL7,
        num: u8,
        digits: [u8; 8],
        idx: u8,
    },
    B {
        ptr: *const BranchB,
        current_digit: u16,
    },
    U {
        ptr: *const BranchU,
        current_digit: u16,
    },
}

#[derive(Clone, Copy)]
struct StackFrame {
    kind: BranchKind,
    level: u8,
    prefix: u64,
}

/// State of an active leaf node yielding elements directly.
#[derive(Clone, Copy)]
enum LeafCursor {
    Empty,
    RootLeaf {
        keys: *const u64,
        values: *const u64,
        pop: u32,
        idx: u32,
    },
    Linear {
        keys_ptr: *const u8,
        values_ptr: *const u64,
        kb: u8,
        pop: u16,
        idx: u16,
        prefix: u64,
    },
    BitmapSet {
        words: [u64; 4],
        word_idx: u8,
        prefix: u64,
    },
    BitmapMap {
        bitmap: Bitmap256,
        values: [*mut u64; 8],
        sub: u8,
        current_word: u32,
        rank: u16,
        prefix: u64,
    },
    ImmedSingle {
        key: u64,
        value: u64,
        done: bool,
    },
    ImmedMulti {
        keys: [u64; 15],
        values: [u64; 15],
        count: u8,
        idx: u8,
    },
    FullExpanse {
        next_key: u64,
        max_key: u64,
    },
}

/// A zero-allocation, stack-based in-order trie iterator.
pub struct RawIter<const MAP: bool> {
    leaf: LeafCursor,
    stack: [StackFrame; 8],
    depth: usize,
}

impl<const MAP: bool> Default for RawIter<MAP> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const MAP: bool> RawIter<MAP> {
    /// Creates an empty iterator.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            leaf: LeafCursor::Empty,
            stack: [StackFrame {
                kind: BranchKind::U {
                    ptr: core::ptr::null(),
                    current_digit: 256,
                },
                level: 0,
                prefix: 0,
            }; 8],
            depth: 0,
        }
    }

    /// Initializes iteration from a root leaf.
    pub fn from_root_leaf(keys: *const u64, values: *const u64, pop: usize) -> Self {
        let mut iter = Self::new();
        iter.leaf = LeafCursor::RootLeaf {
            keys,
            values,
            pop: pop as u32,
            idx: 0,
        };
        iter
    }

    /// Initializes iteration from a root leaf starting at `start_key`.
    ///
    /// # Safety
    /// `keys` and `values` must point to live, initialized allocations of at least `pop` entries.
    pub unsafe fn from_root_leaf_range(
        keys: *const u64,
        values: *const u64,
        pop: usize,
        start_key: u64,
    ) -> Self {
        let mut iter = Self::new();
        // SAFETY: root leaf contains `pop` sorted u64 keys per contract.
        let slice = unsafe { core::slice::from_raw_parts(keys, pop) };
        let idx = slice.partition_point(|&k| k < start_key);
        iter.leaf = LeafCursor::RootLeaf {
            keys,
            values,
            pop: pop as u32,
            idx: idx as u32,
        };
        iter
    }

    /// Initializes iteration from a root edge.
    ///
    /// # Safety
    /// `top` must point to a live, well-formed trie edge.
    pub unsafe fn from_tree(top: &Edge) -> Self {
        let mut iter = Self::new();
        // SAFETY: top points to a live trie per contract.
        unsafe {
            iter.descend(top, 8, 0);
        }
        iter
    }

    /// Initializes iteration from a root edge starting at `start_key`.
    ///
    /// # Safety
    /// `top` must point to a live, well-formed trie edge.
    pub unsafe fn from_tree_range(top: &Edge, start_key: u64) -> Self {
        let mut iter = Self::new();
        // SAFETY: top points to a live trie per contract.
        unsafe {
            iter.descend_seek(top, 8, 0, start_key);
            if matches!(iter.leaf, LeafCursor::Empty) {
                iter.advance_leaf();
            }
        }
        iter
    }

    /// Descends from `edge` into the leftmost leaf, pushing branches onto `stack`.
    ///
    /// # Safety
    /// `edge` must point to a live node / leaf at `level`.
    unsafe fn descend(&mut self, edge: &Edge, level: u8, prefix: u64) {
        let mut cur_edge = *edge;
        let mut cur_level = level;
        let mut cur_prefix = prefix;

        loop {
            if cur_edge.is_null() {
                return;
            }

            let tag = match cur_edge.tag() {
                Some(t) => t,
                None => return,
            };

            match tag {
                EdgeTag::Structural(EdgeType::Null) => return,

                EdgeTag::Immed(im) => {
                    let n = im.key_count();
                    // Single-key immediates dominate sparse-key tries (every
                    // leaf holds one key). Decode the one key/value directly,
                    // skipping the two 15-wide staging arrays and the wide
                    // cursor copy the multi-key path pays per element.
                    if n == 1 {
                        let kb = im.key_bytes() as usize;
                        let (low, value) = if MAP {
                            // Map immediate: key in the low `kb` aux bytes,
                            // value in word 0.
                            let aux = cur_edge.aux_bytes();
                            let mut kbuf = [0u8; 8];
                            kbuf[..kb].copy_from_slice(&aux[..kb]);
                            (
                                u64::from_le_bytes(kbuf),
                                u64::from_le_bytes(cur_edge.imm_bytes()),
                            )
                        } else {
                            // Set immediate: key packed in word 0.
                            let w0 = cur_edge.imm_bytes();
                            let mut kbuf = [0u8; 8];
                            kbuf[..kb].copy_from_slice(&w0[..kb]);
                            (u64::from_le_bytes(kbuf), 0)
                        };
                        self.leaf = LeafCursor::ImmedSingle {
                            key: cur_prefix | low,
                            value,
                            done: false,
                        };
                        return;
                    }
                    let (keys_buf, vals_buf) = if MAP {
                        let k = crate::mutate::immed_map_keys(&cur_edge, im);
                        let mut v = [0u64; 15];
                        if n == 1 {
                            v[0] = u64::from_le_bytes(cur_edge.imm_bytes());
                        } else {
                            let v_ptr = cur_edge.node_ptr().cast::<u64>();
                            for (i, val) in v.iter_mut().enumerate().take(n as usize) {
                                // SAFETY: live value array for n entries.
                                *val = unsafe { *v_ptr.add(i) };
                            }
                        }
                        (k, v)
                    } else {
                        (crate::mutate::immed_keys(&cur_edge, im), [0u64; 15])
                    };
                    let mut keys = [0u64; 15];
                    for i in 0..n as usize {
                        keys[i] = cur_prefix | keys_buf[i];
                    }
                    self.leaf = LeafCursor::ImmedMulti {
                        keys,
                        values: vals_buf,
                        count: n,
                        idx: 0,
                    };
                    return;
                }

                EdgeTag::Structural(
                    t @ (EdgeType::Leaf1
                    | EdgeType::Leaf2
                    | EdgeType::Leaf3
                    | EdgeType::Leaf4
                    | EdgeType::Leaf5
                    | EdgeType::Leaf6
                    | EdgeType::Leaf7),
                ) => {
                    let kb = t.leaf_key_bytes().expect("leaf tag");
                    let pop = cur_edge.pop0(kb) as u16 + 1;
                    let dv = if kb < cur_level {
                        decode_value(&cur_edge, kb, cur_level)
                    } else {
                        0
                    };
                    let leaf_prefix = cur_prefix | (dv << (8 * u32::from(kb)));
                    let base = cur_edge.node_ptr();
                    let (keys_ptr, values_ptr): (*const u8, *const u64) = if MAP {
                        // SAFETY: map leaf layout: values then class-sized keys.
                        let keys = unsafe { base.add(leaf::map_keys_offset(pop as usize)) };
                        (keys, base.cast::<u64>())
                    } else {
                        (base, core::ptr::null())
                    };
                    self.leaf = LeafCursor::Linear {
                        keys_ptr,
                        values_ptr,
                        kb,
                        pop,
                        idx: 0,
                        prefix: leaf_prefix,
                    };
                    return;
                }

                EdgeTag::Structural(EdgeType::LeafB1) => {
                    let dv = if cur_level > 1 {
                        decode_value(&cur_edge, 1, cur_level)
                    } else {
                        0
                    };
                    let leaf_prefix = cur_prefix | (dv << 8);
                    if MAP {
                        // SAFETY: live LeafBitmapL node pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<LeafBitmapL>() };
                        let current_word = (b.bitmap.words[0] & 0xFFFF_FFFF) as u32;
                        self.leaf = LeafCursor::BitmapMap {
                            bitmap: b.bitmap,
                            values: b.values,
                            sub: 0,
                            current_word,
                            rank: 0,
                            prefix: leaf_prefix,
                        };
                    } else {
                        // SAFETY: live LeafBitmap1 node pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<LeafBitmap1>() };
                        self.leaf = LeafCursor::BitmapSet {
                            words: b.bitmap.words,
                            word_idx: 0,
                            prefix: leaf_prefix,
                        };
                    }
                    return;
                }

                EdgeTag::Structural(EdgeType::FullExpanse) => {
                    let max = if cur_level == 8 {
                        u64::MAX
                    } else {
                        (1u64 << (8 * u32::from(cur_level))) - 1
                    };
                    self.leaf = LeafCursor::FullExpanse {
                        next_key: cur_prefix,
                        max_key: cur_prefix | max,
                    };
                    return;
                }

                EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
                    // SAFETY: live branch per contract.
                    let bl = unsafe { crate::mutate::branch_form_level(&cur_edge, t, cur_level) };
                    let dv = if bl < cur_level {
                        decode_value(&cur_edge, bl, cur_level)
                    } else {
                        0
                    };
                    let branch_prefix = if bl < cur_level {
                        cur_prefix | (dv << (8 * u32::from(bl)))
                    } else {
                        cur_prefix
                    };

                    if matches!(t, EdgeType::BranchL3) {
                        // SAFETY: live BranchL3 pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<BranchL3>() };
                        let num = b.hdr.num;
                        if num == 0 {
                            return;
                        }
                        let d = b.hdr.digits[0];
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: slot 0 is valid for non-empty branch.
                        let next_child = b.edges[0];

                        self.stack[self.depth] = StackFrame {
                            kind: BranchKind::L3 {
                                ptr: b,
                                num,
                                digits: b.hdr.digits,
                                idx: 1,
                            },
                            level: bl,
                            prefix: branch_prefix,
                        };
                        self.depth += 1;

                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    } else {
                        // SAFETY: live BranchL7 pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<BranchL7>() };
                        let num = b.hdr.num;
                        if num == 0 {
                            return;
                        }
                        let d = b.hdr.digits[0];
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: slot 0 is valid for non-empty branch.
                        let next_child = b.edges[0];

                        self.stack[self.depth] = StackFrame {
                            kind: BranchKind::L7 {
                                ptr: b,
                                num,
                                digits: b.hdr.digits,
                                idx: 1,
                            },
                            level: bl,
                            prefix: branch_prefix,
                        };
                        self.depth += 1;

                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    }
                }

                EdgeTag::Structural(EdgeType::BranchB) => {
                    // SAFETY: live BranchB pointer.
                    let b = unsafe { &*cur_edge.node_ptr().cast::<BranchB>() };
                    let bl = b.level;
                    let dv = if bl < cur_level {
                        decode_value(&cur_edge, bl, cur_level)
                    } else {
                        0
                    };
                    let branch_prefix = if bl < cur_level {
                        cur_prefix | (dv << (8 * u32::from(bl)))
                    } else {
                        cur_prefix
                    };

                    let Some(first_d) = b.bitmap.next_set(0) else {
                        return;
                    };
                    let slot = b.bitmap.subexpanse_rank(first_d) as usize;
                    let sub = (first_d >> 5) as usize;
                    // SAFETY: first_d is set in bitmap → subarrays[sub] is non-null.
                    let next_child = unsafe { *b.subarrays[sub].add(slot) };
                    let child_prefix =
                        branch_prefix | (u64::from(first_d) << (8 * u32::from(bl - 1)));

                    self.stack[self.depth] = StackFrame {
                        kind: BranchKind::B {
                            ptr: b,
                            current_digit: u16::from(first_d) + 1,
                        },
                        level: bl,
                        prefix: branch_prefix,
                    };
                    self.depth += 1;

                    cur_edge = next_child;
                    cur_level = bl - 1;
                    cur_prefix = child_prefix;
                }

                EdgeTag::Structural(EdgeType::BranchU) => {
                    // SAFETY: live BranchU pointer.
                    let b = unsafe { &*cur_edge.node_ptr().cast::<BranchU>() };
                    let bl = cur_level;
                    let mut d = 0u16;
                    while d < 256 && b.edges[d as usize].is_null() {
                        d += 1;
                    }
                    if d == 256 {
                        return;
                    }
                    let next_child = b.edges[d as usize];
                    let child_prefix = cur_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));

                    self.stack[self.depth] = StackFrame {
                        kind: BranchKind::U {
                            ptr: b,
                            current_digit: d + 1,
                        },
                        level: bl,
                        prefix: cur_prefix,
                    };
                    self.depth += 1;

                    cur_edge = next_child;
                    cur_level = bl - 1;
                    cur_prefix = child_prefix;
                }
            }
        }
    }

    /// Descends from `edge` targeting the smallest key `>= start`, pushing branches onto `stack`.
    ///
    /// # Safety
    /// `edge` must point to a live node / leaf at `level`.
    unsafe fn descend_seek(&mut self, edge: &Edge, level: u8, prefix: u64, start: u64) {
        let mut cur_edge = *edge;
        let mut cur_level = level;
        let mut cur_prefix = prefix;

        loop {
            if cur_edge.is_null() {
                self.leaf = LeafCursor::Empty;
                return;
            }

            let tag = match cur_edge.tag() {
                Some(t) => t,
                None => {
                    self.leaf = LeafCursor::Empty;
                    return;
                }
            };

            match tag {
                EdgeTag::Structural(EdgeType::Null) => {
                    self.leaf = LeafCursor::Empty;
                    return;
                }

                EdgeTag::Immed(im) => {
                    let n = im.key_count();
                    let (keys_buf, vals_buf) = if MAP {
                        let k = crate::mutate::immed_map_keys(&cur_edge, im);
                        let mut v = [0u64; 15];
                        if n == 1 {
                            v[0] = u64::from_le_bytes(cur_edge.imm_bytes());
                        } else {
                            let v_ptr = cur_edge.node_ptr().cast::<u64>();
                            for (i, val) in v.iter_mut().enumerate().take(n as usize) {
                                // SAFETY: live value array for n entries.
                                *val = unsafe { *v_ptr.add(i) };
                            }
                        }
                        (k, v)
                    } else {
                        (crate::mutate::immed_keys(&cur_edge, im), [0u64; 15])
                    };
                    let mut keys = [0u64; 15];
                    let mut first_idx = None;
                    for i in 0..n as usize {
                        let full_k = cur_prefix | keys_buf[i];
                        keys[i] = full_k;
                        if first_idx.is_none() && full_k >= start {
                            first_idx = Some(i as u8);
                        }
                    }
                    if let Some(idx) = first_idx {
                        self.leaf = LeafCursor::ImmedMulti {
                            keys,
                            values: vals_buf,
                            count: n,
                            idx,
                        };
                    } else {
                        self.leaf = LeafCursor::Empty;
                    }
                    return;
                }

                EdgeTag::Structural(
                    t @ (EdgeType::Leaf1
                    | EdgeType::Leaf2
                    | EdgeType::Leaf3
                    | EdgeType::Leaf4
                    | EdgeType::Leaf5
                    | EdgeType::Leaf6
                    | EdgeType::Leaf7),
                ) => {
                    let kb = t.leaf_key_bytes().expect("leaf tag");
                    let pop = cur_edge.pop0(kb) as u16 + 1;
                    let dv = if kb < cur_level {
                        decode_value(&cur_edge, kb, cur_level)
                    } else {
                        0
                    };
                    let leaf_prefix = cur_prefix | (dv << (8 * u32::from(kb)));
                    let base = cur_edge.node_ptr();
                    let (keys_ptr, values_ptr): (*const u8, *const u64) = if MAP {
                        // SAFETY: map leaf layout: values then class-sized keys.
                        let keys = unsafe { base.add(leaf::map_keys_offset(pop as usize)) };
                        (keys, base.cast::<u64>())
                    } else {
                        (base, core::ptr::null())
                    };

                    let shift = 8 * u32::from(kb);
                    let start_high = if shift >= 64 { 0 } else { start >> shift };
                    let prefix_high = if shift >= 64 { 0 } else { leaf_prefix >> shift };

                    if start_high > prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let idx = if start_high < prefix_high {
                        0u16
                    } else {
                        let start_low = crate::mutate::key_low(start, kb);
                        // SAFETY: keys_ptr points to pop packed keys in live leaf.
                        let at = unsafe {
                            leaf::locate(keys_ptr, pop as usize, kb, start_low)
                                .unwrap_or_else(|insert_idx| insert_idx)
                        };
                        if at >= pop as usize {
                            self.leaf = LeafCursor::Empty;
                            return;
                        }
                        at as u16
                    };

                    self.leaf = LeafCursor::Linear {
                        keys_ptr,
                        values_ptr,
                        kb,
                        pop,
                        idx,
                        prefix: leaf_prefix,
                    };
                    return;
                }

                EdgeTag::Structural(EdgeType::LeafB1) => {
                    let dv = if cur_level > 1 {
                        decode_value(&cur_edge, 1, cur_level)
                    } else {
                        0
                    };
                    let leaf_prefix = cur_prefix | (dv << 8);

                    let start_high = start >> 8;
                    let prefix_high = leaf_prefix >> 8;

                    if start_high > prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let target_d = if start_high < prefix_high {
                        0u8
                    } else {
                        (start & 0xFF) as u8
                    };

                    if MAP {
                        // SAFETY: live LeafBitmapL node pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<LeafBitmapL>() };
                        if let Some(first_d) = b.bitmap.next_set(target_d) {
                            let sub = first_d >> 5;
                            let rank = b.bitmap.subexpanse_rank(first_d) as u16;
                            let sub_word = (b.bitmap.words[sub as usize / 2]
                                >> ((sub as usize % 2) * 32))
                                as u32;
                            let bit_in_sub = (first_d & 31) as u32;
                            let current_word = sub_word & (!0u32 << bit_in_sub);
                            self.leaf = LeafCursor::BitmapMap {
                                bitmap: b.bitmap,
                                values: b.values,
                                sub,
                                current_word,
                                rank,
                                prefix: leaf_prefix,
                            };
                        } else {
                            self.leaf = LeafCursor::Empty;
                        }
                    } else {
                        // SAFETY: live LeafBitmap1 node pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<LeafBitmap1>() };
                        if let Some(first_d) = b.bitmap.next_set(target_d) {
                            let word_idx = first_d >> 6;
                            let mut words = b.bitmap.words;
                            for w in words.iter_mut().take(word_idx as usize) {
                                *w = 0;
                            }
                            let bit_in_word = (first_d & 63) as u32;
                            words[word_idx as usize] &= !0u64 << bit_in_word;
                            self.leaf = LeafCursor::BitmapSet {
                                words,
                                word_idx,
                                prefix: leaf_prefix,
                            };
                        } else {
                            self.leaf = LeafCursor::Empty;
                        }
                    }
                    return;
                }

                EdgeTag::Structural(EdgeType::FullExpanse) => {
                    let max = if cur_level == 8 {
                        u64::MAX
                    } else {
                        (1u64 << (8 * u32::from(cur_level))) - 1
                    };
                    let max_key = cur_prefix | max;
                    let next_key = start.max(cur_prefix);
                    if next_key <= max_key {
                        self.leaf = LeafCursor::FullExpanse { next_key, max_key };
                    } else {
                        self.leaf = LeafCursor::Empty;
                    }
                    return;
                }

                EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
                    // SAFETY: live branch per contract.
                    let bl = unsafe { crate::mutate::branch_form_level(&cur_edge, t, cur_level) };
                    let dv = if bl < cur_level {
                        decode_value(&cur_edge, bl, cur_level)
                    } else {
                        0
                    };
                    let branch_prefix = if bl < cur_level {
                        cur_prefix | (dv << (8 * u32::from(bl)))
                    } else {
                        cur_prefix
                    };

                    let shift = 8 * u32::from(bl);
                    let start_high = if shift >= 64 { 0 } else { start >> shift };
                    let prefix_high = if shift >= 64 {
                        0
                    } else {
                        branch_prefix >> shift
                    };

                    if start_high > prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let (num, digits, edges_ptr, ptr_raw) = if matches!(t, EdgeType::BranchL3) {
                        // SAFETY: cur_edge is a live BranchL3 pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<BranchL3>() };
                        (
                            b.hdr.num,
                            b.hdr.digits,
                            b.edges.as_ptr(),
                            b as *const BranchL3 as *const (),
                        )
                    } else {
                        // SAFETY: cur_edge is a live BranchL7 pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<BranchL7>() };
                        (
                            b.hdr.num,
                            b.hdr.digits,
                            b.edges.as_ptr(),
                            b as *const BranchL7 as *const (),
                        )
                    };

                    if num == 0 {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    if start_high < prefix_high {
                        let d = digits[0];
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: slot 0 is valid for non-empty branch.
                        let next_child = unsafe { *edges_ptr };

                        if matches!(t, EdgeType::BranchL3) {
                            self.stack[self.depth] = StackFrame {
                                kind: BranchKind::L3 {
                                    ptr: ptr_raw.cast(),
                                    num,
                                    digits,
                                    idx: 1,
                                },
                                level: bl,
                                prefix: branch_prefix,
                            };
                        } else {
                            self.stack[self.depth] = StackFrame {
                                kind: BranchKind::L7 {
                                    ptr: ptr_raw.cast(),
                                    num,
                                    digits,
                                    idx: 1,
                                },
                                level: bl,
                                prefix: branch_prefix,
                            };
                        }
                        self.depth += 1;

                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    } else {
                        let target_d = crate::types::digit(start, bl);
                        let mut match_slot = None;
                        for (s, &d) in digits.iter().enumerate().take(num as usize) {
                            if d >= target_d {
                                match_slot = Some(s);
                                break;
                            }
                        }

                        let Some(slot) = match_slot else {
                            self.leaf = LeafCursor::Empty;
                            return;
                        };

                        let d = digits[slot];
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: slot < num and edges_ptr has num entries.
                        let next_child = unsafe { *edges_ptr.add(slot) };

                        if matches!(t, EdgeType::BranchL3) {
                            self.stack[self.depth] = StackFrame {
                                kind: BranchKind::L3 {
                                    ptr: ptr_raw.cast(),
                                    num,
                                    digits,
                                    idx: (slot + 1) as u8,
                                },
                                level: bl,
                                prefix: branch_prefix,
                            };
                        } else {
                            self.stack[self.depth] = StackFrame {
                                kind: BranchKind::L7 {
                                    ptr: ptr_raw.cast(),
                                    num,
                                    digits,
                                    idx: (slot + 1) as u8,
                                },
                                level: bl,
                                prefix: branch_prefix,
                            };
                        }
                        self.depth += 1;

                        if d == target_d {
                            cur_edge = next_child;
                            cur_level = bl - 1;
                            cur_prefix = child_prefix;
                        } else {
                            // First child >= target_d is strictly greater than target_d; descend leftmost
                            // SAFETY: next_child is a live edge at bl - 1.
                            unsafe {
                                self.descend(&next_child, bl - 1, child_prefix);
                            }
                            return;
                        }
                    }
                }

                EdgeTag::Structural(EdgeType::BranchB) => {
                    // SAFETY: live BranchB pointer.
                    let b = unsafe { &*cur_edge.node_ptr().cast::<BranchB>() };
                    let bl = b.level;
                    let dv = if bl < cur_level {
                        decode_value(&cur_edge, bl, cur_level)
                    } else {
                        0
                    };
                    let branch_prefix = if bl < cur_level {
                        cur_prefix | (dv << (8 * u32::from(bl)))
                    } else {
                        cur_prefix
                    };

                    let shift = 8 * u32::from(bl);
                    let start_high = if shift >= 64 { 0 } else { start >> shift };
                    let prefix_high = if shift >= 64 {
                        0
                    } else {
                        branch_prefix >> shift
                    };

                    if start_high > prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let target_d = if start_high < prefix_high {
                        0u8
                    } else {
                        crate::types::digit(start, bl)
                    };

                    let Some(first_d) = b.bitmap.next_set(target_d) else {
                        self.leaf = LeafCursor::Empty;
                        return;
                    };

                    let slot = b.bitmap.subexpanse_rank(first_d) as usize;
                    let sub = (first_d >> 5) as usize;
                    // SAFETY: first_d is set in bitmap → subarrays[sub] is non-null.
                    let next_child = unsafe { *b.subarrays[sub].add(slot) };
                    let child_prefix =
                        branch_prefix | (u64::from(first_d) << (8 * u32::from(bl - 1)));

                    self.stack[self.depth] = StackFrame {
                        kind: BranchKind::B {
                            ptr: b,
                            current_digit: u16::from(first_d) + 1,
                        },
                        level: bl,
                        prefix: branch_prefix,
                    };
                    self.depth += 1;

                    if first_d == target_d && start_high == prefix_high {
                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    } else {
                        // Descend leftmost into next_child
                        // SAFETY: next_child is a live edge at bl - 1.
                        unsafe {
                            self.descend(&next_child, bl - 1, child_prefix);
                        }
                        return;
                    }
                }

                EdgeTag::Structural(EdgeType::BranchU) => {
                    // SAFETY: live BranchU pointer.
                    let b = unsafe { &*cur_edge.node_ptr().cast::<BranchU>() };
                    let bl = cur_level;
                    let shift = 8 * u32::from(bl);
                    let start_high = if shift >= 64 { 0 } else { start >> shift };
                    let prefix_high = if shift >= 64 { 0 } else { cur_prefix >> shift };

                    if start_high > prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let target_d = if start_high < prefix_high {
                        0u16
                    } else {
                        u16::from(crate::types::digit(start, bl))
                    };

                    let mut d = target_d;
                    while d < 256 && b.edges[d as usize].is_null() {
                        d += 1;
                    }
                    if d == 256 {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let next_child = b.edges[d as usize];
                    let child_prefix = cur_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));

                    self.stack[self.depth] = StackFrame {
                        kind: BranchKind::U {
                            ptr: b,
                            current_digit: d + 1,
                        },
                        level: bl,
                        prefix: cur_prefix,
                    };
                    self.depth += 1;

                    if d == target_d && start_high == prefix_high {
                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    } else {
                        // Descend leftmost into next_child
                        // SAFETY: next_child is a live edge at bl - 1.
                        unsafe {
                            self.descend(&next_child, bl - 1, child_prefix);
                        }
                        return;
                    }
                }
            }
        }
    }

    /// Advances the iterator to the next leaf in tree order.
    ///
    /// # Safety
    /// Stack must contain valid node pointers per tree invariants.
    unsafe fn advance_leaf(&mut self) -> bool {
        while self.depth > 0 {
            let top = &mut self.stack[self.depth - 1];
            let bl = top.level;
            let branch_prefix = top.prefix;

            match &mut top.kind {
                BranchKind::L3 {
                    ptr,
                    num,
                    digits,
                    idx,
                } => {
                    if *idx < *num {
                        let slot = *idx as usize;
                        let d = digits[slot];
                        *idx += 1;
                        // SAFETY: ptr is valid BranchL3 and slot < num.
                        let child = unsafe { (*(*ptr)).edges[slot] };
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: child is live edge at bl - 1.
                        unsafe {
                            self.descend(&child, bl - 1, child_prefix);
                        }
                        return true;
                    }
                }
                BranchKind::L7 {
                    ptr,
                    num,
                    digits,
                    idx,
                } => {
                    if *idx < *num {
                        let slot = *idx as usize;
                        let d = digits[slot];
                        *idx += 1;
                        // SAFETY: ptr is valid BranchL7 and slot < num.
                        let child = unsafe { (*(*ptr)).edges[slot] };
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: child is live edge at bl - 1.
                        unsafe {
                            self.descend(&child, bl - 1, child_prefix);
                        }
                        return true;
                    }
                }
                BranchKind::B { ptr, current_digit } => {
                    if *current_digit <= 255 {
                        // SAFETY: ptr is valid BranchB.
                        let b = unsafe { &**ptr };
                        if let Some(next_d) = b.bitmap.next_set(*current_digit as u8) {
                            *current_digit = u16::from(next_d) + 1;
                            let slot = b.bitmap.subexpanse_rank(next_d) as usize;
                            let sub = (next_d >> 5) as usize;
                            // SAFETY: next_d is set in bitmap → subarrays[sub] is non-null.
                            let child = unsafe { *b.subarrays[sub].add(slot) };
                            let child_prefix =
                                branch_prefix | (u64::from(next_d) << (8 * u32::from(bl - 1)));
                            // SAFETY: child is live edge at bl - 1.
                            unsafe {
                                self.descend(&child, bl - 1, child_prefix);
                            }
                            return true;
                        }
                    }
                }
                BranchKind::U { ptr, current_digit } => {
                    // SAFETY: ptr is valid BranchU.
                    let b = unsafe { &**ptr };
                    while *current_digit < 256 {
                        let d = *current_digit;
                        *current_digit += 1;
                        let child = b.edges[d as usize];
                        if !child.is_null() {
                            let child_prefix =
                                branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                            // SAFETY: child is live edge at bl - 1.
                            unsafe {
                                self.descend(&child, bl - 1, child_prefix);
                            }
                            return true;
                        }
                    }
                }
            }

            self.depth -= 1;
        }

        false
    }

    /// Yields the next element in ascending key order.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<(Key, u64)> {
        loop {
            match &mut self.leaf {
                LeafCursor::Empty => return None,

                LeafCursor::RootLeaf {
                    keys,
                    values,
                    pop,
                    idx,
                } => {
                    if *idx < *pop {
                        let i = *idx as usize;
                        *idx += 1;
                        // SAFETY: i < pop per check.
                        let k = unsafe { *keys.add(i) };
                        let v = if MAP {
                            // SAFETY: values array holds at least pop entries.
                            unsafe { *values.add(i) }
                        } else {
                            0
                        };
                        return Some((k, v));
                    }
                    self.leaf = LeafCursor::Empty;
                    return None;
                }

                LeafCursor::Linear {
                    keys_ptr,
                    values_ptr,
                    kb,
                    pop,
                    idx,
                    prefix,
                } => {
                    if *idx < *pop {
                        let i = *idx as usize;
                        *idx += 1;
                        // SAFETY: i < pop and keys_ptr is valid for pop packed keys.
                        let low = unsafe { crate::mutate::read_packed(*keys_ptr, i, *kb as usize) };
                        let v = if MAP {
                            // SAFETY: values_ptr holds at least pop values.
                            unsafe { *values_ptr.add(i) }
                        } else {
                            0
                        };
                        return Some((*prefix | low, v));
                    }
                }

                LeafCursor::BitmapSet {
                    words,
                    word_idx,
                    prefix,
                } => {
                    while *word_idx < 4 {
                        let w = words[*word_idx as usize];
                        if w != 0 {
                            let bit = w.trailing_zeros();
                            words[*word_idx as usize] = w & (w - 1);
                            let d = (u64::from(*word_idx) << 6) | u64::from(bit);
                            return Some((*prefix | d, 0));
                        }
                        *word_idx += 1;
                    }
                }

                LeafCursor::BitmapMap {
                    bitmap,
                    values,
                    sub,
                    current_word,
                    rank,
                    prefix,
                } => {
                    while *sub < 8 {
                        if *current_word != 0 {
                            let bit = current_word.trailing_zeros();
                            *current_word &= *current_word - 1;
                            let d = (u64::from(*sub) << 5) | u64::from(bit);
                            // SAFETY: bit is set in subexpanse → value array has entry at rank.
                            let v = unsafe { *values[*sub as usize].add(*rank as usize) };
                            *rank += 1;
                            return Some((*prefix | d, v));
                        }
                        *sub += 1;
                        if *sub < 8 {
                            *current_word = (bitmap.words[*sub as usize / 2]
                                >> ((*sub as usize % 2) * 32))
                                as u32;
                            *rank = 0;
                        }
                    }
                }

                LeafCursor::ImmedSingle { key, value, done } => {
                    if !*done {
                        *done = true;
                        return Some((*key, *value));
                    }
                }

                LeafCursor::ImmedMulti {
                    keys,
                    values,
                    count,
                    idx,
                } => {
                    if *idx < *count {
                        let i = *idx as usize;
                        *idx += 1;
                        return Some((keys[i], values[i]));
                    }
                }

                LeafCursor::FullExpanse { next_key, max_key } => {
                    if *next_key <= *max_key {
                        let k = *next_key;
                        *next_key += 1;
                        return Some((k, 0));
                    }
                }
            }

            // Current leaf exhausted; ascend to next leaf in tree walk.
            // SAFETY: tree invariant holds.
            let has_more = unsafe { self.advance_leaf() };
            if !has_more {
                self.leaf = LeafCursor::Empty;
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::ExpanseMap;
    use crate::set::ExpanseSet;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn test_empty_iter() {
        let mut empty_set_iter = RawIter::<false>::new();
        assert_eq!(empty_set_iter.next(), None);

        let mut empty_map_iter = RawIter::<true>::new();
        assert_eq!(empty_map_iter.next(), None);
    }

    #[test]
    fn test_set_iter_dense_and_sparse() {
        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();

        // 1. Immediate
        for k in [10, 20, 30] {
            set.insert(k);
            model.insert(k);
        }
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            model.iter().copied().collect::<Vec<_>>()
        );

        // 2. Linear leaf
        for k in 100..120 {
            set.insert(k);
            model.insert(k);
        }
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            model.iter().copied().collect::<Vec<_>>()
        );

        // 3. Bitmap leaf & multi-level branches
        for k in (1000..2000).step_by(3) {
            set.insert(k);
            model.insert(k);
        }
        for k in [100_000, 1_000_000, 100_000_000, 0x1234_5678_9ABC_DEF0] {
            set.insert(k);
            model.insert(k);
        }
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            model.iter().copied().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_iter_sparse_single_key_immediates() {
        // `keys(i) = i << 40` (benches/compare.rs "sparse"): bytes 0..4 are
        // always zero, so every leaf is a single-key immediate. Exercises the
        // ImmedSingle fast path for both set and map flavors against the
        // ordered std baselines.
        let n = 20_000u64;

        let mut set = ExpanseSet::new();
        let mut set_model = BTreeSet::new();
        for i in 0..n {
            let k = i << 40;
            set.insert(k);
            set_model.insert(k);
        }
        assert!(
            set.iter().eq(set_model.iter().copied()),
            "sparse set ordered iteration"
        );

        let mut map = ExpanseMap::new();
        let mut map_model = BTreeMap::new();
        for i in 0..n {
            let k = i << 40;
            let v = k.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            map.insert(k, v);
            map_model.insert(k, v);
        }
        assert!(
            map.iter().eq(map_model.iter().map(|(&k, &v)| (k, v))),
            "sparse map ordered iteration"
        );
    }

    #[test]
    fn test_map_iter_dense_and_sparse() {
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();

        for i in 0..5000u64 {
            let k = (i * 37 + 13) ^ (i << 12);
            let v = k.wrapping_mul(3);
            map.insert(k, v);
            model.insert(k, v);
        }

        let map_items: Vec<_> = map.iter().collect();
        let model_items: Vec<_> = model.iter().map(|(&k, &v)| (k, v)).collect();
        assert_eq!(map_items, model_items);
    }
}
