use novadraw::{
    connection::{
        AnchorSite, ChildLocatorPlacement, ConnectionEndpointLocator, ConnectionLocator,
        ConnectionLocatorStrategy, LocatorError, LocatorInput, LocatorPlacement, RouteEnd,
        RouteEndpoint, RouteMetadata,
    },
    geometry::{Dimension, Point, PointList, Rectangle, Translatable, Vec2},
};

fn metadata(start: Point, end: Point, normal: Option<Vec2>) -> RouteMetadata {
    RouteMetadata {
        source: RouteEndpoint {
            reference: end,
            site: AnchorSite {
                point: start,
                outward_normal: normal,
            },
        },
        target: RouteEndpoint {
            reference: start,
            site: AnchorSite {
                point: end,
                outward_normal: normal,
            },
        },
    }
}

#[test]
fn owner_sides_and_v_sign_preserve_measured_bounds_at_both_ends() {
    let owner = Rectangle::new(0.0, 0.0, 100.0, 60.0);
    let size = Dimension::new(20.0, 10.0);
    for (endpoint, neighbor, expected_positive, expected_zero, expected_negative) in [
        (
            Point::new(100.0, 30.0),
            Point::new(150.0, 30.0),
            (108.0, 34.0),
            (108.0, 25.0),
            (108.0, 16.0),
        ),
        (
            Point::new(0.0, 30.0),
            Point::new(-50.0, 30.0),
            (-28.0, 34.0),
            (-28.0, 25.0),
            (-28.0, 16.0),
        ),
        (
            Point::new(50.0, 0.0),
            Point::new(50.0, -50.0),
            (54.0, -18.0),
            (40.0, -18.0),
            (26.0, -18.0),
        ),
        (
            Point::new(50.0, 60.0),
            Point::new(50.0, 110.0),
            (54.0, 68.0),
            (40.0, 68.0),
            (26.0, 68.0),
        ),
    ] {
        for end in [RouteEnd::Source, RouteEnd::Target] {
            let points = PointList::from_points(if end == RouteEnd::Source {
                vec![endpoint, neighbor]
            } else {
                vec![neighbor, endpoint]
            });
            let meta = metadata(points.get(0).unwrap(), points.get(1).unwrap(), None);
            let input = LocatorInput::new(&points, &meta, size, Some(owner), Some(owner)).unwrap();
            for (v, expected) in [
                (4.0, expected_positive),
                (0.0, expected_zero),
                (-4.0, expected_negative),
            ] {
                let result = ConnectionEndpointLocator::new(end, 8.0, v)
                    .unwrap()
                    .locate_child(&input)
                    .unwrap()
                    .child_bounds(size)
                    .unwrap();
                assert_eq!(result, Rectangle::new(expected.0, expected.1, 20.0, 10.0));
            }
        }
    }
}

#[test]
fn corner_tie_slope_clamp_and_half_pixel_centering_are_explicit() {
    let points = PointList::from_points(vec![Point::new(100.0, 60.0), Point::new(110.0, 100.0)]);
    let meta = metadata(points.get(0).unwrap(), points.get(1).unwrap(), None);
    let size = Dimension::new(21.0, 11.0);
    let input = LocatorInput::new(
        &points,
        &meta,
        size,
        Some(Rectangle::new(0.0, 0.0, 100.0, 60.0)),
        None,
    )
    .unwrap();
    let bounds = ConnectionEndpointLocator::new(RouteEnd::Source, 8.0, 0.0)
        .unwrap()
        .locate_child(&input)
        .unwrap()
        .child_bounds(size)
        .unwrap();
    assert_eq!(bounds, Rectangle::new(108.0, 62.5, 21.0, 11.0));
}

#[test]
fn ownerless_and_zero_owner_use_segment_and_repeated_terminal_uses_normal() {
    let size = Dimension::new(20.0, 10.0);
    let points = PointList::from_points(vec![Point::new(10.0, 10.0), Point::new(60.0, 10.0)]);
    let meta = metadata(points.get(0).unwrap(), points.get(1).unwrap(), None);
    let locator = ConnectionEndpointLocator::with_default_distances(RouteEnd::Source);
    for owner in [None, Some(Rectangle::ZERO)] {
        let input = LocatorInput::new(&points, &meta, size, owner, None).unwrap();
        assert_eq!(
            locator
                .locate_child(&input)
                .unwrap()
                .child_bounds(size)
                .unwrap(),
            Rectangle::new(24.0, 14.0, 20.0, 10.0)
        );
    }
    let points = PointList::from_points(vec![Point::ZERO, Point::ZERO]);
    let meta = metadata(Point::ZERO, Point::ZERO, Some(Vec2::new(1.0, 0.0)));
    let input = LocatorInput::new(&points, &meta, size, None, None).unwrap();
    assert_eq!(
        locator
            .locate_child(&input)
            .unwrap()
            .child_bounds(size)
            .unwrap(),
        Rectangle::new(14.0, 4.0, 20.0, 10.0)
    );
    let meta = metadata(Point::ZERO, Point::ZERO, None);
    let input = LocatorInput::new(&points, &meta, size, None, None).unwrap();
    assert_eq!(
        locator.locate_child(&input),
        Err(LocatorError::DegenerateRoute)
    );
    let meta = metadata(Point::ZERO, Point::ZERO, Some(Vec2::new(f64::NAN, 0.0)));
    let input = LocatorInput::new(&points, &meta, size, None, None).unwrap();
    assert_eq!(
        locator.locate_child(&input),
        Err(LocatorError::DegenerateRoute)
    );
}

#[test]
fn legacy_adapter_and_invalid_external_bounds_are_checked() {
    let points = PointList::from_points(vec![Point::ZERO, Point::new(100.0, 0.0)]);
    let meta = metadata(points.get(0).unwrap(), points.get(1).unwrap(), None);
    let size = Dimension::new(20.0, 10.0);
    let input = LocatorInput::new(&points, &meta, size, None, None).unwrap();
    assert_eq!(
        ConnectionLocator::Middle
            .locate_child(&input)
            .unwrap()
            .child_bounds(size)
            .unwrap(),
        Rectangle::new(40.0, -5.0, 20.0, 10.0)
    );
    let placement = LocatorPlacement {
        point: Point::ZERO,
        reference: Point::new(1.0, 0.0),
    };
    for bounds in [
        Rectangle::new(f64::NAN, 0.0, 20.0, 10.0),
        Rectangle::new(0.0, 0.0, -20.0, 10.0),
        Rectangle::new(f64::MAX, 0.0, f64::MAX, 10.0),
    ] {
        assert_eq!(
            ChildLocatorPlacement::with_bounds(placement, bounds).child_bounds(size),
            Err(LocatorError::NonFinitePlacement)
        );
    }
    assert_eq!(
        ChildLocatorPlacement::with_bounds(placement, Rectangle::new(0.0, 0.0, 21.0, 10.0))
            .child_bounds(size),
        Err(LocatorError::ChildSizeMismatch)
    );
    assert!(matches!(
        LocatorInput::new(&points, &meta, Dimension::new(f64::NAN, 1.0), None, None),
        Err(LocatorError::ChildMeasurementFailed)
    ));
    assert!(ConnectionEndpointLocator::new(RouteEnd::Source, f64::INFINITY, 4.0).is_err());
}

struct ExternalBoundsLocator {
    wrong_size: bool,
}

impl ConnectionLocatorStrategy for ExternalBoundsLocator {
    fn locate(&self, _: &PointList) -> Result<LocatorPlacement, LocatorError> {
        panic!("Runtime must supply measured inputs to an external child locator")
    }
    fn locate_child(
        &self,
        input: &LocatorInput<'_>,
    ) -> Result<ChildLocatorPlacement, LocatorError> {
        let owner = input.owner_bounds(RouteEnd::Source).expect("owner input");
        let size = input.child_size();
        Ok(ChildLocatorPlacement::with_bounds(
            LocatorPlacement {
                point: input.points().get(0).unwrap(),
                reference: input.points().get(1).unwrap(),
            },
            Rectangle::new(
                owner.x + owner.width,
                owner.y,
                size.width + if self.wrong_size { 1.0 } else { 0.0 },
                size.height,
            ),
        ))
    }
}

/// Deliberately never reads Anchor or owner geometry: Locator tracking must provide it.
struct FixedRouter;
impl novadraw::connection::ConnectionRouter for FixedRouter {
    fn route(
        &self,
        _: novadraw::connection::RouteRequest<'_>,
    ) -> Result<novadraw::connection::RouteOutput, novadraw::connection::RouteError> {
        let points = PointList::from_points(vec![Point::new(100.0, 50.0), Point::new(200.0, 50.0)]);
        novadraw::connection::RouteOutput::new(
            points,
            metadata(Point::new(100.0, 50.0), Point::new(200.0, 50.0), None),
        )
    }
}

#[test]
fn runtime_external_locator_tracks_owner_and_rejects_size_override_atomically() {
    use novadraw::{connection::*, figure::RectangleFigure, runtime::Runtime};
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 300.0)))
        .unwrap();
    let source = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)))
        .unwrap();
    let target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(300.0, 40.0, 80.0, 40.0)))
        .unwrap();
    let figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .unwrap();
    let child = runtime
        .figure(figure)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0)))
        .unwrap();
    let a = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let b = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.connections().register_router(Box::new(FixedRouter));
    let connection = runtime
        .connections()
        .register_state(
            figure,
            Some(a),
            Some(b),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    runtime
        .connections()
        .set_locator(
            connection,
            child,
            Box::new(ExternalBoundsLocator { wrong_size: false }),
        )
        .unwrap();
    runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    let parent = runtime.tree().figure_bounds(figure).unwrap();
    let bounds = runtime.tree().figure_bounds(child).unwrap();
    assert_eq!((bounds.x + parent.x, bounds.y + parent.y), (100.0, 30.0));
    runtime
        .figure(source)
        .unwrap()
        .set_bounds(Rectangle::new(40.0, 60.0, 100.0, 40.0))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    let parent = runtime.tree().figure_bounds(figure).unwrap();
    let bounds = runtime.tree().figure_bounds(child).unwrap();
    assert_eq!((bounds.x + parent.x, bounds.y + parent.y), (140.0, 60.0));
    let before = bounds;
    runtime
        .connections()
        .set_locator(
            connection,
            child,
            Box::new(ExternalBoundsLocator { wrong_size: true }),
        )
        .unwrap();
    assert!(matches!(
        runtime
            .connections()
            .resolve_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::LocatorFailed {
                error: LocatorError::ChildSizeMismatch,
                ..
            }
        ))
    ));
    assert_eq!(runtime.tree().figure_bounds(child).unwrap(), before);
    runtime
        .connections()
        .set_locator(
            connection,
            child,
            Box::new(ExternalBoundsLocator { wrong_size: false }),
        )
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { .. }
    ));
    runtime.figure(root).unwrap().remove(source).unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Unresolved(_)
    ));
    runtime
        .connections()
        .set_source(connection, Some(b))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { .. }
    ));
    let parent = runtime.tree().figure_bounds(figure).unwrap();
    let bounds = runtime.tree().figure_bounds(child).unwrap();
    assert_eq!((bounds.x + parent.x, bounds.y + parent.y), (380.0, 40.0));
}

struct TransformedContainer(novadraw::geometry::Affine2D);
impl novadraw::figure::Figure for TransformedContainer {
    fn name(&self) -> &'static str {
        "TransformedContainer"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(50.0, 70.0, 300.0, 200.0)
    }
    fn register_capabilities(
        &self,
        out: &mut novadraw::figure::FigureCapabilityBuilder,
    ) -> Result<(), novadraw::figure::FigureCapabilityRegistrationError> {
        out.register(
            novadraw::figure::CONTAINER,
            novadraw::figure::ContainerCapability::of::<Self>(),
        )
    }
}
impl novadraw::figure::FigureContainer for TransformedContainer {
    fn child_transform(&self) -> novadraw::figure::ChildTransform {
        novadraw::figure::ChildTransform::from_affine(self.0)
    }
}

#[test]
fn owner_envelope_is_mapped_through_nested_scale_and_rotation() {
    use novadraw::{connection::*, figure::RectangleFigure, geometry::Affine2D, runtime::Runtime};
    for affine in [
        Affine2D::from_uniform_scale(2.0),
        Affine2D::new(0.0, 1.0, -1.0, 0.0, 0.0, 0.0),
    ] {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)))
            .unwrap();
        let group = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(TransformedContainer(affine)))
            .unwrap();
        let owner = runtime
            .figure(group)
            .unwrap()
            .add(Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)))
            .unwrap();
        let figure = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(ConnectionFigure::new()))
            .unwrap();
        let child = runtime
            .figure(figure)
            .unwrap()
            .add(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0)))
            .unwrap();
        let a = runtime
            .connections()
            .register_anchor(Box::new(ChopboxAnchor::new(owner)));
        let router = runtime.connections().register_router(Box::new(FixedRouter));
        let connection = runtime
            .connections()
            .register_state(
                figure,
                Some(a),
                Some(a),
                RouterBinding::Explicit { router },
                None,
            )
            .unwrap();
        runtime
            .connections()
            .set_locator(
                connection,
                child,
                Box::new(ExternalBoundsLocator { wrong_size: false }),
            )
            .unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        let mut expected = Rectangle::new(0.0, 0.0, 80.0, 40.0);
        expected.transform(runtime.tree().local_to_surface_transform(owner).unwrap());
        let bounds = runtime.tree().figure_bounds(child).unwrap();
        let connection_bounds = runtime.tree().figure_bounds(figure).unwrap();
        assert_eq!(
            (
                bounds.x + connection_bounds.x,
                bounds.y + connection_bounds.y
            ),
            (expected.x + expected.width, expected.y)
        );
    }
}
