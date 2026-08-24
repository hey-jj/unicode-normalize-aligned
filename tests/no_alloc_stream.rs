//! A byte-counting allocator proves the streaming iterators reclaim
//! emitted items: a long stream-safe input allocates zero bytes, with
//! growth through `realloc` counted. Separate from no_alloc.rs so each
//! binary runs exactly one test, which keeps the harness quiet while a
//! counter is read.

// Dev-only target: never compiled at the MSRV floor, so post-1.63 APIs are fine.
#![allow(clippy::incompatible_msrv)]
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

mod common;

struct CountingBytes;

static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingBytes {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::SeqCst);
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::SeqCst);
        System.alloc_zeroed(layout)
    }

    // Growth through realloc must count. A buffer that never reclaims
    // emitted items grows this way and a call counter never sees it.
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size > layout.size() {
            ALLOCATED_BYTES.fetch_add(new_size - layout.size(), Ordering::SeqCst);
        }
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: CountingBytes = CountingBytes;

#[test]
fn long_stream_safe_input_allocates_no_bytes() {
    // 50,000 starter+mark pairs. The pending buffer must reclaim emitted
    // items as it streams. Before that compaction this input grew the
    // buffer past 8 MB through realloc alone.
    let mut input = Vec::with_capacity(100_000);
    for _ in 0..50_000 {
        input.push('a');
        input.push('\u{0301}');
    }
    for form in common::FORMS {
        let before = ALLOCATED_BYTES.load(Ordering::SeqCst);
        let mut last = ('\0', 0isize);
        for pair in common::normalize_iter(form, &input) {
            last = pair;
        }
        std::hint::black_box(last);
        let bytes = ALLOCATED_BYTES.load(Ordering::SeqCst) - before;
        assert_eq!(
            bytes, 0,
            "{form} allocated {bytes} bytes on stream-safe text"
        );
    }
}
