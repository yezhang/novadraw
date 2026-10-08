use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use novadraw::{
    Color, Dimension, Figure, FigureMeasurement, FigureTree, Rectangle, RectangleFigure, Runtime,
    animation::{
        AnimationError, AnimationPlan, AnimationStart, AnimationState, InteractionGeometryPolicy,
        Motion, PresentationBinding, PresentationFamily, PresentationValues, Tween,
    },
    connection::ConnectionFigure,
    event::{EventContext, FigureEventHandler, MonotonicTime, MouseButton, MouseEvent},
    figure::{
        BORDER, CLICKABLE, CONNECTION, CONTAINER, CapabilityKey, CapabilityQueryError,
        ClickableFigure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureDrawing,
        FigurePresentation, INPUT, InputCapability,
    },
    graphics::{GraphicsError, PaintContext},
    runtime::{
        CapabilityUpdateError, ComponentInvalidation, FigureCapabilityContext,
        FigureCapabilityUpdate, PreparedCapabilityUpdate,
    },
    tree::GraphMutationError,
};

const COUNTER: CapabilityKey<CounterCapability> = CapabilityKey::new("example.counter");
const SCALAR_PRESENTATION: CapabilityKey<PresentationBinding<f64>> =
    CapabilityKey::new("example.scalar-presentation");
const COLOR_PRESENTATION: CapabilityKey<PresentationBinding<Color>> =
    CapabilityKey::new("example.color-presentation");

struct ScalarPresentationFamily;
struct ColorPresentationFamily;

impl PresentationFamily<ExternalPresentationFigure> for ScalarPresentationFamily {
    fn prepare(
        figure: &ExternalPresentationFigure,
        values: PresentationValues<'_>,
        bounds: Rectangle,
    ) -> FigurePresentation {
        figure.scalar_presentation(values, bounds)
    }
}

impl PresentationFamily<ExternalPresentationFigure> for ColorPresentationFamily {
    fn prepare(
        figure: &ExternalPresentationFigure,
        values: PresentationValues<'_>,
        bounds: Rectangle,
    ) -> FigurePresentation {
        figure.color_presentation(values, bounds)
    }
}

#[derive(Clone, Debug)]
struct CounterCapability {
    value: Arc<AtomicU64>,
}

impl CounterCapability {
    fn snapshot(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }
}

struct ExternalCapabilityFigure {
    bounds: Rectangle,
    value: Arc<AtomicU64>,
    duplicate_registration: bool,
}

impl ExternalCapabilityFigure {
    fn new(value: u64) -> Self {
        Self {
            bounds: Rectangle::new(10.0, 10.0, 40.0, 20.0),
            value: Arc::new(AtomicU64::new(value)),
            duplicate_registration: false,
        }
    }

    fn with_duplicate_registration(mut self) -> Self {
        self.duplicate_registration = true;
        self
    }
}

impl Figure for ExternalCapabilityFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ExternalCapabilityFigure"
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())?;
        out.register(
            COUNTER,
            CounterCapability {
                value: Arc::clone(&self.value),
            },
        )?;
        if self.duplicate_registration {
            out.register(
                COUNTER,
                CounterCapability {
                    value: Arc::clone(&self.value),
                },
            )?;
        }
        Ok(())
    }
}

impl FigureEventHandler for ExternalCapabilityFigure {
    fn on_mouse_pressed(&self, _event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        context.update_capability_later(SetCounter(9));
        true
    }
}

struct SetCounter(u64);

impl FigureCapabilityUpdate for SetCounter {
    type Capability = CounterCapability;
    type Prepared = u64;
    type Error = &'static str;

    const KEY: CapabilityKey<Self::Capability> = COUNTER;

    fn prepare(
        self,
        _capability: &Self::Capability,
        _context: FigureCapabilityContext,
    ) -> Result<PreparedCapabilityUpdate<Self::Prepared>, Self::Error> {
        if self.0 == 0 {
            return Err("counter must be non-zero");
        }
        Ok(PreparedCapabilityUpdate::paint(self.0))
    }

    fn commit(capability: &Self::Capability, prepared: Self::Prepared, _target: &mut dyn Figure) {
        capability.value.store(prepared, Ordering::SeqCst);
    }
}

fn tree_with_external_child() -> (FigureTree, novadraw::FigureId, novadraw::FigureId) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .unwrap();
    let external = tree
        .builder()
        .add_child(root, Box::new(ExternalCapabilityFigure::new(1)))
        .unwrap();
    (tree, root, external)
}

struct ExternalPresentationFigure {
    bounds: Rectangle,
    paints: Arc<AtomicU64>,
}

impl ExternalPresentationFigure {
    fn new(paints: Arc<AtomicU64>) -> Self {
        Self {
            bounds: Rectangle::new(10.0, 10.0, 40.0, 20.0),
            paints,
        }
    }

    fn committed_scalar(&self) -> f64 {
        1.0
    }

    fn scalar_presentation(
        &self,
        values: PresentationValues<'_>,
        bounds: Rectangle,
    ) -> FigurePresentation {
        let value = values.get::<f64>().copied().unwrap_or(1.0);
        self.presentation(Color::rgba(value, 0.0, 0.0, 1.0), bounds)
    }

    fn committed_color(&self) -> Color {
        Color::RED
    }

    fn color_presentation(
        &self,
        values: PresentationValues<'_>,
        bounds: Rectangle,
    ) -> FigurePresentation {
        self.presentation(values.get::<Color>().copied().unwrap_or(Color::RED), bounds)
    }

    fn presentation(&self, color: Color, bounds: Rectangle) -> FigurePresentation {
        FigurePresentation::new(
            FigureMeasurement::new(bounds.width, bounds.height, None),
            Dimension::new(bounds.width, bounds.height),
            Rectangle::new(0.0, 0.0, bounds.width, bounds.height),
            Arc::new(ExternalPresentationDrawing {
                paints: Arc::clone(&self.paints),
                color,
                bounds,
            }),
        )
        .unwrap()
    }
}

impl Figure for ExternalPresentationFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ExternalPresentationFigure"
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(
            SCALAR_PRESENTATION,
            PresentationBinding::of::<Self, ScalarPresentationFamily>(Self::committed_scalar),
        )?;
        out.register(
            COLOR_PRESENTATION,
            PresentationBinding::of::<Self, ColorPresentationFamily>(Self::committed_color),
        )
    }
}

struct ExternalPresentationDrawing {
    paints: Arc<AtomicU64>,
    color: Color,
    bounds: Rectangle,
}

impl FigureDrawing for ExternalPresentationDrawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        self.paints.fetch_add(1, Ordering::SeqCst);
        context.set_fill_paint(self.color);
        context.fill_rect(Rectangle::new(
            0.0,
            0.0,
            self.bounds.width,
            self.bounds.height,
        ))
    }
}

#[test]
fn external_capability_supports_typed_query_and_shared_runtime_commit() {
    let (tree, _root, external) = tree_with_external_child();
    let capability = tree.capability(external, COUNTER).unwrap().unwrap();
    assert_eq!(capability.snapshot(), 1);
    assert_eq!(tree.component_revision(external), Some(0));

    let mut runtime = Runtime::new(tree);
    let receipt = runtime
        .figure(external)
        .unwrap()
        .update_capability(SetCounter(4))
        .unwrap();
    assert_eq!(receipt.figure, external);
    assert_eq!(receipt.previous_revision, 0);
    assert_eq!(receipt.revision, 1);
    assert_eq!(receipt.invalidation, ComponentInvalidation::Paint);
    assert_eq!(
        runtime
            .tree()
            .capability(external, COUNTER)
            .unwrap()
            .unwrap()
            .snapshot(),
        4
    );

    let rejected = runtime
        .figure(external)
        .unwrap()
        .update_capability(SetCounter(0))
        .unwrap_err();
    assert_eq!(
        rejected,
        CapabilityUpdateError::Rejected("counter must be non-zero")
    );
    assert_eq!(runtime.tree().component_revision(external), Some(1));

    runtime.dispatch_mouse_pressed(20.0, 15.0, MouseButton::Left);
    assert_eq!(
        runtime
            .tree()
            .capability(external, COUNTER)
            .unwrap()
            .unwrap()
            .snapshot(),
        9
    );
    assert_eq!(runtime.tree().component_revision(external), Some(2));
    assert!(runtime.take_deferred_mutation_errors().is_empty());
}

#[test]
fn capability_identity_and_absence_have_structured_results() {
    let (tree, root, external) = tree_with_external_child();
    assert!(tree.capability(root, COUNTER).unwrap().is_none());

    let mut foreign_tree = FigureTree::new();
    let foreign = foreign_tree
        .builder()
        .set_contents(Box::new(ExternalCapabilityFigure::new(2)))
        .unwrap();
    assert_eq!(
        tree.capability(foreign, COUNTER).unwrap_err(),
        CapabilityQueryError::ForeignRuntime(foreign)
    );

    let mut runtime = Runtime::new(tree);
    let absent = runtime
        .figure(root)
        .unwrap()
        .update_capability(SetCounter(3))
        .unwrap_err();
    assert_eq!(
        absent,
        CapabilityUpdateError::Missing {
            figure: root,
            capability: "example.counter",
        }
    );

    assert!(runtime.container(root).unwrap().remove(external).unwrap());
    assert_eq!(
        runtime.tree().capability(external, COUNTER).unwrap_err(),
        CapabilityQueryError::UnknownOrDisposedFigure(external)
    );
}

#[test]
fn duplicate_descriptor_rejects_admission_before_publication() {
    let mut tree = FigureTree::new();
    let error = tree
        .builder()
        .set_contents(Box::new(
            ExternalCapabilityFigure::new(1).with_duplicate_registration(),
        ))
        .unwrap_err();
    assert_eq!(
        error,
        GraphMutationError::CapabilityRegistration(
            FigureCapabilityRegistrationError::DuplicateCapability {
                registered: "example.counter",
                attempted: "example.counter",
            }
        )
    );
}

#[test]
fn standard_capabilities_are_discovered_without_concrete_type_branches() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .unwrap();
    let clickable = tree
        .builder()
        .add_child(
            root,
            Box::new(ClickableFigure::new(Rectangle::new(0.0, 0.0, 20.0, 20.0))),
        )
        .unwrap();
    let connection = tree
        .builder()
        .add_child(root, Box::new(ConnectionFigure::new()))
        .unwrap();

    assert!(tree.capability(root, BORDER).unwrap().is_some());
    assert!(tree.capability(root, CONTAINER).unwrap().is_some());
    assert!(tree.capability(root, CLICKABLE).unwrap().is_none());
    assert!(tree.capability(clickable, CLICKABLE).unwrap().is_some());
    assert!(tree.capability(clickable, INPUT).unwrap().is_some());
    assert!(tree.capability(connection, CONNECTION).unwrap().is_some());
}

#[test]
fn external_presentation_binding_drives_immutable_content_without_core_dispatch() {
    let paints = Arc::new(AtomicU64::new(0));
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .unwrap();
    let external = tree
        .builder()
        .add_child(
            root,
            Box::new(ExternalPresentationFigure::new(Arc::clone(&paints))),
        )
        .unwrap();
    assert!(
        tree.capability(external, SCALAR_PRESENTATION)
            .unwrap()
            .is_some()
    );

    let mut runtime = Runtime::new(tree);
    runtime.advance_time(MonotonicTime::from_micros(0)).unwrap();
    let scalar = runtime
        .animations()
        .bind_presentation(
            external,
            SCALAR_PRESENTATION,
            InteractionGeometryPolicy::Committed,
        )
        .unwrap();
    let plan = AnimationPlan::track(
        scalar,
        Motion::Tween(Tween::between(0.0, 1.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    let AnimationStart::Running(animation) = runtime.animations().start(plan).unwrap() else {
        panic!("external presentation binding must start");
    };

    runtime.record_full_frame();
    assert!(paints.load(Ordering::SeqCst) > 0);
    runtime
        .advance_time(MonotonicTime::from_micros(50_000))
        .unwrap();
    assert_eq!(runtime.animations().value(scalar).unwrap(), 0.5);

    let color = runtime
        .animations()
        .bind_presentation(
            external,
            COLOR_PRESENTATION,
            InteractionGeometryPolicy::Committed,
        )
        .unwrap();
    let replacement = AnimationPlan::track(
        color,
        Motion::Tween(
            Tween::between(Color::BLACK, Color::RED, Duration::from_millis(100)).unwrap(),
        ),
    )
    .unwrap();
    assert!(matches!(
        runtime.animations().start(replacement).unwrap(),
        AnimationStart::Running(_)
    ));
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Cancelled
    );

    assert!(runtime.container(root).unwrap().remove(external).unwrap());
    assert_eq!(
        runtime.animations().value(scalar).unwrap_err(),
        AnimationError::UnknownChannel
    );
}

#[test]
fn presentation_binding_absence_and_parallel_content_conflict_are_structured() {
    let paints = Arc::new(AtomicU64::new(0));
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .unwrap();
    let external = tree
        .builder()
        .add_child(
            root,
            Box::new(ExternalPresentationFigure::new(Arc::clone(&paints))),
        )
        .unwrap();
    let mut runtime = Runtime::new(tree);

    assert_eq!(
        runtime
            .animations()
            .bind_presentation(
                root,
                SCALAR_PRESENTATION,
                InteractionGeometryPolicy::Committed,
            )
            .unwrap_err(),
        AnimationError::UnsupportedPresentationBinding {
            capability: "example.scalar-presentation",
        }
    );

    let scalar = runtime
        .animations()
        .bind_presentation(
            external,
            SCALAR_PRESENTATION,
            InteractionGeometryPolicy::Committed,
        )
        .unwrap();
    let color = runtime
        .animations()
        .bind_presentation(
            external,
            COLOR_PRESENTATION,
            InteractionGeometryPolicy::Committed,
        )
        .unwrap();
    let parallel = AnimationPlan::parallel(vec![
        AnimationPlan::track(
            scalar,
            Motion::Tween(Tween::between(0.0, 1.0, Duration::from_millis(100)).unwrap()),
        )
        .unwrap(),
        AnimationPlan::track(
            color,
            Motion::Tween(
                Tween::between(Color::BLACK, Color::RED, Duration::from_millis(100)).unwrap(),
            ),
        )
        .unwrap(),
    ])
    .unwrap();
    assert_eq!(
        runtime.animations().start(parallel).unwrap_err(),
        AnimationError::OverlappingTracks
    );
    assert_eq!(runtime.animations().active_animation_count(), 0);
}
