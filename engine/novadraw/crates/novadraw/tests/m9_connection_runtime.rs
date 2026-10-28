mod support;

use support::{prepare_next_submission, record_full_submission};

use std::{any::TypeId, marker::PhantomData};

use novadraw::connection::{
    Bendpoint, BendpointConnectionRouter, BendpointConstraint, ChopboxAnchor, ConnectionFigure,
    ConnectionLocator, ConnectionResolution, ConnectionRouter, ConnectionRuntimeError,
    CoordinateSpace, DecorationError, DirectRouter, EndpointLocator, FanRouter, LocatorError,
    MANHATTAN_DEFAULT_LANE_SPACING, MANHATTAN_DEFAULT_MINIMUM_STUB, ManhattanConnectionRouter,
    PathFractionLocator, PolygonDecorationFigure, PolylineDecorationFigure, RouteError,
    RouteOutput, RouteRequest, RouterBinding, UnresolvedConnection, XYAnchor,
};
use novadraw::container::ViewportFigure;
use novadraw::geometry::{Point, PointList, Vec2};
use novadraw::layout::{XYConstraint, XYLayout};
use novadraw::render::{
    BackendCapabilities, DEFAULT_STROKE_MITER_LIMIT, RenderDisposition, SurfaceInfo,
    command::RenderCommandKind,
};
use novadraw::runtime::RuntimeMutationError;
use novadraw::runtime::{
    FigureComponentContext, FigureComponentUpdate, FigureUpdateEffect, FigureUpdateEffectContext,
    PreparedFigureUpdate,
};
use novadraw::{figure::RectangleFigure, runtime::Runtime, tree::FigureId};

struct ConstraintA;
struct ConstraintB;

struct MoveConnectionBatch(Vec<FigureId>);

impl FigureUpdateEffect for MoveConnectionBatch {
    fn prepare(
        self: Box<Self>,
        context: &mut FigureUpdateEffectContext<'_>,
    ) -> Result<(), RuntimeMutationError> {
        for parent in self.0 {
            context.reparent(context.figure_id(), parent)?;
        }
        Ok(())
    }
}

impl FigureComponentUpdate for MoveConnectionBatch {
    type Figure = ConnectionFigure;
    type Prepared = ();
    type Error = ();

    fn prepare(
        self,
        _: &ConnectionFigure,
        _: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<()>, ()> {
        Ok(PreparedFigureUpdate::geometry_and_paint(()).with_effect(self))
    }

    fn commit(_: (), _: &mut ConnectionFigure) {}
}

struct TypedRouter<T>(PhantomData<T>);

impl<T: 'static> ConnectionRouter for TypedRouter<T> {
    fn route(&self, _request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        Err(RouteError::InvalidConstraint)
    }

    fn constraint_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<T>())
    }

    fn constraint_type_name(&self) -> Option<&'static str> {
        Some(std::any::type_name::<T>())
    }
}

fn runtime_fixture() -> (Runtime, FigureId, FigureId, FigureId, FigureId) {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 300.0)))
        .expect("valid Runtime mutation");
    let source = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)))
        .expect("valid Runtime mutation");
    let target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(300.0, 170.0, 100.0, 60.0)))
        .expect("valid Runtime mutation");
    let connection = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .expect("valid Runtime mutation");
    (runtime, root, source, target, connection)
}

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 500.0,
        logical_height: 300.0,
        pixel_width: 500,
        pixel_height: 300,
        scale_factor: 1.0,
    }
}

#[test]
fn inherited_router_defaults_to_draw2d_equivalent_direct_routing() {
    let (mut runtime, root, source, _target, connection_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Inherited { layer: root },
            None,
        )
        .unwrap();

    let output = runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert_eq!(output.points().len(), 2);
    assert_eq!(output.points().get(0), output.points().get(1));
}

#[test]
fn independent_routing_supplies_one_order_entry_per_connection() {
    const CONNECTION_COUNT: usize = 128;

    let (mut runtime, root, source, target, first_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.connections().direct_router();
    let mut figures = vec![first_figure];
    for _ in 1..CONNECTION_COUNT {
        figures.push(
            runtime
                .figure(root)
                .unwrap()
                .add(Box::new(ConnectionFigure::new()))
                .unwrap(),
        );
    }
    let connections = figures
        .into_iter()
        .map(|figure| {
            runtime
                .connections()
                .register_state(
                    figure,
                    Some(source_anchor),
                    Some(target_anchor),
                    RouterBinding::Explicit { router },
                    None,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();

    for connection in connections {
        runtime
            .connections()
            .resolve_route(connection, CoordinateSpace::ChildContent(root))
            .unwrap();
    }

    let stats = runtime.connections().routing_stats();
    assert_eq!(stats.route_calculations, CONNECTION_COUNT as u64);
    assert_eq!(stats.routing_order_entries, CONNECTION_COUNT as u64);
}

#[test]
fn invalid_connection_geometry_never_commits_resolved_state() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 300.0)))
        .expect("valid Runtime mutation");
    let source = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)))
        .expect("valid Runtime mutation");
    let target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(300.0, 170.0, 100.0, 60.0)))
        .expect("valid Runtime mutation");
    let connection_figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            // The style is finite, but doubling its envelope overflows route bounds.
            ConnectionFigure::new().with_stroke_style(
                novadraw::graphics::StrokeStyle::default()
                    .with_width(2.0)
                    .unwrap()
                    .with_miter_limit(f64::MAX)
                    .unwrap(),
            ),
        ))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();

    assert_eq!(
        runtime
            .connections()
            .resolve_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::InvalidGeometry(connection_figure),
        ))
    );
    assert_eq!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Unresolved(UnresolvedConnection::InvalidGeometry(connection_figure))
    );
    assert_eq!(
        runtime.tree().figure_bounds(connection_figure),
        Ok(novadraw::geometry::Rectangle::ZERO)
    );
    assert!(
        runtime
            .tree()
            .connection_route_points(connection_figure)
            .unwrap()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn runtime_relocates_bound_connection_children_after_route_commit() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let label = runtime
        .figure(connection_figure)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0)))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();
    runtime
        .connections()
        .set_locator(connection, label, Box::new(ConnectionLocator::Middle))
        .unwrap();
    runtime
        .figure(label)
        .unwrap()
        .set_preferred_size(novadraw::geometry::Dimension::new(60.0, 30.0))
        .unwrap();

    runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    let points = runtime
        .tree()
        .connection_route_points(connection_figure)
        .unwrap()
        .unwrap();
    let start = points.get(0).unwrap();
    let expected = start + (points.get(1).unwrap() - start) / 2.0;
    let label_bounds = runtime.tree().figure_bounds(label).unwrap();
    assert_eq!(label_bounds.center(), expected);
    assert_eq!((label_bounds.width, label_bounds.height), (60.0, 30.0));

    let old_label_bounds = label_bounds;
    runtime
        .figure(label)
        .unwrap()
        .set_preferred_size(novadraw::geometry::Dimension::new(80.0, 40.0))
        .unwrap();
    assert!(
        runtime
            .figure(target)
            .unwrap()
            .set_bounds(novadraw::geometry::Rectangle::new(
                360.0, 210.0, 100.0, 60.0
            ),)
            .expect("valid Runtime mutation")
    );
    prepare_next_submission(&mut runtime).expect("reroute and Locator layout");
    assert_ne!(runtime.tree().figure_bounds(label), Ok(old_label_bounds));
    let updated = runtime.tree().figure_bounds(label).unwrap();
    assert_eq!((updated.width, updated.height), (80.0, 40.0));
}

#[test]
fn locator_remeasures_child_size_without_endpoint_movement() {
    let (mut runtime, root, source, target, figure) = runtime_fixture();
    let child = runtime
        .figure(figure)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0)))
        .unwrap();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    runtime
        .connections()
        .set_locator(connection, child, Box::new(ConnectionLocator::Middle))
        .unwrap();
    runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    let route = runtime
        .tree()
        .connection_route_points(figure)
        .unwrap()
        .unwrap()
        .clone();
    let center = runtime.tree().figure_bounds(child).unwrap().center();
    runtime
        .figure(child)
        .unwrap()
        .set_preferred_size(novadraw::geometry::Dimension::new(90.0, 35.0))
        .unwrap();
    prepare_next_submission(&mut runtime).expect("child measurement invalidates Locator");
    let bounds = runtime.tree().figure_bounds(child).unwrap();
    assert_eq!((bounds.width, bounds.height), (90.0, 35.0));
    assert_eq!(bounds.center(), center);
    assert_eq!(
        runtime
            .tree()
            .connection_route_points(figure)
            .unwrap()
            .unwrap(),
        &route
    );
    runtime
        .figure(child)
        .unwrap()
        .clear_preferred_size()
        .unwrap();
    prepare_next_submission(&mut runtime).expect("size reset invalidates Locator");
    let bounds = runtime.tree().figure_bounds(child).unwrap();
    assert_eq!((bounds.width, bounds.height), (20.0, 10.0));
    assert_eq!(bounds.center(), center);
    let generation = runtime.connections().state(connection).unwrap().resolution;
    assert!(
        !runtime
            .figure(child)
            .unwrap()
            .clear_preferred_size()
            .unwrap()
    );
    runtime
        .frames()
        .stabilize_for_query()
        .expect("unchanged measurement settles");
    assert_eq!(
        runtime.connections().state(connection).unwrap().resolution,
        generation
    );
}

struct InvalidLocatorMeasurement;

impl novadraw::figure::Figure for InvalidLocatorMeasurement {
    fn initial_bounds(&self) -> novadraw::geometry::Rectangle {
        novadraw::geometry::Rectangle::new(0.0, 0.0, 20.0, 10.0)
    }

    fn name(&self) -> &'static str {
        "InvalidLocatorMeasurement"
    }

    fn intrinsic_measurement(
        &self,
        _: novadraw::figure::MeasureConstraints,
    ) -> novadraw::figure::FigureMeasurement {
        novadraw::figure::FigureMeasurement::new(f64::NAN, 10.0, None)
    }
}

#[test]
fn locator_measurement_rejection_does_not_publish_child_geometry_and_can_recover() {
    let (mut runtime, root, source, target, figure) = runtime_fixture();
    let child = runtime
        .figure(figure)
        .unwrap()
        .add(Box::new(InvalidLocatorMeasurement))
        .unwrap();
    let original = runtime.tree().figure_bounds(child).unwrap();
    let source = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            figure,
            Some(source),
            Some(target),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    runtime
        .connections()
        .set_locator(connection, child, Box::new(ConnectionLocator::Middle))
        .unwrap();
    assert_eq!(
        runtime
            .connections()
            .resolve_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::LocatorFailed {
                child,
                error: LocatorError::ChildMeasurementFailed,
            }
        ))
    );
    assert_eq!(runtime.tree().figure_bounds(child).unwrap(), original);
    assert!(
        runtime
            .tree()
            .connection_route_points(figure)
            .unwrap()
            .is_none_or(|points| points.is_empty())
    );
    assert!(!runtime.is_faulted());

    runtime
        .figure(child)
        .unwrap()
        .set_preferred_size(novadraw::geometry::Dimension::new(60.0, 30.0))
        .unwrap();
    prepare_next_submission(&mut runtime)
        .expect("measurement change recovers Locator automatically");
    let bounds = runtime.tree().figure_bounds(child).unwrap();
    assert_eq!((bounds.width, bounds.height), (60.0, 30.0));
    runtime
        .figure(child)
        .unwrap()
        .clear_preferred_size()
        .unwrap();
    let _ = prepare_next_submission(&mut runtime);
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Unresolved(UnresolvedConnection::LocatorFailed {
            error: LocatorError::ChildMeasurementFailed,
            ..
        })
    ));
    runtime
        .frames()
        .stabilize_for_query()
        .expect("unchanged failure must settle");
    assert!(!runtime.is_faulted());
}

#[test]
fn runtime_orients_endpoint_decorations_and_offsets_labels_in_terminal_frames() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_decoration = runtime
        .figure(connection_figure)
        .unwrap()
        .add(Box::new(PolylineDecorationFigure::arrow()))
        .expect("valid Runtime mutation");
    let target_decoration = runtime
        .figure(connection_figure)
        .unwrap()
        .add(Box::new(PolygonDecorationFigure::triangle()))
        .expect("valid Runtime mutation");
    let target_label = runtime
        .figure(connection_figure)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0)))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();
    runtime
        .connections()
        .set_locator(
            connection,
            source_decoration,
            Box::new(ConnectionLocator::Source),
        )
        .unwrap();
    runtime
        .connections()
        .set_locator(
            connection,
            target_decoration,
            Box::new(ConnectionLocator::Target),
        )
        .unwrap();
    runtime
        .connections()
        .set_locator(
            connection,
            target_label,
            Box::new(EndpointLocator::target(12.0, 6.0).unwrap()),
        )
        .unwrap();

    let output = runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    let connection_bounds = runtime.tree().figure_bounds(connection_figure).unwrap();
    let source_local =
        output.points().get(0).unwrap() - Vec2::new(connection_bounds.x, connection_bounds.y);
    let target_local = output.points().get(output.points().len() - 1).unwrap()
        - Vec2::new(connection_bounds.x, connection_bounds.y);
    let source_bounds = runtime.tree().figure_bounds(source_decoration).unwrap();
    let target_bounds = runtime.tree().figure_bounds(target_decoration).unwrap();
    assert!(source_bounds.contains(source_local));
    assert!(target_bounds.contains(target_local));

    let route_direction = target_local - source_local;
    assert!((source_local - source_bounds.center()).dot(-route_direction) > 0.0);
    assert!((target_local - target_bounds.center()).dot(route_direction) > 0.0);

    let tangent = route_direction / route_direction.length();
    let normal = Vec2::new(-tangent.y(), tangent.x());
    let expected_label_center = target_local + tangent * 12.0 + normal * 6.0;
    let actual_label_center = runtime.tree().figure_bounds(target_label).unwrap().center();
    assert!((actual_label_center - expected_label_center).length() < 1.0e-9);

    assert!(
        record_full_submission(&mut runtime)
            .commands
            .iter()
            .any(|command| { matches!(command.kind, RenderCommandKind::FillPath { .. }) })
    );
}

#[test]
fn invalid_decoration_template_is_rejected_before_attachment() {
    assert!(matches!(
        PolygonDecorationFigure::from_template(PointList::from_points(vec![
            Point::ZERO,
            Point::new(1.0, 0.0),
        ])),
        Err(DecorationError::TooFewTemplatePoints {
            minimum: 3,
            actual: 2,
        })
    ));
    assert!(matches!(
        PolylineDecorationFigure::from_template(PointList::from_points(vec![
            Point::ZERO,
            Point::new(f64::NAN, 0.0),
        ])),
        Err(DecorationError::NonFiniteGeometry)
    ));
}

#[test]
fn first_locator_preflight_failure_recovers_when_an_observed_owner_moves() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 300.0)))
        .expect("valid Runtime mutation");
    let source = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)))
        .expect("valid Runtime mutation");
    let target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)))
        .expect("valid Runtime mutation");
    let connection_figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .expect("valid Runtime mutation");
    let label = runtime
        .figure(connection_figure)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0)))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();
    runtime
        .connections()
        .set_locator(
            connection,
            label,
            Box::new(PathFractionLocator::new(0.5).unwrap()),
        )
        .unwrap();

    assert_eq!(
        runtime
            .connections()
            .resolve_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::LocatorFailed {
                child: label,
                error: LocatorError::DegenerateRoute,
            },
        ))
    );
    assert!(
        runtime
            .connections()
            .state(connection)
            .unwrap()
            .dependency_count
            >= 4
    );

    assert!(
        runtime
            .figure(target)
            .unwrap()
            .set_bounds(novadraw::geometry::Rectangle::new(
                300.0, 170.0, 100.0, 60.0
            ),)
            .expect("valid Runtime mutation")
    );
    assert_eq!(runtime.connections().dirty(), vec![connection]);
    prepare_next_submission(&mut runtime).expect("owner movement must reroute");
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { generation: 1 }
    ));
}

#[test]
fn locator_preflight_failure_after_success_refreshes_recovery_dependencies() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let label = runtime
        .figure(connection_figure)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0)))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();
    runtime
        .connections()
        .set_locator(
            connection,
            label,
            Box::new(PathFractionLocator::new(0.5).unwrap()),
        )
        .unwrap();
    runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert!(
        runtime
            .figure(target)
            .unwrap()
            .set_bounds(novadraw::geometry::Rectangle::new(20.0, 30.0, 80.0, 40.0),)
            .expect("valid Runtime mutation")
    );
    assert!(matches!(
        runtime.connections().resolve_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::LocatorFailed {
                child,
                error: LocatorError::DegenerateRoute,
            },
        )) if child == label
    ));
    prepare_next_submission(&mut runtime)
        .expect("a rejected preflight with current observations must converge");
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Unresolved(UnresolvedConnection::LocatorFailed {
            child,
            error: LocatorError::DegenerateRoute,
        }) if child == label
    ));

    assert!(
        runtime
            .figure(target)
            .unwrap()
            .set_bounds(novadraw::geometry::Rectangle::new(
                360.0, 210.0, 100.0, 60.0
            ),)
            .expect("valid Runtime mutation")
    );
    assert_eq!(runtime.connections().dirty(), vec![connection]);
    prepare_next_submission(&mut runtime).expect("updated dependency must reroute");
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { generation: 2 }
    ));
}

#[test]
fn connection_locator_rejects_non_child_targets() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();

    assert_eq!(
        runtime
            .connections()
            .set_locator(connection, root, Box::new(ConnectionLocator::Middle)),
        Err(ConnectionRuntimeError::InvalidLocatorChild {
            connection,
            child: root,
        })
    );
}

#[test]
fn resolved_route_replaces_dependencies_and_targeted_invalidation_marks_dirty() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();

    let output = runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert_eq!(output.points().len(), 2);
    let path_bounds = output
        .points()
        .bounds()
        .unwrap()
        .inflate(DEFAULT_STROKE_MITER_LIMIT, DEFAULT_STROKE_MITER_LIMIT);
    assert_eq!(
        runtime.tree().figure_bounds(connection_figure),
        Ok(path_bounds)
    );
    let local_points = runtime
        .tree()
        .connection_route_points(connection_figure)
        .unwrap()
        .unwrap();
    assert_eq!(
        local_points.get(0),
        output
            .points()
            .get(0)
            .map(|point| point - (Point::new(path_bounds.x, path_bounds.y) - Point::ORIGIN))
    );
    let source_point = output.points().get(0).unwrap();
    let target_point = output.points().get(1).unwrap();
    let midpoint = Point::new(
        (source_point.x() + target_point.x()) / 2.0,
        (source_point.y() + target_point.y()) / 2.0,
    );
    assert_eq!(
        runtime.tree().hit_test_simple((midpoint.x(), midpoint.y())),
        Some(connection_figure)
    );
    assert!(
        record_full_submission(&mut runtime)
            .commands
            .iter()
            .any(|command| matches!(&command.kind, RenderCommandKind::Polyline { .. }))
    );
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { generation: 1 }
    ));
    assert!(
        runtime
            .connections()
            .state(connection)
            .unwrap()
            .dependency_count
            >= 4
    );

    assert!(
        runtime
            .figure(source)
            .unwrap()
            .set_bounds(novadraw::geometry::Rectangle::new(25.0, 35.0, 80.0, 40.0),)
            .expect("valid Runtime mutation")
    );
    assert_eq!(runtime.connections().dirty(), vec![connection]);
}

#[test]
fn removing_connection_figure_cleans_runtime_state() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();

    assert!(
        runtime
            .figure(root)
            .unwrap()
            .remove(connection_figure)
            .expect("valid Runtime mutation")
    );
    assert_eq!(
        runtime.connections().state(connection),
        Err(ConnectionRuntimeError::UnknownConnection(connection))
    );
}

#[test]
fn normal_frame_automatically_resolves_dirty_connection_routes() {
    let (mut runtime, _root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();

    assert!(runtime.frames().has_pending_update());
    let initial = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .expect("dirty route must schedule a frame");
    let _ = runtime
        .frames()
        .complete_submission(
            initial.session_id,
            initial.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .expect("frame completion");
    let initial_bounds = runtime.tree().figure_bounds(connection_figure).unwrap();
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { .. }
    ));
    assert!(!runtime.frames().has_pending_update());

    assert!(
        runtime
            .figure(source)
            .unwrap()
            .set_bounds(novadraw::geometry::Rectangle::new(80.0, 60.0, 80.0, 40.0),)
            .expect("valid Runtime mutation")
    );
    assert!(runtime.frames().has_pending_update());
    let moved = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .expect("owner geometry change must reroute in the same frame");
    let _ = runtime
        .frames()
        .complete_submission(
            moved.session_id,
            moved.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .expect("frame completion");
    assert_ne!(
        runtime.tree().figure_bounds(connection_figure),
        Ok(initial_bounds)
    );
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { .. }
    ));
    assert!(!runtime.frames().has_pending_update());
}

#[test]
fn layout_output_geometry_automatically_invalidates_and_reroutes() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    runtime
        .figure(root)
        .unwrap()
        .set_layout_manager(Box::new(XYLayout::new()))
        .unwrap();
    runtime
        .figure(source)
        .unwrap()
        .set_layout_constraint(XYConstraint::at_size(20.0, 30.0, 80.0, 40.0))
        .unwrap();
    runtime
        .figure(target)
        .unwrap()
        .set_layout_constraint(XYConstraint::at_size(300.0, 170.0, 100.0, 60.0))
        .unwrap();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();

    prepare_next_submission(&mut runtime).expect("initial stable frame");
    let initial_bounds = runtime.tree().figure_bounds(connection_figure).unwrap();

    runtime
        .figure(source)
        .unwrap()
        .set_layout_constraint(XYConstraint::at_size(90.0, 70.0, 80.0, 40.0))
        .unwrap();
    prepare_next_submission(&mut runtime).expect("layout commit must schedule route work");

    assert_ne!(
        runtime.tree().figure_bounds(connection_figure),
        Ok(initial_bounds)
    );
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { generation: 2 }
    ));
}

#[test]
fn missing_endpoint_is_unresolved_and_binding_it_restores_dirty_state() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            None,
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();

    assert_eq!(
        runtime
            .connections()
            .resolve_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::MissingTarget,
        ))
    );
    assert_eq!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Unresolved(UnresolvedConnection::MissingTarget)
    );

    assert!(
        runtime
            .connections()
            .set_target(connection, Some(target_anchor))
            .unwrap()
    );
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Dirty { .. }
    ));
}

#[test]
fn unresolved_transition_clears_previously_committed_geometry() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();
    runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();

    runtime.connections().set_target(connection, None).unwrap();
    assert_eq!(
        runtime
            .connections()
            .resolve_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::MissingTarget,
        ))
    );
    assert_eq!(
        runtime.tree().figure_bounds(connection_figure),
        Ok(novadraw::geometry::Rectangle::ZERO)
    );
    assert!(
        runtime
            .tree()
            .connection_route_points(connection_figure)
            .unwrap()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn shared_router_and_anchor_cannot_be_removed_while_referenced() {
    let (mut runtime, _root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime
        .connections()
        .register_router(Box::new(DirectRouter));
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();

    assert!(matches!(
        runtime.connections().remove_router(router),
        Err(ConnectionRuntimeError::RouterInUse(candidate)) if candidate == router
    ));
    assert!(matches!(
        runtime.connections().remove_anchor(source_anchor),
        Err(ConnectionRuntimeError::AnchorInUse(candidate)) if candidate == source_anchor
    ));

    runtime.connections().remove_state(connection).unwrap();
    runtime.connections().remove_router(router).unwrap();
    runtime.connections().remove_anchor(source_anchor).unwrap();
    runtime.connections().remove_anchor(target_anchor).unwrap();
}

#[test]
fn incompatible_router_change_is_rejected_atomically() {
    let (mut runtime, _root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router_a = runtime
        .connections()
        .register_router(Box::new(TypedRouter::<ConstraintA>(PhantomData)));
    let router_b = runtime
        .connections()
        .register_router(Box::new(TypedRouter::<ConstraintB>(PhantomData)));
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router: router_a },
            Some(Box::new(ConstraintA)),
        )
        .unwrap();

    assert!(matches!(
        runtime.connections().set_route_configuration(
            connection,
            RouterBinding::Explicit { router: router_b },
            Some(Box::new(ConstraintA)),
        ),
        Err(ConnectionRuntimeError::ConstraintTypeMismatch { .. })
    ));
    assert_eq!(
        runtime.connections().state(connection).unwrap().router,
        RouterBinding::Explicit { router: router_a }
    );
}

#[test]
fn connection_cannot_anchor_to_its_own_subtree() {
    let (mut runtime, root, _source, target, connection_figure) = runtime_fixture();
    let self_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(connection_figure)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(self_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();

    assert_eq!(
        runtime
            .connections()
            .resolve_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::RouteFailed(RouteError::DependencyCycle),
        ))
    );
}

#[test]
fn bendpoint_router_preserves_absolute_and_relative_constraints() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime
        .connections()
        .register_router(Box::new(BendpointConnectionRouter));
    let constraint = BendpointConstraint::new(vec![
        Bendpoint::Absolute(Point::new(160.0, 80.0)),
        Bendpoint::Relative {
            source_offset: Vec2::new(10.0, 0.0),
            target_offset: Vec2::new(-10.0, 0.0),
            weight: 0.5,
        },
    ]);
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            Some(Box::new(constraint.clone())),
        )
        .unwrap();

    let output = runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert_eq!(output.points().len(), 4);
    assert_eq!(output.points().get(1), Some(Point::new(160.0, 80.0)));
    assert_eq!(output.points().get(2), Some(Point::new(205.0, 125.0)));
    assert_eq!(output.metadata().source.reference, Point::new(160.0, 80.0));
    assert_eq!(output.metadata().target.reference, Point::new(205.0, 125.0));

    let resolved = runtime.connections().state(connection).unwrap().resolution;
    assert!(
        !runtime
            .connections()
            .set_route_configuration(
                connection,
                RouterBinding::Explicit { router },
                Some(Box::new(constraint)),
            )
            .unwrap()
    );
    assert_eq!(
        runtime.connections().state(connection).unwrap().resolution,
        resolved,
        "equivalent Bendpoint constraints must not invalidate a resolved route"
    );
}

#[test]
fn reparenting_connection_maps_absolute_bendpoints_into_the_new_routing_domain() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 300.0)))
        .expect("valid Runtime mutation");
    let old_parent = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)))
        .expect("valid Runtime mutation");
    let new_parent = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(100.0, 0.0, 200.0, 200.0)))
        .expect("valid Runtime mutation");
    let connection_figure = runtime
        .figure(old_parent)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(XYAnchor::new(
            Point::new(10.0, 20.0),
            CoordinateSpace::ChildContent(root),
        )));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(XYAnchor::new(
            Point::new(180.0, 80.0),
            CoordinateSpace::ChildContent(root),
        )));
    let router = runtime
        .connections()
        .register_router(Box::new(BendpointConnectionRouter));
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            Some(Box::new(BendpointConstraint::new(vec![
                Bendpoint::Absolute(Point::new(50.0, 30.0)),
                Bendpoint::Relative {
                    source_offset: Vec2::new(10.0, 0.0),
                    target_offset: Vec2::new(-10.0, 0.0),
                    weight: 0.5,
                },
            ]))),
        )
        .unwrap();

    let initial = runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(old_parent))
        .unwrap();
    assert_eq!(initial.points().get(1), Some(Point::new(50.0, 30.0)));

    assert!(
        runtime
            .figure(connection_figure)
            .unwrap()
            .reparent(new_parent)
            .unwrap()
    );
    let reparented = runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(new_parent))
        .unwrap();
    assert_eq!(reparented.points().get(1), Some(Point::new(-50.0, 30.0)));
    assert_eq!(
        reparented.points().get(2),
        initial
            .points()
            .get(2)
            .map(|point| point - Vec2::new(100.0, 0.0))
    );
    // Three moves must remap the preceding prepared constraint on every step.
    runtime
        .figure(connection_figure)
        .unwrap()
        .update_component(MoveConnectionBatch(vec![
            old_parent, new_parent, old_parent,
        ]))
        .unwrap();
    let roundtrip = runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(old_parent))
        .unwrap();
    assert_eq!(roundtrip.points().get(1), initial.points().get(1));
    assert_eq!(roundtrip.points().get(2), initial.points().get(2));
}

#[test]
fn reparenting_connection_with_unknown_constraint_is_rejected_atomically() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 300.0)))
        .expect("valid Runtime mutation");
    let old_parent = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)))
        .expect("valid Runtime mutation");
    let new_parent = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(100.0, 0.0, 200.0, 200.0)))
        .expect("valid Runtime mutation");
    let connection_figure = runtime
        .figure(old_parent)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .expect("valid Runtime mutation");
    let router = runtime
        .connections()
        .register_router(Box::new(TypedRouter::<ConstraintA>(PhantomData)));
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            None,
            None,
            RouterBinding::Explicit { router },
            Some(Box::new(ConstraintA)),
        )
        .unwrap();
    let before = runtime.connections().state(connection).unwrap();

    assert!(matches!(
        runtime.figure(connection_figure).unwrap().reparent(new_parent),
        Err(RuntimeMutationError::Connection(
            ConnectionRuntimeError::UnsupportedConstraintReparent {
                connection: candidate,
                ..
            }
        )) if candidate == connection
    ));
    assert_eq!(
        runtime.tree().parent_id(connection_figure),
        Ok(Some(old_parent))
    );
    assert_eq!(runtime.connections().state(connection).unwrap(), before);
}

#[test]
fn manhattan_router_emits_only_orthogonal_non_duplicate_segments() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime
        .connections()
        .register_router(Box::new(ManhattanConnectionRouter));
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();

    let output = runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert!(output.points().len() >= 3);
    for segment in output.points().as_slice().windows(2) {
        assert_ne!(segment[0], segment[1]);
        assert!(segment[0].x() == segment[1].x() || segment[0].y() == segment[1].y());
    }
    let segments = output.points().as_slice().windows(2).collect::<Vec<_>>();
    for segment in [segments.first().unwrap(), segments.last().unwrap()] {
        assert!((segment[1] - segment[0]).length() >= MANHATTAN_DEFAULT_MINIMUM_STUB);
    }
}

#[test]
fn fan_router_separates_bidirectional_connections() {
    let (mut runtime, root, source, target, first_figure) = runtime_fixture();
    let second_figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.connections().register_router(Box::new(
        FanRouter::new(Box::new(DirectRouter), 16.0).unwrap(),
    ));
    let first = runtime
        .connections()
        .register_state(
            first_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    let second = runtime
        .connections()
        .register_state(
            second_figure,
            Some(target_anchor),
            Some(source_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();

    let first_output = runtime
        .connections()
        .resolve_route(first, CoordinateSpace::ChildContent(root))
        .unwrap();
    let second_output = runtime
        .connections()
        .resolve_route(second, CoordinateSpace::ChildContent(root))
        .unwrap();
    let first_middle = first_output.points().get(1).unwrap();
    let second_middle = second_output.points().get(1).unwrap();

    assert_eq!(first_output.points().len(), 3);
    assert_eq!(second_output.points().len(), 3);
    assert_eq!(first_output.points().get(0), second_output.points().get(2));
    assert_eq!(first_output.points().get(2), second_output.points().get(0));
    assert_ne!(first_middle, second_middle);
}

#[test]
fn fan_router_uses_stable_mixed_direction_order_and_recenters_after_removal() {
    let (mut runtime, root, source, target, first_figure) = runtime_fixture();
    let second_figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .expect("valid Runtime mutation");
    let third_figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.connections().register_router(Box::new(
        FanRouter::new(Box::new(DirectRouter), 16.0).unwrap(),
    ));
    let mut connections = Vec::new();
    for (figure, source, target) in [
        (first_figure, source_anchor, target_anchor),
        (second_figure, target_anchor, source_anchor),
        (third_figure, source_anchor, target_anchor),
    ] {
        connections.push(
            runtime
                .connections()
                .register_state(
                    figure,
                    Some(source),
                    Some(target),
                    RouterBinding::Explicit { router },
                    None,
                )
                .unwrap(),
        );
    }

    let outputs: Vec<_> = connections
        .iter()
        .map(|connection| {
            runtime
                .connections()
                .resolve_route(*connection, CoordinateSpace::ChildContent(root))
                .unwrap()
        })
        .collect();

    assert_eq!(outputs[0].points().len(), 3);
    assert_eq!(outputs[1].points().len(), 2);
    assert_eq!(outputs[2].points().len(), 3);
    assert_ne!(outputs[0].points().get(1), outputs[2].points().get(1));

    assert!(
        runtime
            .figure(root)
            .unwrap()
            .remove(first_figure)
            .expect("valid Runtime mutation")
    );
    assert_eq!(runtime.connections().dirty(), connections[1..]);
    let reverse = runtime
        .connections()
        .resolve_route(connections[1], CoordinateSpace::ChildContent(root))
        .unwrap();
    let forward = runtime
        .connections()
        .resolve_route(connections[2], CoordinateSpace::ChildContent(root))
        .unwrap();
    assert_eq!(reverse.points().len(), 3);
    assert_eq!(forward.points().len(), 3);
    assert_ne!(reverse.points().get(1), forward.points().get(1));
}

#[test]
fn shared_manhattan_reserves_lanes_across_different_anchor_pairs() {
    let (mut runtime, root, source, target, first_figure) = runtime_fixture();
    assert!(
        runtime
            .figure(target)
            .unwrap()
            .set_bounds(novadraw::geometry::Rectangle::new(300.0, 70.0, 100.0, 60.0),)
            .expect("valid Runtime mutation")
    );
    let second_source = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 130.0, 80.0, 40.0)))
        .expect("valid Runtime mutation");
    let second_target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(300.0, 170.0, 100.0, 60.0)))
        .expect("valid Runtime mutation");
    let second_figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .expect("valid Runtime mutation");
    let router = runtime
        .connections()
        .register_router(Box::new(ManhattanConnectionRouter));
    let first_source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let first_target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let second_source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(second_source)));
    let second_target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(second_target)));
    let first = runtime
        .connections()
        .register_state(
            first_figure,
            Some(first_source_anchor),
            Some(first_target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    let second = runtime
        .connections()
        .register_state(
            second_figure,
            Some(second_source_anchor),
            Some(second_target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();

    let first_output = runtime
        .connections()
        .resolve_route(first, CoordinateSpace::ChildContent(root))
        .unwrap();
    assert!(matches!(
        runtime.connections().state(second).unwrap().resolution,
        ConnectionResolution::Resolved { generation: 1 }
    ));
    let second_output = runtime
        .connections()
        .resolve_route(second, CoordinateSpace::ChildContent(root))
        .unwrap();
    let first_lane = first_output.points().get(1).unwrap().x();
    let second_lane = second_output.points().get(1).unwrap().x();
    assert_eq!(
        (first_lane - second_lane).abs(),
        MANHATTAN_DEFAULT_LANE_SPACING
    );

    assert!(
        runtime
            .figure(second_source)
            .unwrap()
            .set_bounds(novadraw::geometry::Rectangle::new(25.0, 130.0, 80.0, 40.0),)
            .expect("valid Runtime mutation")
    );
    assert_eq!(runtime.connections().dirty(), vec![first, second]);
    runtime
        .connections()
        .resolve_route(first, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert!(
        runtime
            .figure(root)
            .unwrap()
            .remove(first_figure)
            .expect("valid Runtime mutation")
    );
    let recentered = runtime
        .connections()
        .resolve_route(second, CoordinateSpace::ChildContent(root))
        .unwrap();
    let expected_lane = (recentered.metadata().source.site.point.x()
        + recentered.metadata().target.site.point.x())
        / 2.0;
    assert_eq!(recentered.points().get(1).unwrap().x(), expected_lane);
}

#[test]
fn manhattan_reservations_are_isolated_by_router_id() {
    let (mut runtime, root, source, target, first_figure) = runtime_fixture();
    let second_figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let first_router = runtime
        .connections()
        .register_router(Box::new(ManhattanConnectionRouter));
    let second_router = runtime
        .connections()
        .register_router(Box::new(ManhattanConnectionRouter));
    let first = runtime
        .connections()
        .register_state(
            first_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: first_router,
            },
            None,
        )
        .unwrap();
    let second = runtime
        .connections()
        .register_state(
            second_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: second_router,
            },
            None,
        )
        .unwrap();

    let first_output = runtime
        .connections()
        .resolve_route(first, CoordinateSpace::ChildContent(root))
        .unwrap();
    let second_output = runtime
        .connections()
        .resolve_route(second, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert_eq!(
        first_output.points().get(1).unwrap().x(),
        second_output.points().get(1).unwrap().x()
    );
}

#[test]
fn divergent_viewport_topology_is_rejected_and_reparent_recovers() {
    let mut tree = novadraw::tree::FigureTree::new();
    let mut builder = tree.builder();
    let root = builder
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 400.0)))
        .expect("valid FigureTree construction");
    let viewport = builder
        .add_child(
            root,
            Box::new(ViewportFigure::new(20.0, 20.0, 240.0, 180.0)),
        )
        .expect("valid FigureTree construction");
    let contents = builder
        .add_child(
            viewport,
            Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)),
        )
        .expect("valid FigureTree construction");
    let source = builder
        .add_child(
            contents,
            Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)),
        )
        .expect("valid FigureTree construction");
    let target = builder
        .add_child(
            root,
            Box::new(RectangleFigure::new(400.0, 200.0, 100.0, 60.0)),
        )
        .expect("valid FigureTree construction");
    let connection_figure = builder
        .add_child(root, Box::new(ConnectionFigure::new()))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();

    assert_eq!(
        runtime
            .connections()
            .resolve_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::RouteFailed(RouteError::UnsupportedViewportTopology),
        ))
    );
    assert_eq!(
        runtime.tree().figure_bounds(connection_figure),
        Ok(novadraw::geometry::Rectangle::ZERO)
    );

    assert!(
        runtime
            .figure(source)
            .unwrap()
            .reparent(root)
            .expect("valid Runtime mutation")
    );
    let recovered = runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    assert_eq!(recovered.points().len(), 2);
}

#[test]
fn matching_viewport_topology_routes_normally() {
    let mut tree = novadraw::tree::FigureTree::new();
    let mut builder = tree.builder();
    let root = builder
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 400.0)))
        .expect("valid FigureTree construction");
    let viewport = builder
        .add_child(
            root,
            Box::new(ViewportFigure::new(20.0, 20.0, 300.0, 220.0)),
        )
        .expect("valid FigureTree construction");
    let contents = builder
        .add_child(
            viewport,
            Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 350.0)),
        )
        .expect("valid FigureTree construction");
    let source = builder
        .add_child(
            contents,
            Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)),
        )
        .expect("valid FigureTree construction");
    let target = builder
        .add_child(
            contents,
            Box::new(RectangleFigure::new(300.0, 170.0, 100.0, 60.0)),
        )
        .expect("valid FigureTree construction");
    let connection_figure = builder
        .add_child(contents, Box::new(ConnectionFigure::new()))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();

    let output = runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(contents))
        .unwrap();
    assert_eq!(output.points().len(), 2);
}

#[test]
fn manhattan_scope_failure_clears_the_complete_batch_and_recovers_atomically() {
    let mut tree = novadraw::tree::FigureTree::new();
    let mut builder = tree.builder();
    let root = builder
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 700.0, 420.0)))
        .expect("valid FigureTree construction");
    let valid_source = builder
        .add_child(root, Box::new(RectangleFigure::new(20.0, 40.0, 80.0, 40.0)))
        .expect("valid FigureTree construction");
    let valid_target = builder
        .add_child(
            root,
            Box::new(RectangleFigure::new(420.0, 90.0, 100.0, 60.0)),
        )
        .expect("valid FigureTree construction");
    let first_figure = builder
        .add_child(root, Box::new(ConnectionFigure::new()))
        .expect("valid FigureTree construction");
    let viewport = builder
        .add_child(
            root,
            Box::new(ViewportFigure::new(20.0, 200.0, 260.0, 180.0)),
        )
        .expect("valid FigureTree construction");
    let contents = builder
        .add_child(
            viewport,
            Box::new(RectangleFigure::new(0.0, 0.0, 420.0, 300.0)),
        )
        .expect("valid FigureTree construction");
    let invalid_source = builder
        .add_child(
            contents,
            Box::new(RectangleFigure::new(20.0, 20.0, 80.0, 40.0)),
        )
        .expect("valid FigureTree construction");
    let invalid_target = builder
        .add_child(
            root,
            Box::new(RectangleFigure::new(440.0, 260.0, 100.0, 60.0)),
        )
        .expect("valid FigureTree construction");
    let second_figure = builder
        .add_child(root, Box::new(ConnectionFigure::new()))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    let router = runtime
        .connections()
        .register_router(Box::new(ManhattanConnectionRouter));
    let valid_source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(valid_source)));
    let valid_target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(valid_target)));
    let invalid_source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(invalid_source)));
    let invalid_target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(invalid_target)));
    let first = runtime
        .connections()
        .register_state(
            first_figure,
            Some(valid_source_anchor),
            Some(valid_target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    let second = runtime
        .connections()
        .register_state(
            second_figure,
            Some(invalid_source_anchor),
            Some(invalid_target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();

    assert_eq!(
        runtime
            .connections()
            .resolve_route(first, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::RouteFailed(RouteError::UnsupportedViewportTopology),
        ))
    );
    for (connection, figure) in [(first, first_figure), (second, second_figure)] {
        assert_eq!(
            runtime.connections().state(connection).unwrap().resolution,
            ConnectionResolution::Unresolved(UnresolvedConnection::RouteFailed(
                RouteError::UnsupportedViewportTopology,
            ))
        );
        assert_eq!(
            runtime.tree().figure_bounds(figure),
            Ok(novadraw::geometry::Rectangle::ZERO)
        );
    }

    assert!(
        runtime
            .figure(invalid_source)
            .unwrap()
            .reparent(root)
            .expect("valid Runtime mutation")
    );
    runtime
        .connections()
        .resolve_route(first, CoordinateSpace::ChildContent(root))
        .unwrap();
    for (connection, figure) in [(first, first_figure), (second, second_figure)] {
        assert!(matches!(
            runtime.connections().state(connection).unwrap().resolution,
            ConnectionResolution::Resolved { .. }
        ));
        assert_ne!(
            runtime.tree().figure_bounds(figure),
            Ok(novadraw::geometry::Rectangle::ZERO)
        );
    }
}
