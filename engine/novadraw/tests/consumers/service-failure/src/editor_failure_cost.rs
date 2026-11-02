use novadraw_editor::command::{Command, CommandError, CommandStack};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    error::Error,
    fmt,
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
};

struct Counter;
static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
unsafe impl GlobalAlloc for Counter {
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
static ALLOCATOR: Counter = Counter;
#[derive(Debug)]
struct Cause;
impl fmt::Display for Cause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cause")
    }
}
impl Error for Cause {}

struct Noop;
impl Command<()> for Noop {
    fn label(&self) -> &str {
        "noop"
    }
    fn execute(&mut self, _: &mut ()) -> Result<(), CommandError> {
        Ok(())
    }
    fn undo(&mut self, _: &mut ()) -> Result<(), CommandError> {
        Ok(())
    }
}

fn sample(name: &str, mut operation: impl FnMut()) {
    const SAMPLES: usize = 201;
    const OPERATIONS: usize = 10_000;
    let mut times = Vec::with_capacity(SAMPLES);
    for _ in 0..OPERATIONS {
        operation();
    }
    ALLOCATIONS.store(0, Ordering::Relaxed);
    for _ in 0..SAMPLES {
        COUNTING.store(true, Ordering::Relaxed);
        let start = Instant::now();
        for _ in 0..OPERATIONS {
            operation();
        }
        let elapsed = start.elapsed().as_nanos();
        COUNTING.store(false, Ordering::Relaxed);
        times.push(elapsed);
    }
    times.sort_unstable();
    println!(
        "{name} samples={SAMPLES} operations={OPERATIONS} p50_ns={} p95_ns={} allocations={}",
        times[SAMPLES / 2],
        times[SAMPLES * 95 / 100],
        ALLOCATIONS.load(Ordering::Relaxed)
    );
}

fn main() {
    sample("text_error", || {
        black_box(CommandError::operation("rejection"));
    });
    sample("owned_error", || {
        black_box(CommandError::operation_with_source("rejection", Cause));
    });
    let error = CommandError::operation_with_source("rejection", Cause);
    sample("owned_clone", || {
        black_box(error.clone());
    });
    sample("source_downcast", || {
        black_box(error.source().unwrap().downcast_ref::<Cause>());
    });
    let stack = CommandStack::<()>::new();
    sample("history_query", || {
        black_box(stack.has_undo());
    });
    let mut stack = CommandStack::new();
    stack.execute(&mut (), Box::new(Noop)).unwrap();
    sample("checked_history_query", || {
        black_box(stack.check_undo().unwrap());
    });
}
