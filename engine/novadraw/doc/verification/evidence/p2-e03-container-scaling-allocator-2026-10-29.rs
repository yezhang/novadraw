//! Native test instrumentation. Counts only allocations on the measured thread.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    time::Instant,
};

thread_local! {
    static COUNTS: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

struct CountingAllocator;

fn record(bytes: usize) {
    let _ = COUNTS.try_with(|counts| {
        if let Some((calls, requested)) = counts.get() {
            counts.set(Some((calls + 1, requested + bytes)));
        }
    });
}

// System retains allocation ownership; this wrapper only observes requested sizes.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc(layout) };
        if !result.is_null() {
            record(layout.size());
        }
        result
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc_zeroed(layout) };
        if !result.is_null() {
            record(layout.size());
        }
        result
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(ptr, layout, new_size) };
        if !result.is_null() {
            record(new_size);
        }
        result
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

pub fn measure<T>(operation: impl FnOnce() -> T) -> (T, (u128, usize, usize)) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            COUNTS.with(|counts| counts.set(None));
        }
    }
    let reset = Reset;
    COUNTS.with(|counts| {
        assert!(counts.get().is_none(), "nested allocation measurement");
        counts.set(Some((0, 0)));
    });
    let began = Instant::now();
    let result = operation();
    let elapsed = began.elapsed().as_nanos();
    let (calls, bytes) = COUNTS.with(|counts| counts.get().unwrap());
    drop(reset);
    (result, (elapsed, calls, bytes))
}
