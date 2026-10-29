use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
};

struct CountingAllocator;
static ENABLED: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ENABLED.load(Ordering::Relaxed) {
            CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if ENABLED.load(Ordering::Relaxed) {
            CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(size as u64, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn main() {
    const SAMPLES: usize = 101;
    const UPDATES: usize = 120;
    for mode in 0..3 {
        std::hint::black_box(novadraw_style_input_consumer::frame_workload(mode, UPDATES));
        let mut elapsed = Vec::with_capacity(SAMPLES);
        CALLS.store(0, Ordering::Relaxed);
        BYTES.store(0, Ordering::Relaxed);
        for _ in 0..SAMPLES {
            ENABLED.store(true, Ordering::Relaxed);
            let start = Instant::now();
            std::hint::black_box(novadraw_style_input_consumer::frame_workload(mode, UPDATES));
            let duration = start.elapsed().as_nanos();
            ENABLED.store(false, Ordering::Relaxed);
            elapsed.push(duration);
        }
        elapsed.sort_unstable();
        println!(
            "frame_mode={mode} samples={SAMPLES} updates={UPDATES} nodes=64 p50_ns={} p95_ns={} allocations={} requested_bytes={}",
            elapsed[SAMPLES / 2],
            elapsed[SAMPLES * 95 / 100],
            CALLS.load(Ordering::Relaxed),
            BYTES.load(Ordering::Relaxed)
        );
    }
    for mode in 0..4 {
        std::hint::black_box(novadraw_style_input_consumer::border_workload(
            mode, UPDATES,
        ));
        let mut elapsed = Vec::with_capacity(SAMPLES);
        CALLS.store(0, Ordering::Relaxed);
        BYTES.store(0, Ordering::Relaxed);
        for _ in 0..SAMPLES {
            ENABLED.store(true, Ordering::Relaxed);
            let start = Instant::now();
            std::hint::black_box(novadraw_style_input_consumer::border_workload(
                mode, UPDATES,
            ));
            let duration = start.elapsed().as_nanos();
            ENABLED.store(false, Ordering::Relaxed);
            elapsed.push(duration);
        }
        elapsed.sort_unstable();
        println!(
            "border_mode={mode} samples={SAMPLES} updates={UPDATES} p50_ns={} p95_ns={} allocations={} requested_bytes={}",
            elapsed[SAMPLES / 2],
            elapsed[SAMPLES * 95 / 100],
            CALLS.load(Ordering::Relaxed),
            BYTES.load(Ordering::Relaxed)
        );
    }
    std::hint::black_box(novadraw_style_input_consumer::style_workload(
        12.0, 0.5, UPDATES,
    ));
    let mut elapsed = Vec::with_capacity(SAMPLES);
    CALLS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    for _ in 0..SAMPLES {
        ENABLED.store(true, Ordering::Relaxed);
        let start = Instant::now();
        std::hint::black_box(novadraw_style_input_consumer::style_workload(
            12.0, 0.5, UPDATES,
        ));
        let duration = start.elapsed().as_nanos();
        ENABLED.store(false, Ordering::Relaxed);
        elapsed.push(duration);
    }
    elapsed.sort_unstable();
    println!(
        "samples={SAMPLES} updates_per_sample={UPDATES} nodes=2 p50_ns={} p95_ns={} allocations={} requested_bytes={}",
        elapsed[SAMPLES / 2],
        elapsed[SAMPLES * 95 / 100],
        CALLS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed)
    );
}
