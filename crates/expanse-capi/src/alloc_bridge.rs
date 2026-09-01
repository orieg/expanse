//! Bare-metal allocator and panic bridge for the `no_std` staticlib (#558).
//!
//! `expanse-trie` is `no_std + alloc`, so a bare-metal link needs a
//! `#[global_allocator]`. The host supplies it: on ESP-IDF the C side already
//! wraps `heap_caps_malloc` in `components/expanse/src/expanse_esp_idf.c`, and
//! any other target can provide the same two symbols.
//!
//! **Alignment is not optional here.** Every 32-bit branch and leaf node is
//! `#[repr(C, align(32))]` with a compile-time assert (`node32.rs`), while
//! `malloc` and `heap_caps_malloc` guarantee only 4–8 bytes on a 32-bit
//! target. Handing back an under-aligned block is undefined behaviour on the
//! first branch allocation and would silently destroy the cache-line packing
//! the layout exists for, so this bridge over-allocates and aligns by hand
//! rather than assuming the host heap is generous.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe extern "C" {
    /// Host allocator. Must return a block of at least `size` bytes, or null.
    fn expanse_host_malloc(size: usize) -> *mut u8;
    /// Frees a pointer previously returned by [`expanse_host_malloc`].
    fn expanse_host_free(ptr: *mut u8);
}

/// Bytes reserved ahead of every block to store thehost pointer for `dealloc`.
const HEADER: usize = core::mem::size_of::<usize>();

/// Routes Rust allocations to the host's `malloc`/`free`, honouring alignment.
pub struct HostAlloc;

// SAFETY: `alloc` returns either null or a block of at least `layout.size()`
// bytes aligned to `layout.align()`, and `dealloc` frees exactly the pointer
// the host returned — recovered from the header written immediately before the
// aligned address. The two are inverses, which is `GlobalAlloc`'s contract.
unsafe impl GlobalAlloc for HostAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(core::mem::align_of::<usize>());
        // Worst case: the host block starts one byte past an aligned address,
        // so reserve a full `align` of slack plus room for the header.
        let total = match layout.size().checked_add(align + HEADER) {
            Some(t) => t,
            None => return ptr::null_mut(),
        };
        // SAFETY: FFI call into the host allocator; null is handled below.
        let raw = unsafe { expanse_host_malloc(total) };
        if raw.is_null() {
            return ptr::null_mut();
        }
        let base = raw as usize + HEADER;
        let aligned = (base + align - 1) & !(align - 1);
        // SAFETY: `aligned - HEADER >= raw`, inside the block just allocated.
        unsafe { ptr::write_unaligned((aligned - HEADER) as *mut usize, raw as usize) };
        aligned as *mut u8
    }

    unsafe fn dealloc(&self, ptr_in: *mut u8, _layout: Layout) {
        if ptr_in.is_null() {
            return;
        }
        // SAFETY: written by `alloc` immediately before the returned address.
        let raw = unsafe { ptr::read_unaligned((ptr_in as usize - HEADER) as *const usize) };
        // SAFETY: `raw` is exactly what the host allocator returned.
        unsafe { expanse_host_free(raw as *mut u8) };
    }
}

#[global_allocator]
static ALLOC: HostAlloc = HostAlloc;

/// Aborts on panic. Opt-in: a host that already defines a panic handler must
/// be able to link this library without colliding with it.
#[cfg(feature = "embedded-panic-handler")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
