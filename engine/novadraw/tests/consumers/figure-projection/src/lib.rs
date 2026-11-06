use novadraw::{
    figure::{
        CONTAINER, ChildTransform, ContainerCapability, Figure, FigureCapabilityBuilder,
        FigureCapabilityRegistrationError, FigureContainer, FigureProjection, PROJECTION,
        ProjectionCapability, ProjectionContribution, ProjectionError, ProjectionInputKey,
        ProjectionInputPayload, ProjectionInputRegistry, ProjectionScene,
    },
    geometry::{Affine2D, Rectangle},
};
use std::sync::{
    Arc,
    atomic::{AtomicI64, AtomicUsize, Ordering},
};

#[derive(Eq, PartialEq)]
struct Offset(i64);
impl ProjectionInputPayload for Offset {
    fn retained_units(&self) -> u64 {
        0
    }
}
const OFFSET: ProjectionInputKey<Offset> = ProjectionInputKey::new("consumer.offset");
const RETAINED_BYTES: usize = 1024 * 1024;

/// Shared model is deliberately not a Core component revision.
pub struct SharedProjection {
    registry: ProjectionInputRegistry,
    calls: Arc<AtomicUsize>,
    bounds: Rectangle,
    extra: Affine2D,
    fallback_calls: Option<Arc<AtomicUsize>>,
}
impl SharedProjection {
    pub fn new(model: Arc<AtomicI64>, calls: Arc<AtomicUsize>, bounds: Rectangle) -> Self {
        let mut registry = ProjectionInputRegistry::new(RETAINED_BYTES);
        registry
            .register(OFFSET, move |_, _: &ProjectionScene<'_>| {
                Ok(Offset(model.load(Ordering::Relaxed)))
            })
            .unwrap();
        Self {
            registry,
            calls,
            bounds,
            extra: Affine2D::IDENTITY,
            fallback_calls: None,
        }
    }
    pub fn with_extra(mut self, extra: Affine2D) -> Self {
        self.extra = extra;
        self
    }
}
impl Figure for SharedProjection {
    fn name(&self) -> &'static str {
        "SharedProjectionConsumer"
    }
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())?;
        out.register(PROJECTION, ProjectionCapability::of::<Self>())
    }
}
impl FigureContainer for SharedProjection {
    fn child_transform(&self) -> ChildTransform {
        if let Some(calls) = &self.fallback_calls {
            calls.fetch_add(1, Ordering::Relaxed);
            return ChildTransform::from_affine(self.extra);
        }
        panic!("admitted projection must be the only source")
    }
}
impl FigureProjection for SharedProjection {
    fn project(
        &self,
        scene: &ProjectionScene<'_>,
    ) -> Result<ProjectionContribution, ProjectionError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let snapshot = scene.input(&self.registry, OFFSET, scene.root())?;
        let offset = snapshot.value(OFFSET).unwrap().0 as f64;
        Ok(ProjectionContribution::new(ChildTransform::from_affine(
            Affine2D::from_translation(offset, 0.0) * self.extra,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{
        figure::{PreparationInputErrorKind, RectangleFigure},
        geometry::{Insets, Point},
        tree::{CoordinateQueryError, FigureTree},
    };

    struct SwitchSource {
        enabled: bool,
        invalid: bool,
        offset: i64,
    }
    impl novadraw::runtime::FigureComponentUpdate for SwitchSource {
        type Figure = SharedProjection;
        type Prepared = i64;
        type Error = FigureCapabilityRegistrationError;
        fn prepare(
            self,
            _: &SharedProjection,
            _: novadraw::runtime::FigureComponentContext,
        ) -> Result<novadraw::runtime::PreparedFigureUpdate<i64>, Self::Error> {
            let mut capabilities = FigureCapabilityBuilder::new();
            if !self.invalid {
                capabilities.register(CONTAINER, ContainerCapability::of::<SharedProjection>())?;
            }
            if self.enabled {
                capabilities
                    .register(PROJECTION, ProjectionCapability::of::<SharedProjection>())?;
            }
            Ok(
                novadraw::runtime::PreparedFigureUpdate::geometry_and_paint(self.offset)
                    .with_capabilities(capabilities),
            )
        }
        fn commit(offset: i64, target: &mut SharedProjection) {
            target.extra = Affine2D::from_translation(offset as f64, 0.0);
        }
    }

    #[test]
    fn rejected_source_switch_keeps_component_registry_and_old_receipt() {
        let model = Arc::new(AtomicI64::new(5));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tree = FigureTree::new();
        let owner = tree
            .builder()
            .set_contents(Box::new(SharedProjection::new(
                model,
                calls.clone(),
                Rectangle::new(0.0, 0.0, 100.0, 100.0),
            )))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(RectangleFigure::new(3.0, 4.0, 10.0, 10.0).unwrap()),
            )
            .unwrap();
        let mut runtime = novadraw::runtime::Runtime::new(tree);
        let old = runtime.tree().local_to_surface_transform(child).unwrap();
        let revision = runtime.tree().component_revision(owner).unwrap();
        assert!(
            runtime
                .figure(owner)
                .unwrap()
                .update_component(SwitchSource {
                    enabled: true,
                    invalid: true,
                    offset: 99,
                })
                .is_err()
        );
        assert_eq!(runtime.tree().component_revision(owner).unwrap(), revision);
        assert_eq!(
            runtime.tree().local_to_surface_transform(child).unwrap(),
            old
        );
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let receipt = runtime
            .figure(owner)
            .unwrap()
            .update_component(SwitchSource {
                enabled: true,
                invalid: false,
                offset: 20,
            })
            .unwrap();
        assert_eq!(receipt.revision, revision + 1);
        assert!(
            runtime
                .tree()
                .capability(owner, PROJECTION)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            runtime
                .tree()
                .local_to_surface_transform(child)
                .unwrap()
                .transform_point(Point::ZERO),
            Point::new(28.0, 4.0)
        );
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn explicit_remove_and_readd_change_only_the_authorized_source() {
        let model = Arc::new(AtomicI64::new(5));
        let calls = Arc::new(AtomicUsize::new(0));
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let mut figure =
            SharedProjection::new(model, calls.clone(), Rectangle::new(0.0, 0.0, 100.0, 100.0));
        figure.fallback_calls = Some(fallback_calls.clone());
        let mut tree = FigureTree::new();
        let owner = tree.builder().set_contents(Box::new(figure)).unwrap();
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(RectangleFigure::new(3.0, 4.0, 10.0, 10.0).unwrap()),
            )
            .unwrap();
        let mut runtime = novadraw::runtime::Runtime::new(tree);
        runtime.tree().local_to_surface_transform(child).unwrap();
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 0);
        runtime
            .figure(owner)
            .unwrap()
            .update_component(SwitchSource {
                enabled: false,
                invalid: false,
                offset: 20,
            })
            .unwrap();
        assert!(
            runtime
                .tree()
                .capability(owner, PROJECTION)
                .unwrap()
                .is_none()
        );
        for _ in 0..2 {
            assert_eq!(
                runtime
                    .tree()
                    .local_to_surface_transform(child)
                    .unwrap()
                    .transform_point(Point::ZERO),
                Point::new(23.0, 4.0)
            );
        }
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 2);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        runtime
            .figure(owner)
            .unwrap()
            .update_component(SwitchSource {
                enabled: true,
                invalid: false,
                offset: 30,
            })
            .unwrap();
        assert_eq!(
            runtime
                .tree()
                .local_to_surface_transform(child)
                .unwrap()
                .transform_point(Point::ZERO),
            Point::new(38.0, 4.0)
        );
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn shared_change_is_a_miss_and_does_not_change_component_revision() {
        let model = Arc::new(AtomicI64::new(5));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tree = FigureTree::new();
        let owner = tree
            .builder()
            .set_contents(Box::new(SharedProjection::new(
                model.clone(),
                calls.clone(),
                Rectangle::new(10.0, 20.0, 100.0, 100.0),
            )))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(RectangleFigure::new(3.0, 4.0, 10.0, 10.0).unwrap()),
            )
            .unwrap();
        let revision = tree.component_revision(owner).unwrap();
        assert_eq!(
            tree.local_to_surface_transform(child)
                .unwrap()
                .transform_point(Point::ZERO),
            Point::new(18.0, 24.0)
        );
        let first_calls = calls.load(Ordering::Relaxed);
        tree.surface_to_local_transform(child).unwrap();
        tree.child_content_to_surface_transform(owner).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), first_calls);
        model.store(30, Ordering::Relaxed);
        assert_eq!(
            tree.local_to_surface_transform(child)
                .unwrap()
                .transform_point(Point::ZERO),
            Point::new(43.0, 24.0)
        );
        assert_eq!(calls.load(Ordering::Relaxed), first_calls + 1);
        assert_eq!(tree.component_revision(owner).unwrap(), revision);
    }

    struct MutatingAncestor(Arc<AtomicI64>);
    impl Figure for MutatingAncestor {
        fn name(&self) -> &'static str {
            "MutatingUnqualifiedAncestor"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 100.0)
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(CONTAINER, ContainerCapability::of::<Self>())
        }
    }
    impl FigureContainer for MutatingAncestor {
        fn child_transform(&self) -> ChildTransform {
            self.0.fetch_add(1, Ordering::Relaxed);
            ChildTransform::IDENTITY
        }
    }

    #[test]
    fn later_callback_conflict_rejects_candidate_and_retry_recomputes() {
        let model = Arc::new(AtomicI64::new(1));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(MutatingAncestor(model.clone())))
            .unwrap();
        let owner = tree
            .builder()
            .add_child(
                root,
                Box::new(SharedProjection::new(
                    model,
                    calls.clone(),
                    Rectangle::new(0.0, 0.0, 50.0, 50.0),
                )),
            )
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap()),
            )
            .unwrap();
        for expected in 1..=2 {
            assert!(matches!(tree.local_to_surface_transform(child),
                Err(CoordinateQueryError::Projection(error))
                if matches!(error.as_ref(), ProjectionError::Input(input)
                    if matches!(input.kind, PreparationInputErrorKind::Changed { .. }))));
            assert_eq!(calls.load(Ordering::Relaxed), expected);
        }
    }

    #[test]
    fn noncommuting_insets_and_placement_remain_valid() {
        struct InsetChild;
        impl Figure for InsetChild {
            fn name(&self) -> &'static str {
                "InsetConsumer"
            }
            fn initial_bounds(&self) -> Rectangle {
                Rectangle::new(3.0, 4.0, 10.0, 10.0)
            }
            fn initial_insets(&self) -> Insets {
                Insets::new(1.0, 2.0, 3.0, 4.0)
            }
        }
        let model = Arc::new(AtomicI64::new(7));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tree = FigureTree::new();
        let extra = Affine2D::new(0.0, 2.0, -3.0, 0.0, 0.0, 0.0);
        let owner = tree
            .builder()
            .set_contents(Box::new(
                SharedProjection::new(model, calls, Rectangle::new(10.0, 20.0, 100.0, 100.0))
                    .with_extra(extra),
            ))
            .unwrap();
        let child = tree
            .builder()
            .add_child(owner, Box::new(InsetChild))
            .unwrap();
        let world = tree.local_to_surface_transform(child).unwrap();
        assert_eq!(world.transform_point(Point::ZERO), Point::new(5.0, 26.0));
        let content = tree.child_content_to_surface_transform(child).unwrap();
        assert_eq!(content, world * Affine2D::from_translation(2.0, 1.0));
        let inverse = tree.surface_to_local_transform(child).unwrap().unwrap();
        assert_eq!(
            inverse.transform_point(world.transform_point(Point::new(2.0, 3.0))),
            Point::new(2.0, 3.0)
        );
    }

    #[test]
    fn singular_projection_is_forward_only_and_budget_failure_is_explicit() {
        let mut tree = FigureTree::new();
        let owner = tree
            .builder()
            .set_contents(Box::new(
                SharedProjection::new(
                    Arc::new(AtomicI64::new(0)),
                    Arc::new(AtomicUsize::new(0)),
                    Rectangle::new(0.0, 0.0, 10.0, 10.0),
                )
                .with_extra(Affine2D::from_scale(0.0, 2.0)),
            ))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()),
            )
            .unwrap();
        assert!(tree.local_to_surface_transform(child).is_ok());
        assert_eq!(tree.surface_to_local_transform(child).unwrap(), None);
        let mut bounded =
            FigureTree::with_projection_budget(novadraw::figure::PreparationBudget::new(0, 0), 0);
        let id = bounded
            .builder()
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()))
            .unwrap();
        assert!(matches!(bounded.local_to_surface_transform(id),
            Err(CoordinateQueryError::Projection(error))
            if matches!(error.as_ref(), ProjectionError::Budget(_))));
    }

    #[test]
    fn failed_inverse_does_not_install_a_partially_qualified_contribution() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tree = FigureTree::new();
        let owner = tree
            .builder()
            .set_contents(Box::new(
                SharedProjection::new(
                    Arc::new(AtomicI64::new(0)),
                    calls.clone(),
                    Rectangle::new(0.0, 0.0, 10.0, 10.0),
                )
                .with_extra(Affine2D::from_scale(1.0e-320, 1.0)),
            ))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()),
            )
            .unwrap();
        assert!(matches!(
            tree.surface_to_local_transform(child),
            Err(CoordinateQueryError::Geometry {
                reason: novadraw::tree::CoordinateFailure::InverseUnrepresentable,
                ..
            })
        ));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        tree.local_to_surface_transform(child).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        tree.local_to_surface_transform(child).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn retention_exhaustion_preserves_its_domain_error() {
        let mut tree =
            FigureTree::with_projection_budget(novadraw::figure::PreparationBudget::default(), 0);
        let owner = tree
            .builder()
            .set_contents(Box::new(SharedProjection::new(
                Arc::new(AtomicI64::new(0)),
                Arc::new(AtomicUsize::new(0)),
                Rectangle::new(0.0, 0.0, 10.0, 10.0),
            )))
            .unwrap();
        assert!(matches!(tree.child_content_to_surface_transform(owner),
            Err(CoordinateQueryError::Projection(error))
            if matches!(error.as_ref(), ProjectionError::Retention(_))));
    }

    fn hit_tree(model: Arc<AtomicI64>) -> (FigureTree, novadraw::tree::FigureId) {
        let mut tree = FigureTree::new();
        let owner = tree
            .builder()
            .set_contents(Box::new(SharedProjection::new(
                model,
                Arc::new(AtomicUsize::new(0)),
                Rectangle::new(0.0, 0.0, 100.0, 100.0),
            )))
            .unwrap();
        (tree, owner)
    }

    fn assert_changed(error: &CoordinateQueryError) {
        assert!(std::error::Error::source(error).is_some());
        assert!(matches!(error,
            CoordinateQueryError::Projection(error)
            if matches!(error.as_ref(), ProjectionError::Input(input)
                if matches!(input.kind, PreparationInputErrorKind::Changed { .. }))));
    }

    #[test]
    fn hit_search_rechecks_after_strategy_callbacks_and_recovers() {
        struct MutatingSearch(Arc<AtomicI64>);
        impl novadraw::tree::TreeSearch for MutatingSearch {
            fn accept(&mut self, _: novadraw::tree::TreeSearchContext<'_>) -> bool {
                self.0.store(20, Ordering::Relaxed);
                true
            }
        }
        let model = Arc::new(AtomicI64::new(5));
        let (mut tree, owner) = hit_tree(model.clone());
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(RectangleFigure::new(3.0, 4.0, 10.0, 10.0).unwrap()),
            )
            .unwrap();
        assert_changed(
            &tree
                .hit_test_with((9.0, 5.0), &mut MutatingSearch(model))
                .unwrap_err(),
        );
        assert_eq!(tree.hit_test_simple((24.0, 5.0)).unwrap(), Some(child));
    }

    #[test]
    fn hit_uses_only_admitted_projection_and_distinguishes_budget_failure() {
        let model = Arc::new(AtomicI64::new(5));
        let (mut tree, owner) = hit_tree(model.clone());
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(RectangleFigure::new(3.0, 4.0, 10.0, 10.0).unwrap()),
            )
            .unwrap();
        assert_eq!(tree.hit_test_simple((9.0, 5.0)).unwrap(), Some(child));
        model.store(20, Ordering::Relaxed);
        assert_eq!(tree.hit_test_simple((24.0, 5.0)).unwrap(), Some(child));
        assert_eq!(tree.hit_test_simple((9.0, 5.0)).unwrap(), Some(owner));
        let bounded =
            FigureTree::with_projection_budget(novadraw::figure::PreparationBudget::new(0, 0), 0);
        assert!(matches!(bounded.hit_test((0.0, 0.0)),
            Err(CoordinateQueryError::Projection(error))
            if matches!(error.as_ref(), ProjectionError::Budget(_))));
    }

    struct MutatingHandler {
        model: Arc<AtomicI64>,
        mutate: Arc<std::sync::atomic::AtomicBool>,
    }
    impl Figure for MutatingHandler {
        fn name(&self) -> &'static str {
            "ProjectionEventConsumer"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(3.0, 4.0, 10.0, 10.0)
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(
                novadraw::figure::INPUT,
                novadraw::figure::InputCapability::of::<Self>(),
            )
        }
    }
    impl novadraw::event::FigureEventHandler for MutatingHandler {
        fn on_mouse_pressed(
            &self,
            _: &novadraw::event::MouseEvent,
            ctx: &mut novadraw::event::EventContext<'_>,
        ) -> bool {
            if self.mutate.swap(false, Ordering::Relaxed) {
                self.model.store(20, Ordering::Relaxed);
            }
            ctx.effects().invalidate();
            true
        }
    }

    #[test]
    fn dispatch_preserves_input_failure_and_commits_legal_effects() {
        let model = Arc::new(AtomicI64::new(5));
        let (mut tree, owner) = hit_tree(model.clone());
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(MutatingHandler {
                    model,
                    mutate: Arc::new(std::sync::atomic::AtomicBool::new(true)),
                }),
            )
            .unwrap();
        let mut runtime = novadraw::runtime::Runtime::new(tree);
        let rejected =
            runtime
                .input()
                .dispatch_mouse_pressed(9.0, 5.0, novadraw::event::MouseButton::Left);
        assert_changed(
            rejected
                .error()
                .expect("handler shared write must reject dispatch"),
        );
        assert_eq!(rejected.target(), None);
        assert!(!rejected.is_handled());
        assert_eq!(rejected.capture(), None);
        let recovered =
            runtime
                .input()
                .dispatch_mouse_pressed(24.0, 5.0, novadraw::event::MouseButton::Left);
        assert_eq!(recovered.error(), None);
        assert_eq!(recovered.target(), Some(child));
        assert!(recovered.is_handled());
        assert_eq!(recovered.capture(), Some(child));
        assert!(!runtime.tree().is_valid(child).unwrap());
    }

    struct PollMutationHandler {
        model: Arc<AtomicI64>,
        polls: AtomicUsize,
    }
    impl Figure for PollMutationHandler {
        fn name(&self) -> &'static str {
            "ProjectionFinalBoundaryConsumer"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(3.0, 4.0, 10.0, 10.0)
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(
                novadraw::figure::INPUT,
                novadraw::figure::InputCapability::of::<Self>(),
            )?;
            out.register(
                novadraw::figure::ACTIVITY,
                novadraw::figure::ActivityCapability::of::<Self>(),
            )
        }
    }
    impl novadraw::event::FigureEventHandler for PollMutationHandler {
        fn on_mouse_pressed(
            &self,
            _: &novadraw::event::MouseEvent,
            ctx: &mut novadraw::event::EventContext<'_>,
        ) -> bool {
            ctx.effects().invalidate();
            true
        }
    }
    impl novadraw::figure::FigureActivity for PollMutationHandler {
        fn poll(
            &self,
            _: novadraw::event::MonotonicTime,
            _: bool,
            _: &mut novadraw::event::EventContext<'_>,
        ) {
            if self.polls.fetch_add(1, Ordering::Relaxed) == 1 {
                self.model.store(20, Ordering::Relaxed);
            }
        }
    }

    #[test]
    fn runtime_final_check_keeps_actual_capture_after_late_activity_conflict() {
        let model = Arc::new(AtomicI64::new(5));
        let (mut tree, owner) = hit_tree(model.clone());
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(PollMutationHandler {
                    model,
                    polls: AtomicUsize::new(0),
                }),
            )
            .unwrap();
        let mut runtime = novadraw::runtime::Runtime::new(tree);
        let rejected =
            runtime
                .input()
                .dispatch_mouse_pressed(9.0, 5.0, novadraw::event::MouseButton::Left);
        assert_changed(
            rejected
                .error()
                .expect("post-dispatch activity changed consumed input"),
        );
        assert_eq!(rejected.target(), None);
        assert!(!rejected.is_handled());
        assert_eq!(rejected.capture(), Some(child));
        let recovered =
            runtime
                .input()
                .dispatch_mouse_released(24.0, 5.0, novadraw::event::MouseButton::Left);
        assert_eq!(recovered.error(), None);
        assert_eq!(recovered.capture(), None);
    }

    #[test]
    fn domain_error_wrappers_keep_coordinate_source() {
        let mut tree =
            FigureTree::with_projection_budget(novadraw::figure::PreparationBudget::new(0, 0), 0);
        let id = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()))
            .unwrap();
        let error = tree.local_to_surface_transform(id).unwrap_err();
        let layout = novadraw::layout::LayoutError::Coordinate(error.clone());
        let frame = novadraw::runtime::FramePreparationError::Coordinate(error);
        assert!(
            std::error::Error::source(&layout)
                .unwrap()
                .downcast_ref::<CoordinateQueryError>()
                .is_some()
        );
        assert!(
            std::error::Error::source(&frame)
                .unwrap()
                .downcast_ref::<CoordinateQueryError>()
                .is_some()
        );
    }

    fn surface() -> novadraw::render::SurfaceInfo {
        novadraw::render::SurfaceInfo {
            logical_width: 200.0,
            logical_height: 200.0,
            pixel_width: 200,
            pixel_height: 200,
            scale_factor: 1.0,
        }
    }

    struct ChangeDuringPaint {
        model: Arc<AtomicI64>,
        armed: Arc<std::sync::atomic::AtomicBool>,
    }
    impl Figure for ChangeDuringPaint {
        fn name(&self) -> &'static str {
            "ExternalLateProjectionInput"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(3.0, 4.0, 10.0, 10.0)
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(
                novadraw::figure::ACCESSIBILITY,
                novadraw::figure::AccessibilityCapability::of::<Self>(),
            )
        }
        fn paint(
            &self,
            context: &mut novadraw::graphics::PaintContext<'_>,
            bounds: Rectangle,
        ) -> Result<(), novadraw::graphics::GraphicsError> {
            if self.armed.swap(false, Ordering::Relaxed) {
                self.model.fetch_add(20, Ordering::Relaxed);
            }
            context.set_fill_paint(novadraw::graphics::Color::BLUE);
            context.fill_rect(bounds)
        }
    }

    impl novadraw::figure::AccessibleFigure for ChangeDuringPaint {
        fn accessible_name(&self) -> Option<&str> {
            Some("External painted child")
        }
    }

    #[test]
    fn renderer_uses_projection_and_late_conflict_preserves_frame_identity_and_accessibility() {
        use novadraw::{
            render::{BackendCapabilities, RenderCommandKind, RenderDisposition},
            runtime::FramePreparation,
        };
        let model = Arc::new(AtomicI64::new(5));
        let calls = Arc::new(AtomicUsize::new(0));
        let armed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut tree = FigureTree::new();
        let owner = tree
            .builder()
            .set_contents(Box::new(SharedProjection::new(
                model.clone(),
                calls.clone(),
                Rectangle::new(0.0, 0.0, 100.0, 100.0),
            )))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(ChangeDuringPaint {
                    model: model.clone(),
                    armed: armed.clone(),
                }),
            )
            .unwrap();
        let mut runtime = novadraw::runtime::Runtime::new(tree);
        let FramePreparation::Ready(first) = runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("admitted provider must record without invoking its fallback");
        };
        assert!(first.commands.iter().any(|command| matches!(
            command.kind,
            RenderCommandKind::ConcatTransform { matrix } if matrix == Affine2D::from_translation(5.0, 0.0)
        )));
        let _ = runtime
            .frames()
            .complete_submission(
                first.session_id,
                first.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .unwrap();
        let accessibility = runtime.accessibility().snapshot().unwrap().clone();
        assert!(
            accessibility
                .nodes
                .iter()
                .any(|node| node.name == "External painted child" && node.bounds.x == 8.0)
        );
        runtime.accessibility().take_updates();
        // The candidate publishes x=13 before painting changes the input again.
        // Failure must restore x=8 rather than leave that candidate installed.
        model.store(10, Ordering::Relaxed);
        armed.store(true, Ordering::Relaxed);
        runtime.figure(child).unwrap().repaint(None).unwrap();
        runtime.frames().request_full_redraw();
        let FramePreparation::Error(error) = runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("paint changed a shared input consumed by accessibility and recording");
        };
        let mut source: &dyn std::error::Error = &error;
        loop {
            if let Some(coordinate) = source.downcast_ref::<CoordinateQueryError>() {
                assert_changed(coordinate);
                break;
            }
            source = source
                .source()
                .expect("frame failure must preserve coordinate source");
        }
        assert!(!runtime.is_faulted());
        assert_eq!(runtime.accessibility().snapshot(), Some(&accessibility));
        assert!(runtime.accessibility().take_updates().is_empty());
        let FramePreparation::Ready(recovered) = runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("fresh frame must recover from late conflict");
        };
        assert_eq!(recovered.frame_id, first.frame_id.next());
        assert_eq!(recovered.damage.mode(), novadraw::render::DamageMode::Full);
        assert!(recovered.commands.iter().any(|command| matches!(
            command.kind,
            RenderCommandKind::ConcatTransform { matrix } if matrix == Affine2D::from_translation(30.0, 0.0)
        )));
    }

    #[test]
    fn independent_query_does_not_publish_shared_projection_or_coordinate_notifications() {
        use novadraw::{
            event::{CoordinateListener, FigureEvent, ListenerDirective},
            render::{BackendCapabilities, DamageMode, RenderDisposition},
            runtime::FramePreparation,
        };
        struct Coordinates {
            owner: novadraw::tree::FigureId,
            calls: Arc<AtomicUsize>,
        }
        impl CoordinateListener for Coordinates {
            fn coordinate_system_changed(&self, event: FigureEvent) -> ListenerDirective {
                if matches!(event, FigureEvent::CoordinateSystemChanged { figure_id, .. }
                    if figure_id == self.owner)
                {
                    self.calls.fetch_add(1, Ordering::Relaxed);
                }
                ListenerDirective::default()
            }
        }
        let model = Arc::new(AtomicI64::new(5));
        let mut tree = FigureTree::new();
        let owner = tree
            .builder()
            .set_contents(Box::new(SharedProjection::new(
                model.clone(),
                Arc::new(AtomicUsize::new(0)),
                Rectangle::new(0.0, 0.0, 100.0, 100.0),
            )))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(RectangleFigure::new(3.0, 4.0, 10.0, 10.0).unwrap()),
            )
            .unwrap();
        let mut runtime = novadraw::runtime::Runtime::new(tree);
        let FramePreparation::Ready(first) = runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("first projection must record")
        };
        let _ = runtime
            .frames()
            .complete_submission(
                first.session_id,
                first.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .unwrap();
        let notifications = Arc::new(AtomicUsize::new(0));
        runtime
            .observations()
            .add_coordinate_listener(Box::new(Coordinates {
                owner,
                calls: notifications.clone(),
            }))
            .unwrap();
        let revision = runtime.tree().component_revision(owner).unwrap();
        model.store(10, Ordering::Relaxed);
        assert_eq!(
            runtime
                .tree()
                .local_to_surface_transform(child)
                .unwrap()
                .transform_point(Point::ZERO),
            Point::new(13.0, 4.0)
        );
        assert_eq!(notifications.load(Ordering::Relaxed), 0);
        assert_eq!(runtime.tree().component_revision(owner).unwrap(), revision);
        runtime.frames().request_full_redraw();
        let FramePreparation::Ready(second) = runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("explicit request must consume the new shared input")
        };
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
        assert_eq!(second.damage.mode(), DamageMode::Full);
        let _ = runtime
            .frames()
            .complete_submission(
                second.session_id,
                second.frame_id,
                Ok(RenderDisposition::Skipped),
            )
            .unwrap();
        // A rejected display is not a failed stable publish. An unchanged
        // retry must repair the display without repeating coordinate facts.
        let FramePreparation::Ready(retry) = runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("skipped display must preserve recovery work")
        };
        assert_eq!(retry.damage.mode(), DamageMode::Full);
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
        let _ = runtime
            .frames()
            .complete_submission(
                retry.session_id,
                retry.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .unwrap();
        model.store(20, Ordering::Relaxed);
        runtime.frames().request_full_redraw();
        let FramePreparation::Ready(third) = runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("second shared change must publish another coordinate fact")
        };
        assert_eq!(third.damage.mode(), DamageMode::Full);
        assert_eq!(notifications.load(Ordering::Relaxed), 2);
        assert_eq!(runtime.tree().component_revision(owner).unwrap(), revision);
    }

    #[test]
    fn source_move_freezes_damage_with_the_admitted_provider() {
        use novadraw::{
            render::{BackendCapabilities, RenderDisposition},
            runtime::FramePreparation,
        };
        let model = Arc::new(AtomicI64::new(5));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tree = FigureTree::new();
        let owner = tree
            .builder()
            .set_contents(Box::new(SharedProjection::new(
                model.clone(),
                calls.clone(),
                Rectangle::new(0.0, 0.0, 100.0, 100.0),
            )))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(RectangleFigure::new(3.0, 4.0, 10.0, 10.0).unwrap()),
            )
            .unwrap();
        let mut runtime = novadraw::runtime::Runtime::new(tree);
        let FramePreparation::Ready(first) = runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("initial frame must record");
        };
        let _ = runtime
            .frames()
            .complete_submission(
                first.session_id,
                first.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .unwrap();
        model.store(25, Ordering::Relaxed);
        let reads_before_source = calls.load(Ordering::Relaxed);
        runtime
            .figure(child)
            .unwrap()
            .set_bounds(Rectangle::new(30.0, 4.0, 10.0, 10.0))
            .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), reads_before_source);
        let FramePreparation::Ready(moved) = runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("source damage must use projection, never the fallback");
        };
        // The external painter has no complete frozen envelope contract.
        // Full protects old pixels without rereading its shared projection.
        assert_eq!(moved.damage.mode(), novadraw::render::DamageMode::Full);
    }

    #[test]
    fn animation_cache_rechecks_projection_without_replacing_local_presentation_transform() {
        use novadraw::{
            animation::{
                AnimationPlan, GeometryBinding, InteractionGeometryPolicy, LayoutBounds, Motion,
                Tween,
            },
            render::{BackendCapabilities, RenderCommandKind, RenderDisposition},
            runtime::FramePreparation,
        };
        let model = Arc::new(AtomicI64::new(5));
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 200.0, 200.0).unwrap(),
            ))
            .unwrap();
        let owner = tree
            .builder()
            .add_child(
                root,
                Box::new(SharedProjection::new(
                    model.clone(),
                    Arc::new(AtomicUsize::new(0)),
                    Rectangle::new(0.0, 0.0, 100.0, 100.0),
                )),
            )
            .unwrap();
        let sibling_bounds = Rectangle::new(120.0, 120.0, 10.0, 10.0);
        let sibling = tree
            .builder()
            .add_child(
                root,
                Box::new(
                    RectangleFigure::new(
                        sibling_bounds.x,
                        sibling_bounds.y,
                        sibling_bounds.width,
                        sibling_bounds.height,
                    )
                    .unwrap(),
                ),
            )
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                owner,
                Box::new(RectangleFigure::new(3.0, 4.0, 10.0, 10.0).unwrap()),
            )
            .unwrap();
        let mut runtime = novadraw::runtime::Runtime::new(tree);
        let channel = runtime
            .animation()
            .timeline()
            .bind_figure(child, InteractionGeometryPolicy::Committed)
            .unwrap()
            .transform();
        let local = Affine2D::from_translation(0.0, 7.0);
        let plan = AnimationPlan::track(
            channel,
            Motion::Tween(
                Tween::between(
                    local,
                    Affine2D::IDENTITY,
                    std::time::Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        runtime.animation().timeline().start(plan).unwrap();
        // A separate geometry group forces complete geometry materialization
        // without freezing the projection owner's subtree as old animation input.
        let sibling_channel = runtime
            .animation()
            .timeline()
            .bind_geometry(sibling, GeometryBinding::layout_bounds())
            .unwrap();
        let bounds_plan = AnimationPlan::track(
            sibling_channel,
            Motion::Tween(
                Tween::between(
                    LayoutBounds::try_new(Rectangle::new(130.0, 120.0, 10.0, 10.0)).unwrap(),
                    LayoutBounds::try_new(sibling_bounds).unwrap(),
                    std::time::Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        runtime.animation().timeline().start(bounds_plan).unwrap();
        let FramePreparation::Ready(first) = runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("animated frame must record");
        };
        let _ = runtime
            .frames()
            .complete_submission(
                first.session_id,
                first.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .unwrap();
        let revision = runtime.tree().component_revision(owner).unwrap();
        model.store(25, Ordering::Relaxed);
        let changed = match runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        {
            FramePreparation::Ready(changed) => changed,
            other => panic!("changed shared projection must invalidate geometry cache: {other:?}"),
        };
        assert_eq!(runtime.tree().component_revision(owner).unwrap(), revision);
        assert_eq!(changed.damage.mode(), novadraw::render::DamageMode::Full);
        for expected in [Affine2D::from_translation(25.0, 0.0), local] {
            assert!(
                changed.commands.iter().any(|command| matches!(
                    command.kind,
                    RenderCommandKind::ConcatTransform { matrix } if matrix == expected
                )),
                "missing transform {expected:?}"
            );
        }
    }
}
