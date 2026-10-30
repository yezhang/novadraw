use novadraw::{
    animation::{
        AnimationBehavior, AnimationBehaviorContext, AnimationError, AnimationMode, AnimationPlan,
        AnimationStart, AnimationState, AnimationTrigger, InteractionGeometryPolicy, Motion, Tween,
    },
    figure::RectangleFigure,
    geometry::Rectangle,
    graphics::Opacity,
    render::{BackendCapabilities, SurfaceInfo},
    runtime::{FramePreparation, FramePreparationError, Runtime},
};
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[test]
fn healthy_same_mode_has_no_revision_or_frame_work() {
    let mut runtime = Runtime::empty();
    runtime.frames().stabilize_for_query().unwrap();
    let before = runtime.animation().timeline().sample_version();
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .set_mode(AnimationMode::Enabled),
        Ok(false)
    );
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .set_mode(AnimationMode::Disabled),
        Ok(true)
    );
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .set_mode(AnimationMode::Disabled),
        Ok(false)
    );
    assert_eq!(runtime.animation().timeline().sample_version(), before);
    assert_eq!(runtime.animation().timeline().active_animation_count(), 0);
    assert_eq!(runtime.animation().timeline().next_wake_deadline(), None);
}

fn fixture() -> (Runtime, novadraw::tree::FigureId) {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap(),
        ))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    (runtime, figure)
}

#[test]
fn mixed_behavior_success_and_rejection_preserve_prefix_and_consume_facts_once() {
    let (mut runtime, figure) = fixture();
    let channel = runtime
        .animation()
        .timeline()
        .bind_figure(figure, InteractionGeometryPolicy::Committed)
        .unwrap()
        .opacity();
    let calls = Arc::new(AtomicUsize::new(0));
    let success_calls = calls.clone();
    runtime
        .animation()
        .timeline()
        .install_behavior(AnimationBehavior::new(
            AnimationTrigger::FigureBoundsChanged,
            move |_: AnimationBehaviorContext<'_>| {
                success_calls.fetch_add(1, Ordering::Relaxed);
                Ok(Some(AnimationPlan::track(
                    channel,
                    Motion::Tween(Tween::between(
                        Opacity::TRANSPARENT,
                        Opacity::OPAQUE,
                        Duration::from_millis(100),
                    )?),
                )?))
            },
        ))
        .unwrap();
    let failure_calls = calls.clone();
    let rejected = runtime
        .animation()
        .timeline()
        .install_behavior(AnimationBehavior::new(
            AnimationTrigger::FigureBoundsChanged,
            move |_: AnimationBehaviorContext<'_>| {
                failure_calls.fetch_add(1, Ordering::Relaxed);
                Err(AnimationError::InvalidValue)
            },
        ))
        .unwrap();
    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert!(!runtime.is_faulted());
    assert_eq!(runtime.tree().figure_bounds(figure).unwrap().x, 20.0);
    assert_eq!(runtime.animation().timeline().active_animation_count(), 1);
    assert_eq!(
        runtime.animation().timeline().value(channel).unwrap(),
        Opacity::TRANSPARENT
    );
    let failures = runtime.animation().timeline().take_behavior_failures();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].behavior(), rejected);
    assert_eq!(failures[0].error(), &AnimationError::InvalidValue);
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert!(
        runtime
            .animation()
            .timeline()
            .take_behavior_failures()
            .is_empty()
    );
}

#[test]
fn factory_panic_faults_owner_and_same_mode_still_returns_error() {
    let (mut runtime, figure) = fixture();
    let channel = runtime.animation().timeline().create_channel(1.0).unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(Tween::between(0.0, 1.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    let AnimationStart::Running(id) = runtime.animation().timeline().start(plan).unwrap() else {
        panic!("animation must run");
    };
    runtime
        .animation()
        .timeline()
        .install_behavior(AnimationBehavior::new(
            AnimationTrigger::FigureBoundsChanged,
            |_: AnimationBehaviorContext<'_>| -> Result<Option<AnimationPlan>, AnimationError> {
                panic!("s09 consumer factory panic")
            },
        ))
        .unwrap();
    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0))
        .unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.frames().stabilize_for_query()
    }));
    assert!(panic.is_err());
    assert!(runtime.is_faulted());
    assert_eq!(runtime.tree().figure_bounds(figure).unwrap().x, 20.0);
    assert_eq!(runtime.animation().timeline().active_animation_count(), 0);
    assert_eq!(
        runtime.animation().timeline().state(id).unwrap(),
        AnimationState::Cancelled
    );
    assert_eq!(
        runtime.animation().timeline().mode(),
        AnimationMode::Enabled
    );
    for mode in [AnimationMode::Enabled, AnimationMode::Disabled] {
        assert_eq!(
            runtime.animation().timeline().set_mode(mode),
            Err(AnimationError::RuntimeFaulted)
        );
    }
    assert_eq!(
        runtime.frames().stabilize_for_query(),
        Err(FramePreparationError::Faulted)
    );
    let surface = SurfaceInfo {
        logical_width: 0.0,
        logical_height: 0.0,
        pixel_width: 0,
        pixel_height: 0,
        scale_factor: 1.0,
    };
    assert!(matches!(
        runtime
            .frames()
            .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL),
        FramePreparation::Error(FramePreparationError::Faulted)
    ));
}

#[test]
fn frame_animation_error_preserves_domain_source() {
    let error = FramePreparationError::Animation(AnimationError::RuntimeFaulted);
    assert_eq!(
        error.source().unwrap().downcast_ref::<AnimationError>(),
        Some(&AnimationError::RuntimeFaulted)
    );
}
