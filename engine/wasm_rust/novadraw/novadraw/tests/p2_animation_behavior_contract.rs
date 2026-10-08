//! External contracts for P2-M01 animation behaviors and committed-fact triggers.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use novadraw::{
    Affine2D, FigureTree, Rectangle, RectangleFigure, Runtime,
    animation::{
        AnimationBehavior, AnimationBehaviorContext, AnimationError, AnimationFact, AnimationMode,
        AnimationPlan, AnimationTrigger, InteractionGeometryPolicy, Motion, Opacity, Tween,
    },
    event::{FigureEvent, FigureListener, ListenerDirective, MonotonicTime, property},
    render::{BackendCapabilities, RenderOutcome, SurfaceInfo},
};

fn time(milliseconds: u64) -> MonotonicTime {
    MonotonicTime::from_micros(milliseconds * 1_000)
}

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 200.0,
        logical_height: 120.0,
        pixel_width: 200,
        pixel_height: 120,
        scale_factor: 1.0,
    }
}

fn flush_initial_state(runtime: &mut Runtime) {
    runtime.record_full_frame();
}

#[derive(Clone)]
struct FigureEventRecorder(Arc<Mutex<Vec<FigureEvent>>>);

impl FigureListener for FigureEventRecorder {
    fn figure_moved(&self, event: FigureEvent) -> ListenerDirective {
        self.0.lock().unwrap().push(event);
        ListenerDirective::Keep
    }
}

#[test]
fn ordinary_mutation_stays_static_without_an_installed_behavior() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    flush_initial_state(&mut runtime);

    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(40.0, 0.0, 10.0, 10.0))
        .unwrap();
    runtime.record_full_frame();

    assert_eq!(runtime.animations().behavior_count(), 0);
    assert_eq!(runtime.animations().active_animation_count(), 0);
}

#[test]
fn bounds_trigger_coalesces_one_transaction_and_installs_override_before_recording() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    flush_initial_state(&mut runtime);
    runtime.advance_time(time(0)).unwrap();
    let transform = runtime
        .animations()
        .bind_figure(figure, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let factory_calls = calls.clone();
    let factory_observed = observed.clone();
    runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::FigureBoundsChanged,
                move |context: AnimationBehaviorContext<'_>| {
                    factory_calls.fetch_add(1, Ordering::SeqCst);
                    factory_observed
                        .lock()
                        .unwrap()
                        .extend_from_slice(context.facts());
                    Ok(Some(AnimationPlan::track(
                        transform,
                        Motion::Tween(
                            Tween::between(
                                Affine2D::from_translation(-40.0, 0.0),
                                Affine2D::IDENTITY,
                                Duration::from_millis(100),
                            )
                            .unwrap(),
                        ),
                    )?))
                },
            )
            .scoped_to(figure),
        )
        .unwrap();
    let listener_events = Arc::new(Mutex::new(Vec::new()));
    runtime.add_figure_listener(Box::new(FigureEventRecorder(listener_events.clone())));

    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0))
        .unwrap();
    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(40.0, 0.0, 10.0, 10.0))
        .unwrap();
    runtime.record_full_frame();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[AnimationFact::FigureBoundsChanged {
            figure,
            old_bounds: Rectangle::new(0.0, 0.0, 10.0, 10.0),
            new_bounds: Rectangle::new(40.0, 0.0, 10.0, 10.0),
        }]
    );
    assert_eq!(
        runtime.animations().value(transform).unwrap(),
        Affine2D::from_translation(-40.0, 0.0)
    );
    assert_eq!(listener_events.lock().unwrap().len(), 2);
}

#[test]
fn property_state_and_transaction_triggers_match_committed_facts() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    flush_initial_state(&mut runtime);
    let property_calls = Arc::new(AtomicUsize::new(0));
    let state_calls = Arc::new(AtomicUsize::new(0));
    let transaction_facts = Arc::new(Mutex::new(Vec::new()));

    let calls = property_calls.clone();
    runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::property_changed(property::ENABLED),
                move |_: AnimationBehaviorContext<'_>| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(None)
                },
            )
            .scoped_to(figure),
        )
        .unwrap();
    let calls = state_calls.clone();
    runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::state_changed(property::ENABLED),
                move |_: AnimationBehaviorContext<'_>| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(None)
                },
            )
            .scoped_to(figure),
        )
        .unwrap();
    let facts = transaction_facts.clone();
    runtime
        .animations()
        .install_behavior(AnimationBehavior::new(
            AnimationTrigger::Transaction,
            move |context: AnimationBehaviorContext<'_>| {
                facts.lock().unwrap().extend_from_slice(context.facts());
                Ok(None)
            },
        ))
        .unwrap();

    runtime.figure(figure).unwrap().set_enabled(false).unwrap();
    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0))
        .unwrap();
    runtime.record_full_frame();

    assert_eq!(property_calls.load(Ordering::SeqCst), 1);
    assert_eq!(state_calls.load(Ordering::SeqCst), 1);
    assert_eq!(transaction_facts.lock().unwrap().len(), 2);
}

#[test]
fn retry_and_idle_do_not_replay_consumed_facts() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    flush_initial_state(&mut runtime);
    let calls = Arc::new(AtomicUsize::new(0));
    let factory_calls = calls.clone();
    runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::FigureBoundsChanged,
                move |_: AnimationBehaviorContext<'_>| {
                    factory_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(None)
                },
            )
            .scoped_to(figure),
        )
        .unwrap();
    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0))
        .unwrap();

    let submission = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(runtime.complete_submission(
        submission.session_id,
        submission.frame_id,
        RenderOutcome::Retry
    ));
    let retried = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(runtime.complete_submission(
        retried.session_id,
        retried.frame_id,
        RenderOutcome::Presented
    ));
    let _ = runtime.prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn disabled_skips_factories_and_reduced_motion_uses_only_its_fallback() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    flush_initial_state(&mut runtime);
    runtime.advance_time(time(0)).unwrap();
    let channel = runtime
        .animations()
        .create_channel(Opacity::OPAQUE)
        .unwrap();
    let enabled_calls = Arc::new(AtomicUsize::new(0));
    let reduced_calls = Arc::new(AtomicUsize::new(0));
    let primary_calls = enabled_calls.clone();
    let fallback_calls = reduced_calls.clone();
    runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::state_changed(property::ENABLED),
                move |_: AnimationBehaviorContext<'_>| {
                    primary_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(None)
                },
            )
            .scoped_to(figure)
            .with_reduced_motion(move |_: AnimationBehaviorContext<'_>| {
                fallback_calls.fetch_add(1, Ordering::SeqCst);
                Ok(Some(AnimationPlan::track(
                    channel,
                    Motion::Tween(
                        Tween::between(
                            Opacity::TRANSPARENT,
                            Opacity::OPAQUE,
                            Duration::from_millis(20),
                        )
                        .unwrap(),
                    ),
                )?))
            }),
        )
        .unwrap();

    runtime.animations().set_mode(AnimationMode::Disabled);
    runtime.figure(figure).unwrap().set_enabled(false).unwrap();
    runtime.record_full_frame();
    assert_eq!(enabled_calls.load(Ordering::SeqCst), 0);
    assert_eq!(reduced_calls.load(Ordering::SeqCst), 0);

    runtime.animations().set_mode(AnimationMode::ReducedMotion);
    runtime.figure(figure).unwrap().set_enabled(true).unwrap();
    runtime.record_full_frame();
    assert_eq!(enabled_calls.load(Ordering::SeqCst), 0);
    assert_eq!(reduced_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        runtime.animations().value(channel).unwrap(),
        Opacity::TRANSPARENT
    );
    assert_eq!(runtime.animations().active_animation_count(), 1);
}

#[test]
fn behavior_identity_scope_disposal_and_failure_are_bounded() {
    let mut tree = FigureTree::new();
    let parent = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .expect("valid FigureTree construction");
    let child = tree
        .builder()
        .add_child(parent, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    let mut runtime = Runtime::new(tree);
    flush_initial_state(&mut runtime);
    let calls = Arc::new(AtomicUsize::new(0));
    let factory_calls = calls.clone();
    let id = runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::FigureBoundsChanged,
                move |_: AnimationBehaviorContext<'_>| {
                    factory_calls.fetch_add(1, Ordering::SeqCst);
                    Err(AnimationError::InvalidValue)
                },
            )
            .scoped_to(child),
        )
        .unwrap();

    runtime
        .figure(parent)
        .unwrap()
        .set_bounds(Rectangle::new(10.0, 0.0, 100.0, 100.0))
        .unwrap();
    runtime.record_full_frame();
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    runtime
        .figure(child)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0))
        .unwrap();
    runtime.record_full_frame();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let failures = runtime.animations().take_behavior_failures();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].behavior(), id);
    assert_eq!(failures[0].error(), &AnimationError::InvalidValue);

    runtime.dispose_subtree(child).unwrap();
    assert_eq!(runtime.animations().behavior_count(), 0);
    assert_eq!(
        runtime.animations().remove_behavior(id),
        Err(AnimationError::UnknownBehavior)
    );
}

#[test]
fn behavior_factory_panic_faults_runtime_and_clears_presentation() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    flush_initial_state(&mut runtime);
    runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::FigureBoundsChanged,
                move |_: AnimationBehaviorContext<'_>| -> Result<Option<AnimationPlan>, AnimationError> {
                    panic!("behavior factory panic")
                },
            )
            .scoped_to(figure),
        )
        .unwrap();
    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0))
        .unwrap();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.record_full_frame();
    }));
    assert!(panic.is_err());
    assert!(runtime.is_faulted());
    assert_eq!(runtime.animations().active_animation_count(), 0);
}
