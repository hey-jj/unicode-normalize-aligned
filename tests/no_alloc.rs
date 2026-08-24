//! A counting allocator proves the iterators allocate nothing on stream-safe
//! text and exactly once on an oversized combining run.

// Dev-only target: never compiled at the MSRV floor, so post-1.63 APIs are fine.
#![allow(clippy::incompatible_msrv)]
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

mod common;
#[path = "common/ucd.rs"]
mod ucd;

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

// The test harness allocates too, so the counter is only read around
// single-threaded regions inside the one test function below. The byte
// counting variant lives in no_alloc_stream.rs for the same reason: one
// test per binary keeps the harness quiet while a counter is read.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        System.alloc_zeroed(layout)
    }

    // Growing an existing allocation is not a new allocation. The counting
    // in no_alloc_stream.rs covers growth through realloc.
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn count_allocations<R>(f: impl FnOnce() -> R) -> usize {
    let before = ALLOCATIONS.load(Ordering::SeqCst);
    let result = f();
    std::hint::black_box(result);
    ALLOCATIONS.load(Ordering::SeqCst) - before
}

fn drain(form: &str, input: &[char]) -> (char, isize) {
    let mut last = ('\0', 0);
    for pair in common::normalize_iter(form, input) {
        last = pair;
    }
    last
}

#[test]
fn allocation_budget() {
    // Every NormalizationTest input column, all four forms: zero allocations.
    let lines = ucd::test_lines();
    let mut inputs: Vec<Vec<char>> = Vec::new();
    for line in &lines {
        inputs.extend(line.columns.iter().cloned());
    }
    for input in &inputs {
        for form in common::FORMS {
            let n = count_allocations(|| drain(form, input));
            assert_eq!(
                n, 0,
                "{form} allocated on a NormalizationTest input: {input:?}"
            );
        }
    }

    // A 30-non-starter run between starters stays inline.
    let mut run30 = vec!['a'];
    run30.extend(std::iter::repeat('\u{0301}').take(30));
    run30.push('b');
    for form in common::FORMS {
        assert_eq!(count_allocations(|| drain(form, &run30)), 0, "{form} run30");
    }

    // Oversized runs allocate a fixed number of times. Decomposition
    // spills its pending-run buffer from 32 marks on, reused afterwards,
    // and allocates an ordering scratch from 33 on when the run closes.
    // Composition adds a blocked-mark buffer, which holds one mark fewer
    // than the run (the first mark composes with the starter) and so
    // spills from 34 marks on.
    for (len, decomposed, composed) in [
        (31usize, 0, 0),
        (32, 1, 1),
        (33, 2, 2),
        (34, 2, 3),
        (100, 2, 3),
    ] {
        let mut run = vec!['a'];
        run.extend(std::iter::repeat('\u{0301}').take(len));
        run.push('b');
        for form in common::FORMS {
            let expected = match form {
                "NFC" | "NFKC" => composed,
                _ => decomposed,
            };
            assert_eq!(
                count_allocations(|| drain(form, &run)),
                expected,
                "{form} run of {len}"
            );
        }
    }
}
