mod support;

use support::record_full_submission;

use novadraw::{
    animation::{
        AnimationError, AnimationPlan, AnimationStart, AnimationState, InteractionGeometryPolicy,
        Motion, PresentationBinding, PresentationFamily, PresentationValues, Tween,
    },
    connection::ConnectionFigure,
    event::{EventContext, FigureEventHandler, MonotonicTime, MouseButton, MouseEvent},
    figure::Figure,
    figure::FigureMeasurement,
    figure::RectangleFigure,
    figure::{
        BORDER, CLICKABLE, CONNECTION, CONTAINER, CapabilityKey, CapabilityQueryError,
        ClickableFigure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureDrawing,
        FigurePresentation, INPUT, InputCapability,
    },
    geometry::Dimension,
    geometry::Rectangle,
    graphics::Color,
    graphics::{GraphicsError, PaintContext},
    runtime::Runtime,
    runtime::{
        CapabilityUpdateError, ComponentInvalidation, FigureCapabilityContext,
        FigureCapabilityUpdate, FigureUpdateEffect, FigureUpdateEffectContext,
        PreparedCapabilityUpdate, RuntimeMutationError,
    },
    tree::FigureTree,
    tree::GraphMutationError,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

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
        context.effects().update_capability_later(SetCounter(9));
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
        _target: &dyn Figure,
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

struct ReparentEffect {
    child: novadraw::tree::FigureId,
    new_parent: novadraw::tree::FigureId,
}

impl FigureUpdateEffect for ReparentEffect {
    fn prepare(
        self: Box<Self>,
        context: &mut FigureUpdateEffectContext<'_>,
    ) -> Result<(), RuntimeMutationError> {
        context.reparent(self.child, self.new_parent)
    }
}

struct MoveCounter {
    value: u64,
    new_parent: novadraw::tree::FigureId,
}

struct ConflictingMoves {
    first: novadraw::tree::FigureId,
    second: novadraw::tree::FigureId,
}

struct CounterEffects {
    parents: Vec<novadraw::tree::FigureId>,
    resource: Option<novadraw::render::ResourceId>,
    bounds: Option<Rectangle>,
    unchanged: bool,
    swallow_error: bool,
}

impl FigureUpdateEffect for CounterEffects {
    fn prepare(
        self: Box<Self>,
        context: &mut FigureUpdateEffectContext<'_>,
    ) -> Result<(), RuntimeMutationError> {
        for parent in self.parents {
            context.reparent(context.figure_id(), parent)?;
        }
        if let Some(resource) = self.resource {
            context.resource_dependency_change(None, Some(resource))?;
        }
        if let Some(bounds) = self.bounds {
            let result = context.set_bounds(bounds);
            if !self.swallow_error {
                result?;
            }
        }
        Ok(())
    }
}

impl FigureCapabilityUpdate for CounterEffects {
    type Capability = CounterCapability;
    type Prepared = u64;
    type Error = &'static str;
    const KEY: CapabilityKey<Self::Capability> = COUNTER;

    fn prepare(
        self,
        _: &Self::Capability,
        _: &dyn Figure,
        _: FigureCapabilityContext,
    ) -> Result<PreparedCapabilityUpdate<u64>, Self::Error> {
        let prepared = if self.unchanged {
            PreparedCapabilityUpdate::unchanged(7)
        } else {
            PreparedCapabilityUpdate::paint(7)
        };
        Ok(prepared.with_effect(self))
    }

    fn commit(capability: &Self::Capability, value: u64, _: &mut dyn Figure) {
        capability.value.store(value, Ordering::SeqCst);
    }
}

impl FigureCapabilityUpdate for ConflictingMoves {
    type Capability = CounterCapability;
    type Prepared = u64;
    type Error = &'static str;

    const KEY: CapabilityKey<Self::Capability> = COUNTER;

    fn prepare(
        self,
        _capability: &Self::Capability,
        _target: &dyn Figure,
        _context: FigureCapabilityContext,
    ) -> Result<PreparedCapabilityUpdate<Self::Prepared>, Self::Error> {
        Ok(PreparedCapabilityUpdate::paint(7)
            .with_effect(ReparentEffect {
                child: self.first,
                new_parent: self.second,
            })
            .with_effect(ReparentEffect {
                child: self.second,
                new_parent: self.first,
            }))
    }

    fn commit(capability: &Self::Capability, prepared: Self::Prepared, _target: &mut dyn Figure) {
        capability.value.store(prepared, Ordering::SeqCst);
    }
}

impl FigureCapabilityUpdate for MoveCounter {
    type Capability = CounterCapability;
    type Prepared = u64;
    type Error = &'static str;

    const KEY: CapabilityKey<Self::Capability> = COUNTER;

    fn prepare(
        self,
        _capability: &Self::Capability,
        _target: &dyn Figure,
        context: FigureCapabilityContext,
    ) -> Result<PreparedCapabilityUpdate<Self::Prepared>, Self::Error> {
        Ok(
            PreparedCapabilityUpdate::paint(self.value).with_effect(ReparentEffect {
                child: context.figure_id,
                new_parent: self.new_parent,
            }),
        )
    }

    fn commit(capability: &Self::Capability, prepared: Self::Prepared, _target: &mut dyn Figure) {
        capability.value.store(prepared, Ordering::SeqCst);
    }
}

fn tree_with_external_child() -> (
    FigureTree,
    novadraw::tree::FigureId,
    novadraw::tree::FigureId,
) {
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

    runtime
        .input()
        .dispatch_mouse_pressed(20.0, 15.0, MouseButton::Left);
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
fn external_capability_effect_uses_validated_runtime_structural_commit() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)))
        .unwrap();
    let left = tree
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .unwrap();
    let right = tree
        .builder()
        .add_child(
            root,
            Box::new(RectangleFigure::new(100.0, 0.0, 100.0, 100.0)),
        )
        .unwrap();
    let external = tree
        .builder()
        .add_child(left, Box::new(ExternalCapabilityFigure::new(1)))
        .unwrap();
    let mut runtime = Runtime::new(tree);

    runtime
        .figure(external)
        .unwrap()
        .update_capability(MoveCounter {
            value: 7,
            new_parent: right,
        })
        .unwrap();

    assert_eq!(runtime.tree().parent_id(external), Some(right));
    assert_eq!(
        runtime
            .tree()
            .capability(external, COUNTER)
            .unwrap()
            .unwrap()
            .snapshot(),
        7
    );
}

#[test]
fn conflicting_effects_reject_before_component_or_structure_commit() {
    let (mut tree, root, external) = tree_with_external_child();
    let sibling = tree
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(50.0, 0.0, 20.0, 20.0)))
        .unwrap();
    let mut runtime = Runtime::new(tree);
    record_full_submission(&mut runtime);
    let before_children = runtime.tree().child_order(root).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime
            .figure(external)
            .unwrap()
            .update_capability(ConflictingMoves {
                first: external,
                second: sibling,
            })
    }));
    assert!(
        result.is_ok(),
        "conflicting effects must return an error, not panic"
    );
    assert!(matches!(result.unwrap(), Err(CapabilityUpdateError::Effect(
        RuntimeMutationError::EffectTargetMismatch { target, requested }
    )) if target == external && requested == sibling));
    assert_eq!(runtime.tree().parent_id(external), Some(root));
    assert_eq!(runtime.tree().parent_id(sibling), Some(root));
    assert_eq!(runtime.tree().child_order(root).unwrap(), before_children);
    assert_eq!(runtime.tree().component_revision(external), Some(0));
    assert!(!runtime.frames().has_pending_update());
    assert_eq!(
        runtime
            .tree()
            .capability(external, COUNTER)
            .unwrap()
            .unwrap()
            .snapshot(),
        1
    );
    runtime
        .figure(external)
        .unwrap()
        .update_capability(SetCounter(2))
        .unwrap();
}

#[test]
fn target_effect_batch_validates_sequential_moves_and_rejects_late_failure() {
    let (mut tree, root, external) = tree_with_external_child();
    let sibling = tree
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(50.0, 0.0, 20.0, 20.0)))
        .unwrap();
    let descendant = tree
        .builder()
        .add_child(
            external,
            Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)),
        )
        .unwrap();
    let mut runtime = Runtime::new(tree);
    record_full_submission(&mut runtime);
    let before = runtime.tree().child_order(root).unwrap();
    let error = runtime
        .figure(external)
        .unwrap()
        .update_capability(CounterEffects {
            parents: vec![sibling, descendant],
            resource: None,
            bounds: None,
            unchanged: false,
            swallow_error: false,
        })
        .unwrap_err();
    assert_eq!(
        error,
        CapabilityUpdateError::Effect(RuntimeMutationError::Graph(
            GraphMutationError::CycleDetected
        ))
    );
    assert_eq!(runtime.tree().child_order(root).unwrap(), before);
    assert_eq!(runtime.tree().parent_id(external), Some(root));
    assert_eq!(runtime.tree().parent_id(descendant), Some(external));
    assert_eq!(runtime.tree().component_revision(external), Some(0));
    assert_eq!(
        runtime
            .tree()
            .capability(external, COUNTER)
            .unwrap()
            .unwrap()
            .snapshot(),
        1
    );
    assert!(!runtime.frames().has_pending_update());

    // Reusing the committed original parent must not be mistaken for DuplicateChild.
    runtime
        .figure(external)
        .unwrap()
        .update_capability(CounterEffects {
            parents: vec![sibling, root, sibling, sibling],
            resource: None,
            bounds: None,
            unchanged: false,
            swallow_error: false,
        })
        .unwrap();
    assert_eq!(runtime.tree().parent_id(external), Some(sibling));
    assert_eq!(
        runtime.tree().depth(descendant),
        Some(runtime.tree().depth(external).unwrap() + 1)
    );
    assert_eq!(runtime.tree().component_revision(external), Some(1));
    assert_eq!(
        runtime
            .tree()
            .capability(external, COUNTER)
            .unwrap()
            .unwrap()
            .snapshot(),
        7
    );
}

#[test]
fn late_resource_and_bounds_errors_leave_the_entire_effect_batch_unchanged() {
    let (mut tree, root, external) = tree_with_external_child();
    let sibling = tree
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(50.0, 0.0, 20.0, 20.0)))
        .unwrap();
    let mut runtime = Runtime::new(tree);
    let resource = runtime.resources().register_image().unwrap().resource_id();
    let mut foreign = Runtime::new(FigureTree::new());
    let foreign_resource = foreign.resources().register_image().unwrap().resource_id();
    record_full_submission(&mut runtime);
    let bounds = runtime.tree().figure_bounds(external).unwrap();
    for (new_resource, invalid_bounds, swallow_error) in [
        (foreign_resource, None, false),
        (resource, Some(Rectangle::new(0.0, 0.0, -1.0, 10.0)), false),
        (
            resource,
            Some(Rectangle::new(f64::NAN, 0.0, 10.0, 10.0)),
            true,
        ),
    ] {
        assert!(
            runtime
                .figure(external)
                .unwrap()
                .update_capability(CounterEffects {
                    parents: vec![sibling],
                    resource: Some(new_resource),
                    bounds: invalid_bounds,
                    unchanged: false,
                    swallow_error,
                })
                .is_err()
        );
        assert_eq!(runtime.tree().parent_id(external), Some(root));
        assert_eq!(runtime.tree().figure_bounds(external), Some(bounds));
        assert_eq!(runtime.tree().component_revision(external), Some(0));
        assert_eq!(
            runtime
                .tree()
                .capability(external, COUNTER)
                .unwrap()
                .unwrap()
                .snapshot(),
            1
        );
        assert!(
            !runtime
                .resources()
                .remove_dependency(resource, external)
                .unwrap()
        );
        assert!(!runtime.frames().has_pending_update());
    }
    let receipt = runtime
        .figure(external)
        .unwrap()
        .update_capability(CounterEffects {
            parents: vec![external],
            resource: Some(foreign_resource),
            bounds: None,
            unchanged: true,
            swallow_error: false,
        })
        .unwrap();
    assert!(!receipt.changed);
    assert!(!runtime.frames().has_pending_update());
    assert_eq!(runtime.tree().component_revision(external), Some(0));
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

    assert!(runtime.figure(root).unwrap().remove(external).unwrap());
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
    runtime
        .animation()
        .advance_time(MonotonicTime::from_micros(0))
        .unwrap();
    let scalar = runtime
        .animation()
        .timeline()
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
    let AnimationStart::Running(animation) = runtime.animation().timeline().start(plan).unwrap()
    else {
        panic!("external presentation binding must start");
    };

    record_full_submission(&mut runtime);
    assert!(paints.load(Ordering::SeqCst) > 0);
    runtime
        .animation()
        .advance_time(MonotonicTime::from_micros(50_000))
        .unwrap();
    assert_eq!(runtime.animation().timeline().value(scalar).unwrap(), 0.5);

    let color = runtime
        .animation()
        .timeline()
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
        runtime.animation().timeline().start(replacement).unwrap(),
        AnimationStart::Running(_)
    ));
    assert_eq!(
        runtime.animation().timeline().state(animation).unwrap(),
        AnimationState::Cancelled
    );

    assert!(runtime.figure(root).unwrap().remove(external).unwrap());
    assert_eq!(
        runtime.animation().timeline().value(scalar).unwrap_err(),
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
            .animation()
            .timeline()
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
        .animation()
        .timeline()
        .bind_presentation(
            external,
            SCALAR_PRESENTATION,
            InteractionGeometryPolicy::Committed,
        )
        .unwrap();
    let color = runtime
        .animation()
        .timeline()
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
        runtime.animation().timeline().start(parallel).unwrap_err(),
        AnimationError::OverlappingTracks
    );
    assert_eq!(runtime.animation().timeline().active_animation_count(), 0);
}
