use novadraw::{
    AnchorGeometry, AnchorGeometryKey, Bendpoint, BendpointConnectionRouter, BendpointConstraint,
    ChopboxAnchor, Color, ConnectionFigure, ConnectionId, ConnectionLayerFigure, ConnectionLocator,
    ConnectionLocatorStrategy, CoordinateSpace, DirectRouter, EllipseAnchor, EllipseFigure,
    FAN_DEFAULT_SEPARATION, FanRouter, LabelAnchor, ManhattanConnectionRouter, PolygonFigure,
    Rectangle, RectangleFigure, RoundedRectangleAnchor, RoundedRectangleFigure, RouterBinding,
    Runtime, XYAnchor,
};
use novadraw_geometry::{Dimension, Point, Vector};

use crate::{DemoSuite, SceneSpec};

const WIDTH: f64 = 800.0;
const HEIGHT: f64 = 600.0;
const NODE_WIDTH: f64 = 100.0;
const NODE_HEIGHT: f64 = 52.0;
const SOURCE_X: f64 = 70.0;
const TARGET_X: f64 = 630.0;
const CONNECTION_WIDTH: f64 = 3.0;
const ARROW_LENGTH: f64 = 15.0;
const ARROW_LINE_OVERLAP: f64 = 1.0;

fn background() -> RectangleFigure {
    RectangleFigure::new_with_color(0.0, 0.0, WIDTH, HEIGHT, Color::rgba(0.93, 0.94, 0.95, 1.0))
}

fn connection(color: Color) -> ConnectionFigure {
    ConnectionFigure::new()
        .with_stroke(color, CONNECTION_WIDTH)
        .with_decoration_insets(0.0, ARROW_LENGTH - ARROW_LINE_OVERLAP)
}

fn resolve_with_arrow(
    runtime: &mut Runtime,
    connection: ConnectionId,
    connection_figure: novadraw::FigureId,
    root: novadraw::FigureId,
) {
    runtime
        .resolve_connection_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    let points = runtime
        .tree()
        .connection_route_points(connection_figure)
        .unwrap()
        .clone();
    let arrow_color = runtime
        .tree()
        .connection_stroke_color(connection_figure)
        .unwrap();
    let placement = ConnectionLocator::Target.locate(&points).unwrap();
    let direction = placement.point - placement.reference;
    let length = direction.length();
    if length <= f64::EPSILON {
        return;
    }
    let direction = direction / length;
    let perpendicular = Vector::new(-direction.y(), direction.x());
    let base = placement.point - direction * ARROW_LENGTH;
    let arrow = PolygonFigure::from_points(vec![
        placement.point,
        base + perpendicular * 6.0,
        base - perpendicular * 6.0,
    ])
    .with_fill_color(arrow_color)
    .with_stroke(arrow_color, 1.0);
    runtime.add_figure(connection_figure, Box::new(arrow));
}

fn anchor_matrix() -> novadraw::FigureTree {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(background()));
    let connection_ids: Vec<_> = (0..5)
        .map(|index| {
            runtime.add_figure(
                root,
                Box::new(connection(Color::rgba(
                    0.10 + index as f64 * 0.08,
                    0.28,
                    0.62,
                    1.0,
                ))),
            )
        })
        .collect();

    for (index, connection_figure) in connection_ids.into_iter().enumerate() {
        let y = 30.0 + index as f64 * 108.0;
        let (source, target) = match index {
            1 => (
                runtime.add_figure(
                    root,
                    Box::new(EllipseFigure::new_with_color(
                        SOURCE_X,
                        y,
                        NODE_WIDTH,
                        NODE_HEIGHT,
                        Color::rgba(0.20, 0.65, 0.56, 1.0),
                    )),
                ),
                runtime.add_figure(
                    root,
                    Box::new(EllipseFigure::new_with_color(
                        TARGET_X,
                        y,
                        NODE_WIDTH,
                        NODE_HEIGHT,
                        Color::rgba(0.88, 0.40, 0.38, 1.0),
                    )),
                ),
            ),
            2 => (
                runtime.add_figure(
                    root,
                    Box::new(RoundedRectangleFigure::new_with_color(
                        SOURCE_X,
                        y,
                        NODE_WIDTH,
                        NODE_HEIGHT,
                        24.0,
                        Color::rgba(0.20, 0.65, 0.56, 1.0),
                    )),
                ),
                runtime.add_figure(
                    root,
                    Box::new(RoundedRectangleFigure::new_with_color(
                        TARGET_X,
                        y,
                        NODE_WIDTH,
                        NODE_HEIGHT,
                        24.0,
                        Color::rgba(0.88, 0.40, 0.38, 1.0),
                    )),
                ),
            ),
            _ => (
                runtime.add_figure(
                    root,
                    Box::new(RectangleFigure::new_with_color(
                        SOURCE_X,
                        y,
                        NODE_WIDTH,
                        NODE_HEIGHT,
                        Color::rgba(0.20, 0.65, 0.56, 1.0),
                    )),
                ),
                runtime.add_figure(
                    root,
                    Box::new(RectangleFigure::new_with_color(
                        TARGET_X,
                        y,
                        NODE_WIDTH,
                        NODE_HEIGHT,
                        Color::rgba(0.88, 0.40, 0.38, 1.0),
                    )),
                ),
            ),
        };

        let (source_anchor, target_anchor) = match index {
            0 => (
                runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source))),
                runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target))),
            ),
            1 => (
                runtime.register_connection_anchor(Box::new(EllipseAnchor::new(source))),
                runtime.register_connection_anchor(Box::new(EllipseAnchor::new(target))),
            ),
            2 => (
                runtime.register_connection_anchor(Box::new(
                    RoundedRectangleAnchor::with_corner_dimensions(
                        source,
                        Dimension::new(48.0, 48.0),
                    ),
                )),
                runtime.register_connection_anchor(Box::new(
                    RoundedRectangleAnchor::with_corner_dimensions(
                        target,
                        Dimension::new(48.0, 48.0),
                    ),
                )),
            ),
            3 => {
                let icon = AnchorGeometryKey::icon();
                runtime
                    .set_anchor_geometry(
                        source,
                        icon.clone(),
                        AnchorGeometry::Rectangle(Rectangle::new(10.0, 12.0, 28.0, 28.0)),
                    )
                    .unwrap();
                runtime
                    .set_anchor_geometry(
                        target,
                        icon,
                        AnchorGeometry::Rectangle(Rectangle::new(62.0, 12.0, 28.0, 28.0)),
                    )
                    .unwrap();
                runtime.add_figure(
                    source,
                    Box::new(EllipseFigure::new_with_color(
                        10.0,
                        12.0,
                        28.0,
                        28.0,
                        Color::rgba(0.95, 0.82, 0.22, 1.0),
                    )),
                );
                runtime.add_figure(
                    target,
                    Box::new(EllipseFigure::new_with_color(
                        62.0,
                        12.0,
                        28.0,
                        28.0,
                        Color::rgba(0.95, 0.82, 0.22, 1.0),
                    )),
                );
                (
                    runtime.register_connection_anchor(Box::new(LabelAnchor::new(source))),
                    runtime.register_connection_anchor(Box::new(LabelAnchor::new(target))),
                )
            }
            _ => (
                runtime.register_connection_anchor(Box::new(XYAnchor::new(
                    Point::new(SOURCE_X + NODE_WIDTH, y + NODE_HEIGHT / 2.0),
                    CoordinateSpace::ChildContent(root),
                ))),
                runtime.register_connection_anchor(Box::new(XYAnchor::new(
                    Point::new(TARGET_X, y + NODE_HEIGHT / 2.0),
                    CoordinateSpace::ChildContent(root),
                ))),
            ),
        };
        let connection_id = runtime
            .register_connection_state(
                connection_figure,
                Some(source_anchor),
                Some(target_anchor),
                RouterBinding::Explicit {
                    router: runtime.direct_connection_router(),
                },
                None,
            )
            .unwrap();
        resolve_with_arrow(&mut runtime, connection_id, connection_figure, root);
    }
    runtime.into_tree()
}

fn bendpoint_scene() -> novadraw::FigureTree {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(background()));
    let connection_figure = runtime.add_figure(
        root,
        Box::new(connection(Color::rgba(0.28, 0.35, 0.75, 1.0))),
    );
    let source = runtime.add_figure(
        root,
        Box::new(RectangleFigure::new_with_color(
            70.0,
            250.0,
            NODE_WIDTH,
            NODE_HEIGHT,
            Color::rgba(0.20, 0.65, 0.56, 1.0),
        )),
    );
    let target = runtime.add_figure(
        root,
        Box::new(RectangleFigure::new_with_color(
            630.0,
            250.0,
            NODE_WIDTH,
            NODE_HEIGHT,
            Color::rgba(0.88, 0.40, 0.38, 1.0),
        )),
    );
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.register_connection_router(Box::new(BendpointConnectionRouter));
    let constraint = BendpointConstraint::new(vec![
        Bendpoint::Absolute(Point::new(250.0, 130.0)),
        Bendpoint::Relative {
            source_offset: Vector::new(0.0, 54.0),
            target_offset: Vector::new(0.0, 54.0),
            weight: 0.5,
        },
        Bendpoint::Absolute(Point::new(540.0, 410.0)),
    ]);
    let id = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            Some(Box::new(constraint)),
        )
        .unwrap();
    resolve_with_arrow(&mut runtime, id, connection_figure, root);
    runtime.into_tree()
}

fn manhattan_scene() -> novadraw::FigureTree {
    manhattan_scene_with_moved_nodes(false)
}

fn manhattan_scene_with_moved_nodes(moved: bool) -> novadraw::FigureTree {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(background()));
    let router = runtime.register_connection_router(Box::new(ManhattanConnectionRouter));
    for index in 0..3 {
        let connection_figure = runtime.add_figure(
            root,
            Box::new(connection(Color::rgba(
                0.20,
                0.30 + index as f64 * 0.12,
                0.68,
                1.0,
            ))),
        );
        let y = 75.0 + index as f64 * 165.0;
        let source_x = 80.0 + if moved && index == 1 { 75.0 } else { 0.0 };
        let target_y_offset = if moved && index != 1 { 55.0 } else { 0.0 };
        let source = runtime.add_figure(
            root,
            Box::new(RectangleFigure::new_with_color(
                source_x,
                y,
                NODE_WIDTH,
                NODE_HEIGHT,
                Color::rgba(0.20, 0.65, 0.56, 1.0),
            )),
        );
        let target = runtime.add_figure(
            root,
            Box::new(RectangleFigure::new_with_color(
                590.0,
                y + if index % 2 == 0 { 80.0 } else { -45.0 } + target_y_offset,
                NODE_WIDTH,
                NODE_HEIGHT,
                Color::rgba(0.88, 0.40, 0.38, 1.0),
            )),
        );
        let source_anchor =
            runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
        let target_anchor =
            runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
        let id = runtime
            .register_connection_state(
                connection_figure,
                Some(source_anchor),
                Some(target_anchor),
                RouterBinding::Explicit { router },
                None,
            )
            .unwrap();
        resolve_with_arrow(&mut runtime, id, connection_figure, root);
    }
    runtime.into_tree()
}

fn fan_scene() -> novadraw::FigureTree {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(background()));
    let connection_figures: Vec<_> = (0..5)
        .map(|index| {
            runtime.add_figure(
                root,
                Box::new(connection(Color::rgba(
                    0.12 + index as f64 * 0.08,
                    0.28,
                    0.70 - index as f64 * 0.06,
                    1.0,
                ))),
            )
        })
        .collect();
    let source = runtime.add_figure(
        root,
        Box::new(EllipseFigure::new_with_color(
            80.0,
            250.0,
            120.0,
            82.0,
            Color::rgba(0.20, 0.65, 0.56, 1.0),
        )),
    );
    let target = runtime.add_figure(
        root,
        Box::new(EllipseFigure::new_with_color(
            600.0,
            250.0,
            120.0,
            82.0,
            Color::rgba(0.88, 0.40, 0.38, 1.0),
        )),
    );
    let source_anchor = runtime.register_connection_anchor(Box::new(EllipseAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(EllipseAnchor::new(target)));
    let router = runtime.register_connection_router(Box::new(
        FanRouter::new(Box::new(DirectRouter), FAN_DEFAULT_SEPARATION).unwrap(),
    ));
    let ids: Vec<_> = connection_figures
        .into_iter()
        .map(|figure| {
            runtime
                .register_connection_state(
                    figure,
                    Some(source_anchor),
                    Some(target_anchor),
                    RouterBinding::Explicit { router },
                    None,
                )
                .unwrap()
        })
        .collect();
    for (id, figure) in ids.into_iter().zip(
        runtime
            .tree()
            .child_order(root)
            .unwrap()
            .into_iter()
            .take(5),
    ) {
        resolve_with_arrow(&mut runtime, id, figure, root);
    }
    runtime.into_tree()
}

fn moved_nodes_scene() -> novadraw::FigureTree {
    manhattan_scene_with_moved_nodes(true)
}

fn connection_layer_scene() -> novadraw::FigureTree {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(background()));
    let layer = runtime.add_figure(
        root,
        Box::new(ConnectionLayerFigure::new(0.0, 0.0, WIDTH, HEIGHT)),
    );
    let connection_figure = runtime.add_figure(
        layer,
        Box::new(connection(Color::rgba(0.24, 0.40, 0.76, 1.0))),
    );
    let source = runtime.add_figure(
        root,
        Box::new(RoundedRectangleFigure::new_with_color(
            110.0,
            250.0,
            120.0,
            70.0,
            20.0,
            Color::rgba(0.20, 0.65, 0.56, 1.0),
        )),
    );
    let target = runtime.add_figure(
        root,
        Box::new(RoundedRectangleFigure::new_with_color(
            580.0,
            180.0,
            120.0,
            70.0,
            20.0,
            Color::rgba(0.88, 0.40, 0.38, 1.0),
        )),
    );
    let source_anchor = runtime.register_connection_anchor(Box::new(
        RoundedRectangleAnchor::with_corner_dimensions(source, Dimension::new(40.0, 40.0)),
    ));
    let target_anchor = runtime.register_connection_anchor(Box::new(
        RoundedRectangleAnchor::with_corner_dimensions(target, Dimension::new(40.0, 40.0)),
    ));
    let router = runtime.register_connection_router(Box::new(ManhattanConnectionRouter));
    runtime.set_connection_layer_router(layer, router).unwrap();
    let id = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Inherited { layer },
            None,
        )
        .unwrap();
    resolve_with_arrow(&mut runtime, id, connection_figure, layer);
    runtime.into_tree()
}

pub fn suite() -> DemoSuite {
    DemoSuite::new(
        "connection",
        "Connection / Anchor / Router",
        vec![
            SceneSpec::visual("anchor-matrix", "anchor_matrix", (800, 600), anchor_matrix),
            SceneSpec::visual("bendpoint", "bendpoint", (800, 600), bendpoint_scene),
            SceneSpec::visual("manhattan", "manhattan", (800, 600), manhattan_scene),
            SceneSpec::visual("fan", "fan", (800, 600), fan_scene),
            SceneSpec::visual("moved-nodes", "moved_nodes", (800, 600), moved_nodes_scene),
            SceneSpec::visual(
                "connection-layer",
                "connection_layer",
                (800, 600),
                connection_layer_scene,
            ),
        ],
    )
}
