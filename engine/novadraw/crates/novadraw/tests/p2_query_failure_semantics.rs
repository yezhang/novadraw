use std::cell::Cell;
use std::rc::Rc;

use novadraw::figure::{
    CONTAINER, ContainerCapability, Figure, FigureCapabilityBuilder,
    FigureCapabilityRegistrationError, FigureContainer, FigureMeasurement, MeasureConstraints,
    RectangleFigure,
};
use novadraw::geometry::{Dimension, Rectangle};
use novadraw::layout::{
    LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery, MeasurementFailure,
};
use novadraw::runtime::{FramePreparationError, Runtime};
use novadraw::tree::{FigureId, FigureTree, TreeQueryError, ValidationError};

const CONSTRAINTS: MeasureConstraints = MeasureConstraints::UNBOUNDED;

mod support;

struct ExternalMeasurement {
    value: Rc<Cell<FigureMeasurement>>,
    calls: Rc<Cell<usize>>,
    target: Option<FigureId>,
}

impl LayoutManager for ExternalMeasurement {
    fn preferred_measurement(
        &self,
        _: FigureId,
        constraints: MeasureConstraints,
        query: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        self.calls.set(self.calls.get() + 1);
        if let Some(target) = self.target {
            return query.preferred_measurement(target, constraints);
        }
        Ok(self.value.get())
    }

    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        query: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(self
            .preferred_measurement(container, constraints, query)?
            .size())
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

fn tree_with_layout(
    value: Rc<Cell<FigureMeasurement>>,
    calls: Rc<Cell<usize>>,
) -> (FigureTree, FigureId) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(
            RectangleFigure::from_bounds(Rectangle::ZERO).expect("valid figure bounds"),
        ))
        .unwrap();
    tree.builder()
        .set_layout_manager(
            root,
            Box::new(ExternalMeasurement {
                value,
                calls,
                target: None,
            }),
        )
        .unwrap();
    (tree, root)
}

#[test]
fn invalid_callback_success_is_rejected_and_never_cached() {
    let value = Rc::new(Cell::new(FigureMeasurement::default()));
    let calls = Rc::new(Cell::new(0));
    let (tree, root) = tree_with_layout(value.clone(), calls.clone());
    let invalid = [
        (
            FigureMeasurement::new(f64::NAN, 1.0, None),
            MeasurementFailure::InvalidSize,
        ),
        (
            FigureMeasurement::new(-1.0, 1.0, None),
            MeasurementFailure::InvalidSize,
        ),
        (
            FigureMeasurement::new(f64::INFINITY, 1.0, None),
            MeasurementFailure::InvalidSize,
        ),
        (
            FigureMeasurement::new(1.0, 1.0, Some(f64::NAN)),
            MeasurementFailure::InvalidBaseline,
        ),
        (
            FigureMeasurement::new(1.0, 1.0, Some(f64::INFINITY)),
            MeasurementFailure::InvalidBaseline,
        ),
        (
            FigureMeasurement::new(1.0, 1.0, Some(-1.0)),
            MeasurementFailure::InvalidBaseline,
        ),
        (
            FigureMeasurement::new(1.0, 1.0, Some(2.0)),
            MeasurementFailure::InvalidBaseline,
        ),
    ];
    for (measurement, reason) in invalid {
        value.set(measurement);
        let before = calls.get();
        for _ in 0..2 {
            assert_eq!(
                tree.preferred_measurement(root, CONSTRAINTS),
                Err(LayoutError::InvalidMeasurement {
                    figure: root,
                    reason
                })
            );
        }
        assert_eq!(calls.get(), before + 2);
    }
    value.set(FigureMeasurement::new(4.0, 8.0, Some(8.0)));
    assert_eq!(
        tree.preferred_measurement(root, CONSTRAINTS),
        Ok(value.get())
    );
    let before = calls.get();
    assert_eq!(
        tree.preferred_measurement(root, CONSTRAINTS),
        Ok(value.get())
    );
    assert_eq!(calls.get(), before);
}

#[test]
fn minimum_callback_failure_is_not_cached_and_can_retry() {
    let value = Rc::new(Cell::new(FigureMeasurement::new(-1.0, 0.0, None)));
    let calls = Rc::new(Cell::new(0));
    let (tree, root) = tree_with_layout(value.clone(), calls.clone());
    for axis in [f64::NAN, -1.0, f64::INFINITY] {
        for size in [Dimension::new(axis, 0.0), Dimension::new(0.0, axis)] {
            value.set(FigureMeasurement::new(size.width, size.height, None));
            for _ in 0..2 {
                assert_eq!(
                    tree.minimum_size(root, CONSTRAINTS),
                    Err(LayoutError::InvalidMeasurement {
                        figure: root,
                        reason: MeasurementFailure::InvalidSize
                    })
                );
            }
        }
    }
    assert_eq!(calls.get(), 12);
    value.set(FigureMeasurement::default());
    assert_eq!(tree.minimum_size(root, CONSTRAINTS), Ok(Dimension::ZERO));
}

#[test]
fn normal_invalidation_retries_measurements_without_reusing_failed_values() {
    let value = Rc::new(Cell::new(FigureMeasurement::new(4.0, 8.0, Some(6.0))));
    let calls = Rc::new(Cell::new(0));
    let (tree, root) = tree_with_layout(value.clone(), calls.clone());
    let mut runtime = Runtime::new(tree);
    assert_eq!(
        runtime.tree().preferred_measurement(root, CONSTRAINTS),
        Ok(value.get())
    );
    let before = calls.get();
    value.set(FigureMeasurement::new(f64::NAN, 8.0, None));
    // The external change has not invalidated the owner's already successful cache.
    assert_eq!(
        runtime
            .tree()
            .preferred_measurement(root, CONSTRAINTS)
            .unwrap()
            .width,
        4.0
    );
    assert_eq!(calls.get(), before);
    runtime.figure(root).unwrap().revalidate().unwrap();
    for _ in 0..2 {
        assert_eq!(
            runtime.tree().preferred_measurement(root, CONSTRAINTS),
            Err(LayoutError::InvalidMeasurement {
                figure: root,
                reason: MeasurementFailure::InvalidSize
            })
        );
    }
    assert_eq!(calls.get(), before + 2);
    value.set(FigureMeasurement::new(10.0, 12.0, Some(9.0)));
    runtime.figure(root).unwrap().revalidate().unwrap();
    assert_eq!(
        runtime.tree().preferred_measurement(root, CONSTRAINTS),
        Ok(value.get())
    );
    support::prepare_next_submission(&mut runtime).unwrap();
    assert!(runtime.observations().stable_query().is_ok());
}

struct Intrinsic(FigureMeasurement);

impl Figure for Intrinsic {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "ExternalIntrinsic"
    }
    fn intrinsic_measurement(&self, _: MeasureConstraints) -> FigureMeasurement {
        self.0
    }
    fn intrinsic_minimum_measurement(&self, constraints: MeasureConstraints) -> FigureMeasurement {
        self.intrinsic_measurement(constraints)
    }
}

#[test]
fn intrinsic_minimum_baseline_is_checked_before_discarding_it() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(Intrinsic(FigureMeasurement::new(
            1.0,
            1.0,
            Some(2.0),
        ))))
        .unwrap();
    let error = LayoutError::InvalidMeasurement {
        figure: root,
        reason: MeasurementFailure::InvalidBaseline,
    };
    assert_eq!(
        tree.preferred_measurement(root, CONSTRAINTS),
        Err(error.clone())
    );
    assert_eq!(tree.minimum_size(root, CONSTRAINTS), Err(error));
}

struct Projection;

impl Figure for Projection {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "ExternalProjection"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for Projection {
    fn project_preferred_measurement(
        &self,
        mut measurement: FigureMeasurement,
    ) -> FigureMeasurement {
        measurement.width *= 2.0;
        measurement
    }
    fn project_minimum_size(&self, size: Dimension) -> Dimension {
        Dimension::new(size.width * 2.0, size.height)
    }
}

#[test]
fn projection_overflow_is_rejected_before_cache_publication() {
    let value = Rc::new(Cell::new(FigureMeasurement::new(f64::MAX, 1.0, None)));
    let calls = Rc::new(Cell::new(0));
    let mut tree = FigureTree::new();
    let root = tree.builder().set_contents(Box::new(Projection)).unwrap();
    tree.builder()
        .set_layout_manager(
            root,
            Box::new(ExternalMeasurement {
                value: value.clone(),
                calls: calls.clone(),
                target: None,
            }),
        )
        .unwrap();
    let error = LayoutError::InvalidMeasurement {
        figure: root,
        reason: MeasurementFailure::DerivedOverflow,
    };
    for _ in 0..2 {
        assert_eq!(
            tree.preferred_measurement(root, CONSTRAINTS),
            Err(error.clone())
        );
        assert_eq!(tree.minimum_size(root, CONSTRAINTS), Err(error.clone()));
    }
    assert_eq!(calls.get(), 4);
    assert_eq!(
        tree.maximum_size(root),
        Ok(Dimension::new(f64::INFINITY, f64::INFINITY))
    );
    value.set(FigureMeasurement::new(3.0, 1.0, None));
    assert_eq!(
        tree.preferred_measurement(root, CONSTRAINTS).unwrap().width,
        6.0
    );
    assert_eq!(tree.minimum_size(root, CONSTRAINTS).unwrap().width, 6.0);
}

struct QueryFailure {
    target: FigureId,
    survivor: FigureId,
    error: LayoutError,
}

impl LayoutManager for QueryFailure {
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        Err(self.error.clone())
    }
    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Err(self.error.clone())
    }
    fn layout(
        &mut self,
        container: FigureId,
        query: &LayoutSceneQuery<'_>,
        output: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        output.set_child_bounds(self.survivor, Rectangle::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(query.children(self.target), Err(self.error.clone()));
        assert_eq!(
            query.constraint(self.target).err(),
            Some(self.error.clone())
        );
        assert_eq!(query.container_bounds(self.target), Err(self.error.clone()));
        assert_eq!(
            query.preferred_measurement(self.target, CONSTRAINTS),
            Err(self.error.clone())
        );
        assert_eq!(
            query.minimum_size(self.target, CONSTRAINTS),
            Err(self.error.clone())
        );
        assert_eq!(query.maximum_size(self.target), Err(self.error.clone()));
        assert_eq!(query.freeform_extent(self.target), Err(self.error.clone()));
        assert_eq!(query.content_scale(self.target), Err(self.error.clone()));
        assert_eq!(
            query.constraint_as::<Rectangle>(container, self.target),
            Err(self.error.clone())
        );
        query.preferred_measurement(self.target, CONSTRAINTS)?;
        Ok(())
    }
}

#[test]
fn foreign_and_retired_queries_discard_the_entire_candidate_output() {
    for foreign in [false, true] {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::from_bounds(Rectangle::ZERO).expect("valid figure bounds"),
            ))
            .unwrap();
        let survivor = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(
                RectangleFigure::new(10.0, 20.0, 30.0, 40.0).expect("valid figure bounds"),
            ))
            .unwrap();
        support::prepare_next_submission(&mut runtime).unwrap();
        let baseline_epoch = runtime.observations().stable_query().unwrap().epoch();
        let retired = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(
                RectangleFigure::from_bounds(Rectangle::ZERO).expect("valid figure bounds"),
            ))
            .unwrap();
        runtime.figure(root).unwrap().remove(retired).unwrap();
        let replacement = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(
                RectangleFigure::from_bounds(Rectangle::ZERO).expect("valid figure bounds"),
            ))
            .unwrap();
        assert_ne!(retired, replacement);
        let mut other = Runtime::empty();
        let other_id = other
            .set_contents(Box::new(
                RectangleFigure::from_bounds(Rectangle::ZERO).expect("valid figure bounds"),
            ))
            .unwrap();
        let target = if foreign { other_id } else { retired };
        let error = LayoutError::Identity(if foreign {
            TreeQueryError::ForeignFigure(target)
        } else {
            TreeQueryError::UnknownFigure(target)
        });
        let before = runtime.tree().figure_bounds(survivor);
        runtime
            .figure(root)
            .unwrap()
            .set_layout_manager(Box::new(QueryFailure {
                target,
                survivor,
                error: error.clone(),
            }))
            .unwrap();
        assert_eq!(
            runtime.tree().preferred_measurement(root, CONSTRAINTS),
            Err(error.clone())
        );
        assert_eq!(
            runtime.tree().minimum_size(root, CONSTRAINTS),
            Err(error.clone())
        );
        assert_eq!(runtime.tree().maximum_size(target), Err(error.clone()));
        // The callback queues a legal change before failing through the public query.
        // The owner must reject that complete candidate, including the legal prefix.
        assert_eq!(
            runtime.frames().stabilize_for_query(),
            Err(FramePreparationError::Validation(ValidationError::Layout(
                error
            )))
        );
        assert_eq!(runtime.tree().figure_bounds(survivor), before);
        assert!(matches!(
            runtime.observations().stable_query(),
            Err(novadraw::runtime::StableQueryError::NotStable { latest_stable_epoch, .. })
                if latest_stable_epoch == baseline_epoch
        ));
        assert!(
            runtime
                .tree()
                .preferred_measurement(replacement, CONSTRAINTS)
                .is_ok()
        );
    }
}

#[test]
fn valid_absence_and_unbounded_maximum_remain_successful() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(
            RectangleFigure::from_bounds(Rectangle::ZERO).expect("valid figure bounds"),
        ))
        .unwrap();
    assert_eq!(
        tree.preferred_measurement(root, CONSTRAINTS),
        Ok(FigureMeasurement::default())
    );
    assert_eq!(tree.minimum_size(root, CONSTRAINTS), Ok(Dimension::ZERO));
    assert_eq!(
        tree.maximum_size(root),
        Ok(Dimension::new(f64::INFINITY, f64::INFINITY))
    );
}
