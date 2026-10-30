use novadraw::{animation::AnimationMode, runtime::Runtime};
use novadraw_service_failure_consumer::mode_updates;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
};

struct CountingAllocator;
static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn main() {
    const SAMPLES: usize = 201;
    const UPDATES: usize = 10_000;
    for change in [false, true] {
        let mut runtime = Runtime::empty();
        let mut times = Vec::with_capacity(SAMPLES);
        for _ in 0..20 {
            std::hint::black_box(mode_updates(&mut runtime, change, UPDATES));
        }
        ALLOCATIONS.store(0, Ordering::Relaxed);
        for _ in 0..SAMPLES {
            // Reset outside measurement so the first update is comparable.
            let _ = std::hint::black_box(
                runtime
                    .animation()
                    .timeline()
                    .set_mode(AnimationMode::Enabled),
            );
            COUNTING.store(true, Ordering::Relaxed);
            let start = Instant::now();
            std::hint::black_box(mode_updates(&mut runtime, change, UPDATES));
            let elapsed = start.elapsed().as_nanos();
            COUNTING.store(false, Ordering::Relaxed);
            times.push(elapsed);
        }
        times.sort_unstable();
        println!(
            "change={change} samples={SAMPLES} updates={UPDATES} p50_ns={} p95_ns={} allocations={}",
            times[SAMPLES / 2],
            times[SAMPLES * 95 / 100],
            ALLOCATIONS.load(Ordering::Relaxed),
        );
    }
}
