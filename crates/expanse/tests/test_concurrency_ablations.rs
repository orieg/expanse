//! Integration tests verifying concurrency ablation arms (Hypothesis D):
//! - Arm (a): Sharded allocator accounting counters (`ablation-sharded-alloc`)
//! - Arm (b): Striped epoch garbage bins and sharded retained bytes (`ablation-striped-epoch`)
//!
//! Gated by `feature = "std"`.

#![cfg(all(
    feature = "std",
    any(feature = "ablation-sharded-alloc", feature = "ablation-striped-epoch")
))]

#[cfg(feature = "ablation-sharded-alloc")]
use expanse_trie::alloc::NodeAlloc;
#[cfg(feature = "ablation-striped-epoch")]
use expanse_trie::occ::Collector;
#[cfg(feature = "ablation-sharded-alloc")]
use std::ptr::NonNull;
use std::sync::Arc;
#[cfg(feature = "ablation-striped-epoch")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

#[cfg(feature = "ablation-sharded-alloc")]
struct SendPtr(NonNull<u8>);
// SAFETY: SendPtr is a test-only wrapper transferring raw heap allocations across test threads.
#[cfg(feature = "ablation-sharded-alloc")]
unsafe impl Send for SendPtr {}

#[test]
#[cfg(feature = "ablation-sharded-alloc")]
fn test_node_alloc_sharded_counters_cross_thread() {
    let alloc = Arc::new(NodeAlloc::new());
    let pop_threads = 4;
    let ops_per_thread = 500;

    let mut handles = Vec::new();
    for _ in 0..pop_threads {
        let a = Arc::clone(&alloc);
        handles.push(thread::spawn(move || {
            let mut ptrs = Vec::new();
            for _ in 0..ops_per_thread {
                // 32 bytes allocation (Leaf1 / raw byte size)
                let p = a.alloc_bytes(32);
                ptrs.push(SendPtr(p));
            }
            ptrs
        }));
    }

    let mut all_ptrs = Vec::new();
    for h in handles {
        all_ptrs.extend(h.join().unwrap());
    }

    assert_eq!(alloc.total_allocs(), pop_threads * ops_per_thread);
    assert_eq!(alloc.live_allocs(), pop_threads * ops_per_thread);
    assert_eq!(alloc.bytes_in_use(), pop_threads * ops_per_thread * 32);

    // Cross-thread free: free from different threads than the allocating threads
    let chunk_size = all_ptrs.len() / pop_threads;
    let mut free_handles = Vec::new();
    for chunk in all_ptrs.chunks(chunk_size) {
        let a = Arc::clone(&alloc);
        let ptrs: Vec<SendPtr> = chunk.iter().map(|p| SendPtr(p.0)).collect();
        free_handles.push(thread::spawn(move || {
            for p in ptrs {
                // SAFETY: p.0 was allocated with alloc_bytes(32) above
                unsafe { a.free_bytes(p.0, 32) };
            }
        }));
    }

    for h in free_handles {
        h.join().unwrap();
    }

    assert_eq!(alloc.live_allocs(), 0);
    assert_eq!(alloc.bytes_in_use(), 0);
    assert_eq!(alloc.total_allocs(), pop_threads * ops_per_thread);
}

#[test]
#[cfg(feature = "ablation-striped-epoch")]
fn test_striped_epoch_bins_multi_writer() {
    let collector = Arc::new(Collector::new());
    let num_writers = 4;
    let items_per_writer = 200;
    let done = Arc::new(AtomicBool::new(false));

    // Reader thread that continuously pins/unpins
    let r_collector = Arc::clone(&collector);
    let r_done = Arc::clone(&done);
    let reader_handle = thread::spawn(move || {
        let reader = r_collector.register();
        while !r_done.load(Ordering::Relaxed) {
            let _pin = reader.pin();
            thread::yield_now();
        }
    });

    let mut writer_handles = Vec::new();
    for _ in 0..num_writers {
        let c = Arc::clone(&collector);
        writer_handles.push(thread::spawn(move || {
            for _ in 0..items_per_writer {
                let layout = std::alloc::Layout::from_size_align(64, 16).unwrap();
                // SAFETY: layout has non-zero size (64) and valid alignment (16).
                let raw = unsafe { std::alloc::alloc_zeroed(layout) };
                let ptr = core::ptr::NonNull::new(raw).unwrap();
                c.retire(ptr, 64, 16);
                c.try_advance();
            }
        }));
    }

    for h in writer_handles {
        h.join().unwrap();
    }

    // Stop reader and let it unpin
    done.store(true, Ordering::Release);
    reader_handle.join().unwrap();

    // Advance epochs to allow all retired garbage to become freeable
    for _ in 0..10 {
        collector.try_advance();
    }

    // All retired items have passed their grace period and been reclaimed
    assert_eq!(collector.retained_bytes(), 0);
}

#[test]
#[cfg(feature = "ablation-striped-epoch")]
fn test_striped_epoch_single_thread_lifecycle() {
    let collector = Arc::new(Collector::new());
    let reader = collector.register();
    let pin = reader.pin();

    let layout = std::alloc::Layout::from_size_align(64, 16).unwrap();
    // SAFETY: non-zero size (64) and valid alignment (16).
    let raw = unsafe { std::alloc::alloc_zeroed(layout) };
    let ptr = core::ptr::NonNull::new(raw).unwrap();
    collector.retire(ptr, 64, 16);
    assert_eq!(collector.retained_bytes(), 64);

    // Advance attempts under pin must not reclaim the block
    collector.try_advance();
    collector.try_advance();
    assert_eq!(collector.retained_bytes(), 64);

    drop(pin);
    collector.try_advance();
    collector.try_advance();
    assert_eq!(collector.retained_bytes(), 0);
}
