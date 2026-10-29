mod support;

use std::time::Duration;

use novadraw::{
    animation::{
        AnimationError, AnimationPlan, AnimationStart, AnimationValue, BoundsTransition,
        GeometryBinding, GeometryPatch, LayoutBounds, Motion, Tween,
    },
    event::MonotonicTime,
    figure::{
        CONTAINER, ChildClippingStrategy, ChildTransform, ContainerCapability, Figure,
        FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureContainer,
        RectangleFigure,
    },
    geometry::{Affine2D, Insets, Point, Rectangle},
    render::RenderCommandKind,
    runtime::Runtime,
};
use support::record_full_submission;

fn time(milliseconds: u64) -> MonotonicTime {
    MonotonicTime::from_micros(milliseconds * 1_000)
}

#[test]
fn external_bounds_paint_and_compound_border_use_frozen_inputs_and_sampled_size() {
    use novadraw::{
        figure::border::{CompoundBorder, LineBorder, MarginBorder},
        figure::{BOUNDS_SNAPSHOT, BoundsDrawing, BoundsSnapshotCapability, FigureBoundsSnapshot},
        graphics::{GraphicsError, PaintContext},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct ExternalBoundsFigure(Arc<AtomicUsize>);
    impl Figure for ExternalBoundsFigure {
        fn name(&self) -> &'static str {
            "ExternalBoundsFigure"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 80.0)
        }
        fn register_capabilities(
            &self,
            output: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            output.register(BOUNDS_SNAPSHOT, BoundsSnapshotCapability::of::<Self>())
        }
        fn paint(&self, _: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("animated recording must not call the mutable Figure")
        }
    }
    impl FigureBoundsSnapshot for ExternalBoundsFigure {
        fn snapshot_bounds_drawing(&self) -> Result<BoundsDrawing, GraphicsError> {
            Ok(BoundsDrawing::new(
                2.0,
                4096,
                |width, bounds, context| {
                    context.set_stroke_width(*width)?;
                    context.stroke_rect(bounds)
                },
                |width, bounds| {
                    Ok(Rectangle::new(
                        bounds.x - width / 2.0,
                        bounds.y - width / 2.0,
                        bounds.width + width,
                        bounds.height + width,
                    ))
                },
            ))
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 80.0)
                .expect("valid figure bounds")
                .with_border(
                    CompoundBorder::new(
                        MarginBorder::new(Insets::new(3.0, 4.0, 5.0, 6.0)).unwrap(),
                        LineBorder::new(novadraw::graphics::Color::BLACK, 2.0).unwrap(),
                    )
                    .unwrap(),
                )
                .expect("valid detached figure input"),
        ))
        .unwrap();
    let child = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ExternalBoundsFigure(calls.clone())))
        .unwrap();
    let root_channel = runtime
        .animation()
        .timeline()
        .bind_geometry(root, GeometryBinding::layout_bounds())
        .unwrap();
    let child_channel = runtime
        .animation()
        .timeline()
        .bind_geometry(child, GeometryBinding::layout_bounds())
        .unwrap();
    let track = |channel| {
        AnimationPlan::track(
            channel,
            Motion::Tween(
                Tween::between(
                    bounds(Rectangle::new(0.0, 0.0, 20.0, 20.0)),
                    bounds(Rectangle::new(0.0, 0.0, 100.0, 80.0)),
                    Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap()
    };
    let plan = AnimationPlan::parallel(vec![track(root_channel), track(child_channel)]).unwrap();
    runtime.animation().timeline().start(plan).unwrap();
    runtime.animation().advance_time(time(0)).unwrap();
    runtime.animation().advance_time(time(50)).unwrap();
    let submission = record_full_submission(&mut runtime);
    let rectangles: Vec<_> = submission
        .commands
        .iter()
        .filter_map(|command| {
            if let RenderCommandKind::StrokeRect { rect, stroke, .. } = &command.kind {
                Some((*rect, stroke.width()))
            } else {
                None
            }
        })
        .collect();
    assert!(rectangles.contains(&(Rectangle::new(0.0, 0.0, 60.0, 50.0), 2.0)));
    assert!(rectangles.contains(&(Rectangle::new(5.0, 4.0, 48.0, 40.0), 2.0)));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.tree().figure_bounds(child).unwrap().width, 100.0);
}

#[test]
fn bounded_self_paint_overrun_fails_its_owner_and_repairs_with_static_content() {
    use novadraw::{
        animation::{AnimationBudgetKind, AnimationState},
        figure::{BOUNDS_SNAPSHOT, BoundsDrawing, BoundsSnapshotCapability, FigureBoundsSnapshot},
        graphics::GraphicsError,
    };
    struct OversizedPaint;
    impl Figure for OversizedPaint {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 80.0)
        }
        fn name(&self) -> &'static str {
            "OversizedPaint"
        }
        fn register_capabilities(
            &self,
            output: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            output.register(BOUNDS_SNAPSHOT, BoundsSnapshotCapability::of::<Self>())
        }
    }
    impl FigureBoundsSnapshot for OversizedPaint {
        fn snapshot_bounds_drawing(&self) -> Result<BoundsDrawing, GraphicsError> {
            Ok(BoundsDrawing::new(
                (),
                0,
                |_, bounds, context| {
                    if bounds.width > 20.0 {
                        context.stroke_rect(bounds)?;
                    }
                    Ok(())
                },
                |_, bounds| Ok(bounds),
            ))
        }
    }
    let mut runtime = Runtime::empty();
    let host = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 400.0, 200.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let bad = runtime
        .figure(host)
        .unwrap()
        .add(Box::new(OversizedPaint))
        .unwrap();
    let good = runtime
        .figure(host)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(150.0, 0.0, 100.0, 80.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let mut owners = Vec::new();
    for figure in [bad, good] {
        let terminal = runtime.tree().figure_bounds(figure).unwrap();
        let channel = runtime
            .animation()
            .timeline()
            .bind_geometry(figure, GeometryBinding::layout_bounds())
            .unwrap();
        let plan = AnimationPlan::track(
            channel,
            Motion::Tween(
                Tween::between(
                    bounds(Rectangle::new(terminal.x, terminal.y, 20.0, 20.0)),
                    bounds(terminal),
                    Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let AnimationStart::Running(owner) = runtime.animation().timeline().start(plan).unwrap()
        else {
            panic!("expected owner")
        };
        owners.push(owner);
    }
    runtime.animation().advance_time(time(0)).unwrap();
    runtime.animation().advance_time(time(50)).unwrap();
    record_full_submission(&mut runtime);
    assert_eq!(
        runtime.animation().timeline().state(owners[0]),
        Ok(AnimationState::Failed)
    );
    assert!(matches!(
        runtime.animation().timeline().failure(owners[0]).unwrap(),
        Some(AnimationError::ProviderOutputExceeded {
            kind: AnimationBudgetKind::SnapshotBytes, declared: 0, figure, ..
        }) if *figure == bad
    ));
    assert_eq!(
        runtime.animation().timeline().state(owners[1]),
        Ok(AnimationState::Running)
    );
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .reservations()
            .active_animations,
        1
    );
    assert_eq!(
        runtime.animation().timeline().sample_version().time,
        Some(time(50))
    );
}

#[test]
fn captured_content_retains_exact_image_and_charges_payload_until_capture_drop() {
    use novadraw::{
        animation::AnimationState,
        figure::{
            CONTENT_SNAPSHOT, ContentSnapshotCapability, FigureContentSnapshot, FigureDrawing,
            FigureMeasurement, FigurePresentation,
        },
        geometry::Dimension,
        graphics::{GraphicsError, PaintContext},
        render::{ImageData, ImageResourceRef, ResourcePayload},
        text::TextError,
    };
    use std::sync::Arc;

    struct ImageContent(ImageResourceRef);
    impl FigureDrawing for ImageContent {
        fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
            context.draw_image(self.0, Rectangle::new(0.0, 0.0, 8.0, 8.0))
        }
    }
    impl Figure for ImageContent {
        fn name(&self) -> &'static str {
            "ExternalImageContent"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(20.0, 0.0, 10.0, 10.0)
        }
        fn register_capabilities(
            &self,
            output: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            output.register(CONTENT_SNAPSHOT, ContentSnapshotCapability::of::<Self>())
        }
    }
    impl FigureContentSnapshot for ImageContent {
        fn content_revision(&self) -> u64 {
            0
        }
        fn snapshot_content(&self, _: Rectangle) -> Result<FigurePresentation, TextError> {
            FigurePresentation::new(
                FigureMeasurement::new(8.0, 8.0, None),
                Dimension::new(8.0, 8.0),
                Rectangle::new(0.0, 0.0, 8.0, 8.0),
                Arc::new(Self(self.0)),
            )
        }
    }

    let mut runtime = Runtime::empty();
    let image = runtime.resources().register_image().unwrap();
    const PAYLOAD_BYTES: usize = 8 * 8 * 4;
    runtime
        .resources()
        .complete_image(
            image,
            ImageData::from_rgba(8, 8, vec![255; PAYLOAD_BYTES], 1.0),
        )
        .unwrap();
    let resource = runtime
        .resources()
        .registry()
        .snapshot_resource(image.resource_id())
        .unwrap();
    let ResourcePayload::Image(data) = &resource.payload else {
        panic!("expected image")
    };
    let retained = Arc::downgrade(data);
    let root = runtime
        .set_contents(Box::new(ImageContent(ImageResourceRef::new(
            image.resource_id(),
            resource.revision,
            8,
            8,
            1.0,
        ))))
        .unwrap();
    drop(resource);
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(root, GeometryBinding::layout_bounds())
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 10.0, 10.0)),
                bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let AnimationStart::Running(owner) = runtime.animation().timeline().start(plan).unwrap() else {
        panic!("expected animation")
    };
    runtime.animation().advance_time(time(0)).unwrap();
    let capture = runtime.animation().capture_geometry(vec![root]).unwrap();
    assert!(runtime.animation().timeline().reservations().capture_bytes >= PAYLOAD_BYTES);
    let submission = record_full_submission(&mut runtime);
    assert!(submission.retention.as_ref().unwrap().retained_bytes() >= PAYLOAD_BYTES);
    // No explicit add_dependency is required for a resource used by frozen content.
    runtime
        .resources()
        .complete_image(
            image,
            ImageData::from_rgba(8, 8, vec![0; PAYLOAD_BYTES], 1.0),
        )
        .unwrap();
    record_full_submission(&mut runtime);
    assert_eq!(
        runtime.animation().timeline().state(owner),
        Ok(AnimationState::Cancelled)
    );
    drop(runtime);
    assert!(
        retained.upgrade().is_some(),
        "capture owns the old exact image revision"
    );
    drop(capture);
    assert!(
        retained.upgrade().is_some(),
        "submission owns the old resource after capture and Runtime release"
    );
    let clone = submission.clone();
    drop(submission);
    assert!(retained.upgrade().is_some());
    drop(clone);
    assert!(
        retained.upgrade().is_none(),
        "last submission clone releases the old image"
    );
}

fn bounds(rectangle: Rectangle) -> LayoutBounds {
    LayoutBounds::try_new(rectangle).unwrap()
}

#[test]
fn static_modes_suppress_before_freezing_content_or_evaluating_geometry_providers() {
    use novadraw::{
        animation::{AnimationMode, AnimationSuppression, GeometryProvider, GeometryProviderKey},
        figure::{BOUNDS_SNAPSHOT, BoundsDrawing, BoundsSnapshotCapability, FigureBoundsSnapshot},
        graphics::GraphicsError,
    };
    struct StaticFigure;
    impl Figure for StaticFigure {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 80.0)
        }
        fn name(&self) -> &'static str {
            "StaticFigure"
        }
        fn register_capabilities(
            &self,
            output: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            output.register(BOUNDS_SNAPSHOT, BoundsSnapshotCapability::of::<Self>())
        }
    }
    impl FigureBoundsSnapshot for StaticFigure {
        fn snapshot_bounds_drawing(&self) -> Result<BoundsDrawing, GraphicsError> {
            panic!("static fallback must not freeze animated self-paint")
        }
    }
    for (mode, suppression) in [
        (AnimationMode::Disabled, AnimationSuppression::Disabled),
        (
            AnimationMode::ReducedMotion,
            AnimationSuppression::ReducedMotionStaticFallback,
        ),
    ] {
        let mut runtime = Runtime::empty();
        let root = runtime.set_contents(Box::new(StaticFigure)).unwrap();
        let channel = runtime
            .animation()
            .timeline()
            .bind_geometry(root, GeometryBinding::layout_bounds())
            .unwrap();
        runtime.animation().timeline().set_mode(mode);
        let plan = AnimationPlan::track(
            channel,
            Motion::Tween(
                Tween::between(
                    bounds(Rectangle::new(0.0, 0.0, 20.0, 10.0)),
                    bounds(Rectangle::new(0.0, 0.0, 100.0, 80.0)),
                    Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap()
        .with_geometry_provider(GeometryProvider::new(
            GeometryProviderKey::<f64>::new(root),
            [root],
            vec![],
            |_| panic!("static fallback must not evaluate geometry providers"),
            |_| Ok(GeometryPatch::default()),
        ));
        assert_eq!(
            runtime.animation().timeline().start(plan),
            Ok(AnimationStart::Suppressed(suppression))
        );
        assert_eq!(runtime.animation().timeline().active_animation_count(), 0);
        assert_eq!(
            runtime.animation().timeline().reservations().capture_bytes,
            0
        );
        assert_eq!(runtime.animation().timeline().stats().tracks_sampled, 0);
    }
}

#[test]
fn capture_rejects_combined_peak_before_materializing_bounded_content() {
    use novadraw::{
        animation::AnimationBudgetKind,
        figure::{BOUNDS_SNAPSHOT, BoundsDrawing, BoundsSnapshotCapability, FigureBoundsSnapshot},
        graphics::GraphicsError,
    };
    struct PeakDrawing;
    impl Figure for PeakDrawing {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 80.0)
        }
        fn name(&self) -> &'static str {
            "PeakDrawing"
        }
        fn register_capabilities(
            &self,
            output: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            output.register(BOUNDS_SNAPSHOT, BoundsSnapshotCapability::of::<Self>())
        }
    }
    impl FigureBoundsSnapshot for PeakDrawing {
        fn snapshot_bounds_drawing(&self) -> Result<BoundsDrawing, GraphicsError> {
            Ok(BoundsDrawing::new(
                (),
                8192,
                |_, _, _| panic!("insufficient combined capacity must reject before paint"),
                |_, bounds| Ok(bounds),
            ))
        }
    }
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(PeakDrawing)).unwrap();
    let mut limits = runtime.animation().timeline().limits();
    limits.capture_bytes = 4096;
    runtime.animation().timeline().set_limits(limits).unwrap();
    let version = runtime.animation().timeline().sample_version();
    assert!(matches!(
        runtime.animation().capture_geometry([root]),
        Err(novadraw::animation::GeometryCaptureError::Animation(
            AnimationError::BudgetExceeded(AnimationBudgetKind::SnapshotBytes)
        ))
    ));
    assert_eq!(runtime.animation().timeline().sample_version(), version);
    assert_eq!(
        runtime.animation().timeline().reservations().capture_bytes,
        0
    );
}

#[test]
fn title_bar_recording_keeps_glyphs_colors_and_font_leases_while_width_changes() {
    use novadraw::{
        figure::border::{CompoundBorder, MarginBorder, TitleBarBorder},
        graphics::Color,
        render::{BuiltinFont, ResourcePayload, ResourceSync},
    };
    let mut runtime = Runtime::empty();
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let foreground = Color::rgba(0.2, 0.3, 0.4, 1.0);
    let mut rectangle = RectangleFigure::new(0.0, 0.0, 300.0, 100.0)
        .expect("valid figure bounds")
        .with_stroke(foreground, 1.0)
        .expect("valid detached figure input")
        .with_border(
            CompoundBorder::new(
                MarginBorder::new(Insets::new(2.0, 3.0, 4.0, 5.0)).unwrap(),
                TitleBarBorder::new("Frozen title", Color::WHITE),
            )
            .unwrap(),
        )
        .expect("valid detached figure input");
    rectangle.fill_color = Color::WHITE;
    let root = runtime.set_contents(Box::new(rectangle)).unwrap();
    let baseline = record_full_submission(&mut runtime);
    let ResourceSync::Snapshot(resources) = &baseline.resources else {
        panic!("the first submission initializes the backend resource snapshot");
    };
    let font_bytes: usize = resources
        .ready
        .iter()
        .filter_map(|resource| match &resource.payload {
            ResourcePayload::Font(font) => Some(font.shared_bytes().capacity()),
            _ => None,
        })
        .sum();
    assert!(font_bytes > 0);
    let glyphs = |commands: &[novadraw::render::RenderCommand]| {
        commands
            .iter()
            .filter_map(|command| match &command.kind {
                RenderCommandKind::DrawGlyphRun { run, paint, .. } => {
                    Some((run.clone(), paint.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let expected = glyphs(&baseline.commands);
    assert!(!expected.is_empty());
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(root, GeometryBinding::layout_bounds())
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 80.0, 100.0)),
                bounds(Rectangle::new(0.0, 0.0, 300.0, 100.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    runtime.animation().timeline().start(plan).unwrap();
    for milliseconds in [0, 25, 50, 75] {
        runtime
            .animation()
            .advance_time(time(milliseconds))
            .unwrap();
        let capture = runtime.animation().capture_geometry([root]).unwrap();
        assert_eq!(
            capture.geometry().figure(root).unwrap().bounds.width,
            80.0 + 220.0 * milliseconds as f64 / 100.0
        );
        let submission = record_full_submission(&mut runtime);
        assert_eq!(glyphs(&submission.commands), expected);
        assert!(submission.retention.as_ref().unwrap().retained_bytes() >= font_bytes);
        assert_eq!(runtime.tree().figure_bounds(root).unwrap().width, 300.0);
    }
}

#[test]
fn sampled_label_bounds_keep_prepared_glyphs_and_do_not_shape_again() {
    use novadraw::{
        figure::LabelFigure,
        render::{
            BuiltinFont, FontDescriptor, ResourceId, TextConstraints, TextEngine, TextError,
            TextLayout, TextLayoutEngine,
        },
        tree::FigureTree,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct CountingEngine {
        inner: TextEngine,
        calls: Arc<AtomicUsize>,
    }
    impl TextLayoutEngine for CountingEngine {
        fn revision(&self) -> u64 {
            self.inner.revision()
        }
        fn register_font(
            &mut self,
            id: ResourceId,
            revision: u64,
            bytes: &[u8],
        ) -> Result<(), TextError> {
            self.inner.register_font(id, revision, bytes)
        }
        fn remove_font(&mut self, id: ResourceId) {
            self.inner.remove_font(id);
        }
        fn layout(
            &mut self,
            text: &str,
            font: &FontDescriptor,
            constraints: TextConstraints,
        ) -> Result<TextLayout, TextError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.layout(text, font, constraints)
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let mut runtime = Runtime::with_text_layout_engine(
        FigureTree::new(),
        Box::new(CountingEngine {
            inner: TextEngine::new(),
            calls: calls.clone(),
        }),
    );
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 320.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let label = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            LabelFigure::new("Tree title")
                .with_bounds(Rectangle::new(20.0, 20.0, 120.0, 30.0))
                .expect("valid detached figure input"),
        ))
        .unwrap();
    let baseline = record_full_submission(&mut runtime);
    let runs = |commands: &[novadraw::render::RenderCommand]| {
        commands
            .iter()
            .filter_map(|command| match &command.kind {
                RenderCommandKind::DrawGlyphRun { run, .. } => Some(run.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let expected = runs(&baseline.commands);
    assert!(!expected.is_empty());
    let shaping_calls = calls.load(Ordering::SeqCst);
    assert!(shaping_calls > 0);
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(label, GeometryBinding::layout_bounds())
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 20.0, 10.0, 30.0)),
                bounds(Rectangle::new(20.0, 20.0, 120.0, 30.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    runtime.animation().timeline().start(plan).unwrap();
    for milliseconds in [0, 25, 50, 75] {
        runtime
            .animation()
            .advance_time(time(milliseconds))
            .unwrap();
        let submission = record_full_submission(&mut runtime);
        assert_eq!(runs(&submission.commands), expected);
        assert_eq!(calls.load(Ordering::SeqCst), shaping_calls);
        assert_eq!(
            runtime.tree().figure_bounds(label).unwrap(),
            Rectangle::new(20.0, 20.0, 120.0, 30.0)
        );
    }
}

#[test]
fn external_content_snapshot_is_frozen_once_and_revision_cancels_the_group() {
    use novadraw::{
        animation::AnimationState,
        figure::{
            CONTENT_SNAPSHOT, ContentSnapshotCapability, FigureContentSnapshot, FigureDrawing,
            FigureMeasurement, FigurePresentation,
        },
        geometry::Dimension,
        graphics::{GraphicsError, PaintContext},
        text::TextError,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    };

    struct Drawing(Arc<AtomicUsize>);
    impl FigureDrawing for Drawing {
        fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
            context.stroke_line(
                Point::new(0.0, 0.0),
                Point::new(self.0.load(Ordering::SeqCst) as f64, 0.0),
            )
        }
    }
    struct Consumer {
        length: Arc<AtomicUsize>,
        revision: Arc<AtomicU64>,
        snapshots: Arc<AtomicUsize>,
    }
    impl Figure for Consumer {
        fn name(&self) -> &'static str {
            "SnapshotConsumer"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(20.0, 0.0, 10.0, 10.0)
        }
        fn paint(&self, context: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
            Drawing(self.length.clone()).paint(context)
        }
        fn register_capabilities(
            &self,
            output: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            output.register(CONTENT_SNAPSHOT, ContentSnapshotCapability::of::<Self>())
        }
    }
    impl FigureContentSnapshot for Consumer {
        fn content_revision(&self) -> u64 {
            self.revision.load(Ordering::SeqCst)
        }
        fn snapshot_content(&self, _: Rectangle) -> Result<FigurePresentation, TextError> {
            self.snapshots.fetch_add(1, Ordering::SeqCst);
            let length = self.length.load(Ordering::SeqCst) as f64;
            FigurePresentation::new(
                FigureMeasurement::new(length, 1.0, None),
                Dimension::new(0.0, 0.0),
                Rectangle::new(0.0, 0.0, length, 1.0),
                Arc::new(Drawing(self.length.clone())),
            )
        }
    }

    let length = Arc::new(AtomicUsize::new(5));
    let revision = Arc::new(AtomicU64::new(0));
    let snapshots = Arc::new(AtomicUsize::new(0));
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(Consumer {
            length: length.clone(),
            revision: revision.clone(),
            snapshots: snapshots.clone(),
        }))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(root, GeometryBinding::layout_bounds())
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 10.0, 10.0)),
                bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let AnimationStart::Running(owner) = runtime.animation().timeline().start(plan).unwrap() else {
        panic!("expected running geometry group")
    };
    assert_eq!(snapshots.load(Ordering::SeqCst), 1);
    // The recorded drawing cannot observe mutable data hidden inside an opaque callback.
    length.store(9, Ordering::SeqCst);
    for milliseconds in [0, 25, 50] {
        runtime
            .animation()
            .advance_time(time(milliseconds))
            .unwrap();
        let submission = record_full_submission(&mut runtime);
        assert!(submission.commands.iter().any(|command| {
            matches!(command.kind, RenderCommandKind::Line { p2, .. } if p2.x() == 5.0)
        }));
    }
    assert_eq!(snapshots.load(Ordering::SeqCst), 1);
    revision.store(1, Ordering::SeqCst);
    runtime.animation().advance_time(time(75)).unwrap();
    let submission = record_full_submission(&mut runtime);
    assert_eq!(
        runtime.animation().timeline().state(owner),
        Ok(AnimationState::Cancelled)
    );
    assert!(submission.commands.iter().any(|command| {
        matches!(command.kind, RenderCommandKind::Line { p2, .. } if p2.x() == 9.0)
    }));
    assert_eq!(snapshots.load(Ordering::SeqCst), 1);
}

#[test]
fn submission_retention_survives_completion_clones_and_retry_without_evicting_owner() {
    use novadraw::{
        animation::{AnimationBudgetKind, AnimationState},
        render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
        runtime::{FramePreparation, FramePreparationError},
    };
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(20.0, 0.0, 10.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(root, GeometryBinding::layout_bounds())
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 10.0, 10.0)),
                bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let AnimationStart::Running(owner) = runtime.animation().timeline().start(plan).unwrap() else {
        panic!("expected animation")
    };
    runtime.animation().advance_time(time(0)).unwrap();
    let surface = SurfaceInfo {
        logical_width: 100.0,
        logical_height: 100.0,
        pixel_width: 100,
        pixel_height: 100,
        scale_factor: 1.0,
    };
    let FramePreparation::Ready(first) = runtime
        .frames()
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
    else {
        panic!("expected first submission")
    };
    let used = first.retention.as_ref().unwrap().retained_bytes();
    assert!(used > 0);
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .reservations()
            .submission_bytes,
        used
    );
    let clone = first.clone();
    let mut limits = runtime.animation().timeline().limits();
    limits.submission_bytes = used;
    runtime.animation().timeline().set_limits(limits).unwrap();
    let acknowledgement = runtime
        .frames()
        .complete_submission(
            first.session_id,
            first.frame_id,
            Ok(RenderDisposition::Retry),
        )
        .unwrap();
    assert!(matches!(
        acknowledgement,
        novadraw::runtime::SubmissionAcknowledgement::Retry
    ));
    let next_id = first.frame_id.next();
    drop(first);
    drop(clone);
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .reservations()
            .submission_bytes,
        used
    );
    let sample = runtime.animation().timeline().sample_version();
    assert!(matches!(
        runtime
            .frames()
            .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL),
        FramePreparation::Error(FramePreparationError::Animation(
            AnimationError::BudgetExceeded(AnimationBudgetKind::SnapshotBytes)
        ))
    ));
    assert_eq!(runtime.animation().timeline().sample_version(), sample);
    assert_eq!(
        runtime.animation().timeline().state(owner),
        Ok(AnimationState::Running)
    );
    limits.submission_bytes = used - 1;
    assert!(runtime.animation().timeline().set_limits(limits).is_err());
    assert_eq!(
        runtime.animation().timeline().limits().submission_bytes,
        used
    );
    limits.submission_bytes = used * 2;
    runtime.animation().timeline().set_limits(limits).unwrap();
    let FramePreparation::Ready(retry) = runtime
        .frames()
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
    else {
        panic!("expected retry after increasing capacity")
    };
    assert_eq!(retry.frame_id, next_id);
    // The recovery responsibility transferred only after the new one was acquired.
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .reservations()
            .submission_bytes,
        used
    );
    let acknowledgement = runtime
        .frames()
        .complete_submission(
            retry.session_id,
            retry.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .unwrap();
    assert!(matches!(
        acknowledgement,
        novadraw::runtime::SubmissionAcknowledgement::Presented
    ));
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .reservations()
            .submission_bytes,
        used
    );
    drop(retry);
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .reservations()
            .submission_bytes,
        0
    );
}

#[test]
fn owned_capture_retention_rejects_new_capture_and_downsize_until_drop() {
    use novadraw::animation::{AnimationBudgetKind, GeometryCaptureError};
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let first = runtime.animation().capture_geometry([root]).unwrap();
    let used = runtime.animation().timeline().reservations().capture_bytes;
    assert!(used > 0);
    let mut limits = runtime.animation().timeline().limits();
    limits.capture_bytes = used;
    runtime.animation().timeline().set_limits(limits).unwrap();
    let sample = runtime.animation().timeline().sample_version();
    assert!(matches!(
        runtime.animation().capture_geometry([root]),
        Err(GeometryCaptureError::Animation(
            AnimationError::BudgetExceeded(AnimationBudgetKind::SnapshotBytes)
        ))
    ));
    assert_eq!(runtime.animation().timeline().sample_version(), sample);
    assert_eq!(first.geometry().figure(root).unwrap().bounds.width, 100.0);
    limits.capture_bytes = used - 1;
    assert_eq!(
        runtime.animation().timeline().set_limits(limits),
        Err(AnimationError::BudgetExceeded(
            AnimationBudgetKind::SnapshotBytes
        ))
    );
    assert_eq!(runtime.animation().timeline().limits().capture_bytes, used);
    drop(first);
    runtime
        .figure(root)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, 101.0, 100.0))
        .unwrap();
    record_full_submission(&mut runtime);
    assert_eq!(
        runtime.animation().timeline().reservations().capture_bytes,
        0,
        "no capture survives the source change and old cache invalidation"
    );
    runtime
        .animation()
        .timeline()
        .set_limits(novadraw::animation::AnimationLimits::default())
        .unwrap();
    let cached_capture = runtime.animation().capture_geometry([root]).unwrap();
    let cache_used = runtime.animation().timeline().reservations().capture_bytes;
    drop(cached_capture);
    assert_eq!(
        runtime.animation().timeline().reservations().capture_bytes,
        cache_used,
        "the static Runtime cache retains its lease after its capture is released"
    );
    assert!(cache_used > 0);
    let mut cache_limit = runtime.animation().timeline().limits();
    cache_limit.capture_bytes = cache_used - 1;
    assert_eq!(
        runtime.animation().timeline().set_limits(cache_limit),
        Err(AnimationError::BudgetExceeded(
            AnimationBudgetKind::SnapshotBytes
        ))
    );
    runtime
        .figure(root)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, 102.0, 100.0))
        .unwrap();
    record_full_submission(&mut runtime);
    assert_eq!(
        runtime.animation().timeline().reservations().capture_bytes,
        0
    );
    runtime
        .animation()
        .timeline()
        .set_limits(cache_limit)
        .unwrap();
}

#[test]
fn capture_closure_is_bounded_even_without_active_animation() {
    use novadraw::animation::{AnimationBudgetKind, GeometryCaptureError};
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(0.0, 0.0, 10.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let mut limits = runtime.animation().timeline().limits();
    limits.staging_work.nodes = 1;
    runtime.animation().timeline().set_limits(limits).unwrap();
    assert!(matches!(
        runtime.animation().capture_geometry([root]),
        Err(GeometryCaptureError::Animation(
            AnimationError::BudgetExceeded(AnimationBudgetKind::GeometryNodes)
        ))
    ));
    assert_eq!(
        runtime.animation().timeline().reservations().capture_bytes,
        0
    );
}

#[test]
fn drawing_stage_reads_derived_geometry_and_records_owned_bounded_lines() {
    use novadraw::animation::{GeometryLine, GeometryProvider, GeometryProviderKey};
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let child = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(10.0, 0.0, 10.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(child, GeometryBinding::layout_bounds())
        .unwrap();
    let geometry_key = GeometryProviderKey::<f64>::new(child);
    let drawing_key = GeometryProviderKey::<f64>::new(root);
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 10.0, 10.0)),
                bounds(Rectangle::new(10.0, 0.0, 10.0, 10.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap()
    // The drawing is deliberately declared first.
    .with_geometry_provider(GeometryProvider::drawing(
        drawing_key,
        [root, child],
        vec![geometry_key.dependency()],
        1,
        move |context| {
            Ok(context
                .relative_transform(child, root)?
                .unwrap()
                .transform_point(Point::new(0.0, 0.0))
                .x())
        },
        |x, _, drawing| {
            drawing.line(GeometryLine {
                start: Point::new(0.0, 0.0),
                end: Point::new(*x, 0.0),
                width: 2.0,
                color: novadraw::graphics::Color::BLACK,
            })
        },
    ))
    .with_geometry_provider(GeometryProvider::new(
        geometry_key,
        [child],
        vec![],
        move |context| Ok(context.figure(child)?.bounds.x),
        |x| {
            Ok(GeometryPatch {
                local_transform: Some(Affine2D::from_translation(*x, 0.0)),
                ..Default::default()
            })
        },
    ));
    runtime.animation().timeline().start(plan).unwrap();
    runtime.animation().advance_time(time(0)).unwrap();
    runtime.animation().advance_time(time(50)).unwrap();
    let capture = runtime.animation().capture_geometry([root]).unwrap();
    assert_eq!(capture.geometry().value::<f64>(root), Ok(&10.0));
    let frame = record_full_submission(&mut runtime);
    assert!(frame.commands.iter().any(|command| matches!(command.kind,
        RenderCommandKind::Line { p2, .. } if p2 == Point::new(10.0, 0.0))));
    runtime.animation().advance_time(time(75)).unwrap();
    assert_eq!(capture.geometry().value::<f64>(root), Ok(&10.0));
    assert_eq!(
        runtime
            .animation()
            .capture_geometry([root])
            .unwrap()
            .geometry()
            .value::<f64>(root),
        Ok(&15.0)
    );
}

#[test]
fn drawing_output_overrun_fails_only_its_group_and_static_repair_skips_provider() {
    use novadraw::animation::{
        AnimationState, GeometryLine, GeometryProvider, GeometryProviderKey,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(20.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(root, GeometryBinding::layout_bounds())
        .unwrap();
    let counter: &'static AtomicUsize = Box::leak(Box::new(AtomicUsize::new(0)));
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 100.0, 100.0)),
                bounds(Rectangle::new(20.0, 0.0, 100.0, 100.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap()
    .with_geometry_provider(GeometryProvider::drawing(
        GeometryProviderKey::<f64>::new(root),
        [root],
        vec![],
        1,
        move |context| Ok(context.figure(root)?.bounds.x),
        move |x, _, drawing| {
            counter.fetch_add(1, Ordering::SeqCst);
            let line = GeometryLine {
                start: Point::new(0.0, 0.0),
                end: Point::new(5.0, 0.0),
                width: 1.0,
                color: novadraw::graphics::Color::BLACK,
            };
            drawing.line(line)?;
            if *x > 0.0 {
                // A callback cannot publish truncated output by ignoring a builder error.
                let _ = drawing.line(line);
            }
            Ok(())
        },
    ));
    let owner = match runtime.animation().timeline().start(plan).unwrap() {
        AnimationStart::Running(id) => id,
        other => panic!("{other:?}"),
    };
    let independent = runtime
        .animation()
        .timeline()
        .create_channel(1.0_f64)
        .unwrap();
    let healthy_plan = AnimationPlan::track(
        independent,
        Motion::Tween(Tween::between(0.0, 1.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    let healthy = match runtime.animation().timeline().start(healthy_plan).unwrap() {
        AnimationStart::Running(id) => id,
        other => panic!("{other:?}"),
    };
    runtime.animation().advance_time(time(0)).unwrap();
    runtime.animation().advance_time(time(50)).unwrap();
    let repaired = runtime.animation().capture_geometry([root]).unwrap();
    assert_eq!(
        runtime.animation().timeline().state(owner),
        Ok(AnimationState::Failed)
    );
    assert!(
        matches!(runtime.animation().timeline().failure(owner).unwrap(),
        Some(AnimationError::ProviderOutputExceeded {
            figure, value_type, declared: 1, actual: 2, ..
        }) if *figure == root && *value_type == std::any::TypeId::of::<f64>())
    );
    assert_eq!(
        runtime.animation().timeline().state(healthy),
        Ok(AnimationState::Running)
    );
    assert_eq!(runtime.animation().timeline().value(independent), Ok(0.5));
    assert_eq!(repaired.geometry().figure(root).unwrap().bounds.x, 20.0);
    let after_failure = counter.load(Ordering::SeqCst);
    record_full_submission(&mut runtime);
    assert_eq!(counter.load(Ordering::SeqCst), after_failure);
}

#[test]
fn oversized_drawing_declaration_rejects_retarget_before_callback_or_owner_transfer() {
    use novadraw::animation::{AnimationState, GeometryProvider, GeometryProviderKey};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(20.0, 0.0, 50.0, 20.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(figure, GeometryBinding::layout_bounds())
        .unwrap();
    let track = || {
        AnimationPlan::track(
            channel,
            Motion::Tween(
                Tween::between(
                    bounds(Rectangle::new(0.0, 0.0, 50.0, 20.0)),
                    bounds(Rectangle::new(20.0, 0.0, 50.0, 20.0)),
                    Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap()
    };
    let owner = match runtime.animation().timeline().start(track()).unwrap() {
        AnimationStart::Running(id) => id,
        other => panic!("{other:?}"),
    };
    runtime.animation().advance_time(time(0)).unwrap();
    runtime.animation().advance_time(time(50)).unwrap();
    let sample = runtime.animation().timeline().sample_version();
    let reservations = runtime.animation().timeline().reservations();
    let wake = runtime.animation().next_wake_deadline();
    let counter: &'static AtomicUsize = Box::leak(Box::new(AtomicUsize::new(0)));
    let candidate = track().with_geometry_provider(GeometryProvider::drawing(
        GeometryProviderKey::<f64>::new(figure),
        [figure],
        vec![],
        usize::MAX,
        move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(0.0)
        },
        |_, _, _| Ok(()),
    ));
    assert!(matches!(
        runtime.animation().timeline().retarget(owner, candidate),
        Err(AnimationError::BudgetExceeded(_))
    ));
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    assert_eq!(
        runtime.animation().timeline().state(owner),
        Ok(AnimationState::Running)
    );
    assert_eq!(runtime.animation().timeline().sample_version(), sample);
    assert_eq!(runtime.animation().timeline().reservations(), reservations);
    assert_eq!(runtime.animation().next_wake_deadline(), wake);
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .value(channel)
            .unwrap()
            .rectangle()
            .x,
        10.0
    );
}

#[test]
fn true_bounds_capture_uses_last_advance_without_recording_or_affine_size_scale() {
    let mut runtime = Runtime::empty();
    let final_bounds = Rectangle::new(80.0, 60.0, 40.0, 20.0);
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(
                final_bounds.x,
                final_bounds.y,
                final_bounds.width,
                final_bounds.height,
            )
            .expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(figure, GeometryBinding::layout_bounds())
        .unwrap();
    let start = Rectangle::new(10.0, 20.0, 0.0, 0.0);
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(start),
                bounds(final_bounds),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    assert!(matches!(
        runtime.animation().timeline().start(plan),
        Ok(AnimationStart::Running(_))
    ));

    let unanchored = runtime.animation().capture_geometry([figure]).unwrap();
    assert_eq!(unanchored.geometry().version().sample.time, None);
    assert_eq!(unanchored.geometry().figure(figure).unwrap().bounds, start);
    assert_eq!(
        unanchored
            .geometry()
            .figure(figure)
            .unwrap()
            .local_transform,
        Affine2D::IDENTITY
    );
    runtime.animation().advance_time(time(0)).unwrap();
    let recorded = record_full_submission(&mut runtime);
    let commands = recorded.commands.clone();
    runtime.animation().advance_time(time(50)).unwrap();
    assert!(matches!(
        runtime.animation().geometry(),
        Err(AnimationError::GeometryNotPrepared)
    ));
    let advances = runtime.animation().timeline().stats().clock_advances;
    let halfway = runtime.animation().capture_geometry([figure]).unwrap();
    let view = halfway.geometry();
    assert_eq!(view.version().sample.time, Some(time(50)));
    assert_eq!(
        view.figure(figure).unwrap().bounds,
        Rectangle::new(45.0, 40.0, 20.0, 10.0)
    );
    assert_eq!(
        view.figure(figure).unwrap().local_to_surface,
        Affine2D::from_translation(45.0, 40.0)
    );
    assert_eq!(
        view.value::<LayoutBounds>(figure).unwrap().rectangle(),
        view.figure(figure).unwrap().bounds
    );
    assert_eq!(
        runtime.animation().timeline().stats().clock_advances,
        advances
    );
    assert_eq!(
        runtime.animation().geometry().unwrap().version(),
        view.version()
    );
    assert_eq!(runtime.tree().figure_bounds(figure), Ok(final_bounds));
    assert_eq!(recorded.commands, commands);
    assert_eq!(unanchored.geometry().figure(figure).unwrap().bounds, start);
    let halfway_frame = record_full_submission(&mut runtime);
    assert!(halfway_frame.commands.iter().any(|command| {
        matches!(command.kind, RenderCommandKind::ConcatTransform { matrix }
            if matrix == Affine2D::from_translation(45.0, 40.0))
    }));
    assert!(!halfway_frame.commands.iter().any(|command| {
        matches!(command.kind, RenderCommandKind::ConcatTransform { matrix }
            if matrix == Affine2D::from_scale(0.5, 0.5))
    }));
    runtime.animation().advance_time(time(75)).unwrap();
    runtime.animation().advance_time(time(90)).unwrap();
    let latest = runtime.animation().capture_geometry([figure]).unwrap();
    assert_eq!(latest.geometry().version().sample.time, Some(time(90)));
    assert_eq!(
        latest.geometry().figure(figure).unwrap().bounds,
        start.interpolate(&final_bounds, 0.9)
    );
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ExternalRectangle(Rectangle);

impl AnimationValue for ExternalRectangle {
    fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        Self(self.0.interpolate(&target.0, progress))
    }
}

#[test]
fn external_semantics_share_registration_and_queries_without_aliasing_rectangle_values() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(40.0, 20.0, 30.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let layout = runtime
        .animation()
        .timeline()
        .bind_geometry(figure, GeometryBinding::layout_bounds())
        .unwrap();
    let external = runtime
        .animation()
        .timeline()
        .bind_geometry(
            figure,
            GeometryBinding::new(
                |_, inputs| Ok(ExternalRectangle(inputs.bounds)),
                |_| Ok(GeometryPatch::default()),
            ),
        )
        .unwrap();
    let final_bounds = runtime.tree().figure_bounds(figure).unwrap();
    let start = Rectangle::new(0.0, 0.0, 30.0, 10.0);
    let plan = AnimationPlan::parallel(vec![
        AnimationPlan::track(
            layout,
            Motion::Tween(
                Tween::between(
                    bounds(start),
                    bounds(final_bounds),
                    Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap(),
        AnimationPlan::track(
            external,
            Motion::Tween(
                Tween::between(
                    ExternalRectangle(Rectangle::new(8.0, 9.0, 1.0, 2.0)),
                    ExternalRectangle(final_bounds),
                    Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap(),
    ])
    .unwrap();
    runtime.animation().timeline().start(plan).unwrap();
    let capture = runtime.animation().capture_geometry([figure]).unwrap();
    assert_eq!(
        capture
            .geometry()
            .value::<LayoutBounds>(figure)
            .unwrap()
            .rectangle(),
        start
    );
    assert_eq!(
        capture
            .geometry()
            .value::<ExternalRectangle>(figure)
            .unwrap()
            .0,
        Rectangle::new(8.0, 9.0, 1.0, 2.0)
    );
}

#[test]
fn query_rejects_undeclared_identity_and_never_materializes_on_pull() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 30.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    assert!(matches!(
        runtime.animation().geometry(),
        Err(AnimationError::GeometryNotPrepared)
    ));
    let capture = runtime.animation().capture_geometry([figure]).unwrap();
    let mut foreign = Runtime::empty();
    let other = foreign
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 1.0, 1.0).expect("valid figure bounds"),
        ))
        .unwrap();
    assert!(matches!(
        capture.geometry().figure(other),
        Err(AnimationError::ForeignTarget)
    ));
    assert_eq!(
        capture
            .geometry()
            .relative_transform(figure, figure)
            .unwrap(),
        Some(Affine2D::IDENTITY)
    );
    let version = capture.geometry().version();
    runtime.animation().advance_time(time(50)).unwrap();
    assert!(matches!(
        runtime.animation().geometry(),
        Err(AnimationError::GeometryNotPrepared)
    ));
    assert_eq!(capture.geometry().version(), version);
    let latest = runtime.animation().capture_geometry([figure]).unwrap();
    assert_eq!(latest.geometry().version().sample.time, Some(time(50)));
}

#[test]
fn duplicate_geometry_writers_are_rejected_before_any_owner_transfer() {
    let mut runtime = Runtime::empty();
    let end = Rectangle::new(40.0, 20.0, 30.0, 10.0);
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(end.x, end.y, end.width, end.height).expect("valid figure bounds"),
        ))
        .unwrap();
    let layout = runtime
        .animation()
        .timeline()
        .bind_geometry(figure, GeometryBinding::layout_bounds())
        .unwrap();
    let external = runtime
        .animation()
        .timeline()
        .bind_geometry(
            figure,
            GeometryBinding::new(
                |_, inputs| Ok(ExternalRectangle(inputs.bounds)),
                |value| {
                    Ok(GeometryPatch {
                        bounds: Some(value.0),
                        ..GeometryPatch::default()
                    })
                },
            ),
        )
        .unwrap();
    let first = AnimationPlan::track(
        layout,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 30.0, 10.0)),
                bounds(end),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let owner = match runtime.animation().timeline().start(first.clone()).unwrap() {
        AnimationStart::Running(id) => id,
        other => panic!("expected owner, got {other:?}"),
    };
    runtime.animation().advance_time(time(0)).unwrap();
    runtime.animation().advance_time(time(50)).unwrap();
    let value = runtime.animation().timeline().value(layout).unwrap();
    let version = runtime.animation().timeline().sample_version();
    let wake = runtime.animation().next_wake_deadline();
    let duplicate = AnimationPlan::parallel(vec![
        first,
        AnimationPlan::track(
            external,
            Motion::Tween(Tween::to(ExternalRectangle(end), Duration::from_millis(100)).unwrap()),
        )
        .unwrap(),
    ])
    .unwrap();
    assert_eq!(
        runtime.animation().timeline().retarget(owner, duplicate),
        Err(AnimationError::ChannelConflict)
    );
    assert_eq!(runtime.animation().timeline().value(layout).unwrap(), value);
    assert_eq!(runtime.animation().timeline().sample_version(), version);
    assert_eq!(runtime.animation().next_wake_deadline(), wake);
    assert!(
        !runtime
            .animation()
            .timeline()
            .has_override(external)
            .unwrap()
    );
}

#[test]
fn geometry_adapter_panic_crosses_the_runtime_fault_boundary() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 30.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime
            .animation()
            .timeline()
            .bind_geometry::<ExternalRectangle>(
                figure,
                GeometryBinding::new(
                    |_, _| panic!("external geometry reader failed"),
                    |_| Ok(GeometryPatch::default()),
                ),
            )
    }));
    assert!(result.is_err());
    assert!(matches!(
        runtime.animation().capture_geometry([figure]),
        Err(novadraw::animation::GeometryCaptureError::Animation(
            AnimationError::RuntimeFaulted
        ))
    ));
}

#[test]
fn descriptor_refresh_invalidates_cached_geometry_without_advancing_time() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 30.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let capture = runtime.animation().capture_geometry([figure]).unwrap();
    let previous = capture.geometry().version();
    let binding = GeometryBinding::new(
        |_, inputs| Ok(ExternalRectangle(inputs.bounds)),
        |_| Ok(GeometryPatch::default()),
    );
    runtime
        .animation()
        .timeline()
        .bind_geometry(figure, binding)
        .unwrap();
    assert!(matches!(
        runtime.animation().geometry(),
        Err(AnimationError::GeometryNotPrepared)
    ));
    let latest = runtime.animation().capture_geometry([figure]).unwrap();
    assert_eq!(latest.geometry().version().sample, previous.sample);
    assert!(latest.geometry().version().binding_revision > previous.binding_revision);
    assert_eq!(
        latest
            .geometry()
            .value::<ExternalRectangle>(figure)
            .unwrap()
            .0,
        Rectangle::new(0.0, 0.0, 30.0, 10.0)
    );
}

struct ExternalContainer {
    bounds: Rectangle,
    insets: Insets,
    transform: Affine2D,
    clipping: ChildClippingStrategy,
}

impl Figure for ExternalContainer {
    fn name(&self) -> &'static str {
        "ExternalGeometryContainer"
    }
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn initial_insets(&self) -> Insets {
        self.insets
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for ExternalContainer {
    fn child_transform(&self) -> ChildTransform {
        ChildTransform::from_affine(self.transform)
    }
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.clipping
    }
}

#[test]
fn nested_sampled_bounds_preserve_four_insets_and_scale_rotation_edge_composition() {
    let mut runtime = Runtime::empty();
    let insets = Insets {
        left: 3.0,
        top: 5.0,
        right: 7.0,
        bottom: 11.0,
    };
    let transform = Affine2D::new(0.0, 2.0, -2.0, 0.0, 120.0, 0.0);
    let parent = runtime
        .set_contents(Box::new(ExternalContainer {
            bounds: Rectangle::new(10.0, 20.0, 200.0, 160.0),
            insets,
            transform,
            clipping: ChildClippingStrategy::ClipToChildBounds,
        }))
        .unwrap();
    let child = runtime
        .figure(parent)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(20.0, 30.0, 40.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(parent, GeometryBinding::layout_bounds())
        .unwrap();
    let before = Rectangle::new(4.0, 8.0, 100.0, 80.0);
    let after = runtime.tree().figure_bounds(parent).unwrap();
    runtime
        .animation()
        .timeline()
        .start(
            AnimationPlan::track(
                channel,
                Motion::Tween(
                    Tween::between(bounds(before), bounds(after), Duration::from_millis(100))
                        .unwrap(),
                ),
            )
            .unwrap(),
        )
        .unwrap();
    runtime.animation().advance_time(time(0)).unwrap();
    runtime.animation().advance_time(time(50)).unwrap();
    let capture = runtime.animation().capture_geometry([parent]).unwrap();
    let view = capture.geometry();
    let parent_geometry = view.figure(parent).unwrap();
    assert_eq!(
        parent_geometry.bounds,
        Rectangle::new(7.0, 14.0, 150.0, 120.0)
    );
    assert_eq!(
        parent_geometry.client_box,
        Rectangle::new(3.0, 5.0, 140.0, 104.0)
    );
    assert_eq!(parent_geometry.local_transform, Affine2D::IDENTITY);
    let expected = Affine2D::from_translation(7.0, 14.0)
        * Affine2D::from_translation(3.0, 5.0)
        * transform
        * Affine2D::from_translation(20.0, 30.0);
    assert_eq!(view.figure(child).unwrap().local_to_surface, expected);
    assert_eq!(
        view.to_content_transform(child, parent).unwrap(),
        Some(Affine2D::from_translation(20.0, 30.0))
    );
    assert_eq!(
        view.figure(child)
            .unwrap()
            .local_to_surface
            .transform_point(Point::ZERO),
        expected.transform_point(Point::ZERO)
    );
    let commands = record_full_submission(&mut runtime).commands;
    assert!(commands.iter().any(|command| matches!(command.kind,
        RenderCommandKind::ConcatTransform { matrix } if matrix ==
            Affine2D::from_translation(3.0, 5.0) * transform)));
}

#[test]
fn sampled_zero_client_clip_and_overflow_follow_the_same_ancestor_contract() {
    for overflow in [false, true] {
        let mut runtime = Runtime::empty();
        runtime
            .frames()
            .resize_logical_viewport(200.0, 200.0)
            .unwrap();
        let parent = runtime
            .set_contents(Box::new(ExternalContainer {
                bounds: Rectangle::new(0.0, 0.0, 100.0, 100.0),
                insets: Insets::uniform(5.0),
                transform: Affine2D::IDENTITY,
                clipping: if overflow {
                    ChildClippingStrategy::OverflowVisible
                } else {
                    ChildClippingStrategy::ClipToChildBounds
                },
            }))
            .unwrap();
        let child = runtime
            .figure(parent)
            .unwrap()
            .add(Box::new(
                RectangleFigure::new(10.0, 10.0, 20.0, 20.0).expect("valid figure bounds"),
            ))
            .unwrap();
        // Establish the ordinary clipping ancestor's surface before testing overflow.
        record_full_submission(&mut runtime);
        let terminal = runtime.tree().figure_bounds(parent).unwrap();
        let channel = runtime
            .animation()
            .timeline()
            .bind_geometry(parent, GeometryBinding::layout_bounds())
            .unwrap();
        runtime
            .animation()
            .timeline()
            .start(
                AnimationPlan::track(
                    channel,
                    Motion::Tween(
                        Tween::between(
                            bounds(Rectangle::ZERO),
                            bounds(terminal),
                            Duration::from_millis(100),
                        )
                        .unwrap(),
                    ),
                )
                .unwrap(),
            )
            .unwrap();
        let capture = runtime.animation().capture_geometry([parent]).unwrap();
        let parent_geometry = capture.geometry().figure(parent).unwrap();
        assert_eq!(parent_geometry.client_box.width, 0.0);
        assert_eq!(parent_geometry.client_box.height, 0.0);
        let child_geometry = capture.geometry().figure(child).unwrap();
        if overflow {
            assert!(child_geometry.visual_envelope.is_some());
        } else {
            assert_eq!(child_geometry.visual_envelope, None);
            assert!(
                child_geometry
                    .effective_clip
                    .is_some_and(|clip| clip.width == 0.0 || clip.height == 0.0)
            );
        }
    }
}

#[test]
fn runtime_real_bounds_transaction_retargets_from_unrecorded_sample_and_commits_source_once() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 30.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    runtime.animation().advance_time(time(0)).unwrap();
    let transition = BoundsTransition::new(Duration::from_millis(100));
    let (_, first) = runtime
        .animation()
        .transition_layout_bounds([figure], transition, |runtime| {
            runtime
                .figure(figure)
                .unwrap()
                .set_bounds(Rectangle::new(100.0, 0.0, 60.0, 20.0))
        })
        .unwrap();
    let first = match first {
        AnimationStart::Running(id) => id,
        other => panic!("expected first owner, got {other:?}"),
    };
    runtime.animation().advance_time(time(50)).unwrap();
    let (_, second) = runtime
        .animation()
        .transition_layout_bounds([figure], transition, |runtime| {
            runtime
                .figure(figure)
                .unwrap()
                .set_bounds(Rectangle::new(0.0, 0.0, 30.0, 10.0))
        })
        .unwrap();
    assert!(matches!(second, AnimationStart::Running(_)));
    assert_eq!(
        runtime.animation().timeline().state(first).unwrap(),
        novadraw::animation::AnimationState::Cancelled
    );
    let capture = runtime.animation().capture_geometry([figure]).unwrap();
    assert_eq!(capture.geometry().version().sample.time, Some(time(50)));
    assert_eq!(
        capture.geometry().figure(figure).unwrap().bounds,
        Rectangle::new(50.0, 0.0, 45.0, 15.0)
    );
    assert_eq!(
        runtime.tree().figure_bounds(figure),
        Ok(Rectangle::new(0.0, 0.0, 30.0, 10.0))
    );
}

#[test]
fn rejected_real_bounds_candidate_restores_old_terminal_owner_and_version() {
    use novadraw::animation::{AnimationBudgetKind, AnimationState};
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 30.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    runtime.animation().advance_time(time(0)).unwrap();
    let transition = BoundsTransition::new(Duration::from_millis(100));
    let (_, start) = runtime
        .animation()
        .transition_layout_bounds([figure], transition, |runtime| {
            runtime
                .figure(figure)
                .unwrap()
                .set_bounds(Rectangle::new(100.0, 0.0, 60.0, 20.0))
        })
        .unwrap();
    let owner = match start {
        AnimationStart::Running(id) => id,
        other => panic!("{other:?}"),
    };
    runtime.animation().advance_time(time(50)).unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(figure, GeometryBinding::layout_bounds())
        .unwrap();
    let committed = bounds(Rectangle::new(100.0, 0.0, 60.0, 20.0));
    let old_capture = runtime.animation().capture_geometry([figure]).unwrap();
    let wake = runtime.animation().next_wake_deadline();
    let sample = runtime.animation().timeline().sample_version();
    let mut limit = runtime.animation().timeline().limits();
    limit.staging_work.calls = 0;
    runtime.animation().timeline().set_limits(limit).unwrap();
    let error = runtime
        .animation()
        .transition_layout_bounds([figure], transition, |runtime| {
            runtime
                .figure(figure)
                .unwrap()
                .set_bounds(Rectangle::new(0.0, 0.0, 30.0, 10.0))
        })
        .expect_err("candidate must fail");
    assert!(matches!(
        error,
        novadraw::animation::AnimationTransactionError::Transition(AnimationError::BudgetExceeded(
            AnimationBudgetKind::ProviderCalls
        ))
    ));
    assert_eq!(runtime.animation().timeline().sample_version(), sample);
    assert_eq!(runtime.animation().next_wake_deadline(), wake);
    assert_eq!(
        runtime.animation().timeline().state(owner).unwrap(),
        AnimationState::Running
    );
    let current = runtime.animation().capture_geometry([figure]).unwrap();
    assert_eq!(
        current.geometry().figure(figure).unwrap().bounds,
        old_capture.geometry().figure(figure).unwrap().bounds
    );
    runtime.animation().advance_time(time(75)).unwrap();
    let next = runtime.animation().capture_geometry([figure]).unwrap();
    assert_eq!(
        next.geometry().figure(figure).unwrap().bounds,
        Rectangle::new(75.0, 0.0, 52.5, 17.5)
    );
    runtime.animation().advance_time(time(100)).unwrap();
    assert_eq!(
        runtime.animation().timeline().value(channel).unwrap(),
        committed
    );
}

#[test]
fn geometry_work_downsize_is_atomic_and_dependency_styles_cancel_only_their_group() {
    use novadraw::animation::{AnimationBudgetKind, AnimationState};
    let mut runtime = Runtime::empty();
    let parent = runtime
        .set_contents(Box::new(ExternalContainer {
            bounds: Rectangle::new(0.0, 0.0, 300.0, 200.0),
            insets: Insets::ZERO,
            transform: Affine2D::IDENTITY,
            clipping: ChildClippingStrategy::OverflowVisible,
        }))
        .unwrap();
    let a = runtime
        .figure(parent)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(10.0, 10.0, 30.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let b = runtime
        .figure(parent)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(100.0, 10.0, 30.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    runtime.animation().advance_time(time(0)).unwrap();
    let mut owners = Vec::new();
    for id in [a, b] {
        let channel = runtime
            .animation()
            .timeline()
            .bind_geometry(id, GeometryBinding::layout_bounds())
            .unwrap();
        let final_bounds = runtime.tree().figure_bounds(id).unwrap();
        let start = Rectangle::new(final_bounds.x, 0.0, 30.0, 10.0);
        let plan = AnimationPlan::track(
            channel,
            Motion::Tween(
                Tween::between(
                    bounds(start),
                    bounds(final_bounds),
                    Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        owners.push(match runtime.animation().timeline().start(plan).unwrap() {
            AnimationStart::Running(id) => id,
            other => panic!("{other:?}"),
        });
    }
    let limits = runtime.animation().timeline().limits();
    let mut downsize = limits;
    downsize.active_work.nodes = 0;
    assert_eq!(
        runtime.animation().timeline().set_limits(downsize),
        Err(AnimationError::BudgetExceeded(
            AnimationBudgetKind::GeometryNodes
        ))
    );
    assert_eq!(runtime.animation().timeline().limits(), limits);
    runtime
        .figure(a)
        .unwrap()
        .set_style(novadraw::figure::FigureStyle {
            background: Some(novadraw::graphics::Color::RED),
            ..Default::default()
        })
        .unwrap();
    runtime.animation().capture_geometry([b]).unwrap();
    assert_eq!(
        runtime.animation().timeline().state(owners[0]).unwrap(),
        AnimationState::Cancelled
    );
    assert_eq!(
        runtime.animation().timeline().state(owners[1]).unwrap(),
        AnimationState::Running
    );
}

#[test]
fn derived_providers_schedule_typed_dependencies_and_reject_cycles_before_owner_transfer() {
    use novadraw::animation::{GeometryProvider, GeometryProviderKey};
    let mut runtime = Runtime::empty();
    let a = runtime
        .set_contents(Box::new(
            RectangleFigure::new(20.0, 0.0, 30.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let b = runtime
        .figure(a)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(2.0, 0.0, 10.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(a, GeometryBinding::layout_bounds())
        .unwrap();
    let track = || {
        AnimationPlan::track(
            channel,
            Motion::Tween(
                Tween::between(
                    bounds(Rectangle::new(0.0, 0.0, 30.0, 10.0)),
                    bounds(Rectangle::new(20.0, 0.0, 30.0, 10.0)),
                    Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap()
    };
    let first = GeometryProviderKey::<f64>::new(a);
    let second = GeometryProviderKey::<f64>::new(b);
    // Declare the consumer first: dependency order, rather than declaration order, determines
    // execution. Both roles use the same external typed descriptor.
    let plan = track()
        .with_geometry_provider(GeometryProvider::new(
            second,
            [b],
            vec![first.dependency()],
            move |context| Ok(*context.value(first)? * 2.0),
            |value| {
                Ok(GeometryPatch {
                    local_transform: Some(Affine2D::from_translation(*value, 0.0)),
                    ..Default::default()
                })
            },
        ))
        .with_geometry_provider(GeometryProvider::new(
            first,
            [a],
            vec![],
            move |context| Ok(context.figure(a)?.bounds.x),
            |_| Ok(GeometryPatch::default()),
        ));
    let owner = match runtime.animation().timeline().start(plan).unwrap() {
        AnimationStart::Running(id) => id,
        other => panic!("{other:?}"),
    };
    runtime.animation().advance_time(time(0)).unwrap();
    runtime.animation().advance_time(time(50)).unwrap();
    let captured = runtime.animation().capture_geometry([a]).unwrap();
    assert_eq!(captured.geometry().value::<f64>(a).unwrap(), &10.0);
    assert_eq!(captured.geometry().value::<f64>(b).unwrap(), &20.0);
    assert_eq!(
        captured.geometry().figure(b).unwrap().local_transform,
        Affine2D::from_translation(20.0, 0.0)
    );
    let sample = runtime.animation().timeline().sample_version();
    let cycle = track()
        .with_geometry_provider(GeometryProvider::new(
            first,
            [a],
            vec![second.dependency()],
            move |context| Ok(*context.value(second)?),
            |_| Ok(GeometryPatch::default()),
        ))
        .with_geometry_provider(GeometryProvider::new(
            second,
            [b],
            vec![first.dependency()],
            move |context| Ok(*context.value(first)?),
            |_| Ok(GeometryPatch::default()),
        ));
    assert_eq!(
        runtime.animation().timeline().retarget(owner, cycle),
        Err(AnimationError::GeometryDependencyCycle)
    );
    assert_eq!(runtime.animation().timeline().sample_version(), sample);
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .value(channel)
            .unwrap()
            .rectangle()
            .x,
        10.0
    );
}

#[test]
fn derived_query_failure_clears_complete_group_and_cached_geometry_never_reruns_provider() {
    use novadraw::animation::{AnimationState, GeometryProvider, GeometryProviderKey};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(100.0, 0.0, 30.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(figure, GeometryBinding::layout_bounds())
        .unwrap();
    let reject: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    let calls: &'static AtomicUsize = Box::leak(Box::new(AtomicUsize::new(0)));
    let provider_reject = reject;
    let provider_calls = calls;
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 30.0, 10.0)),
                bounds(Rectangle::new(100.0, 0.0, 30.0, 10.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap()
    .with_geometry_provider(GeometryProvider::new(
        GeometryProviderKey::<f64>::new(figure),
        [figure],
        vec![],
        move |context| {
            provider_calls.fetch_add(1, Ordering::SeqCst);
            if provider_reject.load(Ordering::SeqCst) {
                return Err(AnimationError::UndeclaredProviderDependency);
            }
            Ok(context.figure(figure)?.bounds.x)
        },
        |_| Ok(GeometryPatch::default()),
    ));
    let owner = match runtime.animation().timeline().start(plan).unwrap() {
        AnimationStart::Running(id) => id,
        other => panic!("{other:?}"),
    };
    runtime.animation().advance_time(time(0)).unwrap();
    let capture = runtime.animation().capture_geometry([figure]).unwrap();
    let before = calls.load(Ordering::SeqCst);
    for _ in 0..3 {
        runtime.animation().capture_geometry([figure]).unwrap();
        runtime.animation().geometry().unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), before);
    assert_eq!(capture.geometry().figure(figure).unwrap().bounds.x, 0.0);
    reject.store(true, Ordering::SeqCst);
    runtime.animation().advance_time(time(50)).unwrap();
    let fallback = runtime.animation().capture_geometry([figure]).unwrap();
    assert_eq!(fallback.geometry().figure(figure).unwrap().bounds.x, 100.0);
    assert_eq!(
        runtime.animation().timeline().state(owner).unwrap(),
        AnimationState::Failed
    );
    assert!(
        !runtime
            .animation()
            .timeline()
            .has_override(channel)
            .unwrap()
    );
    assert_eq!(
        runtime.animation().timeline().reservations().active_tracks,
        0
    );
}

#[test]
fn provider_prepare_rejects_undeclared_queries_even_inside_captured_subtree() {
    use novadraw::animation::{GeometryProvider, GeometryProviderKey};
    let mut runtime = Runtime::empty();
    let a = runtime
        .set_contents(Box::new(
            RectangleFigure::new(20.0, 0.0, 30.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let b = runtime
        .figure(a)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(2.0, 0.0, 10.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(a, GeometryBinding::layout_bounds())
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 30.0, 10.0)),
                bounds(Rectangle::new(20.0, 0.0, 30.0, 10.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap()
    .with_geometry_provider(GeometryProvider::new(
        GeometryProviderKey::<f64>::new(a),
        [a],
        vec![],
        move |context| Ok(context.figure(b)?.bounds.x),
        |_| Ok(GeometryPatch::default()),
    ));
    let sample = runtime.animation().timeline().sample_version();
    assert_eq!(
        runtime.animation().timeline().start(plan),
        Err(AnimationError::UndeclaredGeometryDependency { figure: b })
    );
    assert_eq!(runtime.animation().timeline().sample_version(), sample);
    assert!(
        !runtime
            .animation()
            .timeline()
            .has_override(channel)
            .unwrap()
    );
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .reservations()
            .active_animations,
        0
    );
}

#[test]
fn repeated_provider_queries_cannot_hide_work_overrun_by_ignoring_errors() {
    use novadraw::animation::{GeometryPatch, GeometryProvider, GeometryProviderKey};
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(10.0, 0.0, 40.0, 20.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(root, GeometryBinding::layout_bounds())
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 40.0, 20.0)),
                bounds(Rectangle::new(10.0, 0.0, 40.0, 20.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap()
    .with_geometry_provider(GeometryProvider::new(
        GeometryProviderKey::<f64>::new(root),
        [root],
        vec![],
        move |context| {
            for _ in 0..17 {
                let _ = context.figure(root);
            }
            Ok(0.0)
        },
        |_| Ok(GeometryPatch::default()),
    ));
    assert!(matches!(runtime.animation().timeline().start(plan),
        Err(AnimationError::ProviderOutputExceeded {
            kind: novadraw::animation::AnimationBudgetKind::GeometryOperations,
            declared: 16, actual: 17, figure, ..
        }) if figure == root));
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .reservations()
            .active_animations,
        0
    );
    assert!(
        !runtime
            .animation()
            .timeline()
            .has_override(channel)
            .unwrap()
    );
}

#[test]
fn nested_capture_counts_each_node_once_and_cached_reads_add_no_geometry_work() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 40.0, 20.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let child = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(2.0, 3.0, 10.0, 5.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let grandchild = runtime
        .figure(child)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(1.0, 1.0, 2.0, 2.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let capture = runtime
        .animation()
        .capture_geometry([root, child, grandchild])
        .unwrap();
    let work = capture.geometry().materialization_work();
    assert_eq!(work.nodes, 4); // three Figures and the synthetic root
    assert_eq!(work.edges, 3);
    assert!(work.matrices > 0);
    assert!(work.envelopes >= work.nodes);
    for _ in 0..3 {
        capture.geometry().figure(grandchild).unwrap();
        capture
            .geometry()
            .relative_transform(grandchild, root)
            .unwrap();
        assert_eq!(capture.geometry().materialization_work(), work);
    }
}

#[test]
fn inline_provider_configuration_is_charged_before_retarget_callbacks() {
    use novadraw::animation::{
        AnimationState, GeometryPatch, GeometryProvider, GeometryProviderKey,
    };
    let mut runtime = Runtime::empty();
    runtime.animation().advance_time(time(0)).unwrap();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(10.0, 0.0, 40.0, 20.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(root, GeometryBinding::layout_bounds())
        .unwrap();
    let track = || {
        AnimationPlan::track(
            channel,
            Motion::Tween(
                Tween::between(
                    bounds(Rectangle::new(0.0, 0.0, 40.0, 20.0)),
                    bounds(Rectangle::new(10.0, 0.0, 40.0, 20.0)),
                    Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap()
    };
    let provider = GeometryProvider::new(
        GeometryProviderKey::<f64>::new(root),
        [root],
        vec![],
        |_| Ok(0.0),
        |_| Ok(GeometryPatch::default()),
    );
    let owner = match runtime
        .animation()
        .timeline()
        .start(track().with_geometry_provider(provider))
        .unwrap()
    {
        AnimationStart::Running(owner) => owner,
        other => panic!("{other:?}"),
    };
    let before = runtime.animation().timeline().reservations();
    let mut limits = runtime.animation().timeline().limits();
    limits.staging_work.bytes = before.work.bytes;
    runtime.animation().timeline().set_limits(limits).unwrap();
    let configuration = [42u64; 4096];
    let candidate = track().with_geometry_provider(GeometryProvider::new(
        GeometryProviderKey::<f64>::new(root),
        [root],
        vec![],
        move |_| {
            std::hint::black_box(configuration);
            panic!("oversized configuration must be rejected before evaluation")
        },
        |_| Ok(GeometryPatch::default()),
    ));
    assert!(matches!(
        runtime.animation().timeline().retarget(owner, candidate),
        Err(AnimationError::BudgetExceeded(
            novadraw::animation::AnimationBudgetKind::SnapshotBytes
        ))
    ));
    assert_eq!(
        runtime.animation().timeline().state(owner),
        Ok(AnimationState::Running)
    );
    assert_eq!(runtime.animation().timeline().reservations(), before);
}

#[test]
fn legacy_heap_value_reserves_storage_and_sample_overrun_fails_only_its_owner() {
    use novadraw::animation::AnimationState;
    #[derive(Clone, PartialEq)]
    struct Growing(Vec<f64>);
    impl AnimationValue for Growing {
        fn storage_bytes(&self) -> usize {
            std::mem::size_of::<Self>() + self.0.capacity() * std::mem::size_of::<f64>()
        }
        fn is_valid(&self) -> bool {
            true
        }
        fn interpolate(&self, _: &Self, progress: f64) -> Self {
            Self(vec![
                progress;
                if progress == 0.25 {
                    self.0.len() * 2
                } else {
                    self.0.len()
                }
            ])
        }
    }
    let mut runtime = Runtime::empty();
    runtime.animation().advance_time(time(0)).unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .create_channel(Growing(vec![1.0; 4]))
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                Growing(vec![0.0; 4]),
                Growing(vec![1.0; 4]),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let owner = match runtime.animation().timeline().start(plan).unwrap() {
        AnimationStart::Running(owner) => owner,
        other => panic!("{other:?}"),
    };
    let declared = std::mem::size_of::<Growing>() + 4 * std::mem::size_of::<f64>();
    let actual = std::mem::size_of::<Growing>() + 8 * std::mem::size_of::<f64>();
    assert!(runtime.animation().timeline().reservations().work.bytes >= 6 * declared);
    let other = runtime
        .animation()
        .timeline()
        .create_channel(1.0f64)
        .unwrap();
    let healthy = match runtime
        .animation()
        .timeline()
        .start(
            AnimationPlan::track(
                other,
                Motion::Tween(Tween::between(0.0, 1.0, Duration::from_millis(100)).unwrap()),
            )
            .unwrap(),
        )
        .unwrap()
    {
        AnimationStart::Running(owner) => owner,
        other => panic!("{other:?}"),
    };
    runtime.animation().advance_time(time(25)).unwrap();
    assert_eq!(
        runtime.animation().timeline().state(owner),
        Ok(AnimationState::Failed)
    );
    assert!(
        matches!(runtime.animation().timeline().failure(owner).unwrap(),
        Some(AnimationError::TrackStorageExceeded { declared: bound, actual: sampled, .. })
        if *bound == declared && *sampled == actual)
    );
    assert_eq!(
        runtime.animation().timeline().state(healthy),
        Ok(AnimationState::Running)
    );
    assert_eq!(runtime.animation().timeline().value(other), Ok(0.25));
}

#[test]
fn explicit_geometry_endpoint_does_not_replace_committed_final_bounds() {
    let mut runtime = Runtime::empty();
    runtime.animation().advance_time(time(0)).unwrap();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(10.0, 0.0, 40.0, 20.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(root, GeometryBinding::layout_bounds())
        .unwrap();
    let plan = AnimationPlan::geometry_track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(100.0, 0.0, 40.0, 20.0)),
                bounds(Rectangle::new(50.0, 0.0, 40.0, 20.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    runtime.animation().timeline().start(plan).unwrap();
    runtime.animation().advance_time(time(50)).unwrap();
    assert_eq!(
        runtime
            .animation()
            .capture_geometry([root])
            .unwrap()
            .geometry()
            .figure(root)
            .unwrap()
            .bounds
            .x,
        75.0
    );
    assert_eq!(runtime.tree().figure_bounds(root).unwrap().x, 10.0);
    runtime.animation().advance_time(time(100)).unwrap();
    assert!(
        !runtime
            .animation()
            .timeline()
            .has_override(channel)
            .unwrap()
    );
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .value(channel)
            .unwrap()
            .rectangle()
            .x,
        10.0
    );
    let detached = runtime
        .animation()
        .timeline()
        .create_channel(1.0f64)
        .unwrap();
    let invalid = AnimationPlan::geometry_track(
        detached,
        Motion::Tween(Tween::between(0.0, 2.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    assert_eq!(
        runtime.animation().timeline().start(invalid),
        Err(AnimationError::InvalidComposition)
    );
}

#[test]
fn collapse_visibility_retention_does_not_trigger_pause_or_finish_suspension() {
    use novadraw::animation::{AnimationState, PresentedVisibility, SuspensionPolicy};
    for policy in [SuspensionPolicy::Pause, SuspensionPolicy::Finish] {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .unwrap();
        let contents = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(
                RectangleFigure::new(0.0, 0.0, 50.0, 50.0).expect("valid figure bounds"),
            ))
            .unwrap();
        let child = runtime
            .figure(contents)
            .unwrap()
            .add(Box::new(
                RectangleFigure::new(0.0, 0.0, 20.0, 20.0).expect("valid figure bounds"),
            ))
            .unwrap();
        let unrelated = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(
                RectangleFigure::new(60.0, 0.0, 10.0, 10.0).expect("valid figure bounds"),
            ))
            .unwrap();
        record_full_submission(&mut runtime);
        runtime
            .figure(contents)
            .unwrap()
            .set_visible(false)
            .unwrap();
        let visible = runtime
            .animation()
            .timeline()
            .bind_geometry(contents, GeometryBinding::visibility())
            .unwrap();
        let bounds = runtime
            .animation()
            .timeline()
            .bind_geometry(child, GeometryBinding::layout_bounds())
            .unwrap();
        let plan = AnimationPlan::parallel(vec![
            AnimationPlan::geometry_track(
                visible,
                Motion::Tween(
                    Tween::between(
                        PresentedVisibility(true),
                        PresentedVisibility(true),
                        Duration::from_millis(100),
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
            AnimationPlan::track(
                bounds,
                Motion::Tween(
                    Tween::between(
                        LayoutBounds::try_new(Rectangle::new(10.0, 0.0, 20.0, 20.0)).unwrap(),
                        LayoutBounds::try_new(Rectangle::new(0.0, 0.0, 20.0, 20.0)).unwrap(),
                        Duration::from_millis(100),
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
        ])
        .unwrap()
        .with_suspension(policy);
        let AnimationStart::Running(id) = runtime.animation().timeline().start(plan).unwrap()
        else {
            panic!("collapse group");
        };
        // An unrelated visibility mutation reconciles all groups without invalidating this
        // group's frozen subtree. Suspension must use the retained local visibility.
        runtime
            .figure(unrelated)
            .unwrap()
            .set_visible(false)
            .unwrap();
        runtime.animation().advance_time(time(0)).unwrap();
        record_full_submission(&mut runtime);
        runtime.animation().advance_time(time(50)).unwrap();
        let capture = runtime.animation().capture_geometry([root]).unwrap();
        assert!(capture.geometry().figure(child).unwrap().effective_visible);
        assert_eq!(capture.geometry().figure(child).unwrap().bounds.x, 5.0);
        runtime.animation().advance_time(time(100)).unwrap();
        assert_eq!(
            runtime.animation().timeline().state(id).unwrap(),
            AnimationState::Completed
        );
        assert!(!runtime.tree().is_effectively_visible(child).unwrap());
    }
}

#[test]
fn external_ancestor_hide_obeys_group_suspension_and_revokes_local_retention() {
    use novadraw::animation::{AnimationState, PresentedVisibility, SuspensionPolicy};
    for policy in [
        SuspensionPolicy::Advance,
        SuspensionPolicy::Pause,
        SuspensionPolicy::Finish,
    ] {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .unwrap();
        let child = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(
                RectangleFigure::new(0.0, 0.0, 20.0, 20.0).expect("valid figure bounds"),
            ))
            .unwrap();
        record_full_submission(&mut runtime);
        let channel = runtime
            .animation()
            .timeline()
            .bind_geometry(child, GeometryBinding::layout_bounds())
            .unwrap();
        let visible = runtime
            .animation()
            .timeline()
            .bind_geometry(child, GeometryBinding::visibility())
            .unwrap();
        let plan = AnimationPlan::parallel(vec![
            AnimationPlan::geometry_track(
                channel,
                Motion::Tween(
                    Tween::between(
                        LayoutBounds::try_new(Rectangle::new(10.0, 0.0, 20.0, 20.0)).unwrap(),
                        LayoutBounds::try_new(Rectangle::new(0.0, 0.0, 20.0, 20.0)).unwrap(),
                        Duration::from_millis(100),
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
            AnimationPlan::geometry_track(
                visible,
                Motion::Tween(
                    Tween::between(
                        PresentedVisibility(true),
                        PresentedVisibility(true),
                        Duration::from_millis(100),
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
        ])
        .unwrap()
        .with_suspension(policy);
        let AnimationStart::Running(owner) = runtime.animation().timeline().start(plan).unwrap()
        else {
            panic!("group");
        };
        runtime.animation().advance_time(time(0)).unwrap();
        runtime.animation().advance_time(time(25)).unwrap();
        runtime.figure(root).unwrap().set_visible(false).unwrap();
        let hidden = runtime.animation().capture_geometry([root]).unwrap();
        assert!(!hidden.geometry().figure(child).unwrap().effective_visible);
        runtime.animation().advance_time(time(75)).unwrap();
        match policy {
            SuspensionPolicy::Finish => assert_eq!(
                runtime.animation().timeline().state(owner).unwrap(),
                AnimationState::Completed
            ),
            SuspensionPolicy::Pause => assert_eq!(
                runtime.animation().timeline().state(owner).unwrap(),
                AnimationState::Paused
            ),
            SuspensionPolicy::Advance => assert_eq!(
                runtime.animation().timeline().state(owner).unwrap(),
                AnimationState::Running
            ),
            _ => unreachable!(),
        }
        runtime.figure(root).unwrap().set_visible(true).unwrap();
        runtime.animation().advance_time(time(100)).unwrap();
        if policy == SuspensionPolicy::Pause {
            assert_eq!(
                runtime.animation().timeline().state(owner).unwrap(),
                AnimationState::Running
            );
            runtime.animation().advance_time(time(150)).unwrap();
        }
        assert_eq!(
            runtime.animation().timeline().state(owner).unwrap(),
            AnimationState::Completed
        );
    }
}

#[test]
fn origin_takeover_materializes_latest_sample_and_failure_preserves_all_owners() {
    use novadraw::animation::{AnimationState, GeometryCaptureError, ViewportOrigin};
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let viewport = runtime
        .viewports()
        .add(root, Rectangle::new(0.0, 0.0, 50.0, 50.0))
        .unwrap();
    let contents = runtime
        .viewports()
        .set_contents(
            viewport.figure_id(),
            Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 500.0).expect("valid figure bounds")),
        )
        .unwrap();
    runtime
        .figure(contents)
        .unwrap()
        .set_preferred_size(novadraw::geometry::Dimension::new(500.0, 500.0))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    runtime
        .viewports()
        .set_view_location(viewport.figure_id(), 100.0, 100.0)
        .unwrap();
    let origin = runtime
        .animation()
        .timeline()
        .bind_geometry(
            viewport.figure_id(),
            GeometryBinding::viewport_origin(1.0).unwrap(),
        )
        .unwrap();
    let plan = AnimationPlan::geometry_track(
        origin,
        Motion::Tween(
            Tween::between(
                ViewportOrigin(Point::ZERO),
                ViewportOrigin(Point::new(100.0, 100.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let AnimationStart::Running(owner) = runtime.animation().timeline().start(plan).unwrap() else {
        panic!("group");
    };
    runtime.animation().advance_time(time(0)).unwrap();
    record_full_submission(&mut runtime);
    runtime.animation().advance_time(time(50)).unwrap();
    let sample = runtime.animation().timeline().sample_version();
    let mut limits = runtime.animation().timeline().limits();
    let old_limits = limits;
    limits.capture_bytes = 0;
    runtime.animation().timeline().set_limits(limits).unwrap();
    assert!(matches!(
        runtime.animation().capture_geometry([viewport.figure_id()]),
        Err(GeometryCaptureError::Animation(_))
    ));
    assert!(matches!(
        runtime
            .viewports()
            .scroll_by(viewport.figure_id(), 10.0, 20.0),
        Err(novadraw::runtime::RuntimeMutationError::Animation(_))
    ));
    assert_eq!(runtime.animation().timeline().sample_version(), sample);
    assert_eq!(
        runtime.animation().timeline().state(owner).unwrap(),
        AnimationState::Running
    );
    assert_eq!(viewport.view_location(), Point::new(100.0, 100.0));
    runtime
        .animation()
        .timeline()
        .set_limits(old_limits)
        .unwrap();
    runtime
        .viewports()
        .scroll_by(viewport.figure_id(), 10.0, 20.0)
        .unwrap();
    assert_eq!(viewport.view_location(), Point::new(60.0, 70.0));
    assert_eq!(
        runtime.animation().timeline().sample_version().time,
        sample.time
    );
    assert_eq!(
        runtime.animation().timeline().state(owner).unwrap(),
        AnimationState::Cancelled
    );
}

#[test]
fn bootstrap_root_does_not_clip_presentation_repair_envelopes_or_make_midframe_idle() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let child = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(0.0, 0.0, 20.0, 20.0).expect("valid figure bounds"),
        ))
        .unwrap();
    record_full_submission(&mut runtime);
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(child, GeometryBinding::layout_bounds())
        .unwrap();
    let plan = AnimationPlan::geometry_track(
        channel,
        Motion::Tween(
            Tween::between(
                LayoutBounds::try_new(Rectangle::new(10.0, 0.0, 20.0, 20.0)).unwrap(),
                LayoutBounds::try_new(Rectangle::new(0.0, 0.0, 20.0, 20.0)).unwrap(),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    runtime.animation().timeline().start(plan).unwrap();
    runtime.animation().advance_time(time(0)).unwrap();
    record_full_submission(&mut runtime);
    runtime.animation().advance_time(time(50)).unwrap();
    let capture = runtime.animation().capture_geometry([root]).unwrap();
    assert!(
        capture
            .geometry()
            .figure(child)
            .unwrap()
            .visual_envelope
            .is_some()
    );
    let submission = runtime
        .frames()
        .prepare_submission(
            novadraw::render::SurfaceInfo {
                logical_width: 100.0,
                logical_height: 100.0,
                pixel_width: 100,
                pixel_height: 100,
                scale_factor: 1.0,
            },
            novadraw::render::BackendCapabilities::RETAINED_PARTIAL,
        )
        .into_ready()
        .unwrap();
    assert!(!submission.damage.is_empty());
}

#[test]
fn legacy_contents_transform_rejects_new_origin_owner_without_double_pan() {
    use novadraw::animation::{AnimationState, InteractionGeometryPolicy, ViewportOrigin};
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let viewport = runtime
        .viewports()
        .add(root, Rectangle::new(0.0, 0.0, 50.0, 50.0))
        .unwrap();
    let contents = runtime
        .viewports()
        .set_contents(
            viewport.figure_id(),
            Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 500.0).expect("valid figure bounds")),
        )
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    let old_channel = runtime
        .animation()
        .timeline()
        .bind_figure(contents, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();
    let old_plan = AnimationPlan::track(
        old_channel,
        Motion::Tween(
            Tween::between(
                Affine2D::from_translation(-10.0, -20.0),
                Affine2D::IDENTITY,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let AnimationStart::Running(owner) = runtime.animation().timeline().start(old_plan).unwrap()
    else {
        panic!("owner");
    };
    runtime.animation().advance_time(time(0)).unwrap();
    let new_channel = runtime
        .animation()
        .timeline()
        .bind_geometry(
            viewport.figure_id(),
            GeometryBinding::viewport_origin(1.0).unwrap(),
        )
        .unwrap();
    let new_plan = AnimationPlan::geometry_track(
        new_channel,
        Motion::Tween(
            Tween::between(
                ViewportOrigin(Point::ZERO),
                ViewportOrigin(Point::new(20.0, 20.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    assert_eq!(
        runtime.animation().timeline().start(new_plan),
        Err(AnimationError::ChannelConflict)
    );
    assert_eq!(
        runtime.animation().timeline().state(owner).unwrap(),
        AnimationState::Running
    );
    assert_eq!(runtime.animation().timeline().active_animation_count(), 1);
}

#[test]
fn inflight_capture_and_reset_keep_latest_sample_and_do_not_restore_old_geometry() {
    use novadraw::render::{BackendCapabilities, RenderDisposition, SurfaceInfo};
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(100.0, 0.0, 20.0, 20.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(root, GeometryBinding::layout_bounds())
        .unwrap();
    let plan = AnimationPlan::geometry_track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 20.0, 20.0)),
                bounds(Rectangle::new(100.0, 0.0, 20.0, 20.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    runtime.animation().timeline().start(plan).unwrap();
    let unanchored = runtime.animation().capture_geometry([root]).unwrap();
    assert_eq!(runtime.animation().timeline().sample_version().time, None);
    assert_eq!(unanchored.geometry().figure(root).unwrap().bounds.x, 0.0);
    runtime.animation().advance_time(time(0)).unwrap();
    let surface = SurfaceInfo {
        logical_width: 200.0,
        logical_height: 100.0,
        pixel_width: 200,
        pixel_height: 100,
        scale_factor: 1.0,
    };
    let first = runtime
        .frames()
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    runtime.animation().advance_time(time(50)).unwrap();
    let current = runtime.animation().capture_geometry([root]).unwrap();
    assert_eq!(current.geometry().figure(root).unwrap().bounds.x, 50.0);
    let _ = runtime
        .frames()
        .complete_submission(
            first.session_id,
            first.frame_id,
            Ok(RenderDisposition::Retry),
        )
        .unwrap();
    runtime.animation().advance_time(time(75)).unwrap();
    runtime.frames().reset_backend_session().unwrap();
    let sample = runtime.animation().timeline().sample_version();
    let after = runtime.animation().capture_geometry([root]).unwrap();
    assert_eq!(after.geometry().figure(root).unwrap().bounds.x, 75.0);
    assert_eq!(runtime.animation().timeline().sample_version(), sample);
    let retry = runtime
        .frames()
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_ne!(retry.session_id, first.session_id);
    assert_eq!(runtime.animation().timeline().sample_version(), sample);
    assert_eq!(current.geometry().figure(root).unwrap().bounds.x, 50.0);
}

#[test]
fn overlapping_nested_targets_materialize_linear_geometry_work() {
    for count in [8, 32, 128] {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .unwrap();
        let mut targets = vec![root];
        for _ in 1..count {
            let parent = *targets.last().unwrap();
            let child = runtime
                .figure(parent)
                .unwrap()
                .add(Box::new(
                    RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
                ))
                .unwrap();
            targets.push(child);
        }
        let capture = runtime.animation().capture_geometry(targets).unwrap();
        let work = capture.geometry().materialization_work();
        assert_eq!(work.nodes, count + 1);
        assert_eq!(work.edges, count);
        assert!(work.matrices <= 96 * work.nodes + 3 * work.edges);
        assert!(work.clips <= 3 * work.nodes);
        assert!(work.envelopes <= 3 * work.nodes);
    }
}

#[test]
fn sampled_geometry_surface_pause_resumes_without_advancing_local_motion() {
    use novadraw::{
        animation::{AnimationState, SuspensionPolicy},
        render::{BackendCapabilities, SurfaceInfo},
        runtime::FramePreparation,
    };
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(100.0, 0.0, 20.0, 20.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let channel = runtime
        .animation()
        .timeline()
        .bind_geometry(root, GeometryBinding::layout_bounds())
        .unwrap();
    runtime.animation().advance_time(time(0)).unwrap();
    let plan = AnimationPlan::geometry_track(
        channel,
        Motion::Tween(
            Tween::between(
                bounds(Rectangle::new(0.0, 0.0, 20.0, 20.0)),
                bounds(Rectangle::new(100.0, 0.0, 20.0, 20.0)),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap()
    .with_suspension(SuspensionPolicy::Pause);
    let AnimationStart::Running(owner) = runtime.animation().timeline().start(plan).unwrap() else {
        panic!("owner");
    };
    assert!(matches!(
        runtime.frames().prepare_submission(
            SurfaceInfo {
                logical_width: 0.0,
                logical_height: 100.0,
                pixel_width: 0,
                pixel_height: 100,
                scale_factor: 1.0
            },
            BackendCapabilities::RETAINED_PARTIAL,
        ),
        FramePreparation::Suspended
    ));
    runtime.animation().advance_time(time(100)).unwrap();
    assert_eq!(
        runtime.animation().timeline().state(owner).unwrap(),
        AnimationState::Paused
    );
    assert_eq!(
        runtime
            .animation()
            .capture_geometry([root])
            .unwrap()
            .geometry()
            .figure(root)
            .unwrap()
            .bounds
            .x,
        0.0
    );
    record_full_submission(&mut runtime);
    runtime.animation().advance_time(time(150)).unwrap();
    assert_eq!(
        runtime
            .animation()
            .capture_geometry([root])
            .unwrap()
            .geometry()
            .figure(root)
            .unwrap()
            .bounds
            .x,
        50.0
    );
}
