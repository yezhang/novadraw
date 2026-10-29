use novadraw::{
    figure::{Figure, FigureStyle, ImageFigure, LabelFigure, RectangleFigure},
    geometry::{Affine2D, Rectangle},
    graphics::{Color, GraphicsError, Paint, PaintContext},
    render::{
        BackendCapabilities, BuiltinFont, ImageData, RenderCommandKind, RenderDisposition,
        SurfaceInfo,
    },
    runtime::{FramePreparation, Runtime, SubmissionAcknowledgement},
    scene::{
        SOURCE_LOCAL_CAPTURE, SceneCaptureInput, SceneCapturePolicy, SceneCapturePolicyKey,
        SceneCloseReason, SceneError, SceneEvent, SourceLocalCapture,
    },
    tree::FigureId,
};

fn fixture() -> (Runtime, FigureId, FigureId, FigureId) {
    let mut runtime = Runtime::empty();
    let ancestor = runtime
        .set_contents(Box::new(
            RectangleFigure::new(10.0, 20.0, 800.0, 600.0).unwrap(),
        ))
        .unwrap();
    runtime
        .figure(ancestor)
        .unwrap()
        .set_style(FigureStyle {
            foreground: Some(Color::RED),
            background: Some(Color::WHITE),
            alpha: Some(novadraw::graphics::Opacity::try_new(0.5).unwrap()),
            font: Some(novadraw::text::FontDescriptor::parse("20px Inter Variable").unwrap()),
            ..Default::default()
        })
        .unwrap();
    let source = runtime
        .figure(ancestor)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(120.0, 90.0, 200.0, 100.0).unwrap(),
        ))
        .unwrap();
    let child = runtime
        .figure(source)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(20.0, 10.0, 40.0, 30.0).unwrap(),
        ))
        .unwrap();
    runtime
        .figure(child)
        .unwrap()
        .set_style(FigureStyle {
            alpha: Some(novadraw::graphics::Opacity::try_new(0.8).unwrap()),
            ..Default::default()
        })
        .unwrap();
    (runtime, ancestor, source, child)
}

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 800.0,
        logical_height: 600.0,
        pixel_width: 1600,
        pixel_height: 1200,
        scale_factor: 2.0,
    }
}

#[test]
fn capture_freezes_root_local_domain_and_nearest_inherited_alpha() {
    let (mut runtime, _, root, _) = fixture();
    let source = runtime.scenes().register_source(root).unwrap();
    let rect = Rectangle::new(5.0, 7.0, 150.0, 70.0);
    let (subscription, config, handoff) = runtime
        .scenes()
        .subscribe(
            source,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture {
                rectangle: Some(rect),
            },
        )
        .unwrap();
    assert!(runtime.scenes().needs_capture(subscription).unwrap());
    let snapshot = runtime.scenes().capture(subscription).unwrap();
    assert!(!runtime.scenes().needs_capture(subscription).unwrap());
    assert_eq!(snapshot.source_rectangle(), rect);
    assert_eq!(snapshot.config_revision(), config);
    assert_eq!(snapshot.resolved_style().alpha.get(), 0.5);
    assert_eq!(
        snapshot.resolved_style().font,
        novadraw::text::FontDescriptor::parse("20px Inter Variable").unwrap()
    );
    let commands = snapshot.drawing().commands();
    assert!(commands.iter().any(|c| matches!(c.kind, RenderCommandKind::Clip { rect: r } if r == Rectangle::new(0.0, 0.0, 150.0, 70.0))));
    let paints: Vec<_> = commands
        .iter()
        .flat_map(|c| c.kind.paints())
        .filter_map(|paint| match paint {
            Paint::Solid(color) => Some(*color),
            _ => None,
        })
        .collect();
    assert!(paints.iter().any(|c| (c.alpha() - 0.5).abs() < 1e-6));
    assert!(paints.iter().any(|c| (c.alpha() - 0.8).abs() < 1e-6));
    assert!(!paints.iter().any(|c| (c.alpha() - 0.4).abs() < 1e-6));
    assert!(matches!(
        handoff.take().unwrap(),
        Some(SceneEvent::Ready(_))
    ));
    assert!(handoff.take().unwrap().is_none());
}

#[test]
fn config_only_capture_keeps_source_epoch_and_replaces_pending_candidate() {
    let (mut runtime, ancestor, root, _) = fixture();
    let source = runtime.scenes().register_source(root).unwrap();
    let (subscription, _, handoff) = runtime
        .scenes()
        .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
        .unwrap();
    let first = runtime.scenes().capture(subscription).unwrap();
    let config = runtime
        .scenes()
        .configure(
            subscription,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture {
                rectangle: Some(Rectangle::new(3.0, 4.0, 10.0, 15.0)),
            },
        )
        .unwrap();
    assert!(handoff.take().unwrap().is_none());
    let second = runtime.scenes().capture(subscription).unwrap();
    assert_eq!(first.stable_source_epoch(), second.stable_source_epoch());
    assert_eq!(second.config_revision(), config);
    assert_ne!(first.config_revision(), config);
    runtime
        .figure(ancestor)
        .unwrap()
        .set_style(FigureStyle {
            alpha: Some(novadraw::graphics::Opacity::try_new(0.25).unwrap()),
            ..Default::default()
        })
        .unwrap();
    assert!(runtime.scenes().needs_capture(subscription).unwrap());
    let third = runtime.scenes().capture(subscription).unwrap();
    assert!(third.stable_source_epoch() > second.stable_source_epoch());
    assert_eq!(third.resolved_style().alpha.get(), 0.25);
    let Some(SceneEvent::Ready(latest)) = handoff.take().unwrap() else {
        panic!("latest complete candidate");
    };
    assert_eq!(latest, third);
    assert!(handoff.take().unwrap().is_none());
}

struct HalfWidth;
impl SceneCapturePolicy<()> for HalfWidth {
    fn source_rectangle(
        &self,
        _: &(),
        input: SceneCaptureInput<'_>,
    ) -> Result<Rectangle, SceneError> {
        Ok(Rectangle::new(
            0.0,
            0.0,
            input.border_box.width / 2.0,
            input.border_box.height,
        ))
    }
}

#[test]
fn external_policy_and_failed_subscription_share_checked_path_without_blocking_others() {
    let (mut runtime, _, root, _) = fixture();
    let key = SceneCapturePolicyKey::new("external", "half-width");
    runtime
        .scenes()
        .register_capture_policy(key, HalfWidth)
        .unwrap();
    assert_eq!(
        runtime.scenes().register_capture_policy(key, HalfWidth),
        Err(SceneError::DuplicatePolicy)
    );
    let source = runtime.scenes().register_source(root).unwrap();
    let (good, _, _) = runtime.scenes().subscribe(source, key, ()).unwrap();
    let (bad, _, errors) = runtime
        .scenes()
        .subscribe(
            source,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture {
                rectangle: Some(Rectangle::new(0.0, 0.0, f64::NAN, 1.0)),
            },
        )
        .unwrap();
    assert_eq!(
        runtime.scenes().capture(bad),
        Err(SceneError::InvalidCaptureRectangle)
    );
    assert!(!runtime.scenes().needs_capture(bad).unwrap());
    assert!(matches!(
        errors.take().unwrap(),
        Some(SceneEvent::Error(SceneError::InvalidCaptureRectangle))
    ));
    assert_eq!(
        runtime
            .scenes()
            .capture(good)
            .unwrap()
            .source_rectangle()
            .width,
        100.0
    );
    assert!(matches!(
        runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL),
        FramePreparation::Ready(_)
    ));
    assert!(!runtime.is_faulted());
    runtime
        .scenes()
        .configure(bad, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
        .unwrap();
    assert!(runtime.scenes().needs_capture(bad).unwrap());
    runtime.scenes().capture(bad).unwrap();
}

#[test]
fn terminal_handoff_survives_disposal_and_is_not_overwritten_by_other_subscribers() {
    let (mut runtime, _, root, _) = fixture();
    let source = runtime.scenes().register_source(root).unwrap();
    let (a, _, mailbox_a) = runtime
        .scenes()
        .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
        .unwrap();
    let (b, _, mailbox_b) = runtime
        .scenes()
        .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
        .unwrap();
    runtime.scenes().capture(a).unwrap();
    runtime.scenes().capture(b).unwrap();
    runtime.scenes().cancel(a).unwrap();
    assert_eq!(
        runtime.scenes().capture(a),
        Err(SceneError::UnknownSubscription)
    );
    let Some(SceneEvent::Closed(closed)) = mailbox_a.take().unwrap() else {
        panic!("closed wins over Ready");
    };
    assert_eq!(closed.reason, SceneCloseReason::Cancelled);
    assert_eq!(closed.subscription, a);
    assert!(mailbox_a.take().unwrap().is_none());
    runtime.dispose_subtree(root).unwrap();
    drop(runtime);
    let Some(SceneEvent::Closed(closed)) = mailbox_b.take().unwrap() else {
        panic!("disposal delivers without repaint");
    };
    assert_eq!(closed.reason, SceneCloseReason::SourceDisposed);
    assert!(mailbox_b.take().unwrap().is_none());
}

#[test]
fn dropping_producer_without_another_frame_delivers_closed() {
    let (mut runtime, _, root, _) = fixture();
    let source = runtime.scenes().register_source(root).unwrap();
    let (_, _, handoff) = runtime
        .scenes()
        .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
        .unwrap();
    drop(runtime);
    let Some(SceneEvent::Closed(closed)) = handoff.take().unwrap() else {
        panic!("owned terminal event");
    };
    assert_eq!(closed.reason, SceneCloseReason::ProducerDisposed);
}

#[test]
fn capture_does_not_consume_source_inflight_or_resource_pending_prefix() {
    let (mut runtime, _, root, child) = fixture();
    let image = runtime.resources().register_image().unwrap();
    runtime
        .resources()
        .complete_image(image, ImageData::from_rgba(1, 1, vec![255, 0, 0, 255], 1.0))
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            ImageFigure::new(image)
                .with_bounds(Rectangle::new(0.0, 0.0, 10.0, 10.0))
                .unwrap(),
        ))
        .unwrap();
    let source = runtime.scenes().register_source(root).unwrap();
    let (subscription, _, _) = runtime
        .scenes()
        .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
        .unwrap();
    let frozen = runtime
        .frames()
        .prepare_submission(
            surface(),
            BackendCapabilities::RETAINED_PARTIAL.with_image_resources(),
        )
        .into_ready()
        .unwrap();
    runtime
        .figure(child)
        .unwrap()
        .set_bounds(Rectangle::new(30.0, 20.0, 40.0, 30.0))
        .unwrap();
    runtime
        .resources()
        .complete_image(image, ImageData::from_rgba(1, 1, vec![0, 255, 0, 255], 1.0))
        .unwrap();
    let snapshot = runtime.scenes().capture(subscription).unwrap();
    assert_eq!(snapshot.drawing().resources().ready.len(), 1);
    assert_eq!(snapshot.drawing().resources().ready[0].revision, 2);
    assert!(matches!(
        runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL),
        FramePreparation::AwaitingCompletion
    ));
    assert_eq!(
        runtime
            .frames()
            .complete_submission(
                frozen.session_id,
                frozen.frame_id,
                Ok(RenderDisposition::Presented)
            )
            .unwrap(),
        SubmissionAcknowledgement::Presented
    );
    let current = runtime
        .frames()
        .prepare_submission(
            surface(),
            BackendCapabilities::RETAINED_PARTIAL.with_image_resources(),
        )
        .into_ready()
        .unwrap();
    assert!(current.commands.iter().any(|c| matches!(c.kind, RenderCommandKind::Image { image: reference, .. } if reference.revision() == 2)));
    assert_eq!(
        runtime
            .frames()
            .complete_submission(
                current.session_id,
                current.frame_id,
                Ok(RenderDisposition::Retry)
            )
            .unwrap(),
        SubmissionAcknowledgement::Retry
    );
    assert!(runtime.scenes().capture(subscription).is_ok());
}

#[test]
fn font_and_image_closure_is_exact_and_old_payload_is_owned() {
    let (mut runtime, _, root, _) = fixture();
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let unused = runtime.resources().register_image().unwrap();
    runtime
        .resources()
        .complete_image(unused, ImageData::from_rgba(1, 1, vec![0, 0, 0, 255], 1.0))
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(LabelFigure::new("immutable text")))
        .unwrap();
    let source = runtime.scenes().register_source(root).unwrap();
    let (subscription, _, _) = runtime
        .scenes()
        .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
        .unwrap();
    let snapshot = runtime.scenes().capture(subscription).unwrap();
    assert!(
        snapshot
            .drawing()
            .commands()
            .iter()
            .any(|c| matches!(c.kind, RenderCommandKind::DrawGlyphRun { .. }))
    );
    assert!(
        !snapshot
            .drawing()
            .resources()
            .ready
            .iter()
            .any(|r| r.id == unused.resource_id())
    );
    drop(runtime);
    assert!(!snapshot.drawing().resources().ready.is_empty());
}

struct ReplaceTransform;
impl Figure for ReplaceTransform {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(50.0, 60.0, 20.0, 20.0)
    }
    fn name(&self) -> &'static str {
        "ExternalTransformReplacement"
    }
    fn paint(&self, gc: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
        gc.set_transform(Affine2D::IDENTITY)?;
        gc.fill_rect(Rectangle::new(0.0, 0.0, 20.0, 20.0))?;
        Ok(())
    }
}

#[test]
fn transform_replacement_cannot_escape_capture_origin_and_zero_rect_is_valid() {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(ReplaceTransform)).unwrap();
    let source = runtime.scenes().register_source(root).unwrap();
    let (subscription, _, _) = runtime
        .scenes()
        .subscribe(
            source,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture {
                rectangle: Some(Rectangle::new(5.0, 7.0, 10.0, 10.0)),
            },
        )
        .unwrap();
    let snapshot = runtime.scenes().capture(subscription).unwrap();
    assert!(snapshot.drawing().commands().iter().any(|c| matches!(c.kind, RenderCommandKind::SetTransform { matrix } if matrix == Affine2D::from_translation(-5.0, -7.0))));
    runtime
        .scenes()
        .configure(
            subscription,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture {
                rectangle: Some(Rectangle::new(5.0, 7.0, 0.0, 0.0)),
            },
        )
        .unwrap();
    let empty = runtime.scenes().capture(subscription).unwrap();
    assert!(empty.drawing().commands().is_empty());
    assert!(empty.drawing().resources().is_empty());
}

#[test]
fn paused_surface_and_source_placement_do_not_clip_capture() {
    let (mut runtime, ancestor, root, _) = fixture();
    let source = runtime.scenes().register_source(root).unwrap();
    let (subscription, _, _) = runtime
        .scenes()
        .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
        .unwrap();
    let before = runtime.scenes().capture(subscription).unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_bounds(Rectangle::new(900.0, 700.0, 200.0, 100.0))
        .unwrap();
    runtime
        .figure(ancestor)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, 10.0, 10.0))
        .unwrap();
    let mut zero = surface();
    zero.logical_width = 0.0;
    zero.logical_height = 0.0;
    zero.pixel_width = 0;
    zero.pixel_height = 0;
    assert!(matches!(
        runtime
            .frames()
            .prepare_submission(zero, BackendCapabilities::RETAINED_PARTIAL),
        FramePreparation::Suspended
    ));
    let after = runtime.scenes().capture(subscription).unwrap();
    assert_eq!(before.source_rectangle(), after.source_rectangle());
    // External placement is cancelled before the recursive subtree paints.
    let mut first_transform = Affine2D::IDENTITY;
    for command in after.drawing().commands() {
        match command.kind {
            RenderCommandKind::ConcatTransform { matrix } => first_transform *= matrix,
            RenderCommandKind::FillRect { .. } => break,
            _ => {}
        }
    }
    assert_eq!(first_transform, Affine2D::IDENTITY);
}

struct RejectPaint;
impl Figure for RejectPaint {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 200.0, 100.0)
    }
    fn name(&self) -> &'static str {
        "ExternalRejectPaint"
    }
    fn paint(&self, context: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
        context.set_stroke_width(-1.0)
    }
}

#[test]
fn recording_error_isolated_from_other_capture_but_main_frame_still_rejects() {
    let mut runtime = Runtime::empty();
    let parent = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 400.0, 300.0).unwrap(),
        ))
        .unwrap();
    let bad_figure = runtime
        .figure(parent)
        .unwrap()
        .add(Box::new(RejectPaint))
        .unwrap();
    let child = runtime
        .figure(parent)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(0.0, 0.0, 40.0, 30.0).unwrap(),
        ))
        .unwrap();
    let bad_source = runtime.scenes().register_source(bad_figure).unwrap();
    let good_source = runtime.scenes().register_source(child).unwrap();
    let (bad, _, _) = runtime
        .scenes()
        .subscribe(
            bad_source,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture::default(),
        )
        .unwrap();
    let (good, _, _) = runtime
        .scenes()
        .subscribe(
            good_source,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture::default(),
        )
        .unwrap();
    assert_eq!(
        runtime.scenes().capture(bad),
        Err(SceneError::Graphics(GraphicsError::Input(
            novadraw::graphics::GraphicsInputError::NegativeStrokeWidth
        )))
    );
    runtime.scenes().capture(good).unwrap();
    assert!(matches!(
        runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL),
        FramePreparation::Error(_)
    ));
    assert!(!runtime.is_faulted());
}

#[test]
fn initial_and_terminal_requests_wake_host_outside_mailbox_lock() {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Wake, Waker},
    };
    struct Counter(AtomicUsize);
    impl Wake for Counter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (mut runtime, _, root, _) = fixture();
    let source = runtime.scenes().register_source(root).unwrap();
    let (subscription, _, handoff) = runtime
        .scenes()
        .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
        .unwrap();
    let counter = Arc::new(Counter(AtomicUsize::new(0)));
    handoff.set_waker(Waker::from(counter.clone())).unwrap();
    assert_eq!(counter.0.load(Ordering::SeqCst), 1);
    runtime.scenes().capture(subscription).unwrap();
    assert_eq!(counter.0.load(Ordering::SeqCst), 2);
    runtime.scenes().remove_source(source).unwrap();
    assert_eq!(counter.0.load(Ordering::SeqCst), 3);
    assert!(matches!(
        handoff.take().unwrap(),
        Some(SceneEvent::Closed(_))
    ));
}

#[test]
fn identities_are_producer_owned_and_not_reused_after_removal() {
    let (mut runtime, _, root, _) = fixture();
    let (mut foreign, _, other, _) = fixture();
    let source = runtime.scenes().register_source(root).unwrap();
    assert!(foreign.scenes().register_source(root).is_err());
    assert!(runtime.scenes().register_source(other).is_err());
    assert!(matches!(
        foreign
            .scenes()
            .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default()),
        Err(SceneError::UnknownSource)
    ));
    runtime.scenes().remove_source(source).unwrap();
    assert_ne!(runtime.scenes().register_source(root).unwrap(), source);
}

#[test]
fn surface_frame_phase_notifications_do_not_request_another_source_capture() {
    let (mut runtime, _, root, _) = fixture();
    let source = runtime.scenes().register_source(root).unwrap();
    let (subscription, _, _) = runtime
        .scenes()
        .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
        .unwrap();
    runtime.scenes().capture(subscription).unwrap();
    let submission = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_eq!(
        runtime
            .frames()
            .complete_submission(
                submission.session_id,
                submission.frame_id,
                Ok(RenderDisposition::Presented)
            )
            .unwrap(),
        SubmissionAcknowledgement::Presented
    );
    assert!(!runtime.scenes().needs_capture(subscription).unwrap());
}
