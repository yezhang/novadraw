use std::{cell::Cell, collections::HashMap};

use novadraw::geometry::{ApproxEq, Dimension, Point, PointList, Precision, Rectangle, Vec2};
use novadraw::{
    AnchorError, AnchorGeometry, AnchorGeometryKey, ChopboxAnchor, ConnectionAnchor, ConnectionId,
    ConnectionLocator, ConnectionLocatorStrategy, ConnectionRouter, CoordinateSpace,
    DependencySubject, DirectRouter, EllipseAnchor, FigureId, LabelAnchor, MidpointLocator,
    PathFractionLocator, RoundedRectangleAnchor, RouteError, RouteOutput, RouteRequest,
    SceneQueryError, SceneRead, TrackedSceneQuery, rectangle_boundary_site,
};

const TEST_PRECISION: Precision = Precision::new(1.0e-6);

#[derive(Default)]
struct QueryFixture {
    attached: HashMap<FigureId, bool>,
    origins: HashMap<FigureId, Point>,
    geometry: HashMap<(FigureId, AnchorGeometryKey), AnchorGeometry>,
    flapping_generation: bool,
    generation: Cell<u64>,
}

impl QueryFixture {
    fn insert(
        &mut self,
        figure: FigureId,
        origin: Point,
        key: AnchorGeometryKey,
        geometry: AnchorGeometry,
    ) {
        self.attached.insert(figure, true);
        self.origins.insert(figure, origin);
        self.geometry.insert((figure, key), geometry);
    }

    fn tracked(&self) -> TrackedSceneQuery<'_> {
        TrackedSceneQuery::new(self)
    }

    fn to_surface(&self, point: Point, space: CoordinateSpace) -> Point {
        match space {
            CoordinateSpace::LogicalSurface => point,
            CoordinateSpace::FigureLocal(figure) | CoordinateSpace::ChildContent(figure) => {
                point + (self.origins[&figure] - Point::ORIGIN)
            }
        }
    }

    fn map_from_surface(&self, point: Point, space: CoordinateSpace) -> Point {
        match space {
            CoordinateSpace::LogicalSurface => point,
            CoordinateSpace::FigureLocal(figure) | CoordinateSpace::ChildContent(figure) => {
                point - (self.origins[&figure] - Point::ORIGIN)
            }
        }
    }
}

impl SceneRead for QueryFixture {
    fn is_attached(&self, figure: FigureId) -> bool {
        self.attached.get(&figure).copied().unwrap_or(false)
    }

    fn parent_id(&self, _figure: FigureId) -> Option<FigureId> {
        None
    }

    fn border_box(&self, figure: FigureId) -> Result<Rectangle, SceneQueryError> {
        match SceneRead::anchor_geometry(self, figure, &AnchorGeometryKey::border_box())? {
            AnchorGeometry::Rectangle(bounds)
            | AnchorGeometry::Ellipse(bounds)
            | AnchorGeometry::RoundedRectangle { bounds, .. } => Ok(bounds),
        }
    }

    fn anchor_geometry(
        &self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Result<AnchorGeometry, SceneQueryError> {
        if !self.is_attached(figure) {
            return Err(SceneQueryError::DetachedFigure(figure));
        }
        self.geometry
            .get(&(figure, key.clone()))
            .cloned()
            .ok_or_else(|| SceneQueryError::MissingAnchorGeometry {
                figure,
                key: key.clone(),
            })
    }

    fn map_point(
        &self,
        point: Point,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Point, SceneQueryError> {
        Ok(self.map_from_surface(self.to_surface(point, from), to))
    }

    fn map_rect(
        &self,
        rect: Rectangle,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Rectangle, SceneQueryError> {
        let origin = SceneRead::map_point(self, Point::new(rect.x, rect.y), from, to)?;
        Ok(Rectangle::new(
            origin.x(),
            origin.y(),
            rect.width,
            rect.height,
        ))
    }

    fn map_normal(
        &self,
        normal: Vec2,
        _from: CoordinateSpace,
        _to: CoordinateSpace,
    ) -> Result<Vec2, SceneQueryError> {
        Ok(normal)
    }

    fn dependency_generation(&self, _subject: &DependencySubject) -> u64 {
        if !self.flapping_generation {
            return 0;
        }
        let next = self.generation.get() + 1;
        self.generation.set(next);
        next
    }
}

fn figure_ids(count: usize) -> Vec<FigureId> {
    let mut tree = novadraw::FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(novadraw::RectangleFigure::new(
            0.0, 0.0, 100.0, 100.0,
        )));
    (0..count)
        .map(|_| {
            tree.builder()
                .add_child(
                    root,
                    Box::new(novadraw::RectangleFigure::new(0.0, 0.0, 10.0, 10.0)),
                )
                .expect("valid FigureTree construction")
        })
        .collect()
}

#[test]
fn named_anchor_geometry_key_rejects_empty_values() {
    assert!(AnchorGeometryKey::named("").is_err());
    assert_eq!(
        AnchorGeometryKey::icon(),
        AnchorGeometryKey::named("icon").unwrap()
    );
    assert!(AnchorGeometryKey::border_box().is_border_box());
    assert_eq!(AnchorGeometryKey::icon().name(), Some("icon"));
    assert!(novadraw::AnchorSemanticKey::new(None, "", Vec::new()).is_err());
    assert!(novadraw::AnchorSemanticKey::new(None, "custom", Vec::new()).is_ok());
}

#[test]
fn rectangle_boundary_site_projects_toward_the_reference() {
    let bounds = Rectangle::new(10.0, 20.0, 100.0, 50.0);

    let right = rectangle_boundary_site(bounds, Point::new(200.0, 45.0));
    assert!(
        right
            .point
            .approx_eq(Point::new(110.0, 45.0), TEST_PRECISION)
    );
    assert_eq!(right.outward_normal, Some(Vec2::X));

    let bottom = rectangle_boundary_site(bounds, Point::new(60.0, 200.0));
    assert!(
        bottom
            .point
            .approx_eq(Point::new(60.0, 70.0), TEST_PRECISION)
    );
    assert_eq!(bottom.outward_normal, Some(Vec2::Y));
}

#[test]
fn chopbox_anchor_maps_owner_geometry_into_requested_space() {
    let owner = figure_ids(1)[0];
    let mut query = QueryFixture::default();
    query.insert(
        owner,
        Point::new(10.0, 20.0),
        AnchorGeometryKey::border_box(),
        AnchorGeometry::Rectangle(Rectangle::new(0.0, 0.0, 100.0, 50.0)),
    );
    let anchor = ChopboxAnchor::new(owner);
    let mut tracked = query.tracked();

    let site = anchor
        .location(
            &mut tracked,
            Point::new(200.0, 45.0),
            CoordinateSpace::LogicalSurface,
            CoordinateSpace::LogicalSurface,
        )
        .unwrap();

    assert!(
        site.point
            .approx_eq(Point::new(110.0, 45.0), TEST_PRECISION)
    );
    assert_eq!(site.outward_normal, Some(Vec2::X));
    assert_eq!(
        tracked
            .observations()
            .iter()
            .map(|observation| observation.subject.clone())
            .collect::<Vec<_>>(),
        vec![
            DependencySubject::FigureGeometry(owner),
            DependencySubject::RelativeTransform(
                CoordinateSpace::LogicalSurface,
                CoordinateSpace::FigureLocal(owner),
            ),
            DependencySubject::RelativeTransform(
                CoordinateSpace::FigureLocal(owner),
                CoordinateSpace::LogicalSurface,
            ),
        ]
    );
}

#[test]
fn ellipse_and_rounded_rectangle_follow_their_actual_outlines() {
    let ids = figure_ids(2);
    let ellipse_owner = ids[0];
    let rounded_owner = ids[1];
    let mut query = QueryFixture::default();
    query.insert(
        ellipse_owner,
        Point::ZERO,
        AnchorGeometryKey::border_box(),
        AnchorGeometry::Ellipse(Rectangle::new(0.0, 0.0, 100.0, 60.0)),
    );
    query.insert(
        rounded_owner,
        Point::ZERO,
        AnchorGeometryKey::border_box(),
        AnchorGeometry::RoundedRectangle {
            bounds: Rectangle::new(0.0, 0.0, 100.0, 60.0),
            corner: Dimension::new(20.0, 20.0),
        },
    );

    let ellipse = EllipseAnchor::new(ellipse_owner);
    let mut tracked = query.tracked();
    let ellipse_site = ellipse
        .location(
            &mut tracked,
            Point::new(150.0, 30.0),
            CoordinateSpace::LogicalSurface,
            CoordinateSpace::LogicalSurface,
        )
        .unwrap();
    assert!(
        ellipse_site
            .point
            .approx_eq(Point::new(100.0, 30.0), TEST_PRECISION)
    );

    let rounded = RoundedRectangleAnchor::new(rounded_owner);
    let mut tracked = query.tracked();
    let rounded_site = rounded
        .location(
            &mut tracked,
            Point::new(-50.0, -30.0),
            CoordinateSpace::LogicalSurface,
            CoordinateSpace::LogicalSurface,
        )
        .unwrap();
    assert!(rounded_site.point.x() > 0.0 && rounded_site.point.x() < 10.0);
    assert!(rounded_site.point.y() > 0.0 && rounded_site.point.y() < 10.0);
}

#[test]
fn label_anchor_reads_named_icon_geometry() {
    let owner = figure_ids(1)[0];
    let icon = AnchorGeometryKey::icon();
    let mut query = QueryFixture::default();
    query.insert(
        owner,
        Point::new(100.0, 50.0),
        icon.clone(),
        AnchorGeometry::Rectangle(Rectangle::new(20.0, 10.0, 30.0, 20.0)),
    );

    let mut tracked = query.tracked();
    let site = LabelAnchor::new(owner)
        .location(
            &mut tracked,
            Point::new(300.0, 70.0),
            CoordinateSpace::LogicalSurface,
            CoordinateSpace::LogicalSurface,
        )
        .unwrap();

    assert!(
        site.point
            .approx_eq(Point::new(150.0, 70.0), TEST_PRECISION)
    );
    assert_eq!(
        tracked.observations()[0].subject,
        DependencySubject::NamedAnchorRegion(owner, icon)
    );
}

#[test]
fn direct_router_uses_opposite_reference_points() {
    let ids = figure_ids(3);
    let connection = ConnectionId::from_figure(ids[0]);
    let source = novadraw::XYAnchor::new(Point::new(10.0, 20.0), CoordinateSpace::LogicalSurface);
    let target = novadraw::XYAnchor::new(Point::new(80.0, 60.0), CoordinateSpace::LogicalSurface);
    let query = QueryFixture::default();
    let mut tracked = query.tracked();

    let output = DirectRouter
        .route(RouteRequest {
            connection,
            routing_space: CoordinateSpace::LogicalSurface,
            source: &source,
            target: &target,
            constraint: None,
            scene: &mut tracked,
            group: None,
        })
        .unwrap();

    assert_eq!(
        output.points().as_slice(),
        &[Point::new(10.0, 20.0), Point::new(80.0, 60.0)]
    );
    assert_eq!(output.metadata().source.reference, Point::new(80.0, 60.0));
    assert_eq!(output.metadata().target.reference, Point::new(10.0, 20.0));
}

#[test]
fn direct_router_rejects_constraints_without_partial_output() {
    #[derive(Debug)]
    struct UnexpectedConstraint;

    let ids = figure_ids(1);
    let anchor = novadraw::XYAnchor::new(Point::new(10.0, 20.0), CoordinateSpace::LogicalSurface);
    let constraint = UnexpectedConstraint;
    let query = QueryFixture::default();
    let mut tracked = query.tracked();

    let result = DirectRouter.route(RouteRequest {
        connection: ConnectionId::from_figure(ids[0]),
        routing_space: CoordinateSpace::LogicalSurface,
        source: &anchor,
        target: &anchor,
        constraint: Some(&constraint),
        scene: &mut tracked,
        group: None,
    });

    assert!(matches!(
        result,
        Err(RouteError::ConstraintTypeMismatch { .. })
    ));
}

#[test]
fn route_rejects_dependency_generation_drift() {
    let ids = figure_ids(1);
    let anchor = novadraw::XYAnchor::new(Point::new(10.0, 20.0), CoordinateSpace::LogicalSurface);
    let query = QueryFixture {
        flapping_generation: true,
        ..QueryFixture::default()
    };
    let mut tracked = query.tracked();

    let result = DirectRouter.route(RouteRequest {
        connection: ConnectionId::from_figure(ids[0]),
        routing_space: CoordinateSpace::LogicalSurface,
        source: &anchor,
        target: &anchor,
        constraint: None,
        scene: &mut tracked,
        group: None,
    });

    assert!(matches!(
        result,
        Err(RouteError::Source(AnchorError::Scene(
            SceneQueryError::SceneChangedDuringQuery(_)
        )))
    ));
}

#[test]
fn anchor_semantic_keys_group_equivalent_owner_geometry() {
    let owner = figure_ids(1)[0];
    let first = ChopboxAnchor::new(owner);
    let second = ChopboxAnchor::new(owner);
    let ellipse = EllipseAnchor::new(owner);
    let xy = novadraw::XYAnchor::new(Point::new(10.0, 20.0), CoordinateSpace::LogicalSurface);

    assert_eq!(first.semantic_group_key(), second.semantic_group_key());
    assert_ne!(first.semantic_group_key(), ellipse.semantic_group_key());
    assert_eq!(xy.semantic_group_key(), None);
}

#[test]
fn xy_anchor_maps_from_its_declared_space() {
    let owner = figure_ids(1)[0];
    let mut query = QueryFixture::default();
    query.origins.insert(owner, Point::new(30.0, 40.0));
    let anchor = novadraw::XYAnchor::new(Point::new(5.0, 7.0), CoordinateSpace::FigureLocal(owner));

    let mut tracked = query.tracked();
    assert_eq!(
        anchor
            .reference_point(&mut tracked, CoordinateSpace::LogicalSurface)
            .unwrap(),
        Point::new(35.0, 47.0)
    );
}

#[test]
fn missing_named_geometry_is_a_structured_anchor_error() {
    let owner = figure_ids(1)[0];
    let mut query = QueryFixture::default();
    query.attached.insert(owner, true);
    let mut tracked = query.tracked();

    let error = LabelAnchor::new(owner)
        .reference_point(&mut tracked, CoordinateSpace::LogicalSurface)
        .unwrap_err();

    assert!(matches!(
        error,
        AnchorError::Scene(SceneQueryError::MissingAnchorGeometry { figure, .. })
            if figure == owner
    ));
}

#[test]
fn route_output_rejects_short_and_non_finite_point_lists() {
    let ids = figure_ids(1);
    let anchor = novadraw::XYAnchor::new(Point::new(10.0, 20.0), CoordinateSpace::LogicalSurface);
    let query = QueryFixture::default();
    let mut tracked = query.tracked();
    let valid = DirectRouter
        .route(RouteRequest {
            connection: ConnectionId::from_figure(ids[0]),
            routing_space: CoordinateSpace::LogicalSurface,
            source: &anchor,
            target: &anchor,
            constraint: None,
            scene: &mut tracked,
            group: None,
        })
        .unwrap();
    let metadata = *valid.metadata();

    assert_eq!(
        RouteOutput::new(PointList::from_points(vec![Point::ZERO]), metadata),
        Err(RouteError::TooFewPoints { point_count: 1 })
    );
    assert_eq!(
        RouteOutput::new(
            PointList::from_points(vec![Point::ZERO, Point::new(f64::NAN, 1.0)]),
            metadata,
        ),
        Err(RouteError::NonFinitePoint { index: 1 })
    );

    let mut invalid_metadata = metadata;
    invalid_metadata.source.reference = Point::new(f64::INFINITY, 0.0);
    assert_eq!(
        RouteOutput::new(
            PointList::from_points(vec![Point::ZERO, Point::new(1.0, 1.0)]),
            invalid_metadata,
        ),
        Err(RouteError::NonFiniteEndpointMetadata {
            endpoint: novadraw::RouteEnd::Source,
        })
    );
    assert_eq!(
        RouteOutput::new(
            PointList::from_points(vec![Point::ZERO, metadata.target.site.point]),
            metadata,
        ),
        Err(RouteError::EndpointMismatch {
            endpoint: novadraw::RouteEnd::Source,
        })
    );
}

#[test]
fn locators_preserve_topological_middle_and_arc_fraction_semantics() {
    let points = PointList::from_points(vec![
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
    ]);

    assert_eq!(
        ConnectionLocator::Middle.locate(&points).unwrap().point,
        Point::new(10.0, 0.0)
    );
    assert_eq!(
        MidpointLocator::new(0).locate(&points).unwrap().point,
        Point::new(5.0, 0.0)
    );
    assert_eq!(
        PathFractionLocator::new(0.75)
            .unwrap()
            .locate(&points)
            .unwrap()
            .point,
        Point::new(10.0, 5.0)
    );
}
