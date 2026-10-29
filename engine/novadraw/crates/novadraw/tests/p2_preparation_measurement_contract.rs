//! External producer uses only public preparation and measurement contracts.
use novadraw::{
    figure::{
        CONTAINER, ContainerCapability, Figure, FigureCapabilityBuilder,
        FigureCapabilityRegistrationError, FigureContainer, FigureDrawing, FigureMeasurement,
        FigurePreparation, FigurePresentation, MeasureConstraints, PREPARATION,
        PreparationCapability, PreparationMeasureError, PreparationMeasureErrorKind,
        PreparationMeasurement, PreparationMetric, PreparationScene, PreparedScene,
        border::{Border, MarginBorder},
    },
    geometry::{Dimension, Insets, Rectangle},
    graphics::{GraphicsError, PaintContext},
    layout::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery, StackLayout},
    render::{BackendCapabilities, SurfaceInfo},
    runtime::{FramePreparation, FramePreparationError, Runtime},
    text::{MeasureContext, TextError},
    tree::{FigureId, FigureTree},
};
use std::{
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Empty;
impl FigureDrawing for Empty {
    fn paint(&self, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        Ok(())
    }
}
fn presentation() -> FigurePresentation {
    FigurePresentation::new(
        FigureMeasurement::new(10.0, 10.0, None),
        Dimension::ZERO,
        Rectangle::ZERO,
        Arc::new(Empty),
    )
    .unwrap()
}

struct DefaultUnsupported;
impl Figure for DefaultUnsupported {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "DefaultUnsupported"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigurePreparation for DefaultUnsupported {
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        Ok(presentation())
    }
}

type MeasurementLog = Arc<Mutex<Vec<Result<PreparationMeasurement, PreparationMeasureError>>>>;
struct Probe(MeasurementLog);
impl Figure for Probe {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 120.0, 80.0)
    }
    fn name(&self) -> &'static str {
        "MeasurementProbe"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigurePreparation for Probe {
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        Ok(presentation())
    }
    fn prepare_scene(
        &self,
        context: &mut MeasureContext<'_>,
        _: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        let child = scene.children(scene.root())?[0];
        for width in [40.0, 80.0] {
            let measured =
                scene.measure_figure(context, child, MeasureConstraints::width(width).unwrap());
            self.0.lock().unwrap().push(measured.clone());
            measured?;
        }
        Ok(PreparedScene::new(presentation()))
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Supported,
    Unsupported,
    InvalidSize,
    InvalidBaseline,
    Text,
    Recursive,
    Budget,
}
struct External {
    mode: Mode,
    calls: Arc<AtomicUsize>,
    border: Option<Arc<dyn Border>>,
    scale: f64,
}
impl Figure for External {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 10.0, 10.0)
    }
    fn name(&self) -> &'static str {
        "ExternalMeasured"
    }
    fn initial_insets(&self) -> novadraw::geometry::Insets {
        self.border
            .as_deref()
            .map_or(novadraw::geometry::Insets::ZERO, Border::get_insets)
    }
    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(PREPARATION, PreparationCapability::of::<Self>())?;
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}
impl FigureContainer for External {
    fn layout_constraints(&self, constraints: MeasureConstraints) -> MeasureConstraints {
        MeasureConstraints::new(
            constraints.max_width().map(|width| width / self.scale),
            constraints.max_height().map(|height| height / self.scale),
        )
        .unwrap()
    }
    fn project_preferred_measurement(&self, measurement: FigureMeasurement) -> FigureMeasurement {
        FigureMeasurement::new(
            measurement.width * self.scale,
            measurement.height * self.scale,
            measurement.baseline.map(|baseline| baseline * self.scale),
        )
    }
    fn project_minimum_size(&self, size: Dimension) -> Dimension {
        Dimension::new(size.width * self.scale, size.height * self.scale)
    }
}
impl FigurePreparation for External {
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        Ok(presentation())
    }
    fn measure_scene(
        &self,
        context: &mut MeasureContext<'_>,
        constraints: MeasureConstraints,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let (outer, content) = scene.measurement_constraints().unwrap();
        assert_eq!(content, constraints);
        let failure = |kind| PreparationMeasureError {
            figure: scene.root(),
            outer_constraints: outer,
            content_constraints: Some(content),
            kind,
        };
        let width = constraints.max_width().unwrap_or(100.0);
        match self.mode {
            Mode::Unsupported => {
                return Err(failure(
                    PreparationMeasureErrorKind::UnsupportedConstraintMeasurement,
                ));
            }
            Mode::Text => {
                return Err(failure(PreparationMeasureErrorKind::Text(Box::new(
                    TextError::NoUsableFont,
                ))));
            }
            Mode::Recursive => return scene.measure_figure(context, scene.root(), constraints),
            Mode::Budget => {
                let budget = scene.budget();
                let _ = budget.consume("external measurement", budget.limit().work);
            }
            _ => {}
        }
        Ok(PreparationMeasurement {
            preferred: FigureMeasurement::new(
                if matches!(self.mode, Mode::InvalidSize) {
                    f64::NAN
                } else {
                    width
                },
                400.0 / width,
                if matches!(self.mode, Mode::InvalidBaseline) {
                    Some(100.0)
                } else {
                    None
                },
            ),
            minimum: Dimension::new(5.0, 2.0),
        })
    }
}

fn fixture(mode: Mode) -> (FigureTree, FigureId, MeasurementLog, Arc<AtomicUsize>) {
    projection_fixture(mode, None, 1.0)
}
fn projection_fixture(
    mode: Mode,
    border: Option<Arc<dyn Border>>,
    scale: f64,
) -> (FigureTree, FigureId, MeasurementLog, Arc<AtomicUsize>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(Probe(log.clone())))
        .unwrap();
    let child = tree
        .builder()
        .add_child(
            root,
            Box::new(External {
                mode,
                calls: calls.clone(),
                border,
                scale,
            }),
        )
        .unwrap();
    (tree, child, log, calls)
}
fn run(tree: FigureTree) -> Result<(), FramePreparationError> {
    match Runtime::new(tree).frames().prepare_submission(
        SurfaceInfo {
            logical_width: 120.0,
            logical_height: 80.0,
            pixel_width: 120,
            pixel_height: 80,
            scale_factor: 1.0,
        },
        BackendCapabilities::RETAINED_PARTIAL,
    ) {
        FramePreparation::Ready(_) => Ok(()),
        FramePreparation::Error(error) => Err(error),
        other => panic!("unexpected frame state: {other:?}"),
    }
}

struct TextProbe(MeasurementLog);
impl Figure for TextProbe {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 400.0, 300.0)
    }
    fn name(&self) -> &'static str {
        "TextProbe"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigurePreparation for TextProbe {
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        Ok(presentation())
    }
    fn prepare_scene(
        &self,
        context: &mut MeasureContext<'_>,
        _: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        let child = scene.children(scene.root())?[0];
        for constraints in [
            MeasureConstraints::UNBOUNDED,
            MeasureConstraints::width(80.0).unwrap(),
            MeasureConstraints::width(160.0).unwrap(),
        ] {
            let result = scene.measure_figure(context, child, constraints);
            self.0.lock().unwrap().push(result.clone());
            result?;
        }
        Ok(PreparedScene::new(presentation()))
    }
}

fn text_fixture(figure: impl Figure + 'static) -> (Runtime, FigureId, MeasurementLog) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(TextProbe(log.clone())))
        .unwrap();
    let child = tree.builder().add_child(root, Box::new(figure)).unwrap();
    let mut runtime = Runtime::new(tree);
    runtime
        .resources()
        .register_builtin_font(novadraw::text::BuiltinFont::Inter)
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_style(novadraw::figure::FigureStyle {
            font: Some(novadraw::text::FontDescriptor::parse("16px Inter").unwrap()),
            ..Default::default()
        })
        .unwrap();
    (runtime, child, log)
}

fn prepare_text(runtime: &mut Runtime) -> Result<(), FramePreparationError> {
    match runtime.frames().prepare_submission(
        SurfaceInfo {
            logical_width: 400.0,
            logical_height: 300.0,
            pixel_width: 400,
            pixel_height: 300,
            scale_factor: 1.0,
        },
        BackendCapabilities::RETAINED_PARTIAL,
    ) {
        FramePreparation::Ready(_) => Ok(()),
        FramePreparation::Error(error) => Err(error),
        other => panic!("unexpected text frame: {other:?}"),
    }
}

const WRAPPING_TEXT: &str = "alpha beta gamma delta epsilon zeta eta theta";

#[test]
fn nested_layout_remeasures_text_flow_in_the_authoritative_session() {
    use novadraw::figure::{FlowPage, TextFlowFigure};
    let calls = Arc::new(AtomicUsize::new(0));
    let (mut runtime, container, log) = text_fixture(External {
        mode: Mode::Unsupported,
        calls: calls.clone(),
        border: Some(Arc::new(
            MarginBorder::new(Insets::new(2.0, 3.0, 5.0, 7.0)).unwrap(),
        )),
        scale: 1.0,
    });
    runtime
        .figure(container)
        .unwrap()
        .set_layout_manager(Box::new(StackLayout::new()))
        .unwrap();
    runtime
        .figure(container)
        .unwrap()
        .add(Box::new(
            TextFlowFigure::new(
                Rectangle::new(0.0, 0.0, 300.0, 200.0),
                FlowPage::from_text(WRAPPING_TEXT),
            )
            .expect("valid detached figure input"),
        ))
        .unwrap();
    prepare_text(&mut runtime).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let log = log.lock().unwrap();
    let narrow = log[1].as_ref().unwrap();
    let wide = log[2].as_ref().unwrap();
    assert!(narrow.preferred.height > wide.preferred.height);
    assert!(wide.preferred.height > log[0].as_ref().unwrap().preferred.height);
    assert_eq!(narrow.minimum, narrow.preferred.size());
}

#[test]
fn structured_flow_measures_unbounded_and_candidate_width_without_mutating_attached_bounds() {
    use novadraw::figure::flow::{FlowPageFigure, StructuredTextFigure};
    let initial = Rectangle::new(0.0, 0.0, 300.0, 200.0);
    let (mut runtime, page, log) = text_fixture(
        FlowPageFigure::new(initial)
            .expect("valid detached figure input")
            .with_border(MarginBorder::new(Insets::new(2.0, 3.0, 5.0, 7.0)).unwrap())
            .expect("valid detached figure input"),
    );
    runtime
        .figure(page)
        .unwrap()
        .add(Box::new(StructuredTextFigure::new(WRAPPING_TEXT)))
        .unwrap();
    prepare_text(&mut runtime).unwrap();
    let log = log.lock().unwrap();
    let measurements = log
        .iter()
        .take(3)
        .map(|result| result.as_ref().unwrap())
        .collect::<Vec<_>>();
    assert!(measurements[0].preferred.width > measurements[2].preferred.width);
    assert!(measurements[1].preferred.height > measurements[2].preferred.height);
    for measured in &measurements {
        assert_eq!(measured.minimum, measured.preferred.size());
    }
    assert_eq!(runtime.tree().node(page).unwrap().bounds(), initial);
}

#[test]
fn text_flow_candidate_measurement_uses_wrapping_and_truncation_algorithm() {
    use novadraw::figure::{FlowPage, FlowWrapping, TextFlowFigure};
    for wrapping in [
        FlowWrapping::SoftWrap,
        FlowWrapping::NoWrap,
        FlowWrapping::Truncate { max_lines: 1 },
    ] {
        let (mut runtime, _, log) = text_fixture(
            TextFlowFigure::new(
                Rectangle::new(0.0, 0.0, 300.0, 200.0),
                FlowPage::from_text(WRAPPING_TEXT),
            )
            .expect("valid detached figure input")
            .with_wrapping(wrapping),
        );
        prepare_text(&mut runtime).unwrap();
        let log = log.lock().unwrap();
        let unbounded = log[0].as_ref().unwrap();
        let narrow = log[1].as_ref().unwrap();
        let wide = log[2].as_ref().unwrap();
        match wrapping {
            FlowWrapping::SoftWrap => {
                assert!(narrow.preferred.height > wide.preferred.height);
                assert!(wide.preferred.height > unbounded.preferred.height);
            }
            FlowWrapping::NoWrap => assert_eq!(narrow, unbounded),
            FlowWrapping::Truncate { .. } => {
                assert_eq!(narrow.preferred.height, unbounded.preferred.height);
                assert!(narrow.preferred.width <= 80.0);
            }
        }
        assert!(narrow.preferred.baseline.unwrap() <= narrow.preferred.height);
        assert_eq!(narrow.minimum, narrow.preferred.size());
    }
}

#[test]
fn structured_flow_inline_measurement_preserves_unsupported_leaf_error() {
    use novadraw::figure::flow::{FlowAdapterFigure, FlowPageFigure};
    let (mut runtime, page, log) = text_fixture(
        FlowPageFigure::new(Rectangle::new(0.0, 0.0, 300.0, 200.0))
            .expect("valid detached figure input"),
    );
    let adapter = runtime
        .figure(page)
        .unwrap()
        .add(Box::new(FlowAdapterFigure::new()))
        .unwrap();
    let leaf = runtime
        .figure(adapter)
        .unwrap()
        .add(Box::new(DefaultUnsupported))
        .unwrap();
    prepare_text(&mut runtime).unwrap_err();
    let log = log.lock().unwrap();
    let error = log[0].as_ref().unwrap_err();
    assert_eq!(error.figure, leaf);
    assert_eq!(error.outer_constraints, MeasureConstraints::UNBOUNDED);
    assert_eq!(
        error.kind,
        PreparationMeasureErrorKind::UnsupportedConstraintMeasurement
    );
}

#[test]
fn ignored_consumer_budget_failure_rejects_publication_and_next_session_recovers() {
    struct ExhaustOnce(Arc<AtomicUsize>);
    impl Figure for ExhaustOnce {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 120.0, 80.0)
        }
        fn name(&self) -> &'static str {
            "ExhaustOnce"
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(PREPARATION, PreparationCapability::of::<Self>())
        }
    }
    impl FigurePreparation for ExhaustOnce {
        fn prepare(
            &self,
            _: &mut MeasureContext<'_>,
            _: Rectangle,
        ) -> Result<FigurePresentation, TextError> {
            Ok(presentation())
        }
        fn prepare_scene(
            &self,
            _: &mut MeasureContext<'_>,
            _: Rectangle,
            scene: &PreparationScene<'_>,
        ) -> Result<PreparedScene, TextError> {
            if self.0.fetch_add(1, Ordering::Relaxed) == 0 {
                let budget = scene.budget();
                let _ = budget.allocate("external payload", budget.limit().allocation);
            }
            Ok(PreparedScene::new(presentation()).with_result_version(1))
        }
    }
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(ExhaustOnce(Arc::new(AtomicUsize::new(0)))))
        .unwrap();
    let mut runtime = Runtime::new(tree);
    let surface = SurfaceInfo {
        logical_width: 120.0,
        logical_height: 80.0,
        pixel_width: 120,
        pixel_height: 80,
        scale_factor: 1.0,
    };
    let FramePreparation::Error(FramePreparationError::Text(error)) = runtime
        .frames()
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
    else {
        panic!("ignored budget error must reject submission");
    };
    let TextError::PreparationDependency(error) = error else {
        panic!("budget error lost its structure");
    };
    let novadraw::figure::PreparationDependencyError::Budget(error) = error.as_ref() else {
        panic!("budget exhaustion was mistaken for a cycle");
    };
    assert_eq!(error.stage, "external payload");
    assert!(error.allocation);
    assert!(
        runtime
            .tree()
            .prepared_presentation(root)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        runtime
            .frames()
            .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL),
        FramePreparation::Ready(_)
    ));
}

#[test]
fn external_prepared_measurement_honors_each_constraint_without_using_old_metrics() {
    let (tree, _, log, calls) = fixture(Mode::Supported);
    run(tree).unwrap();
    assert!(calls.load(Ordering::Relaxed) >= 2);
    let log = log.lock().unwrap();
    assert_eq!(
        log[0].as_ref().unwrap().preferred.size(),
        Dimension::new(40.0, 10.0)
    );
    assert_eq!(
        log[1].as_ref().unwrap().preferred.size(),
        Dimension::new(80.0, 5.0)
    );
}

#[test]
fn override_and_layout_skip_an_unsupported_preparation_measurement() {
    for layout in [false, true] {
        let (mut tree, child, log, calls) = fixture(Mode::Unsupported);
        if layout {
            tree.builder()
                .set_layout_manager(child, Box::new(StackLayout::new()))
                .unwrap();
        } else {
            tree.builder()
                .set_preferred_size(child, Some(Dimension::new(17.0, 19.0)))
                .unwrap();
            tree.builder()
                .set_minimum_size(child, Some(Dimension::new(3.0, 4.0)))
                .unwrap();
        }
        run(tree).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        if !layout {
            assert_eq!(
                log.lock().unwrap()[0].as_ref().unwrap().preferred.size(),
                Dimension::new(17.0, 19.0)
            );
        }
    }
}

#[test]
fn preferred_override_does_not_hide_unsupported_minimum() {
    let (mut tree, child, _, calls) = fixture(Mode::Unsupported);
    tree.builder()
        .set_preferred_size(child, Some(Dimension::new(17.0, 19.0)))
        .unwrap();
    assert!(
        matches!(run(tree), Err(FramePreparationError::Text(TextError::PreparationMeasurement(error)))
        if error.figure == child && error.kind == PreparationMeasureErrorKind::UnsupportedConstraintMeasurement)
    );
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn selected_minimum_does_not_validate_overridden_preferred_output() {
    let (mut tree, child, log, _) = fixture(Mode::InvalidSize);
    tree.builder()
        .set_preferred_size(child, Some(Dimension::new(17.0, 19.0)))
        .unwrap();
    run(tree).unwrap();
    assert_eq!(
        log.lock().unwrap()[0].as_ref().unwrap().minimum,
        Dimension::new(5.0, 2.0)
    );
}

#[test]
fn structured_failures_keep_the_subject_constraints_and_original_cause() {
    for mode in [
        Mode::Unsupported,
        Mode::InvalidSize,
        Mode::InvalidBaseline,
        Mode::Text,
        Mode::Recursive,
        Mode::Budget,
    ] {
        let (tree, child, _, _) = fixture(mode);
        let Err(FramePreparationError::Text(TextError::PreparationMeasurement(error))) = run(tree)
        else {
            panic!("expected structured failure");
        };
        assert_eq!(error.figure, child);
        assert_eq!(
            error.outer_constraints,
            MeasureConstraints::width(40.0).unwrap()
        );
        match mode {
            Mode::Unsupported => assert_eq!(
                error.kind,
                PreparationMeasureErrorKind::UnsupportedConstraintMeasurement
            ),
            Mode::InvalidSize => {
                assert!(matches!(
                    error.kind,
                    PreparationMeasureErrorKind::InvalidSize {
                        metric: PreparationMetric::Preferred,
                        ..
                    }
                ));
                assert_eq!(*error, error.as_ref().clone()); // NaN diagnostics remain reflexive.
            }
            Mode::InvalidBaseline => assert!(matches!(
                error.kind,
                PreparationMeasureErrorKind::InvalidBaseline {
                    baseline: 100.0,
                    height: 10.0
                }
            )),
            Mode::Text => assert_eq!(
                error.source().unwrap().downcast_ref::<TextError>(),
                Some(&TextError::NoUsableFont)
            ),
            Mode::Recursive => {
                assert!(
                    matches!(&error.kind, PreparationMeasureErrorKind::Dependency(
                    cause) if matches!(cause.as_ref(), novadraw::figure::PreparationDependencyError::MeasurementCycle { path } if path == &[child, child]))
                );
            }
            Mode::Budget => assert!(matches!(&error.kind,
                PreparationMeasureErrorKind::Dependency(error)
                    if matches!(error.as_ref(), novadraw::figure::PreparationDependencyError::Budget(_)))),
            Mode::Supported => unreachable!(),
        }
    }
}

#[test]
fn asymmetric_border_and_container_projection_are_applied_once() {
    let border = MarginBorder::new(Insets::new(3.0, 5.0, 5.0, 7.0)).unwrap();
    for overrides in [false, true] {
        let (mut tree, child, log, calls) =
            projection_fixture(Mode::Supported, Some(Arc::new(border)), 2.0);
        if overrides {
            tree.builder()
                .set_preferred_size(child, Some(Dimension::new(17.0, 19.0)))
                .unwrap();
            tree.builder()
                .set_minimum_size(child, Some(Dimension::new(3.0, 4.0)))
                .unwrap();
        }
        run(tree).unwrap();
        let log = log.lock().unwrap();
        let measured = log[0].as_ref().unwrap();
        if overrides {
            assert_eq!(calls.load(Ordering::Relaxed), 0);
            assert_eq!(measured.preferred.size(), Dimension::new(34.0, 38.0));
            assert_eq!(measured.minimum, Dimension::new(6.0, 8.0));
        } else {
            // (outer width 40 - border width 12) / scale 2 = content width 14.
            assert_eq!(measured.preferred.width, 52.0);
            assert_eq!(measured.preferred.height, (400.0 / 14.0 + 8.0) * 2.0);
            assert_eq!(measured.minimum, Dimension::new(34.0, 20.0));
        }
    }
}

#[test]
fn finite_inset_overflow_is_rejected_at_admission_before_the_measurement_callback() {
    struct OverflowBorder;
    impl Border for OverflowBorder {
        fn get_insets(&self) -> Insets {
            Insets::uniform(f64::MAX)
        }
        fn paint(&self, _: Rectangle, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
            Ok(())
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(Probe(Arc::new(Mutex::new(Vec::new())))))
        .unwrap();
    let error = tree
        .builder()
        .add_child(
            root,
            Box::new(External {
                mode: Mode::Supported,
                calls: calls.clone(),
                border: Some(Arc::new(OverflowBorder)),
                scale: 1.0,
            }),
        )
        .unwrap_err();
    assert_eq!(
        error,
        novadraw::tree::GraphMutationError::InvalidInitialInsets(
            novadraw::figure::border::BorderInputError::Overflow {
                field: "horizontal insets"
            }
        )
    );
    assert!(tree.child_order(root).unwrap().is_empty());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn default_constraint_measurement_is_explicitly_unsupported_before_a_snapshot_exists() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(Probe(Arc::default())))
        .unwrap();
    let child = tree
        .builder()
        .add_child(root, Box::new(DefaultUnsupported))
        .unwrap();
    assert!(
        matches!(run(tree), Err(FramePreparationError::Text(TextError::PreparationMeasurement(error)))
        if error.figure == child && error.kind == PreparationMeasureErrorKind::UnsupportedConstraintMeasurement)
    );
}

#[test]
fn frame_error_source_chain_reaches_the_original_text_error() {
    let (tree, child, _, _) = fixture(Mode::Text);
    let error = run(tree).unwrap_err();
    let text = error.source().unwrap().downcast_ref::<TextError>().unwrap();
    let measurement = text
        .source()
        .unwrap()
        .downcast_ref::<PreparationMeasureError>()
        .unwrap();
    assert_eq!(measurement.figure, child);
    assert_eq!(
        measurement.source().unwrap().downcast_ref::<TextError>(),
        Some(&TextError::NoUsableFont)
    );
}

fn nested_fixture(mode: Mode) -> (FigureTree, FigureId, MeasurementLog, Arc<AtomicUsize>) {
    let (mut tree, container, log, _) = fixture(Mode::Unsupported);
    tree.builder()
        .set_layout_manager(container, Box::new(StackLayout::new()))
        .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let leaf = tree
        .builder()
        .add_child(
            container,
            Box::new(External {
                mode,
                calls: calls.clone(),
                border: None,
                scale: 1.0,
            }),
        )
        .unwrap();
    (tree, leaf, log, calls)
}

#[test]
fn layout_children_are_remeasured_under_each_explicit_constraint() {
    let (tree, _, log, calls) = nested_fixture(Mode::Supported);
    run(tree).unwrap();
    let log = log.lock().unwrap();
    assert_eq!(
        log[0].as_ref().unwrap().preferred.size(),
        Dimension::new(40.0, 10.0)
    );
    assert_eq!(
        log[1].as_ref().unwrap().preferred.size(),
        Dimension::new(80.0, 5.0)
    );
    assert_eq!(log[0].as_ref().unwrap().minimum, Dimension::new(5.0, 2.0));
    assert!(calls.load(Ordering::Relaxed) >= 4);
}

#[test]
fn nested_layout_failure_preserves_the_leaf_and_original_text_cause() {
    let (tree, leaf, _, _) = nested_fixture(Mode::Text);
    let Err(FramePreparationError::Text(TextError::PreparationMeasurement(error))) = run(tree)
    else {
        panic!("expected nested failure");
    };
    assert_eq!(error.figure, leaf);
    assert_eq!(
        error.outer_constraints,
        MeasureConstraints::width(40.0).unwrap()
    );
    assert_eq!(
        error.content_constraints,
        Some(MeasureConstraints::width(40.0).unwrap())
    );
    assert_eq!(
        error.source().unwrap().downcast_ref::<TextError>(),
        Some(&TextError::NoUsableFont)
    );
}

#[test]
fn nested_measurement_budget_failure_keeps_the_leaf_and_constraints() {
    let (tree, leaf, _, _) = nested_fixture(Mode::Budget);
    let Err(FramePreparationError::Text(TextError::PreparationMeasurement(error))) = run(tree)
    else {
        panic!("expected nested budget failure");
    };
    assert_eq!(error.figure, leaf);
    assert_eq!(
        error.outer_constraints,
        MeasureConstraints::width(40.0).unwrap()
    );
    assert_eq!(
        error.content_constraints,
        Some(MeasureConstraints::width(40.0).unwrap())
    );
    assert!(
        matches!(&error.kind, PreparationMeasureErrorKind::Dependency(error)
        if matches!(error.as_ref(), novadraw::figure::PreparationDependencyError::Budget(error)
            if error.stage == "external measurement"))
    );
    assert!(
        error
            .source()
            .unwrap()
            .source()
            .unwrap()
            .is::<novadraw::figure::PreparationBudgetError>()
    );
}

struct RepeatedPreferredLayout;
impl LayoutManager for RepeatedPreferredLayout {
    fn preferred_measurement(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        scene: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        let child = scene.children(container)?[0].0;
        let first = scene.preferred_measurement(child, constraints)?;
        assert_eq!(first, scene.preferred_measurement(child, constraints)?);
        Ok(first)
    }
    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(Dimension::ZERO)
    }
    fn layout(
        &mut self,
        _: FigureId,
        _: &LayoutSceneQuery<'_>,
        _: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        Ok(())
    }
}

#[test]
fn repeated_nested_preferred_queries_use_the_session_cache_and_skip_invalid_minimum() {
    struct PreferredOnly(Arc<AtomicUsize>);
    impl Figure for PreferredOnly {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::ZERO
        }
        fn name(&self) -> &'static str {
            "PreferredOnly"
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(PREPARATION, PreparationCapability::of::<Self>())
        }
    }
    impl FigurePreparation for PreferredOnly {
        fn prepare(
            &self,
            _: &mut MeasureContext<'_>,
            _: Rectangle,
        ) -> Result<FigurePresentation, TextError> {
            Ok(presentation())
        }
        fn measure_scene(
            &self,
            _: &mut MeasureContext<'_>,
            constraints: MeasureConstraints,
            _: &PreparationScene<'_>,
        ) -> Result<PreparationMeasurement, PreparationMeasureError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(PreparationMeasurement {
                preferred: FigureMeasurement::new(constraints.max_width().unwrap(), 7.0, None),
                minimum: Dimension::new(f64::NAN, -1.0),
            })
        }
    }
    let (mut tree, container, log, _) = fixture(Mode::Unsupported);
    tree.builder()
        .set_layout_manager(container, Box::new(RepeatedPreferredLayout))
        .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    tree.builder()
        .add_child(container, Box::new(PreferredOnly(calls.clone())))
        .unwrap();
    run(tree).unwrap();
    let log = log.lock().unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), log.len());
    assert_eq!(
        log[0].as_ref().unwrap().preferred.size(),
        Dimension::new(40.0, 7.0)
    );
}
