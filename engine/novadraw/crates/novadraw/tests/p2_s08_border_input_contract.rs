// Public consumers exercise checked Border inputs without private access.
use novadraw::{
    figure::{
        BoundsDrawing, RectangleFigure,
        border::{
            BevelBorder, BevelStyle, Border, BorderBuilder, BorderInputError, BorderSide,
            CompoundBorder, EtchedBorder, LineBorder, MarginBorder, RectangleBorder, SchemeBorder,
            TitleBarBorder,
        },
    },
    geometry::{Dimension, Insets, Rectangle},
    graphics::{Color, CustomDash, DashPattern, GraphicsError, PaintContext, StrokeStyle},
    render::{BackendCapabilities, RenderCommandKind, RenderDisposition, SurfaceInfo},
    runtime::{FramePreparation, ReplaceFigureBorder, Runtime},
};
use std::{
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

fn stroke(width: f64) -> StrokeStyle {
    StrokeStyle::default().with_width(width).unwrap()
}

#[test]
fn direct_and_builder_auto_insets_follow_stroke_while_explicit_insets_remain() {
    let explicit = Insets::new(0.0, 1.0, 2.0, 3.0);
    let direct = LineBorder::new(Color::BLACK, 2.0).unwrap();
    let built = BorderBuilder::new(Color::BLACK, 2.0).unwrap().build_line();
    assert_eq!(direct, built);
    assert_eq!(direct.get_insets(), Insets::uniform(2.0));
    let updated = direct.clone().with_stroke(stroke(4.0)).unwrap();
    assert_eq!(updated.get_insets(), Insets::uniform(4.0));
    let explicit_direct = direct
        .with_insets(explicit)
        .unwrap()
        .with_stroke(stroke(4.0))
        .unwrap();
    let explicit_built = BorderBuilder::new(Color::BLACK, 2.0)
        .unwrap()
        .with_insets(explicit)
        .unwrap()
        .with_stroke(stroke(4.0))
        .unwrap()
        .build_line();
    assert_eq!(explicit_direct, explicit_built);
    assert_eq!(explicit_direct.get_insets(), explicit);
    assert_eq!(explicit_direct.get_width(), 4.0);

    let direct = RectangleBorder::from_stroke(Color::RED, stroke(2.0)).unwrap();
    let built = BorderBuilder::from_stroke(Color::RED, stroke(2.0))
        .unwrap()
        .build_rectangle();
    assert_eq!(direct, built);
    assert_eq!(
        direct
            .clone()
            .with_stroke(stroke(5.0))
            .unwrap()
            .get_insets(),
        Insets::uniform(5.0)
    );
    assert_eq!(
        direct
            .with_insets(explicit)
            .unwrap()
            .with_stroke(stroke(5.0))
            .unwrap()
            .get_insets(),
        explicit
    );
}

#[test]
fn all_width_entries_reject_zero_nonfinite_negative_and_derived_overflow() {
    for width in [
        0.0,
        -1.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MAX,
    ] {
        assert!(LineBorder::new(Color::BLACK, width).is_err());
        assert!(RectangleBorder::new(Color::BLACK, width).is_err());
        assert!(BorderBuilder::new(Color::BLACK, width).is_err());
    }
    let zero = stroke(0.0);
    assert_eq!(
        LineBorder::from_stroke(Color::BLACK, zero.clone()).unwrap_err(),
        BorderInputError::NonPositiveWidth
    );
    assert!(
        RectangleBorder::default_border()
            .with_stroke(zero.clone())
            .is_err()
    );
    assert!(BorderBuilder::from_stroke(Color::BLACK, zero.clone()).is_err());
    assert!(
        BorderBuilder::new(Color::BLACK, 1.0)
            .unwrap()
            .with_stroke(zero)
            .is_err()
    );
    let error = LineBorder::new(Color::BLACK, f64::MAX).unwrap_err();
    assert!(
        error
            .source()
            .unwrap()
            .is::<novadraw::graphics::GraphicsInputError>()
    );
    // Graphics retains its independent zero-width contract.
    let mut recorder = novadraw::render::CommandRecorder::new();
    recorder.paint().set_stroke_width(0.0).unwrap();
}

#[test]
fn each_inset_side_and_sum_is_checked_across_all_configuration_entries() {
    for (index, side) in [
        BorderSide::Top,
        BorderSide::Left,
        BorderSide::Bottom,
        BorderSide::Right,
    ]
    .into_iter()
    .enumerate()
    {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
            let mut values = [0.0; 4];
            values[index] = value;
            let insets = Insets::new(values[0], values[1], values[2], values[3]);
            let error = MarginBorder::new(insets).unwrap_err();
            assert!(matches!(error,
                BorderInputError::NonFinite { side: Some(actual), .. } |
                BorderInputError::Negative { side: Some(actual), .. } if actual == side));
            assert!(LineBorder::default_border().with_insets(insets).is_err());
            assert!(
                RectangleBorder::default_border()
                    .with_insets(insets)
                    .is_err()
            );
            assert!(
                RectangleBorder::default_border()
                    .with_corners(Default::default())
                    .with_insets(insets)
                    .is_err()
            );
            assert!(
                BorderBuilder::new(Color::BLACK, 1.0)
                    .unwrap()
                    .with_insets(insets)
                    .is_err()
            );
        }
    }
    for insets in [
        Insets::new(f64::MAX, 0.0, f64::MAX, 0.0),
        Insets::new(0.0, f64::MAX, 0.0, f64::MAX),
    ] {
        assert!(matches!(
            MarginBorder::new(insets),
            Err(BorderInputError::Overflow { .. })
        ));
        assert!(LineBorder::default_border().with_insets(insets).is_err());
        assert!(
            RectangleBorder::default_border()
                .with_insets(insets)
                .is_err()
        );
        assert!(
            BorderBuilder::new(Color::BLACK, 1.0)
                .unwrap()
                .with_insets(insets)
                .is_err()
        );
    }
    let margin = MarginBorder::new(Insets::new(f64::MAX, f64::MAX, 0.0, 0.0)).unwrap();
    assert!(margin.with_bottom(f64::MAX).is_err());
    assert!(margin.with_right(f64::MAX).is_err());
    assert_eq!(
        margin.get_insets(),
        Insets::new(f64::MAX, f64::MAX, 0.0, 0.0)
    );
    assert!(
        MarginBorder::new(Insets::new(0.0, 0.0, f64::MAX, f64::MAX))
            .unwrap()
            .with_top(f64::MAX)
            .is_err()
    );
    assert!(
        MarginBorder::new(Insets::new(0.0, 0.0, f64::MAX, f64::MAX))
            .unwrap()
            .with_left(f64::MAX)
            .is_err()
    );
}

#[test]
fn padding_is_checked_before_any_title_measurement() {
    for (horizontal, vertical) in [
        (f64::NAN, 1.0),
        (1.0, f64::INFINITY),
        (-1.0, 1.0),
        (1.0, -1.0),
        (f64::MAX, 0.0),
        (0.0, f64::MAX),
    ] {
        assert!(
            TitleBarBorder::new("title", Color::WHITE)
                .with_padding(horizontal, vertical)
                .is_err()
        );
    }
    assert!(
        TitleBarBorder::new("title", Color::WHITE)
            .with_padding(0.0, 0.0)
            .is_ok()
    );
    assert!(BevelBorder::new(BevelStyle::Raised, Color::WHITE, Color::BLACK, 0).is_err());
}

#[derive(Clone)]
struct ExternalBorder {
    metrics: Arc<Mutex<(Insets, Dimension)>>,
    insets_calls: Arc<AtomicUsize>,
    preferred_calls: Arc<AtomicUsize>,
    owner_calls: Arc<AtomicUsize>,
}
impl ExternalBorder {
    fn new(insets: Insets, preferred: Dimension) -> Self {
        Self {
            metrics: Arc::new(Mutex::new((insets, preferred))),
            insets_calls: Arc::default(),
            preferred_calls: Arc::default(),
            owner_calls: Arc::default(),
        }
    }
}
impl Border for ExternalBorder {
    fn get_insets(&self) -> Insets {
        self.insets_calls.fetch_add(1, Ordering::Relaxed);
        self.metrics.lock().unwrap().0
    }
    fn preferred_size(&self) -> Dimension {
        self.preferred_calls.fetch_add(1, Ordering::Relaxed);
        self.metrics.lock().unwrap().1
    }
    fn paint(&self, _: Rectangle, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        Ok(())
    }
    fn resolve_owner_snapshot(
        &self,
        _: Option<&novadraw::figure::border::BorderSnapshot>,
        _: &novadraw::text::FontDescriptor,
        _: &mut dyn novadraw::render::TextLayoutEngine,
    ) -> Result<Option<novadraw::figure::border::BorderSnapshot>, novadraw::text::TextError> {
        self.owner_calls.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }
}

#[test]
fn compound_captures_each_direct_base_metric_once_and_does_not_measure_an_owner() {
    let outer = ExternalBorder::new(Insets::uniform(2.0), Dimension::new(3.0, 4.0));
    let inner = ExternalBorder::new(Insets::uniform(1.0), Dimension::new(10.0, 20.0));
    let compound = CompoundBorder::new(outer.clone(), inner.clone()).unwrap();
    for border in [&outer, &inner] {
        assert_eq!(border.insets_calls.load(Ordering::Relaxed), 1);
        assert_eq!(border.preferred_calls.load(Ordering::Relaxed), 1);
        assert_eq!(border.owner_calls.load(Ordering::Relaxed), 0);
    }
    assert_eq!(compound.get_insets(), Insets::uniform(3.0));
    assert_eq!(compound.preferred_size(), Dimension::new(14.0, 24.0));
    *outer.metrics.lock().unwrap() = (Insets::uniform(f64::NAN), Dimension::ZERO);
    let error = CompoundBorder::new(compound, MarginBorder::default())
        .err()
        .unwrap();
    assert!(matches!(
        error,
        BorderInputError::Child { role: "outer", .. }
    ));
    assert!(error.source().unwrap().is::<BorderInputError>());
}

#[test]
fn compound_rejects_invalid_child_metrics_and_finite_composition_overflow() {
    for (insets, preferred) in [
        (Insets::uniform(-1.0), Dimension::ZERO),
        (Insets::ZERO, Dimension::new(f64::NAN, 0.0)),
        (Insets::ZERO, Dimension::new(0.0, -1.0)),
    ] {
        for role in ["outer", "inner"] {
            let bad: Arc<dyn Border> = Arc::new(ExternalBorder::new(insets, preferred));
            let error = if role == "outer" {
                CompoundBorder::from_optional(Some(bad), None)
            } else {
                CompoundBorder::from_optional(None, Some(bad))
            }
            .err()
            .unwrap();
            assert!(
                matches!(error, BorderInputError::Child { role: actual, .. } if actual == role)
            );
        }
    }
    let huge = || MarginBorder::new(Insets::new(f64::MAX, 0.0, 0.0, 0.0)).unwrap();
    assert!(matches!(
        CompoundBorder::new(huge(), huge()),
        Err(BorderInputError::Overflow { .. })
    ));
    let outer = ExternalBorder::new(Insets::new(0.0, f64::MAX, 0.0, 0.0), Dimension::ZERO);
    let inner = ExternalBorder::new(Insets::ZERO, Dimension::new(f64::MAX, 0.0));
    assert!(matches!(
        CompoundBorder::new(outer, inner),
        Err(BorderInputError::Overflow {
            field: "compound preferred width"
        })
    ));
}

#[test]
fn fixed_and_empty_constructors_keep_margin_optional_and_scheme_semantics() {
    assert_eq!(MarginBorder::default().get_insets(), Insets::ZERO);
    assert_eq!(CompoundBorder::default().get_insets(), Insets::ZERO);
    assert_eq!(
        CompoundBorder::from_optional(None, None)
            .unwrap()
            .preferred_size(),
        Dimension::ZERO
    );
    assert_eq!(
        TitleBarBorder::new("", Color::WHITE).get_insets(),
        Insets::ZERO
    );
    assert_eq!(
        EtchedBorder::new(Color::WHITE, Color::BLACK).get_insets(),
        Insets::uniform(2.0)
    );
    assert_eq!(SchemeBorder::new(vec![], vec![]).get_insets(), Insets::ZERO);
    assert_eq!(
        SchemeBorder::new(vec![None], vec![]).get_insets(),
        Insets::new(1.0, 1.0, 0.0, 0.0)
    );
}

fn submission(runtime: &mut Runtime) -> novadraw::render::RenderSubmission {
    runtime.frames().request_full_redraw();
    let FramePreparation::Ready(submission) = runtime.frames().prepare_submission(
        SurfaceInfo {
            logical_width: 100.0,
            logical_height: 80.0,
            pixel_width: 100,
            pixel_height: 80,
            scale_factor: 1.0,
        },
        BackendCapabilities::RETAINED_PARTIAL.with_custom_strokes(),
    ) else {
        panic!("expected ready border drawing");
    };
    let _ = runtime
        .frames()
        .complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .unwrap();
    submission
}

#[test]
fn all_dash_patterns_reach_public_graphics() {
    for dash in [
        DashPattern::Solid,
        DashPattern::Dash,
        DashPattern::Dot,
        DashPattern::Custom(CustomDash::try_new(&[3.0, 1.0, 1.0, 1.0]).unwrap()),
    ] {
        let stroke = stroke(2.0).with_dash_pattern(dash);
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 80.0)
                    .expect("valid figure bounds")
                    .with_border(LineBorder::from_stroke(Color::BLACK, stroke.clone()).unwrap())
                    .expect("valid detached figure input"),
            ))
            .unwrap();
        let submission = submission(&mut runtime);
        assert!(
            submission
                .commands
                .iter()
                .any(|command| matches!(&command.kind,
            RenderCommandKind::StrokeRect { stroke: actual, .. } if actual == &stroke))
        );
    }
}

#[test]
fn rejected_configuration_preserves_mounted_border_and_valid_replacement_recovers() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 80.0)
                .expect("valid figure bounds")
                .with_border(LineBorder::new(Color::BLACK, 2.0).unwrap())
                .expect("valid detached figure input"),
        ))
        .unwrap();
    let before = submission(&mut runtime);
    assert!(LineBorder::new(Color::RED, f64::NAN).is_err());
    assert_eq!(runtime.tree().insets(root), Ok(Insets::uniform(2.0)));
    assert!(!runtime.frames().has_pending_update());
    runtime
        .figure(root)
        .unwrap()
        .update_capability(ReplaceFigureBorder::new(Some(Arc::new(
            LineBorder::new(Color::RED, 3.0).unwrap(),
        ))))
        .unwrap();
    assert_eq!(runtime.tree().insets(root), Ok(Insets::uniform(3.0)));
    assert_ne!(before.frame_id, submission(&mut runtime).frame_id);
}

#[test]
fn external_painter_uses_the_same_bounded_stroke_freezing_entry() {
    use novadraw::{
        animation::{AnimationPlan, GeometryBinding, LayoutBounds, Motion, Tween},
        event::MonotonicTime,
        figure::{
            BOUNDS_SNAPSHOT, BoundsSnapshotCapability, Figure, FigureBoundsSnapshot,
            FigureCapabilityBuilder, FigureCapabilityRegistrationError,
        },
    };
    struct ExternalPainter(StrokeStyle);
    impl Figure for ExternalPainter {
        fn name(&self) -> &'static str {
            "ExternalStrokedPainter"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 80.0)
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(BOUNDS_SNAPSHOT, BoundsSnapshotCapability::of::<Self>())
        }
    }
    impl FigureBoundsSnapshot for ExternalPainter {
        fn snapshot_bounds_drawing(&self) -> Result<BoundsDrawing, GraphicsError> {
            BoundsDrawing::stroked(
                Color::BLACK,
                self.0.clone(),
                4096,
                |color, stroke, bounds, context| {
                    context.set_stroke_paint(*color);
                    context.set_stroke_style(stroke.clone());
                    context.stroke_rect(bounds)
                },
                |_, _, bounds| Ok(bounds),
            )
        }
    }
    let stroke = stroke(2.0).with_dash_pattern(DashPattern::Custom(
        CustomDash::try_new(&[3.0, 1.0]).unwrap(),
    ));
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(ExternalPainter(stroke.clone())))
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
                LayoutBounds::try_new(Rectangle::new(0.0, 0.0, 20.0, 20.0)).unwrap(),
                LayoutBounds::try_new(Rectangle::new(0.0, 0.0, 100.0, 80.0)).unwrap(),
                std::time::Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    runtime.animation().timeline().start(plan).unwrap();
    runtime
        .animation()
        .advance_time(MonotonicTime::from_micros(0))
        .unwrap();
    runtime
        .animation()
        .advance_time(MonotonicTime::from_micros(50_000))
        .unwrap();
    let frame = submission(&mut runtime);
    assert!(frame.commands.iter().any(|command| matches!(&command.kind,
        RenderCommandKind::StrokeRect { rect, stroke: actual, .. }
        if *rect == Rectangle::new(0.0, 0.0, 60.0, 50.0) && actual == &stroke)));
    assert!(
        BoundsDrawing::stroked(
            (),
            stroke,
            usize::MAX,
            |_, _, _, _| Ok(()),
            |_, _, bounds| Ok(bounds)
        )
        .is_err()
    );
}
