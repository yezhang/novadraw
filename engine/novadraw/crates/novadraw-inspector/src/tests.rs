use super::*;
use novadraw::figure::RectangleFigure;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Barrier;

fn inspector(capacity: usize) -> FigureInspector {
    FigureInspector::new(NonZeroUsize::new(capacity).unwrap())
}

fn scene() -> Runtime {
    let mut runtime = Runtime::empty();
    runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 20.0, 20.0).unwrap(),
        ))
        .unwrap();
    runtime
}

fn poison(inspector: &FigureInspector) {
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _guard = inspector.state.lock().unwrap();
            panic!("private event-buffer injection");
        }))
        .is_err()
    );
}

fn record(runtime: &Runtime, sequence: u64) -> NotificationRecord {
    NotificationRecord {
        namespace: runtime.tree().root_id().namespace(),
        source_epoch: 4,
        sequence,
        effect: NotificationEffect::Notify {
            figure_id: runtime.tree().root_id(),
        },
    }
}

#[test]
fn read_and_clear_poison_publish_source_free_first_fault_shared_by_clones() {
    for operation in [InspectorOperation::Events, InspectorOperation::ClearEvents] {
        let inspector = inspector(2);
        let clone = inspector.clone();
        let mut runtime = scene();
        let subscription = inspector.attach(&mut runtime).unwrap();
        poison(&inspector);
        let error = match operation {
            InspectorOperation::Events => inspector.events().unwrap_err(),
            _ => inspector.clear_events().unwrap_err(),
        };
        let expected = InspectorFault {
            operation,
            source: None,
        };
        assert!(
            matches!(error, InspectorError::Faulted { operation: op, first_fault } if op == operation && first_fault == expected)
        );
        assert_eq!(clone.fault(), Some(expected));
        assert!(
            matches!(clone.clear_events(), Err(InspectorError::Faulted { operation: InspectorOperation::ClearEvents, first_fault }) if first_fault == expected)
        );
        runtime.frames().stabilize_for_query().unwrap();
        assert!(!subscription.detach(&mut runtime).unwrap());
        assert_eq!(clone.fault(), Some(expected));
    }
}

#[test]
fn observer_poison_self_removes_without_faulting_runtime_or_other_observers() {
    let mut runtime = scene();
    let inspector = inspector(8);
    let clone = inspector.clone();
    let subscription = inspector.attach(&mut runtime).unwrap();
    let other = FigureInspector::new(NonZeroUsize::new(8).unwrap());
    let other_subscription = other.attach(&mut runtime).unwrap();
    poison(&inspector);
    runtime.frames().stabilize_for_query().unwrap();
    let checkpoint = runtime.observations().checkpoint().unwrap();
    let first = inspector.fault().unwrap();
    assert_eq!(first.operation, InspectorOperation::Observe);
    let source = first.source.unwrap();
    assert_eq!(source.namespace, checkpoint.namespace);
    assert!(source.sequence <= checkpoint.last_published_sequence.unwrap());
    assert_eq!(clone.fault(), Some(first));
    assert!(!subscription.detach(&mut runtime).unwrap());
    assert!(!other.events().unwrap().is_empty());
    assert!(other_subscription.detach(&mut runtime).unwrap());
    assert!(matches!(
        clone.attach(&mut runtime),
        Err(InspectorError::Faulted { .. })
    ));
    assert!(matches!(
        clone.attach_scoped(&mut runtime, ListenerScope::Runtime),
        Err(InspectorError::Faulted { .. })
    ));
    // Failed Inspector admission consumed no Core identity.
    let next = other.attach(&mut runtime).unwrap();
    assert_ne!(next.listener_id(), other_subscription.listener_id());
    assert!(next.detach(&mut runtime).unwrap());
}

#[test]
fn competing_poison_discoveries_publish_one_complete_candidate() {
    let mut runtime = Runtime::empty();
    runtime.frames().stabilize_for_query().unwrap();
    let candidate = InspectorFault {
        operation: InspectorOperation::Observe,
        source: Some(InspectionSource {
            namespace: runtime.tree().root_id().namespace(),
            epoch: 9,
            sequence: 11,
        }),
    };
    for _ in 0..32 {
        let cell = OnceLock::new();
        let barrier = Barrier::new(3);
        std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                barrier.wait();
                publish_fault(&cell, candidate)
            });
            let b = scope.spawn(|| {
                barrier.wait();
                publish_fault(
                    &cell,
                    InspectorFault {
                        operation: InspectorOperation::Events,
                        source: None,
                    },
                )
            });
            barrier.wait();
            let first = a.join().unwrap();
            assert_eq!(first, b.join().unwrap());
            assert!(
                first == candidate
                    || first
                        == InspectorFault {
                            operation: InspectorOperation::Events,
                            source: None
                        }
            );
        });
    }
}

#[test]
fn fifo_evicts_before_push_without_capacity_growth() {
    let runtime = scene();
    let inspector = inspector(3);
    let mut state = inspector.state.lock().unwrap();
    let allocated = state.events.capacity();
    for sequence in 1..=20 {
        state.push(&record(&runtime, sequence));
        assert_eq!(state.events.capacity(), allocated);
    }
    assert_eq!(
        state
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        [18, 19, 20]
    );
}

#[test]
fn healthy_lock_competition_waits_and_preserves_every_record() {
    let runtime = scene();
    let inspector = inspector(8);
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        let guard = inspector.state.lock().unwrap();
        let reader = scope.spawn(|| {
            barrier.wait();
            inspector.events().unwrap()
        });
        barrier.wait();
        drop(guard);
        assert!(reader.join().unwrap().is_empty());
    });
    assert_eq!(inspector.fault(), None);
    let mut runtime = runtime;
    let subscription = inspector.attach(&mut runtime).unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert!(!inspector.events().unwrap().is_empty());
    assert_eq!(inspector.fault(), None);
    subscription.detach(&mut runtime).unwrap();
}

#[test]
fn healthy_observer_waits_for_reader_guard_then_publishes_all_records() {
    use std::sync::mpsc;
    use std::time::Duration;
    let inspector = inspector(32);
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let guard = inspector.state.lock().unwrap();
        let observer = scope.spawn(|| {
            let mut runtime = scene();
            let token = inspector.attach(&mut runtime).unwrap();
            started_tx.send(()).unwrap();
            runtime.frames().stabilize_for_query().unwrap();
            let checkpoint = runtime.observations().checkpoint().unwrap();
            done_tx.send(()).unwrap();
            token.detach(&mut runtime).unwrap();
            checkpoint
        });
        started_rx.recv().unwrap();
        assert!(matches!(
            done_rx.recv_timeout(Duration::from_millis(5)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        drop(guard);
        let checkpoint = observer.join().unwrap();
        let events = inspector.events().unwrap();
        assert_eq!(
            events.len() as u64,
            checkpoint.last_published_sequence.unwrap()
        );
        assert_eq!(
            events.last().unwrap().sequence,
            checkpoint.last_published_sequence.unwrap()
        );
        assert_eq!(inspector.fault(), None);
    });
}

#[test]
fn actual_read_and_observer_poison_race_keeps_complete_first_candidate() {
    for _ in 0..32 {
        let inspector = inspector(8);
        poison(&inspector);
        let barrier = Barrier::new(2);
        std::thread::scope(|scope| {
            let reader = scope.spawn(|| {
                barrier.wait();
                inspector.events().unwrap_err()
            });
            let mut runtime = scene();
            let token = inspector.attach(&mut runtime).unwrap();
            barrier.wait();
            runtime.frames().stabilize_for_query().unwrap();
            let checkpoint = runtime.observations().checkpoint().unwrap();
            let InspectorError::Faulted { first_fault, .. } = reader.join().unwrap() else {
                panic!("read must report local poison");
            };
            assert_eq!(inspector.fault(), Some(first_fault));
            match first_fault.operation {
                InspectorOperation::Events => assert_eq!(first_fault.source, None),
                InspectorOperation::Observe => {
                    let source = first_fault.source.unwrap();
                    assert_eq!(source.namespace, checkpoint.namespace);
                    assert!(source.sequence <= checkpoint.last_published_sequence.unwrap());
                }
                other => panic!("unexpected winner: {other:?}"),
            }
            assert!(!token.detach(&mut runtime).unwrap());
        });
    }
}

#[test]
fn shared_fault_across_runtimes_keeps_winning_source_and_tokens_independent() {
    let mut first = scene();
    let mut second = scene();
    let inspector = inspector(4);
    let a = inspector.attach(&mut first).unwrap();
    let b = inspector.clone().attach(&mut second).unwrap();
    poison(&inspector);
    first.frames().stabilize_for_query().unwrap();
    let fault = inspector.fault().unwrap();
    second.frames().stabilize_for_query().unwrap();
    assert_eq!(inspector.fault(), Some(fault));
    assert_eq!(fault.source.unwrap().namespace, a.owner());
    assert!(!a.detach(&mut first).unwrap());
    assert!(!b.detach(&mut second).unwrap());
    assert!(first.observations().checkpoint().is_ok());
    assert!(second.observations().checkpoint().is_ok());
}

#[test]
fn read_first_fault_survives_later_runtime_callbacks_and_recovery_uses_new_instance() {
    let mut runtime = scene();
    let old = inspector(8);
    let token = old.attach(&mut runtime).unwrap();
    poison(&old);
    old.events().unwrap_err();
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(
        old.fault(),
        Some(InspectorFault {
            operation: InspectorOperation::Events,
            source: None
        })
    );
    assert!(!token.detach(&mut runtime).unwrap());
    let before = runtime.observations().checkpoint().unwrap();
    let fresh = inspector(8);
    let token = fresh.attach(&mut runtime).unwrap();
    fresh.capture_tree(&mut runtime).unwrap();
    assert_eq!(before, runtime.observations().checkpoint().unwrap());
    assert!(fresh.events().unwrap().is_empty());
    runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 20.0, 20.0).unwrap(),
        ))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert!(
        fresh
            .events()
            .unwrap()
            .iter()
            .all(|event| event.sequence > before.last_published_sequence.unwrap())
    );
    token.detach(&mut runtime).unwrap();
    assert!(old.events().is_err());
}
