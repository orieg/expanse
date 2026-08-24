use crate::bits::Bitmap256;
use crate::leaf;
use crate::mutate::{branch_form_level, decode_value, key_low, pow256};
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::types::{EdgeTag, EdgeType, digit};

#[derive(Clone, Copy)]
pub(crate) struct BranchCursor {
    pub(crate) edge: Edge,
    pub(crate) level: u8,
    pub(crate) prefix: u64,
    pub(crate) slot: u16,
}

#[derive(Clone, Copy)]
pub(crate) enum LeafCursor {
    Linear {
        keys: *const u8,
        values: *const u64,
        kb: u8,
        pop: u16,
        slot: u16,
        prefix: u64,
    },
    Bitmap {
        bitmap: *const Bitmap256,
        values: *const [*mut u64; 8],
        d: u16,
        prefix: u64,
    },
    Immed {
        keys: [u64; 15],
        values: [u64; 15],
        pop: u8,
        slot: u8,
    },
    FullExpanse {
        next_key: u64,
        max_key: u64,
    },
}

pub struct RawIter<const MAP: bool> {
    pub(crate) leaf: Option<LeafCursor>,
    pub(crate) stack: [BranchCursor; 8],
    pub(crate) depth: usize,
}
