use super::*;

#[cfg(test)]
mod capability_workset_tests {
    use super::*;
    use crate::figure::{ACTIVITY, ActivityCapability, FigureActivity, FigureCapabilityBuilder};

    const EXTERNAL: CapabilityKey<ExternalDescriptor> = CapabilityKey::new("consumer.workset");
    struct ExternalDescriptor;
    struct ExternalFigure;

    impl crate::Figure for ExternalFigure {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 10.0, 10.0)
        }
        fn name(&self) -> &'static str {
            "ExternalWorkset"
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(EXTERNAL, ExternalDescriptor)?;
            out.register(ACTIVITY, ActivityCapability::of::<Self>())
        }
    }
    impl FigureActivity for ExternalFigure {
        fn poll(&self, _: crate::MonotonicTime, _: bool, _: &mut crate::EventContext<'_>) {}
    }

    fn assert_scan_matches(tree: &FigureTree) {
        let scanned = tree
            .blocks
            .iter()
            .filter_map(|(id, node)| {
                node.capabilities
                    .get(EXTERNAL)
                    .unwrap()
                    .is_some()
                    .then_some(id)
            })
            .collect::<Vec<_>>();
        assert_eq!(tree.capability_ids(EXTERNAL).collect::<Vec<_>>(), scanned);
        let activities = tree
            .blocks
            .iter()
            .filter_map(|(id, node)| node.activity().is_some().then_some(id))
            .collect::<Vec<_>>();
        assert_eq!(tree.activity_ids(), activities);
        let clickables = tree
            .blocks
            .iter()
            .filter_map(|(id, node)| node.clickable().is_some().then_some(id))
            .collect::<Vec<_>>();
        assert_eq!(tree.clickable_ids(), clickables);
    }

    #[test]
    fn capability_members_follow_detach_retirement_and_reused_slots() {
        let mut tree = FigureTree::new();
        let first = tree.add_child_to(tree.root, Box::new(ExternalFigure));
        let second = tree.add_child_to(tree.root, Box::new(ExternalFigure));
        let mut updates = UpdateManager::new();
        assert_scan_matches(&tree);
        tree.remove_child(&mut updates, tree.root, first);
        assert_scan_matches(&tree);
        assert!(tree.capability_ids(EXTERNAL).any(|id| id == first));
        tree.attach_child_checked(tree.root, first).unwrap();
        let ids = tree.disposal_ids(first).unwrap();
        tree.extract_subtree(&ids, &mut updates).unwrap().complete();
        let replacement = tree.add_child_to(tree.root, Box::new(ExternalFigure));
        assert_ne!(first, replacement);
        assert!(!tree.capability_ids(EXTERNAL).any(|id| id == first));
        assert!(tree.capability_ids(EXTERNAL).any(|id| id == second));
        assert_scan_matches(&tree);
    }

    #[test]
    fn rejected_or_panicking_reservations_remove_all_capability_members() {
        let mut tree = FigureTree::new();
        let parent = tree.root;
        let child = tree
            .prepare_child(Box::new(ExternalFigure), parent, false, 0, None)
            .unwrap();
        let rejected =
            tree.insert_prepared_child::<GraphMutationError>(child, parent, None, |_, _| {
                Err(GraphMutationError::ParentNotFound)
            });
        assert_eq!(rejected, Err(GraphMutationError::ParentNotFound));
        assert_scan_matches(&tree);
        assert!(tree.capability_ids(EXTERNAL).next().is_none());
        let child = tree
            .prepare_child(Box::new(ExternalFigure), parent, false, 0, None)
            .unwrap();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tree.insert_prepared_child::<GraphMutationError>(child, parent, None, |_, _| {
                panic!("reservation rejection");
            })
        }));
        assert!(panicked.is_err());
        assert_scan_matches(&tree);
        assert!(tree.capability_ids(EXTERNAL).next().is_none());
    }

    struct ReplaceCapabilities(bool);
    impl crate::FigureComponentUpdate for ReplaceCapabilities {
        type Figure = ExternalFigure;
        type Prepared = ();
        type Error = FigureCapabilityRegistrationError;
        fn prepare(
            self,
            _: &ExternalFigure,
            _: crate::FigureComponentContext,
        ) -> Result<crate::runtime::PreparedFigureUpdate<()>, Self::Error> {
            let mut capabilities = FigureCapabilityBuilder::new();
            if self.0 {
                capabilities.register(EXTERNAL, ExternalDescriptor)?;
                capabilities.register(ACTIVITY, ActivityCapability::of::<ExternalFigure>())?;
            }
            Ok(crate::runtime::PreparedFigureUpdate::paint(()).with_capabilities(capabilities))
        }
        fn commit(_: (), _: &mut ExternalFigure) {}
    }

    #[test]
    fn capability_replacement_updates_all_typed_member_sets_with_the_revision() {
        let mut tree = FigureTree::new();
        let owner = tree.add_child_to(tree.root, Box::new(ExternalFigure));
        let mut runtime = crate::Runtime::new(tree);
        let revision = runtime.tree().component_revision(owner).unwrap();
        runtime
            .figure(owner)
            .unwrap()
            .update_component(ReplaceCapabilities(false))
            .unwrap();
        assert_scan_matches(runtime.tree());
        assert!(runtime.tree().capability_ids(EXTERNAL).next().is_none());
        assert!(runtime.tree().activity_ids().is_empty());
        assert_eq!(
            runtime.tree().component_revision(owner).unwrap(),
            revision + 1
        );
        runtime
            .figure(owner)
            .unwrap()
            .update_component(ReplaceCapabilities(true))
            .unwrap();
        assert_scan_matches(runtime.tree());
        assert_eq!(
            runtime.tree().capability_ids(EXTERNAL).collect::<Vec<_>>(),
            vec![owner]
        );
        assert_eq!(runtime.tree().activity_ids(), vec![owner]);
        assert_eq!(
            runtime.tree().component_revision(owner).unwrap(),
            revision + 2
        );
    }

    #[test]
    #[ignore = "release-only capability query cost evidence"]
    fn capability_query_cost() {
        use std::{hint::black_box, time::Instant};
        const SAMPLE_COUNT: usize = 200;
        const QUERY_COUNT: usize = 100;
        const WARMUP_COUNT: usize = 20;
        for inactive in [300, 10_000] {
            let mut tree = FigureTree::new();
            for _ in 0..inactive {
                tree.add_child_to(
                    tree.root,
                    Box::new(crate::RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap()),
                );
            }
            tree.add_child_to(tree.root, Box::new(ExternalFigure));
            assert_scan_matches(&tree);
            let mut samples = [Vec::new(), Vec::new()];
            for sample in 0..SAMPLE_COUNT + WARMUP_COUNT {
                for side in if sample.is_multiple_of(2) {
                    [0, 1]
                } else {
                    [1, 0]
                } {
                    let start = Instant::now();
                    for _ in 0..QUERY_COUNT {
                        let ids = if side == 0 {
                            black_box(&tree)
                                .blocks
                                .iter()
                                .filter_map(|(id, node)| node.activity().is_some().then_some(id))
                                .collect::<Vec<_>>()
                        } else {
                            black_box(&tree).activity_ids()
                        };
                        assert_eq!(black_box(ids).len(), 1);
                    }
                    if sample >= WARMUP_COUNT {
                        samples[side].push(start.elapsed().as_nanos() as f64 / QUERY_COUNT as f64);
                    }
                }
            }
            for (side, values) in samples.iter_mut().enumerate() {
                values.sort_by(f64::total_cmp);
                println!(
                    "inactive={inactive} side={side} n={} p50_ns={} p95_ns={} p99_ns={}",
                    values.len(),
                    values[values.len() / 2],
                    values[values.len() * 95 / 100],
                    values[values.len() * 99 / 100]
                );
            }
        }
    }
}

/// Numerical reason a coordinate query could not produce a transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateFailure {
    /// At least one affine input coefficient is non-finite.
    NonFiniteInput,
    /// The mathematical inverse exists but a result coefficient exceeds f64 range.
    InverseUnrepresentable,
    /// The calculation cannot reliably determine or represent its intermediate result.
    NumericalFailure,
    /// Finite forward inputs produced a non-finite composed transform.
    DerivedOverflow,
}

#[cfg(test)]
mod s07_tree_identity_tests {
    use super::*;
    use crate::identity::RuntimeHandle;

    #[test]
    fn never_live_identity_is_rejected_by_every_fact_query() {
        let tree = FigureTree::new();
        let id = FigureId::from_local(tree.namespace(), slotmap::KeyData::from_ffi(u64::MAX));
        let error = TreeQueryError::UnknownFigure(id);
        macro_rules! check {
            ($($query:ident),+ $(,)?) => {$(
                assert_eq!(tree.$query(id).err(), Some(error), stringify!($query));
            )+};
        }
        check!(
            node,
            figure_bounds,
            component_revision,
            child_order,
            parent_id,
            depth,
            is_attached,
            is_visible,
            is_enabled,
            is_opaque,
            insets,
            figure_style,
            resolved_style,
            border_is_effectively_opaque,
            is_effectively_visible,
            is_effectively_enabled,
            is_focusable,
            is_focus_traversable,
            can_request_focus,
            can_traverse_focus,
            is_connection_figure,
            connection_route_points,
            connection_stroke_color,
            point_list_points,
            point_list_stroke_style,
            child_clipping_strategy,
            is_valid,
            ancestor_ids,
            descendant_ids,
            layout_manager,
        );
        assert!(!tree.contains(id));
        assert_eq!(tree.viewport_handle(id).err(), Some(error));
        assert_eq!(
            tree.scale_handle(id).err(),
            Some(CapabilityQueryError::UnknownOrDisposedFigure(id))
        );
        assert_eq!(
            tree.image_display_state(id).err(),
            Some(crate::figure::ShapeMutationError::UnknownFigure(id))
        );
        assert_eq!(tree.child_z_index(tree.root, id), Err(error));
        assert_eq!(tree.child_z_index(id, tree.root), Err(error));
        assert_eq!(tree.is_ancestor_of(tree.root, id), Err(error));
        assert_eq!(tree.is_ancestor_of(id, id), Err(error));
        assert_eq!(
            tree.layout_constraint::<Rectangle>(id),
            Err(LayoutError::Identity(error))
        );
        assert_eq!(
            tree.freeform_extent(id),
            Err(FreeformError::UnknownFigure(id))
        );
    }
}

/// Identity, geometry or projection failure of a checked Figure coordinate query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinateQueryError {
    /// The requested identity does not belong to the live tree.
    Identity(TreeQueryError),
    Projection(Box<crate::figure::ProjectionError>),
    /// Transform calculation failed for an otherwise valid identity.
    Geometry {
        /// Identity whose requested transform could not be calculated.
        figure: FigureId,
        /// Numerical classification of the failure.
        reason: CoordinateFailure,
    },
}

impl std::fmt::Display for CoordinateQueryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::Projection(error) => error.fmt(formatter),
            Self::Geometry { figure, reason } => {
                write!(
                    formatter,
                    "coordinate query for {figure:?} failed: {reason:?}"
                )
            }
        }
    }
}

impl std::error::Error for CoordinateQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::Projection(error) => Some(error.as_ref()),
            Self::Geometry { .. } => None,
        }
    }
}

impl From<TreeQueryError> for CoordinateQueryError {
    fn from(error: TreeQueryError) -> Self {
        Self::Identity(error)
    }
}
impl From<crate::figure::ProjectionError> for CoordinateQueryError {
    fn from(error: crate::figure::ProjectionError) -> Self {
        Self::Projection(Box::new(error))
    }
}

fn checked_transform(
    figure: FigureId,
    transform: Affine2D,
    reason: CoordinateFailure,
) -> Result<Affine2D, CoordinateQueryError> {
    if transform.coeffs().into_iter().all(f64::is_finite) {
        Ok(transform)
    } else {
        Err(CoordinateQueryError::Geometry { figure, reason })
    }
}

pub(crate) fn checked_coordinate_inverse(
    figure: FigureId,
    transform: Affine2D,
) -> Result<Option<Affine2D>, CoordinateQueryError> {
    use crate::geometry::InverseFailure;
    transform
        .checked_inverse()
        .map_err(|error| CoordinateQueryError::Geometry {
            figure,
            reason: match error {
                InverseFailure::NonFiniteInput => CoordinateFailure::NonFiniteInput,
                InverseFailure::InverseUnrepresentable => CoordinateFailure::InverseUnrepresentable,
                InverseFailure::NumericalFailure => CoordinateFailure::NumericalFailure,
            },
        })
}

/// One coordinate operation, including callbacks and inverse calculation.
/// Temporary results are not published before the final proof check.
/// A composed prefix only memoizes matrix work. It never grants input-reader
/// or provider omission authority. Weak parent identity avoids owning a deep
/// chain and recursive destruction when a 10,000-level tree retires.
pub(crate) struct WorldPrefix {
    parent: Option<std::rc::Weak<WorldPrefix>>,
    local: Affine2D,
    incoming: Affine2D,
    transform: Affine2D,
    _retention: crate::text::TextContentLease,
}

#[cfg(test)]
mod projection_prefix_tests {
    use super::*;

    struct LocalFigure(Rectangle);
    impl crate::Figure for LocalFigure {
        fn initial_bounds(&self) -> Rectangle {
            self.0
        }
        fn name(&self) -> &'static str {
            "CoreOnlyPrefixFigure"
        }
    }

    #[test]
    fn warm_prefix_reuses_matrices_and_parent_move_rebuilds_descendant_prefix() {
        let mut tree = FigureTree::new();
        let owner = tree.add_child_to(
            tree.root,
            Box::new(LocalFigure(Rectangle::new(10.0, 20.0, 100.0, 100.0))),
        );
        let child = tree.add_child_to(
            owner,
            Box::new(LocalFigure(Rectangle::new(3.0, 4.0, 10.0, 10.0))),
        );
        assert_eq!(
            tree.local_to_surface_transform(child)
                .unwrap()
                .transform_point(Point::ZERO),
            Point::new(13.0, 24.0)
        );
        let old = tree.projection_prefixes.borrow()[&child].clone();
        let root = tree.projection_prefixes.borrow()[&tree.root].clone();
        tree.local_to_surface_transform(child).unwrap();
        assert!(std::rc::Rc::ptr_eq(
            &old,
            &tree.projection_prefixes.borrow()[&child]
        ));
        tree.set_bounds(owner, 30.0, 40.0, 100.0, 100.0);
        assert_eq!(
            tree.local_to_surface_transform(child)
                .unwrap()
                .transform_point(Point::ZERO),
            Point::new(33.0, 44.0)
        );
        assert!(!std::rc::Rc::ptr_eq(
            &old,
            &tree.projection_prefixes.borrow()[&child]
        ));
        assert!(std::rc::Rc::ptr_eq(
            &root,
            &tree.projection_prefixes.borrow()[&tree.root]
        ));
        assert_eq!(
            tree.figure_bounds(child).unwrap(),
            Rectangle::new(3.0, 4.0, 10.0, 10.0)
        );
    }

    #[test]
    fn unpublished_operation_keeps_old_prefixes_until_final_verification() {
        let mut tree = FigureTree::new();
        let child = tree.add_child_to(
            tree.root,
            Box::new(crate::RectangleFigure::new(3.0, 4.0, 10.0, 10.0).unwrap()),
        );
        tree.local_to_surface_transform(child).unwrap();
        let old = tree.projection_prefixes.borrow()[&child].clone();
        tree.set_bounds(child, 30.0, 40.0, 10.0, 10.0);
        let operation = tree.projection_operation();
        let session = CoordinateQuerySession::in_operation(&tree, &operation).unwrap();
        session.finish(session.local_to_surface(child)).unwrap();
        assert!(std::rc::Rc::ptr_eq(
            &old,
            &tree.projection_prefixes.borrow()[&child]
        ));
        operation.publish(&tree, "test publication").unwrap();
        assert!(!std::rc::Rc::ptr_eq(
            &old,
            &tree.projection_prefixes.borrow()[&child]
        ));
    }

    #[test]
    fn nested_scope_shares_budget_without_publishing_and_other_tree_is_independent() {
        let sample = FigureTree::new();
        let sample_scope = sample.begin_projection_scope();
        sample.local_to_surface_transform(sample.root).unwrap();
        let limit = sample_scope.operation.budget().used();
        let tree = FigureTree::with_projection_budget(limit, 1_000_000);
        let outer = tree.begin_projection_scope();
        tree.local_to_surface_transform(tree.root).unwrap();
        assert!(tree.projection_prefixes.borrow().is_empty());
        let nested = tree.begin_projection_scope();
        let error = tree.local_to_surface_transform(tree.root).unwrap_err();
        assert!(matches!(error, CoordinateQueryError::Projection(_)));
        drop(nested);
        let other = FigureTree::new();
        other.local_to_surface_transform(other.root).unwrap();
        assert!(!other.projection_prefixes.borrow().is_empty());
        assert!(outer.finish(&tree, "failed outer scope").is_err());
        assert!(tree.projection_prefixes.borrow().is_empty());
        drop(outer);
        assert!(tree.active_projection.borrow().is_none());
        tree.local_to_surface_transform(tree.root).unwrap();
    }

    #[test]
    fn scope_survives_committed_geometry_and_unwind_releases_the_slot() {
        let mut tree = FigureTree::new();
        let child = tree.add_child_to(
            tree.root,
            Box::new(LocalFigure(Rectangle::new(3.0, 4.0, 10.0, 10.0))),
        );
        let scope = tree.begin_projection_scope();
        tree.local_to_surface_transform(child).unwrap();
        tree.set_bounds(child, 30.0, 40.0, 10.0, 10.0);
        let current = tree.local_to_surface_transform(child).unwrap();
        assert_eq!(current.transform_point(Point::ZERO), Point::new(30.0, 40.0));
        scope.finish(&tree, "committed geometry").unwrap();
        drop(scope);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _scope = tree.begin_projection_scope();
                panic!("extension panic");
            }))
            .is_err()
        );
        assert!(tree.active_projection.borrow().is_none());
        tree.local_to_surface_transform(child).unwrap();
    }
}

impl WorldPrefix {
    fn matches(
        &self,
        parent: Option<&std::rc::Rc<Self>>,
        local: Affine2D,
        incoming: Affine2D,
    ) -> bool {
        let same_parent = match (&self.parent, parent) {
            (None, None) => true,
            (Some(old), Some(current)) => old.ptr_eq(&std::rc::Rc::downgrade(current)),
            _ => false,
        };
        same_parent && self.local == local && self.incoming == incoming
    }
}

pub(crate) struct CoordinateQuerySession<'a> {
    tree: &'a FigureTree,
    scene: crate::figure::ProjectionScene<'a>,
    operation: Option<crate::figure::projection::ProjectionOperation>,
    consumers: RefCell<Vec<(FigureId, crate::figure::ProjectionScene<'a>)>>,
    candidates: RefCell<
        std::collections::BTreeMap<
            FigureId,
            std::rc::Rc<crate::figure::projection::ProjectionReceipt>,
        >,
    >,
    prefixes: RefCell<std::collections::BTreeMap<FigureId, std::rc::Rc<WorldPrefix>>>,
}

impl<'a> CoordinateQuerySession<'a> {
    pub(crate) fn new(
        tree: &'a FigureTree,
        budget: crate::figure::PreparationBudgetHandle,
    ) -> Result<Self, CoordinateQueryError> {
        if let Some(operation) = tree.active_projection.borrow().as_ref() {
            return Self::in_operation(tree, operation);
        }
        Ok(Self {
            tree,
            scene: crate::figure::ProjectionScene::new(tree, tree.root, budget)?,
            operation: None,
            consumers: RefCell::new(Vec::new()),
            candidates: RefCell::new(std::collections::BTreeMap::new()),
            prefixes: RefCell::new(std::collections::BTreeMap::new()),
        })
    }

    pub(crate) fn in_operation(
        tree: &'a FigureTree,
        operation: &crate::figure::projection::ProjectionOperation,
    ) -> Result<Self, CoordinateQueryError> {
        if let Some(error) = operation.coordinate_error() {
            return Err(error);
        }
        Ok(Self {
            tree,
            scene: operation.scene(tree)?,
            operation: Some(operation.clone()),
            consumers: RefCell::new(Vec::new()),
            candidates: RefCell::new(std::collections::BTreeMap::new()),
            prefixes: RefCell::new(std::collections::BTreeMap::new()),
        })
    }

    pub(crate) fn local_to_parent(
        &self,
        figure: FigureId,
    ) -> Result<Affine2D, CoordinateQueryError> {
        self.tree.validate_identity(figure)?;
        self.scene
            .budget()
            .consume("coordinate edge", 1)
            .map_err(crate::figure::ProjectionError::from)?;
        let bounds = self.tree.blocks[figure].figure_bounds();
        checked_transform(
            figure,
            Affine2D::from_translation(bounds.x, bounds.y),
            CoordinateFailure::NonFiniteInput,
        )
    }

    pub(crate) fn child_transform(
        &self,
        figure: FigureId,
    ) -> Result<Affine2D, CoordinateQueryError> {
        self.tree.validate_identity(figure)?;
        self.scene
            .budget()
            .consume("child projection edge", 1)
            .map_err(crate::figure::ProjectionError::from)?;
        let node = &self.tree.blocks[figure];
        let projection = node
            .capabilities
            .get(crate::figure::PROJECTION)
            .map_err(crate::figure::ProjectionError::from)?;
        let extra = if let Some(descriptor) = projection {
            let adopted = self.candidates.borrow().get(&figure).cloned();
            if let Some(receipt) = adopted {
                // Repeated consumers in this operation use the same output.
                // Its one live scope is still checked after every callback.
                receipt.contribution.child_transform.affine()
            } else {
                let provider = descriptor.resolve(node.figure.as_ref()).ok_or(
                    crate::figure::ProjectionError::Capability(
                        CapabilityQueryError::DescriptorTypeMismatch {
                            capability: crate::figure::PROJECTION.name(),
                        },
                    ),
                )?;
                let historical = self
                    .operation
                    .as_ref()
                    .and_then(|operation| operation.contributions.borrow().get(&figure).cloned())
                    .or_else(|| {
                        self.tree
                            .projection_contributions
                            .borrow()
                            .get(&figure)
                            .cloned()
                    });
                let qualified = historical
                    .as_ref()
                    .map(|receipt| receipt.qualify(&self.scene))
                    .transpose()?
                    .flatten();
                let (scope, receipt) = if let Some(scope) = qualified {
                    (scope, historical.expect("qualified receipt exists"))
                } else {
                    let scope = self.scene.owner_scope(figure)?;
                    let contribution = scope.evaluate(provider)?;
                    let receipt = std::rc::Rc::new(
                        scope.receipt(contribution, &self.tree.projection_retention)?,
                    );
                    (scope, receipt)
                };
                self.scene
                    .budget()
                    .allocate(
                        "coordinate projection consumer",
                        (std::mem::size_of::<crate::figure::ProjectionScene<'_>>()
                            + std::mem::size_of::<(
                                FigureId,
                                std::rc::Rc<crate::figure::projection::ProjectionReceipt>,
                            )>()) as u64,
                    )
                    .map_err(crate::figure::ProjectionError::from)?;
                let transform = receipt.contribution.child_transform.affine();
                self.consumers.borrow_mut().push((figure, scope));
                self.candidates.borrow_mut().insert(figure, receipt);
                transform
            }
        } else {
            // Unqualified extensions retain their actual callback. They receive
            // no contribution reuse authority.
            node.container().map_or(Affine2D::IDENTITY, |container| {
                container.child_transform().affine()
            })
        };
        let extra = checked_transform(figure, extra, CoordinateFailure::NonFiniteInput)?;
        let insets = node.state.insets;
        checked_transform(
            figure,
            Affine2D::from_translation(insets.left, insets.top) * extra,
            CoordinateFailure::DerivedOverflow,
        )
    }

    pub(crate) fn local_to_surface(
        &self,
        figure: FigureId,
    ) -> Result<Affine2D, CoordinateQueryError> {
        self.tree.validate_identity(figure)?;
        let result = (|| {
            let mut path = Vec::new();
            let mut current = Some(figure);
            let mut qualified = true;
            while let Some(id) = current {
                self.scene
                    .budget()
                    .allocate(
                        "coordinate prefix ancestry",
                        std::mem::size_of::<FigureId>() as u64,
                    )
                    .map_err(crate::figure::ProjectionError::from)?;
                path.push(id);
                let node = &self.tree.blocks[id];
                current = node.parent;
                // The requested node's children mapping is not consumed.
                if id != figure
                    && node.container().is_some()
                    && node
                        .capabilities
                        .get(crate::figure::PROJECTION)
                        .map_err(crate::figure::ProjectionError::from)?
                        .is_none()
                {
                    qualified = false;
                }
            }
            if qualified {
                return self.qualified_local_to_surface(figure, &path);
            }
            // Live callbacks preserve the original bottom-up call order, and
            // the entire prefix containing them remains uncached.
            let mut transform = Affine2D::IDENTITY;
            let mut current = figure;
            loop {
                transform = checked_transform(
                    figure,
                    self.local_to_parent(current)? * transform,
                    CoordinateFailure::DerivedOverflow,
                )?;
                let Some(parent) = self.tree.blocks[current].parent else {
                    break;
                };
                transform = checked_transform(
                    figure,
                    self.child_transform(parent)? * transform,
                    CoordinateFailure::DerivedOverflow,
                )?;
                current = parent;
            }
            Ok(transform)
        })();
        result.map_err(|error| match error {
            // Numerical errors retain the requested transform's subject.
            // Projection errors retain the provider/input's original source.
            CoordinateQueryError::Geometry { reason, .. } => {
                CoordinateQueryError::Geometry { figure, reason }
            }
            other => other,
        })
    }

    fn qualified_local_to_surface(
        &self,
        figure: FigureId,
        path: &[FigureId],
    ) -> Result<Affine2D, CoordinateQueryError> {
        let mut parent: Option<std::rc::Rc<WorldPrefix>> = None;
        let mut parent_id = None;
        for &id in path.iter().rev() {
            let local = self.local_to_parent(id)?;
            let incoming =
                parent_id.map_or(Ok(Affine2D::IDENTITY), |id| self.child_transform(id))?;
            let cached = self.prefixes.borrow().get(&id).cloned().or_else(|| {
                self.operation
                    .as_ref()
                    .and_then(|operation| operation.prefixes.borrow().get(&id).cloned())
                    .or_else(|| self.tree.projection_prefixes.borrow().get(&id).cloned())
            });
            let prefix = if let Some(cached) =
                cached.filter(|cached| cached.matches(parent.as_ref(), local, incoming))
            {
                cached
            } else {
                let base = parent
                    .as_ref()
                    .map_or(Affine2D::IDENTITY, |parent| parent.transform);
                let transform = checked_transform(
                    figure,
                    base * incoming * local,
                    CoordinateFailure::DerivedOverflow,
                )?;
                let bytes = std::mem::size_of::<WorldPrefix>()
                    + 2 * std::mem::size_of::<usize>()
                    + std::mem::size_of::<(FigureId, std::rc::Rc<WorldPrefix>)>()
                    + 4 * std::mem::size_of::<usize>();
                self.scene
                    .budget()
                    .allocate("coordinate world prefix", bytes as u64)
                    .map_err(crate::figure::ProjectionError::from)?;
                std::rc::Rc::new(WorldPrefix {
                    parent: parent.as_ref().map(std::rc::Rc::downgrade),
                    local,
                    incoming,
                    transform,
                    _retention: self
                        .tree
                        .projection_retention
                        .reserve(bytes)
                        .map_err(crate::figure::ProjectionError::from)?,
                })
            };
            if !self.prefixes.borrow().contains_key(&id) {
                self.scene
                    .budget()
                    .allocate(
                        "coordinate prefix candidate entry",
                        (std::mem::size_of::<(FigureId, std::rc::Rc<WorldPrefix>)>()
                            + 4 * std::mem::size_of::<usize>()) as u64,
                    )
                    .map_err(crate::figure::ProjectionError::from)?;
            }
            self.prefixes.borrow_mut().insert(id, prefix.clone());
            parent = Some(prefix);
            parent_id = Some(id);
        }
        Ok(parent
            .expect("validated Figure supplies a nonempty path")
            .transform)
    }

    pub(crate) fn finish<T>(
        &self,
        result: Result<T, CoordinateQueryError>,
    ) -> Result<T, CoordinateQueryError> {
        let result = self.finish_inner(result);
        if let Some(operation) = &self.operation {
            operation.remember(&result);
        }
        result
    }
    fn finish_inner<T>(
        &self,
        result: Result<T, CoordinateQueryError>,
    ) -> Result<T, CoordinateQueryError> {
        let value = result?;
        for (_, consumer) in self.consumers.borrow().iter() {
            consumer.validate("coordinate query return")?;
        }
        self.scene.check()?;
        // Verification can discover new nested subjects. Persist the completed
        // closure before installing, not only the provider-return receipt.
        for (figure, consumer) in self.consumers.borrow().iter().rev() {
            let receipt = self.candidates.borrow()[figure].clone();
            if !receipt.matches_consumption(consumer)? {
                let completed =
                    consumer.receipt(receipt.contribution, &self.tree.projection_retention)?;
                self.candidates
                    .borrow_mut()
                    .insert(*figure, std::rc::Rc::new(completed));
            }
        }
        self.scene.check()?;
        if let Some(operation) = &self.operation {
            for (_, consumer) in self.consumers.borrow().iter() {
                operation.retain(consumer)?;
            }
            operation
                .contributions
                .borrow_mut()
                .extend(std::mem::take(&mut *self.candidates.borrow_mut()));
            operation
                .prefixes
                .borrow_mut()
                .extend(std::mem::take(&mut *self.prefixes.borrow_mut()));
            return Ok(value);
        }
        // Install together only after the requested forward/inverse result and
        // all adopted consumer proofs have succeeded.
        self.tree
            .projection_contributions
            .borrow_mut()
            .extend(std::mem::take(&mut *self.candidates.borrow_mut()));
        self.tree
            .projection_prefixes
            .borrow_mut()
            .extend(std::mem::take(&mut *self.prefixes.borrow_mut()));
        Ok(value)
    }
}

/// Owns only the tree's scope slot, so source transactions can borrow the tree
/// mutably while their readers retain the same operation. Unwinding clears it.
pub(crate) struct ProjectionScope {
    slot: std::rc::Rc<RefCell<Option<crate::figure::projection::ProjectionOperation>>>,
    pub(crate) operation: crate::figure::projection::ProjectionOperation,
    owns_slot: bool,
}

pub(crate) struct DisplayProjectionInputs {
    figures: Vec<FigureId>,
    _retention: crate::text::TextContentLease,
}
impl Drop for ProjectionScope {
    fn drop(&mut self) {
        if self.owns_slot {
            self.slot.borrow_mut().take();
        }
    }
}
impl ProjectionScope {
    pub(crate) fn finish(
        &self,
        tree: &FigureTree,
        checkpoint: &'static str,
    ) -> Result<(), CoordinateQueryError> {
        if self.owns_slot {
            self.operation.publish(tree, checkpoint)?;
        } else {
            self.operation.checked_validate(tree, checkpoint)?;
        }
        Ok(())
    }
}

impl FigureTree {
    pub(crate) fn retain_projection_state(
        &self,
        bytes: usize,
    ) -> Result<crate::text::TextContentLease, crate::figure::ProjectionError> {
        self.projection_retention.reserve(bytes).map_err(Into::into)
    }

    pub(crate) fn projection_has_input_proofs(&self, figure: FigureId) -> bool {
        let operation = self.projection_operation();
        if let Some(receipt) = operation.contributions.borrow().get(&figure) {
            return receipt.has_input_proofs();
        }
        self.projection_contributions
            .borrow()
            .get(&figure)
            .is_some_and(|receipt| receipt.has_input_proofs())
    }

    pub(crate) fn display_projection_inputs(
        &self,
        figures: impl Iterator<Item = FigureId> + Clone,
    ) -> Result<DisplayProjectionInputs, crate::figure::ProjectionError> {
        let count = figures.clone().count();
        let bytes = count
            .checked_mul(std::mem::size_of::<FigureId>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<DisplayProjectionInputs>()))
            .ok_or(crate::text::TextContentError::AccountingOverflow)?;
        self.projection_operation()
            .budget()
            .allocate("display projection input identities", bytes as u64)?;
        let retention = self.retain_projection_state(bytes)?;
        let mut ids = Vec::with_capacity(count);
        ids.extend(figures);
        Ok(DisplayProjectionInputs {
            figures: ids,
            _retention: retention,
        })
    }

    pub(crate) fn present_projection_inputs(&mut self, inputs: DisplayProjectionInputs) {
        self.displayed_projection_inputs = Some(inputs);
    }

    pub(crate) fn has_projection_ancestor(&self, figure: FigureId) -> bool {
        let Some(inputs) = &self.displayed_projection_inputs else {
            return false;
        };
        let mut current = Some(figure);
        while let Some(id) = current {
            let Some(node) = self.blocks.get(id) else {
                break;
            };
            if inputs.figures.binary_search(&id).is_ok() {
                return true;
            }
            current = node.parent;
        }
        false
    }

    pub(crate) fn begin_projection_scope(&self) -> ProjectionScope {
        let operation = self.projection_operation();
        let mut slot = self.active_projection.borrow_mut();
        let owns_slot = slot.is_none();
        if owns_slot {
            *slot = Some(operation.clone());
        }
        ProjectionScope {
            slot: self.active_projection.clone(),
            operation,
            owns_slot,
        }
    }

    /// The complete Core client-origin and checked extra container transform.
    /// Presentation effects are applied by their own geometry consumer.
    pub(crate) fn checked_child_transform(
        &self,
        figure: FigureId,
    ) -> Result<Affine2D, CoordinateQueryError> {
        let session = CoordinateQuerySession::new(
            self,
            crate::figure::PreparationBudgetHandle::new(self.projection_budget),
        )?;
        session.finish(session.child_transform(figure))
    }

    pub(crate) fn publish_projection_candidates(
        &self,
        contributions: std::collections::BTreeMap<
            FigureId,
            std::rc::Rc<crate::figure::projection::ProjectionReceipt>,
        >,
        prefixes: std::collections::BTreeMap<FigureId, std::rc::Rc<WorldPrefix>>,
    ) {
        self.projection_contributions
            .borrow_mut()
            .extend(contributions);
        self.projection_prefixes.borrow_mut().extend(prefixes);
    }

    pub(crate) fn projection_operation(&self) -> crate::figure::projection::ProjectionOperation {
        if let Some(operation) = self.active_projection.borrow().as_ref() {
            return operation.clone();
        }
        crate::figure::projection::ProjectionOperation::new(
            crate::figure::PreparationBudgetHandle::new(self.projection_budget),
        )
    }

    /// Returns membership in this tree's live identity set without reading facts.
    pub fn contains(&self, id: FigureId) -> bool {
        id.namespace() == self.namespace() && self.blocks.contains_key(id)
    }

    /// Returns the node for a Figure identity.
    pub fn node(&self, id: FigureId) -> Result<&FigureNode, TreeQueryError> {
        self.validate_identity(id)?;
        Ok(&self.blocks[id])
    }

    /// The immutable drawing and metrics retained from the last successful preparation.
    ///
    /// This raw diagnostic view may retain an invalid projection after a rejected candidate.
    /// Routing authority is checked separately by `RouteSceneQuery::prepared_scene`.
    pub fn prepared_presentation(
        &self,
        id: FigureId,
    ) -> Result<Option<&crate::figure::FigurePresentation>, TreeQueryError> {
        Ok(self
            .node(id)?
            .prepared
            .as_ref()
            .map(|prepared| &prepared.presentation))
    }

    /// Queries one currently admitted capability descriptor without causing mutation.
    pub fn capability<C: 'static>(
        &self,
        figure: FigureId,
        key: CapabilityKey<C>,
    ) -> Result<Option<&C>, CapabilityQueryError> {
        if figure.namespace() != self.namespace() {
            return Err(CapabilityQueryError::ForeignRuntime(figure));
        }
        let node = self
            .blocks
            .get(figure)
            .ok_or(CapabilityQueryError::UnknownOrDisposedFigure(figure))?;
        node.capabilities.get(key)
    }

    pub(crate) fn is_layered_pane(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|node| node.child_policy() == ChildPolicy::Layered)
    }

    pub(crate) fn is_layer_figure(&self, id: FigureId) -> bool {
        self.capability(id, LAYER)
            .is_ok_and(|layer| layer.is_some())
    }

    /// 返回指定父节点的 child 顺序。
    ///
    /// 顺序与 Draw2D 一致：数组靠前的 child 先绘制，靠后的 child 后绘制并位于更高 z-order。
    pub fn child_order(&self, parent_id: FigureId) -> Result<Vec<FigureId>, TreeQueryError> {
        Ok(self.node(parent_id)?.children.clone())
    }

    pub(crate) fn child_at_index(
        &self,
        parent_id: FigureId,
        index: usize,
    ) -> Result<Option<FigureId>, TreeQueryError> {
        Ok(self.node(parent_id)?.children.get(index).copied())
    }

    /// Returns the direct parent of a Figure.
    pub fn parent_id(&self, figure_id: FigureId) -> Result<Option<FigureId>, TreeQueryError> {
        Ok(self.node(figure_id)?.parent)
    }

    /// Returns whether a node is currently attached to this tree's root.
    pub fn is_attached(&self, figure_id: FigureId) -> Result<bool, TreeQueryError> {
        self.validate_identity(figure_id)?;
        let mut current = Some(figure_id);
        for _ in 0..=self.blocks.len() {
            let Some(id) = current else {
                return Ok(false);
            };
            if id == self.root {
                return Ok(true);
            }
            current = self.blocks.get(id).and_then(|block| block.parent);
        }
        Ok(false)
    }

    /// 返回 child 在父节点内的 z-order index。
    ///
    /// index 越大表示越靠前绘制、越靠上层。
    pub fn child_z_index(
        &self,
        parent_id: FigureId,
        child_id: FigureId,
    ) -> Result<Option<usize>, TreeQueryError> {
        self.validate_identity(parent_id)?;
        self.validate_identity(child_id)?;
        Ok(self.blocks[parent_id]
            .children
            .iter()
            .position(|&id| id == child_id))
    }

    /// 获取指定块的 Figure bounds。
    pub fn figure_bounds(&self, id: FigureId) -> Result<Rectangle, TreeQueryError> {
        Ok(self.node(id)?.figure_bounds())
    }

    /// Returns the committed private-component revision for a Figure.
    pub fn component_revision(&self, id: FigureId) -> Result<u64, TreeQueryError> {
        Ok(self.node(id)?.component_revision)
    }

    pub fn is_connection_figure(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.connection().is_some())
    }

    pub fn connection_route_points(
        &self,
        id: FigureId,
    ) -> Result<Option<&PointList>, TreeQueryError> {
        Ok(self
            .node(id)?
            .connection()
            .map(|connection| connection.route_points()))
    }

    pub fn connection_stroke_color(
        &self,
        id: FigureId,
    ) -> Result<Option<crate::Color>, TreeQueryError> {
        Ok(self
            .node(id)?
            .connection()
            .map(|connection| connection.connection_stroke_color()))
    }

    pub fn point_list_points(&self, id: FigureId) -> Result<Option<Vec<Point>>, TreeQueryError> {
        let block = self.node(id)?;
        let bounds = block.figure_bounds();
        Ok(block.point_list().map(|points| {
            points
                .local_points()
                .iter()
                .map(|point| Point::new(point.x() + bounds.x, point.y() + bounds.y))
                .collect()
        }))
    }

    pub fn image_display_state(
        &self,
        id: FigureId,
    ) -> Result<crate::figure::ImageDisplayState, crate::figure::ShapeMutationError> {
        self.validate_identity(id).map_err(|error| match error {
            TreeQueryError::ForeignFigure(id) => {
                crate::figure::ShapeMutationError::ForeignFigure(id)
            }
            TreeQueryError::UnknownFigure(id) => {
                crate::figure::ShapeMutationError::UnknownFigure(id)
            }
        })?;
        self.image_figure(id)
            .map(crate::figure::ImageFigure::display_state)
            .ok_or_else(|| {
                if self.contains(id) {
                    crate::figure::ShapeMutationError::WrongCapability(id)
                } else {
                    crate::figure::ShapeMutationError::UnknownFigure(id)
                }
            })
    }

    pub fn point_list_stroke_style(
        &self,
        id: FigureId,
    ) -> Result<Option<crate::render::StrokeStyle>, TreeQueryError> {
        Ok(self
            .node(id)?
            .point_list()
            .map(|points| points.stroke_style().clone()))
    }

    /// 返回节点从 FigureTree 根节点开始计算的深度。
    pub fn depth(&self, id: FigureId) -> Result<usize, TreeQueryError> {
        Ok(self.node(id)?.depth)
    }

    /// 返回节点自身的本地可见性标志。
    pub fn is_visible(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.is_visible)
    }

    /// 返回节点自身的本地启用标志。
    pub fn is_enabled(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.is_enabled)
    }

    pub fn is_opaque(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.is_opaque)
    }

    pub fn insets(&self, id: FigureId) -> Result<Insets, TreeQueryError> {
        Ok(self.node(id)?.insets)
    }

    pub fn border_is_effectively_opaque(
        &self,
        id: FigureId,
    ) -> Result<Option<bool>, TreeQueryError> {
        let block = self.node(id)?;
        let style = self.resolved_style(id)?;
        Ok(block
            .figure
            .get_border()
            .map(|border| border.is_opaque() && style.alpha == crate::graphics::Opacity::OPAQUE))
    }

    pub fn figure_style(&self, id: FigureId) -> Result<&FigureStyle, TreeQueryError> {
        Ok(&self.node(id)?.style)
    }

    pub fn resolved_style(&self, id: FigureId) -> Result<ResolvedStyle, TreeQueryError> {
        self.validate_identity(id)?;
        Ok(self
            .resolved_style_with_node_visits(id)
            .expect("live immutable parent chain")
            .0)
    }

    fn resolved_style_with_node_visits(&self, id: FigureId) -> Option<(ResolvedStyle, u64)> {
        self.blocks.get(id)?;
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(node_id) = current {
            let node = self.blocks.get(node_id)?;
            chain.push(node_id);
            current = node.parent;
        }
        let node_visits = u64::try_from(chain.len()).unwrap_or(u64::MAX);
        let mut result = ResolvedStyle::default();
        for node_id in chain.into_iter().rev() {
            result.apply_override(&self.blocks[node_id].style);
        }
        Some((result, node_visits))
    }

    pub(crate) fn preparation_input_styles(
        &self,
        budget: &crate::figure::PreparationBudgetHandle,
    ) -> Result<std::collections::HashMap<FigureId, ResolvedStyle>, TextError> {
        budget.allocate("preparation input styles", self.blocks.len() as u64)?;
        let (styles, _) = self.resolved_styles_for(|_| true);
        Ok(styles.into_iter().collect())
    }

    pub(super) fn resolved_styles_for(
        &self,
        mut include: impl FnMut(&FigureNode) -> bool,
    ) -> (Vec<(FigureId, ResolvedStyle)>, u64) {
        let mut matches = Vec::new();
        let mut nodes_visited = 0_u64;
        let mut stack = vec![(self.root, ResolvedStyle::default())];
        while let Some((id, mut style)) = stack.pop() {
            let node = &self.blocks[id];
            nodes_visited = nodes_visited.saturating_add(1);
            style.apply_override(&node.style);
            if include(node) {
                matches.push((id, style.clone()));
            }
            for child in node.children.iter().rev() {
                stack.push((*child, style.clone()));
            }
        }
        (matches, nodes_visited)
    }

    pub(crate) fn tooltip_source(&self, hit: FigureId) -> Option<(FigureId, String)> {
        let mut current = Some(hit);
        while let Some(id) = current {
            let node = self.blocks.get(id)?;
            match &node.style.tooltip {
                Some(Some(text)) => return Some((id, text.clone())),
                Some(None) => return None,
                None => current = node.parent,
            }
        }
        None
    }

    pub(crate) fn label(&self, id: FigureId) -> Option<&LabelFigure> {
        self.blocks.get(id)?.label()
    }

    pub(crate) fn image_figure(&self, id: FigureId) -> Option<&ImageFigure> {
        self.blocks.get(id)?.figure.as_ref().as_any().downcast_ref()
    }

    pub(crate) fn clickable_snapshot(&self, id: FigureId) -> Option<ClickableSnapshot> {
        self.blocks
            .get(id)?
            .clickable()
            .map(|clickable| clickable.clickable_model().snapshot())
    }

    pub(crate) fn activity_ids(&self) -> Vec<FigureId> {
        self.capability_ids(crate::figure::ACTIVITY)
            .filter(|id| self.blocks[*id].activity().is_some())
            .collect()
    }

    pub(crate) fn clickable_ids(&self) -> Vec<FigureId> {
        self.capability_ids(CLICKABLE)
            .filter(|id| self.blocks[*id].clickable().is_some())
            .collect()
    }

    pub(crate) fn capability_ids<C: 'static>(
        &self,
        _key: CapabilityKey<C>,
    ) -> impl Iterator<Item = FigureId> + '_ {
        self.capability_members
            .get(&std::any::TypeId::of::<C>())
            .into_iter()
            .flat_map(|members| members.iter().copied())
    }

    pub(crate) fn border_snapshot(&self, id: FigureId) -> Option<&BorderSnapshot> {
        self.blocks.get(id)?.border_snapshot.as_ref()
    }

    /// 返回节点沿父链传播后的有效可见性。
    pub fn is_effectively_visible(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        self.validate_identity(id)?;
        Ok(self.effective_flag_from(id, |block| block.is_visible))
    }

    /// 返回节点沿父链传播后的有效启用状态。
    pub fn is_effectively_enabled(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        self.validate_identity(id)?;
        Ok(self.effective_flag_from(id, |block| block.is_enabled))
    }

    pub fn is_focusable(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.is_focusable)
    }

    pub fn is_focus_traversable(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.is_focus_traversable)
    }

    pub fn can_request_focus(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.is_attached(id)?
            && self.is_effectively_visible(id)?
            && self.is_effectively_enabled(id)?
            && self.is_focusable(id)?)
    }

    pub fn can_traverse_focus(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.is_attached(id)?
            && self.is_effectively_visible(id)?
            && self.is_effectively_enabled(id)?
            && self.is_focus_traversable(id)?)
    }

    pub(crate) fn can_retain_focus(&self, id: FigureId) -> bool {
        self.is_attached(id).unwrap_or(false)
            && self.is_effectively_visible(id).unwrap_or(false)
            && self.is_effectively_enabled(id).unwrap_or(false)
            && (self.is_focusable(id).unwrap_or(false)
                || self.is_focus_traversable(id).unwrap_or(false))
    }

    /// 返回 node-local 到 parent content domain 的变换。
    pub fn local_to_parent_transform(
        &self,
        figure_id: FigureId,
    ) -> Result<Affine2D, CoordinateQueryError> {
        self.validate_identity(figure_id)?;
        let session = CoordinateQuerySession::new(
            self,
            crate::figure::PreparationBudgetHandle::new(self.projection_budget),
        )?;
        session.finish(session.local_to_parent(figure_id))
    }

    /// 返回 parent content domain 到 node-local 的变换。
    pub fn parent_to_local_transform(
        &self,
        figure_id: FigureId,
    ) -> Result<Option<Affine2D>, CoordinateQueryError> {
        self.validate_identity(figure_id)?;
        let session = CoordinateQuerySession::new(
            self,
            crate::figure::PreparationBudgetHandle::new(self.projection_budget),
        )?;
        session.finish(
            session
                .local_to_parent(figure_id)
                .and_then(|transform| checked_coordinate_inverse(figure_id, transform)),
        )
    }

    /// 返回 node-local 到 logical surface 的完整父链变换。
    pub fn local_to_surface_transform(
        &self,
        figure_id: FigureId,
    ) -> Result<Affine2D, CoordinateQueryError> {
        self.validate_identity(figure_id)?;
        let session = CoordinateQuerySession::new(
            self,
            crate::figure::PreparationBudgetHandle::new(self.projection_budget),
        )?;
        session.finish(session.local_to_surface(figure_id))
    }

    /// 返回 logical surface domain 到 node-local 的完整父链变换。
    pub fn surface_to_local_transform(
        &self,
        figure_id: FigureId,
    ) -> Result<Option<Affine2D>, CoordinateQueryError> {
        self.validate_identity(figure_id)?;
        let session = CoordinateQuerySession::new(
            self,
            crate::figure::PreparationBudgetHandle::new(self.projection_budget),
        )?;
        session.finish(
            session
                .local_to_surface(figure_id)
                .and_then(|transform| checked_coordinate_inverse(figure_id, transform)),
        )
    }

    /// Returns the complete child-content to logical-surface transform.
    pub fn child_content_to_surface_transform(
        &self,
        figure_id: FigureId,
    ) -> Result<Affine2D, CoordinateQueryError> {
        self.validate_identity(figure_id)?;
        let session = CoordinateQuerySession::new(
            self,
            crate::figure::PreparationBudgetHandle::new(self.projection_budget),
        )?;
        session.finish((|| {
            let local_to_surface = session.local_to_surface(figure_id)?;
            checked_transform(
                figure_id,
                local_to_surface * session.child_transform(figure_id)?,
                CoordinateFailure::DerivedOverflow,
            )
        })())
    }

    pub(crate) fn presentation_envelope(
        &self,
        figure_id: FigureId,
        local_transform: Affine2D,
    ) -> Option<Rectangle> {
        if !self.is_effectively_visible(figure_id).ok()? {
            return None;
        }
        let root_to_surface = self.local_to_surface_transform(figure_id).ok()?;
        let surface_to_root = root_to_surface.inverse()?;
        let mut ids = vec![figure_id];
        ids.extend(self.descendant_ids(figure_id).ok()?);
        let mut local_envelope = None;
        for id in ids {
            if !self.is_effectively_visible(id).ok()? {
                continue;
            }
            let node = self.blocks.get(id)?;
            let descendant_to_root = surface_to_root * self.local_to_surface_transform(id).ok()?;
            let visual = super::transform_rectangle(descendant_to_root, node.visual_bounds())?;
            local_envelope =
                Some(local_envelope.map_or(visual, |envelope: Rectangle| envelope.union(visual)));
        }
        super::transform_rectangle(root_to_surface * local_transform, local_envelope?)
    }

    fn effective_flag_from(
        &self,
        mut figure_id: FigureId,
        local_flag: fn(&FigureNode) -> bool,
    ) -> bool {
        for _ in 0..self.blocks.len() {
            let Some(block) = self.blocks.get(figure_id) else {
                return false;
            };
            if !local_flag(block) {
                return false;
            }
            let Some(parent_id) = block.parent else {
                return true;
            };
            figure_id = parent_id;
        }
        false
    }
}
