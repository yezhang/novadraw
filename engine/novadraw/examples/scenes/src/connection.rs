use novadraw::connection::{
    AnchorGeometry, AnchorGeometryKey, Bendpoint, BendpointConnectionRouter, BendpointConstraint,
    ChopboxAnchor, ConnectionFigure, ConnectionId, ConnectionLayerFigure, ConnectionLocator,
    ConnectionLocatorStrategy, CoordinateSpace, DirectRouter, EllipseAnchor,
    FAN_DEFAULT_SEPARATION, FanRouter, LabelAnchor, ManhattanConnectionRouter,
    RoundedRectangleAnchor, RouterBinding, XYAnchor,
};
use novadraw::container::ViewportFigure;
use novadraw::geometry::{Dimension, Point, Vec2};
use novadraw::{
    figure::EllipseFigure, figure::PolygonFigure, figure::RectangleFigure,
    figure::RoundedRectangleFigure, geometry::Rectangle, graphics::Color, runtime::Runtime,
};

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
        .expect("valid figure bounds")
}

fn connection(color: Color) -> ConnectionFigure {
    ConnectionFigure::new()
        .with_stroke(color, CONNECTION_WIDTH)
        .expect("valid detached figure input")
        .with_decoration_insets(0.0, ARROW_LENGTH - ARROW_LINE_OVERLAP)
        .expect("valid detached figure input")
}

fn resolve_with_arrow(
    runtime: &mut Runtime,
    connection: ConnectionId,
    connection_figure: novadraw::tree::FigureId,
    root: novadraw::tree::FigureId,
) {
    runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    let points = runtime
        .tree()
        .connection_route_points(connection_figure)
        .unwrap()
        .unwrap()
        .clone();
    let arrow_color = runtime
        .tree()
        .connection_stroke_color(connection_figure)
        .unwrap()
        .unwrap();
    let placement = ConnectionLocator::Target.locate(&points).unwrap();
    let direction = placement.point - placement.reference;
    let length = direction.length();
    if length <= f64::EPSILON {
        return;
    }
    let direction = direction / length;
    let perpendicular = Vec2::new(-direction.y(), direction.x());
    let base = placement.point - direction * ARROW_LENGTH;
    let arrow = PolygonFigure::from_points(vec![
        placement.point,
        base + perpendicular * 6.0,
        base - perpendicular * 6.0,
    ])
    .expect("valid detached figure input")
    .with_fill_color(arrow_color)
    .with_stroke(arrow_color, 1.0)
    .expect("valid detached figure input");
    runtime
        .figure(connection_figure)
        .unwrap()
        .add(Box::new(arrow))
        .expect("valid Runtime mutation");
}

fn anchor_matrix() -> Runtime {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(background()))
        .expect("valid Runtime mutation");
    let connection_ids: Vec<_> = (0..5)
        .map(|index| {
            runtime
                .figure(root)
                .unwrap()
                .add(Box::new(connection(Color::rgba(
                    0.10 + index as f64 * 0.08,
                    0.28,
                    0.62,
                    1.0,
                ))))
                .expect("valid Runtime mutation")
        })
        .collect();

    for (index, connection_figure) in connection_ids.into_iter().enumerate() {
        let y = 30.0 + index as f64 * 108.0;
        let (source, target) = match index {
            1 => (
                runtime
                    .figure(root)
                    .unwrap()
                    .add(Box::new(
                        EllipseFigure::new_with_color(
                            SOURCE_X,
                            y,
                            NODE_WIDTH,
                            NODE_HEIGHT,
                            Color::rgba(0.20, 0.65, 0.56, 1.0),
                        )
                        .expect("valid figure bounds"),
                    ))
                    .expect("valid Runtime mutation"),
                runtime
                    .figure(root)
                    .unwrap()
                    .add(Box::new(
                        EllipseFigure::new_with_color(
                            TARGET_X,
                            y,
                            NODE_WIDTH,
                            NODE_HEIGHT,
                            Color::rgba(0.88, 0.40, 0.38, 1.0),
                        )
                        .expect("valid figure bounds"),
                    ))
                    .expect("valid Runtime mutation"),
            ),
            2 => (
                runtime
                    .figure(root)
                    .unwrap()
                    .add(Box::new(
                        RoundedRectangleFigure::new_with_color(
                            SOURCE_X,
                            y,
                            NODE_WIDTH,
                            NODE_HEIGHT,
                            24.0,
                            Color::rgba(0.20, 0.65, 0.56, 1.0),
                        )
                        .expect("valid figure bounds"),
                    ))
                    .expect("valid Runtime mutation"),
                runtime
                    .figure(root)
                    .unwrap()
                    .add(Box::new(
                        RoundedRectangleFigure::new_with_color(
                            TARGET_X,
                            y,
                            NODE_WIDTH,
                            NODE_HEIGHT,
                            24.0,
                            Color::rgba(0.88, 0.40, 0.38, 1.0),
                        )
                        .expect("valid figure bounds"),
                    ))
                    .expect("valid Runtime mutation"),
            ),
            _ => (
                runtime
                    .figure(root)
                    .unwrap()
                    .add(Box::new(
                        RectangleFigure::new_with_color(
                            SOURCE_X,
                            y,
                            NODE_WIDTH,
                            NODE_HEIGHT,
                            Color::rgba(0.20, 0.65, 0.56, 1.0),
                        )
                        .expect("valid figure bounds"),
                    ))
                    .expect("valid Runtime mutation"),
                runtime
                    .figure(root)
                    .unwrap()
                    .add(Box::new(
                        RectangleFigure::new_with_color(
                            TARGET_X,
                            y,
                            NODE_WIDTH,
                            NODE_HEIGHT,
                            Color::rgba(0.88, 0.40, 0.38, 1.0),
                        )
                        .expect("valid figure bounds"),
                    ))
                    .expect("valid Runtime mutation"),
            ),
        };

        let (source_anchor, target_anchor) = match index {
            0 => (
                runtime
                    .connections()
                    .register_anchor(Box::new(ChopboxAnchor::new(source))),
                runtime
                    .connections()
                    .register_anchor(Box::new(ChopboxAnchor::new(target))),
            ),
            1 => (
                runtime
                    .connections()
                    .register_anchor(Box::new(EllipseAnchor::new(source))),
                runtime
                    .connections()
                    .register_anchor(Box::new(EllipseAnchor::new(target))),
            ),
            2 => (
                runtime.connections().register_anchor(Box::new(
                    RoundedRectangleAnchor::with_corner_dimensions(
                        source,
                        Dimension::new(48.0, 48.0),
                    ),
                )),
                runtime.connections().register_anchor(Box::new(
                    RoundedRectangleAnchor::with_corner_dimensions(
                        target,
                        Dimension::new(48.0, 48.0),
                    ),
                )),
            ),
            3 => {
                let icon = AnchorGeometryKey::icon();
                runtime
                    .connections()
                    .set_anchor_geometry(
                        source,
                        icon.clone(),
                        AnchorGeometry::Rectangle(Rectangle::new(10.0, 12.0, 28.0, 28.0)),
                    )
                    .unwrap();
                runtime
                    .connections()
                    .set_anchor_geometry(
                        target,
                        icon,
                        AnchorGeometry::Rectangle(Rectangle::new(62.0, 12.0, 28.0, 28.0)),
                    )
                    .unwrap();
                runtime
                    .figure(source)
                    .unwrap()
                    .add(Box::new(
                        EllipseFigure::new_with_color(
                            10.0,
                            12.0,
                            28.0,
                            28.0,
                            Color::rgba(0.95, 0.82, 0.22, 1.0),
                        )
                        .expect("valid figure bounds"),
                    ))
                    .expect("valid Runtime mutation");
                runtime
                    .figure(target)
                    .unwrap()
                    .add(Box::new(
                        EllipseFigure::new_with_color(
                            62.0,
                            12.0,
                            28.0,
                            28.0,
                            Color::rgba(0.95, 0.82, 0.22, 1.0),
                        )
                        .expect("valid figure bounds"),
                    ))
                    .expect("valid Runtime mutation");
                (
                    runtime
                        .connections()
                        .register_anchor(Box::new(LabelAnchor::new(source))),
                    runtime
                        .connections()
                        .register_anchor(Box::new(LabelAnchor::new(target))),
                )
            }
            _ => (
                runtime
                    .connections()
                    .register_anchor(Box::new(XYAnchor::new(
                        Point::new(SOURCE_X + NODE_WIDTH, y + NODE_HEIGHT / 2.0),
                        CoordinateSpace::ChildContent(root),
                    ))),
                runtime
                    .connections()
                    .register_anchor(Box::new(XYAnchor::new(
                        Point::new(TARGET_X, y + NODE_HEIGHT / 2.0),
                        CoordinateSpace::ChildContent(root),
                    ))),
            ),
        };
        let direct_router = runtime.connections().direct_router();
        let connection_id = runtime
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
        resolve_with_arrow(&mut runtime, connection_id, connection_figure, root);
    }
    runtime
}

fn bendpoint_scene() -> Runtime {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(background()))
        .expect("valid Runtime mutation");
    let connection_figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(connection(Color::rgba(0.28, 0.35, 0.75, 1.0))))
        .expect("valid Runtime mutation");
    let source = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(
                70.0,
                250.0,
                NODE_WIDTH,
                NODE_HEIGHT,
                Color::rgba(0.20, 0.65, 0.56, 1.0),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid Runtime mutation");
    let target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(
                630.0,
                250.0,
                NODE_WIDTH,
                NODE_HEIGHT,
                Color::rgba(0.88, 0.40, 0.38, 1.0),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid Runtime mutation");
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
        Bendpoint::Absolute(Point::new(250.0, 130.0)),
        Bendpoint::Relative {
            source_offset: Vec2::new(0.0, 54.0),
            target_offset: Vec2::new(0.0, 54.0),
            weight: 0.5,
        },
        Bendpoint::Absolute(Point::new(540.0, 410.0)),
    ]);
    let id = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            Some(Box::new(constraint)),
        )
        .unwrap();
    resolve_with_arrow(&mut runtime, id, connection_figure, root);
    runtime
}

fn manhattan_scene() -> Runtime {
    manhattan_scene_with_moved_nodes(false)
}

fn manhattan_scene_with_moved_nodes(moved: bool) -> Runtime {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(background()))
        .expect("valid Runtime mutation");
    let router = runtime
        .connections()
        .register_router(Box::new(ManhattanConnectionRouter));
    for index in 0..3 {
        let connection_figure = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(connection(Color::rgba(
                0.20,
                0.30 + index as f64 * 0.12,
                0.68,
                1.0,
            ))))
            .expect("valid Runtime mutation");
        let y = 75.0 + index as f64 * 165.0;
        let source_x = 80.0 + if moved && index == 1 { 75.0 } else { 0.0 };
        let target_y_offset = if moved && index != 1 { 55.0 } else { 0.0 };
        let source = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(
                RectangleFigure::new_with_color(
                    source_x,
                    y,
                    NODE_WIDTH,
                    NODE_HEIGHT,
                    Color::rgba(0.20, 0.65, 0.56, 1.0),
                )
                .expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let target = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(
                RectangleFigure::new_with_color(
                    590.0,
                    y + if index % 2 == 0 { 80.0 } else { -45.0 } + target_y_offset,
                    NODE_WIDTH,
                    NODE_HEIGHT,
                    Color::rgba(0.88, 0.40, 0.38, 1.0),
                )
                .expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let source_anchor = runtime
            .connections()
            .register_anchor(Box::new(ChopboxAnchor::new(source)));
        let target_anchor = runtime
            .connections()
            .register_anchor(Box::new(ChopboxAnchor::new(target)));
        let id = runtime
            .connections()
            .register_state(
                connection_figure,
                Some(source_anchor),
                Some(target_anchor),
                RouterBinding::Explicit { router },
                None,
            )
            .unwrap();
        resolve_with_arrow(&mut runtime, id, connection_figure, root);
    }
    runtime
}

fn shared_manhattan_scene() -> Runtime {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(background()))
        .expect("valid Runtime mutation");
    let router = runtime
        .connections()
        .register_router(Box::new(ManhattanConnectionRouter));
    let source = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(
                80.0,
                250.0,
                120.0,
                70.0,
                Color::rgba(0.20, 0.65, 0.56, 1.0),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid Runtime mutation");
    let target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(
                600.0,
                300.0,
                120.0,
                70.0,
                Color::rgba(0.88, 0.40, 0.38, 1.0),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let mut entries = Vec::new();
    for index in 0..4 {
        let connection_figure = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(connection(Color::rgba(
                0.16 + index as f64 * 0.10,
                0.30,
                0.74 - index as f64 * 0.08,
                1.0,
            ))))
            .expect("valid Runtime mutation");
        let connection_id = runtime
            .connections()
            .register_state(
                connection_figure,
                Some(source_anchor),
                Some(target_anchor),
                RouterBinding::Explicit { router },
                None,
            )
            .unwrap();
        entries.push((connection_id, connection_figure));
    }
    for (connection_id, connection_figure) in entries {
        resolve_with_arrow(&mut runtime, connection_id, connection_figure, root);
    }
    runtime
}

fn unsupported_viewport_topology_scene() -> Runtime {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(background()))
        .expect("valid Runtime mutation");
    let viewport = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            ViewportFigure::new(45.0, 90.0, 300.0, 360.0).expect("valid detached figure input"),
        ))
        .expect("valid Runtime mutation");
    let contents = runtime
        .figure(viewport)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(
                0.0,
                0.0,
                500.0,
                420.0,
                Color::rgba(0.84, 0.90, 0.96, 1.0),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid Runtime mutation");
    let source = runtime
        .figure(contents)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(
                60.0,
                135.0,
                NODE_WIDTH,
                NODE_HEIGHT,
                Color::rgba(0.20, 0.65, 0.56, 1.0),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid Runtime mutation");
    let target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(
                610.0,
                250.0,
                NODE_WIDTH,
                NODE_HEIGHT,
                Color::rgba(0.88, 0.40, 0.38, 1.0),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid Runtime mutation");
    let connection_figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(connection(Color::rgba(0.72, 0.18, 0.22, 1.0))))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection_id = runtime
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
    assert!(matches!(
        runtime
            .connections()
            .resolve_route(connection_id, CoordinateSpace::ChildContent(root)),
        Err(novadraw::connection::ConnectionRuntimeError::Unresolved(
            novadraw::connection::UnresolvedConnection::RouteFailed(
                novadraw::connection::RouteError::UnsupportedViewportTopology
            )
        ))
    ));
    runtime
}

fn fan_scene() -> Runtime {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(background()))
        .expect("valid Runtime mutation");
    let connection_figures: Vec<_> = (0..5)
        .map(|index| {
            runtime
                .figure(root)
                .unwrap()
                .add(Box::new(connection(Color::rgba(
                    0.12 + index as f64 * 0.08,
                    0.28,
                    0.70 - index as f64 * 0.06,
                    1.0,
                ))))
                .expect("valid Runtime mutation")
        })
        .collect();
    let source = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            EllipseFigure::new_with_color(
                80.0,
                250.0,
                120.0,
                82.0,
                Color::rgba(0.20, 0.65, 0.56, 1.0),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid Runtime mutation");
    let target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            EllipseFigure::new_with_color(
                600.0,
                250.0,
                120.0,
                82.0,
                Color::rgba(0.88, 0.40, 0.38, 1.0),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid Runtime mutation");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(EllipseAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(EllipseAnchor::new(target)));
    let router = runtime.connections().register_router(Box::new(
        FanRouter::new(Box::new(DirectRouter), FAN_DEFAULT_SEPARATION).unwrap(),
    ));
    let ids: Vec<_> = connection_figures
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
    runtime
}

fn moved_nodes_scene() -> Runtime {
    manhattan_scene_with_moved_nodes(true)
}

fn connection_layer_scene() -> Runtime {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(background()))
        .expect("valid Runtime mutation");
    let layer = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionLayerFigure::new(
            0.0, 0.0, WIDTH, HEIGHT,
        )))
        .expect("valid Runtime mutation");
    let connection_figure = runtime
        .figure(layer)
        .unwrap()
        .add(Box::new(connection(Color::rgba(0.24, 0.40, 0.76, 1.0))))
        .expect("valid Runtime mutation");
    let source = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RoundedRectangleFigure::new_with_color(
                110.0,
                250.0,
                120.0,
                70.0,
                20.0,
                Color::rgba(0.20, 0.65, 0.56, 1.0),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid Runtime mutation");
    let target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RoundedRectangleFigure::new_with_color(
                580.0,
                180.0,
                120.0,
                70.0,
                20.0,
                Color::rgba(0.88, 0.40, 0.38, 1.0),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid Runtime mutation");
    let source_anchor = runtime.connections().register_anchor(Box::new(
        RoundedRectangleAnchor::with_corner_dimensions(source, Dimension::new(40.0, 40.0)),
    ));
    let target_anchor = runtime.connections().register_anchor(Box::new(
        RoundedRectangleAnchor::with_corner_dimensions(target, Dimension::new(40.0, 40.0)),
    ));
    let router = runtime
        .connections()
        .register_router(Box::new(ManhattanConnectionRouter));
    runtime
        .connections()
        .set_layer_router(layer, router)
        .unwrap();
    let id = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Inherited { layer },
            None,
        )
        .unwrap();
    resolve_with_arrow(&mut runtime, id, connection_figure, layer);
    runtime
}

pub fn suite() -> DemoSuite {
    DemoSuite::new(
        "connection",
        "Connection / Anchor / Router",
        vec![
            SceneSpec::runtime_visual("anchor-matrix", "anchor_matrix", (800, 600), anchor_matrix),
            SceneSpec::runtime_visual("bendpoint", "bendpoint", (800, 600), bendpoint_scene),
            SceneSpec::runtime_visual("manhattan", "manhattan", (800, 600), manhattan_scene),
            SceneSpec::runtime_visual(
                "shared-manhattan",
                "shared_manhattan",
                (800, 600),
                shared_manhattan_scene,
            ),
            SceneSpec::runtime_visual("fan", "fan", (800, 600), fan_scene),
            SceneSpec::runtime_visual("moved-nodes", "moved_nodes", (800, 600), moved_nodes_scene),
            SceneSpec::runtime_visual(
                "connection-layer",
                "connection_layer",
                (800, 600),
                connection_layer_scene,
            ),
            SceneSpec::runtime_visual(
                "unsupported-viewport-topology",
                "unsupported_viewport_topology",
                (800, 600),
                unsupported_viewport_topology_scene,
            ),
        ],
    )
}
