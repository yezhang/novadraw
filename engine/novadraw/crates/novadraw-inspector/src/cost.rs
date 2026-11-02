//! Explicitly invoked, private cost probe; allocation counts are thread-local.
use super::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
    time::{Duration, Instant},
};

thread_local! {
    static COUNTS: Cell<(bool, usize, usize)> = const { Cell::new((false, 0, 0)) };
}

struct CountingAllocator;
fn count(bytes: usize) {
    let _ = COUNTS.try_with(|counts| {
        let (enabled, allocations, allocated_bytes) = counts.get();
        if enabled {
            counts.set((enabled, allocations + 1, allocated_bytes + bytes));
        }
    });
}
// Only the test binary uses this allocator; production retains its host allocator.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(size);
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn measure(name: &str, session: usize, mut setup: impl FnMut(), mut action: impl FnMut()) {
    const WARMUP: usize = 20;
    const SAMPLES: usize = 201;
    for _ in 0..WARMUP {
        setup();
        action();
    }
    let mut times = Vec::with_capacity(SAMPLES);
    let mut allocations = 0;
    let mut bytes = 0;
    for _ in 0..SAMPLES {
        setup();
        COUNTS.with(|counts| counts.set((true, 0, 0)));
        let start = Instant::now();
        action();
        let elapsed = start.elapsed().as_nanos();
        let (_, allocated, allocated_bytes) = COUNTS.with(|counts| {
            let value = counts.get();
            counts.set((false, 0, 0));
            value
        });
        times.push(elapsed);
        allocations += allocated;
        bytes += allocated_bytes;
    }
    times.sort_unstable();
    println!(
        "COST session={session} name={name} samples={SAMPLES} p50_ns={} p95_ns={} allocations={allocations} bytes={bytes}",
        times[SAMPLES / 2],
        times[SAMPLES * 95 / 100]
    );
}

#[test]
#[ignore = "explicit release measurement; not a correctness gate"]
fn inspector_service_cost() {
    struct Probe;
    impl ObservationListener for Probe {
        fn observed(
            &self,
            record: &NotificationRecord,
            query: StableSceneQuery<'_>,
        ) -> ListenerDirective {
            run_cost(record, query);
            ListenerDirective::Remove
        }
    }
    let mut runtime = Runtime::empty();
    runtime
        .set_contents(Box::new(
            novadraw::figure::RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap(),
        ))
        .unwrap();
    runtime
        .observations()
        .add_observation_listener(Box::new(Probe))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
}

fn run_cost(source: &NotificationRecord, query: StableSceneQuery<'_>) {
    const BATCH: usize = 256;
    for session in 0..5 {
        let record = NotificationRecord {
            namespace: source.namespace,
            source_epoch: 0,
            sequence: 1,
            effect: source.effect.clone(),
        };
        for capacity in [64, 4096] {
            let inspector = FigureInspector::new(NonZeroUsize::new(capacity).unwrap());
            let observer = InspectorObserver {
                state: inspector.state.clone(),
                first_fault: inspector.first_fault.clone(),
            };
            // An unfilled batch uses at most half the requested capacity.
            let batch = BATCH.min(capacity / 2);
            measure(
                &format!("enqueue_not_full_c{capacity}_b{batch}"),
                session,
                || inspector.clear_events().unwrap(),
                || {
                    for _ in 0..batch {
                        black_box(observer.observed(black_box(&record), query));
                    }
                },
            );
            for _ in 0..capacity {
                observer.observed(&record, query);
            }
            let allocated = inspector.state.lock().unwrap().events.capacity();
            measure(
                &format!("enqueue_full_c{capacity}_b{batch}"),
                session,
                || {},
                || {
                    for _ in 0..batch {
                        black_box(observer.observed(black_box(&record), query));
                    }
                },
            );
            assert_eq!(inspector.state.lock().unwrap().events.capacity(), allocated);
            measure(
                &format!("events_copy_c{capacity}"),
                session,
                || {},
                || {
                    black_box(inspector.events().unwrap());
                },
            );
        }
        for enabled in [false, true] {
            let mut owner = Runtime::empty();
            let figure = owner
                .set_contents(Box::new(
                    novadraw::figure::RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap(),
                ))
                .unwrap();
            let inspector = FigureInspector::new(NonZeroUsize::new(4096).unwrap());
            let token = enabled.then(|| inspector.attach(&mut owner).unwrap());
            owner.frames().stabilize_for_query().unwrap();
            let before = owner
                .observations()
                .checkpoint()
                .unwrap()
                .last_published_sequence
                .unwrap();
            measure(
                if enabled {
                    "runtime_dispatch_inspector_b32"
                } else {
                    "runtime_dispatch_no_inspector_b32"
                },
                session,
                || inspector.clear_events().unwrap(),
                || {
                    for _ in 0..32 {
                        owner.figure(figure).unwrap().translate(1.0, 0.0).unwrap();
                        owner.frames().stabilize_for_query().unwrap();
                    }
                },
            );
            let after = owner
                .observations()
                .checkpoint()
                .unwrap()
                .last_published_sequence
                .unwrap();
            // Each translation publishes Notify and FigureMoved, including warmup batches.
            assert_eq!(after - before, 2 * 32 * (20 + 201));
            if let Some(token) = token {
                assert_eq!(inspector.events().unwrap().len(), 64);
                token.detach(&mut owner).unwrap();
            }
        }
        let cell = std::cell::RefCell::new(OnceLock::new());
        for (name, candidate) in [
            (
                "first_fault_read",
                InspectorFault {
                    operation: InspectorOperation::Events,
                    source: None,
                },
            ),
            (
                "first_fault_observe",
                InspectorFault {
                    operation: InspectorOperation::Observe,
                    source: Some(InspectionSource {
                        namespace: record.namespace,
                        epoch: record.source_epoch,
                        sequence: record.sequence,
                    }),
                },
            ),
        ] {
            measure(
                name,
                session,
                || *cell.borrow_mut() = OnceLock::new(),
                || {
                    black_box(publish_fault(&cell.borrow(), candidate));
                },
            );
        }

        // Each timed call is an actual observer callback blocked by a healthy guard.
        // Thread/channel construction and the ready handshake are outside timing.
        let inspector = FigureInspector::new(NonZeroUsize::new(4096).unwrap());
        let observer = InspectorObserver {
            state: inspector.state.clone(),
            first_fault: inspector.first_fault.clone(),
        };
        for (name, observe) in [
            ("lock_wait_hold_100us", false),
            ("observer_wait_hold_100us", true),
        ] {
            inspector.clear_events().unwrap();
            let mut times = Vec::with_capacity(201);
            let mut allocations = 0;
            let mut bytes = 0;
            for _ in 0..201 {
                std::thread::scope(|scope| {
                    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
                    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
                    let inspector = &inspector;
                    scope.spawn(move || {
                        let _guard = inspector.state.lock().unwrap();
                        ready_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                        let held = Instant::now();
                        while held.elapsed() < Duration::from_micros(100) {
                            std::hint::spin_loop();
                        }
                    });
                    ready_rx.recv().unwrap();
                    release_tx.send(()).unwrap();
                    COUNTS.with(|counts| counts.set((true, 0, 0)));
                    let start = Instant::now();
                    if observe {
                        assert_eq!(observer.observed(&record, query), ListenerDirective::Keep);
                    } else {
                        drop(black_box(inspector.state.lock().unwrap()));
                    }
                    let elapsed = start.elapsed().as_nanos();
                    let (_, allocated, allocated_bytes) = COUNTS.with(|counts| {
                        let value = counts.get();
                        counts.set((false, 0, 0));
                        value
                    });
                    allocations += allocated;
                    bytes += allocated_bytes;
                    times.push(elapsed);
                });
            }
            times.sort_unstable();
            println!(
                "COST session={session} name={name} samples=201 p50_ns={} p95_ns={} allocations={allocations} bytes={bytes}",
                times[100], times[190]
            );
            assert_eq!(
                inspector.events().unwrap().len(),
                if observe { 201 } else { 0 }
            );
        }
    }
}
