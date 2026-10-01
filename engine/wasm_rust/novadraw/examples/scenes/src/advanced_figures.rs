//! Shared visual examples for advanced Figure and connection-routing features.

use novadraw::connection::{
    ChopboxAnchor, ConnectionFigure, ConnectionLocator, CoordinateSpace, EndpointLocator,
    PolygonDecorationFigure, PolylineDecorationFigure, RouterBinding, ShortestPathConnectionRouter,
};
use novadraw::render::{BuiltinFont, LineJoin};
use novadraw::{
    Alignment, Color, FigureId, FigureStyle, FlowPage, FlowParagraph, FlowWrapping,
    InlineTextFragment, LabelFigure, Point, PointList, PolygonScaleMode, Rectangle,
    RectangleFigure, RoundedRectangleFigure, Runtime, ScalablePolygonFigure, TextFlowFigure,
};

use crate::{DemoSuite, SceneSpec};

const WIDTH: f64 = 800.0;
const HEIGHT: f64 = 600.0;
const SCENE_SIZE: (u32, u32) = (WIDTH as u32, HEIGHT as u32);
const BACKGROUND: Color = Color::rgba(0.95, 0.96, 0.97, 1.0);
const PANEL_FILL: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);
const PANEL_STROKE: Color = Color::rgba(0.76, 0.79, 0.83, 1.0);
const INK: Color = Color::rgba(0.10, 0.14, 0.20, 1.0);
const MUTED: Color = Color::rgba(0.34, 0.39, 0.46, 1.0);
const BLUE: Color = Color::rgba(0.05, 0.44, 0.72, 1.0);
const GREEN: Color = Color::rgba(0.06, 0.57, 0.42, 1.0);
const RED: Color = Color::rgba(0.82, 0.24, 0.25, 1.0);
const AMBER: Color = Color::rgba(0.91, 0.59, 0.08, 1.0);
const CONNECTION_WIDTH: f64 = 3.0;
const DECORATION_TEMPLATE_LENGTH: f64 = 10.0;
const DECORATION_LINE_OVERLAP: f64 = 1.0;
const SOURCE_DECORATION_SCALE: f64 = 1.5;
const TARGET_DECORATION_SCALE_X: f64 = 1.8;
const TARGET_DECORATION_SCALE_Y: f64 = 1.6;
const SOURCE_DECORATION_INSET: f64 =
    DECORATION_TEMPLATE_LENGTH * SOURCE_DECORATION_SCALE - DECORATION_LINE_OVERLAP;
const TARGET_DECORATION_INSET: f64 =
    DECORATION_TEMPLATE_LENGTH * TARGET_DECORATION_SCALE_X - DECORATION_LINE_OVERLAP;

pub fn suite() -> DemoSuite {
    DemoSuite::new(
        "advanced-figures",
        "Advanced Figures",
        vec![
            SceneSpec::runtime_visual(
                "connection-decoration",
                "Connection Decoration",
                SCENE_SIZE,
                connection_decoration_scene,
            ),
            SceneSpec::runtime_visual(
                "shortest-path-routing",
                "Shortest Path Routing",
                SCENE_SIZE,
                shortest_path_scene,
            ),
            SceneSpec::runtime_visual(
                "scalable-polygon",
                "Scalable Polygon",
                SCENE_SIZE,
                scalable_polygon_scene,
            ),
            SceneSpec::runtime_visual("text-flow", "Text Flow", SCENE_SIZE, text_flow_scene),
        ],
    )
}

fn scene_runtime(title: &str, subtitle: &str) -> (Runtime, FigureId) {
    let mut runtime = Runtime::empty();
    for font in BuiltinFont::ALL {
        runtime.register_builtin_font(font).expect("built-in font");
    }
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0, 0.0, WIDTH, HEIGHT, BACKGROUND,
        )))
        .expect("valid Runtime mutation");
    runtime
        .figure(root)
        .expect("attached root")
        .set_style(FigureStyle {
            foreground: Some(INK),
            font: Some("16px Inter Variable".to_string()),
            ..FigureStyle::default()
        })
        .expect("valid root style");
    add_label(
        &mut runtime,
        root,
        title,
        Rectangle::new(40.0, 24.0, 720.0, 34.0),
        "24px Inter Variable",
        INK,
    );
    add_label(
        &mut runtime,
        root,
        subtitle,
        Rectangle::new(40.0, 58.0, 720.0, 24.0),
        "14px Inter Variable",
        MUTED,
    );
    (runtime, root)
}

fn add_panel(runtime: &mut Runtime, root: FigureId, bounds: Rectangle) -> FigureId {
    runtime
        .container(root)
        .expect("root container")
        .add(Box::new(
            RoundedRectangleFigure::new_with_color(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                6.0,
                PANEL_FILL,
            )
            .with_stroke(PANEL_STROKE, 1.0),
        ))
        .expect("valid panel")
}

fn add_label(
    runtime: &mut Runtime,
    parent: FigureId,
    text: &str,
    bounds: Rectangle,
    font: &str,
    color: Color,
) -> FigureId {
    let label = runtime
        .container(parent)
        .expect("label parent")
        .add(Box::new(LabelFigure::new(text).with_bounds(bounds)))
        .expect("valid label");
    runtime
        .figure(label)
        .expect("attached label")
        .set_style(FigureStyle {
            foreground: Some(color),
            font: Some(font.to_string()),
            ..FigureStyle::default()
        })
        .expect("valid label style");
    label
}

fn add_node(
    runtime: &mut Runtime,
    root: FigureId,
    bounds: Rectangle,
    text: &str,
    fill: Color,
) -> FigureId {
    let node = runtime
        .container(root)
        .expect("root container")
        .add(Box::new(
            RoundedRectangleFigure::new_with_color(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                10.0,
                fill,
            )
            .with_stroke(INK, 2.0),
        ))
        .expect("valid node");
    add_label(
        runtime,
        node,
        text,
        Rectangle::new(0.0, 0.0, bounds.width, bounds.height),
        "15px Inter Variable",
        INK,
    );
    node
}

fn connection_decoration_scene() -> Runtime {
    let (mut runtime, root) = scene_runtime(
        "Connection decoration",
        "Open and filled endpoint figures rotate with the route; the badge uses tangent/normal offsets.",
    );
    add_panel(
        &mut runtime,
        root,
        Rectangle::new(40.0, 105.0, 720.0, 430.0),
    );

    let connection_figure = runtime
        .container(root)
        .expect("root container")
        .add(Box::new(
            ConnectionFigure::new()
                .with_stroke(BLUE, CONNECTION_WIDTH)
                .with_decoration_insets(SOURCE_DECORATION_INSET, TARGET_DECORATION_INSET),
        ))
        .expect("valid connection");
    let source = add_node(
        &mut runtime,
        root,
        Rectangle::new(105.0, 205.0, 130.0, 70.0),
        "SOURCE",
        Color::rgba(0.75, 0.91, 0.98, 1.0),
    );
    let target = add_node(
        &mut runtime,
        root,
        Rectangle::new(565.0, 350.0, 130.0, 70.0),
        "TARGET",
        Color::rgba(0.75, 0.94, 0.85, 1.0),
    );
    let source_decoration = runtime
        .container(connection_figure)
        .expect("connection container")
        .add(Box::new(
            PolylineDecorationFigure::arrow()
                .with_scale(SOURCE_DECORATION_SCALE, SOURCE_DECORATION_SCALE)
                .expect("valid decoration scale")
                .with_color(BLUE)
                .with_width(3.0)
                .with_join(LineJoin::Miter),
        ))
        .expect("valid source decoration");
    let target_decoration = runtime
        .container(connection_figure)
        .expect("connection container")
        .add(Box::new(
            PolygonDecorationFigure::triangle()
                .with_scale(TARGET_DECORATION_SCALE_X, TARGET_DECORATION_SCALE_Y)
                .expect("valid decoration scale")
                .with_fill_color(RED)
                .with_stroke(INK, 1.5)
                .with_join(LineJoin::Miter),
        ))
        .expect("valid target decoration");
    let badge = add_label(
        &mut runtime,
        connection_figure,
        "offset +u / +v",
        Rectangle::new(0.0, 0.0, 126.0, 26.0),
        "13px Inter Variable",
        RED,
    );
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: runtime.direct_connection_router(),
            },
            None,
        )
        .expect("valid connection state");
    runtime
        .set_connection_locator(
            connection,
            source_decoration,
            Box::new(ConnectionLocator::Source),
        )
        .expect("valid source locator");
    runtime
        .set_connection_locator(
            connection,
            target_decoration,
            Box::new(ConnectionLocator::Target),
        )
        .expect("valid target locator");
    runtime
        .set_connection_locator(
            connection,
            badge,
            Box::new(EndpointLocator::target(25.0, 65.0).expect("valid endpoint offset")),
        )
        .expect("valid badge locator");
    runtime
        .resolve_connection_route(connection, CoordinateSpace::ChildContent(root))
        .expect("decoration route");
    runtime
}

fn shortest_path_scene() -> Runtime {
    let (mut runtime, root) = scene_runtime(
        "Obstacle-aware shortest path",
        "Two connections share one immutable obstacle snapshot and choose deterministic orthogonal paths.",
    );
    add_panel(
        &mut runtime,
        root,
        Rectangle::new(40.0, 105.0, 720.0, 430.0),
    );
    let obstacle = runtime
        .container(root)
        .expect("root container")
        .add(Box::new(
            RoundedRectangleFigure::new_with_color(
                325.0,
                170.0,
                150.0,
                300.0,
                8.0,
                Color::rgba(0.91, 0.92, 0.94, 1.0),
            )
            .with_stroke(MUTED, 2.0),
        ))
        .expect("valid obstacle");
    add_label(
        &mut runtime,
        obstacle,
        "OBSTACLE",
        Rectangle::new(0.0, 130.0, 150.0, 40.0),
        "15px Inter Variable",
        MUTED,
    );

    let pairs = [
        (
            Rectangle::new(80.0, 155.0, 110.0, 60.0),
            Rectangle::new(610.0, 155.0, 110.0, 60.0),
            BLUE,
            "A",
        ),
        (
            Rectangle::new(80.0, 425.0, 110.0, 60.0),
            Rectangle::new(610.0, 425.0, 110.0, 60.0),
            GREEN,
            "B",
        ),
    ];
    let router = runtime.register_connection_router(Box::new(
        ShortestPathConnectionRouter::new([obstacle])
            .with_clearance(16.0)
            .expect("valid clearance")
            .with_bend_penalty(12.0)
            .expect("valid bend penalty")
            .with_minimum_stub(18.0)
            .expect("valid endpoint stub"),
    ));
    let mut connections = Vec::with_capacity(pairs.len());
    for (source_bounds, target_bounds, color, label) in pairs {
        let source = add_node(
            &mut runtime,
            root,
            source_bounds,
            &format!("{label} SOURCE"),
            Color::rgba(0.78, 0.90, 0.98, 1.0),
        );
        let target = add_node(
            &mut runtime,
            root,
            target_bounds,
            &format!("{label} TARGET"),
            Color::rgba(0.78, 0.94, 0.86, 1.0),
        );
        let figure = runtime
            .container(root)
            .expect("root container")
            .add(Box::new(
                ConnectionFigure::new().with_stroke(color, CONNECTION_WIDTH),
            ))
            .expect("valid connection");
        let source_anchor =
            runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
        let target_anchor =
            runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
        let connection = runtime
            .register_connection_state(
                figure,
                Some(source_anchor),
                Some(target_anchor),
                RouterBinding::Explicit { router },
                None,
            )
            .expect("valid connection state");
        connections.push(connection);
    }
    runtime
        .resolve_connection_route(connections[0], CoordinateSpace::ChildContent(root))
        .expect("shortest path batch");
    let stats = runtime.connection_routing_stats();
    add_label(
        &mut runtime,
        root,
        &format!(
            "{} routes / {} obstacle snapshot",
            stats.route_calculations, stats.obstacle_snapshot_builds
        ),
        Rectangle::new(270.0, 495.0, 260.0, 24.0),
        "13px Inter Variable",
        MUTED,
    );
    runtime
}

fn scalable_polygon_scene() -> Runtime {
    let (mut runtime, root) = scene_runtime(
        "Scalable polygon",
        "One canonical asymmetric template maps into different bounds, scale modes, and alignments.",
    );
    let template = PointList::from_points(vec![
        Point::new(0.0, 18.0),
        Point::new(42.0, 0.0),
        Point::new(100.0, 18.0),
        Point::new(76.0, 60.0),
        Point::new(22.0, 52.0),
    ]);
    let cases = [
        (
            Rectangle::new(45.0, 125.0, 220.0, 165.0),
            Rectangle::new(65.0, 175.0, 180.0, 90.0),
            "Stretch",
            PolygonScaleMode::Stretch,
            Alignment::Center,
            Alignment::Center,
            BLUE,
        ),
        (
            Rectangle::new(290.0, 125.0, 220.0, 165.0),
            Rectangle::new(310.0, 175.0, 180.0, 80.0),
            "Preserve / Center",
            PolygonScaleMode::PreserveAspect,
            Alignment::Center,
            Alignment::Center,
            GREEN,
        ),
        (
            Rectangle::new(535.0, 125.0, 220.0, 165.0),
            Rectangle::new(555.0, 175.0, 180.0, 80.0),
            "Runtime: End / Start",
            PolygonScaleMode::Stretch,
            Alignment::Center,
            Alignment::Center,
            AMBER,
        ),
    ];
    let mut runtime_mutated = None;
    for (index, (panel_bounds, polygon_bounds, label, mode, horizontal, vertical, color)) in
        cases.into_iter().enumerate()
    {
        add_panel(&mut runtime, root, panel_bounds);
        add_label(
            &mut runtime,
            root,
            label,
            Rectangle::new(
                panel_bounds.x,
                panel_bounds.y + 10.0,
                panel_bounds.width,
                24.0,
            ),
            "14px Inter Variable",
            INK,
        );
        let polygon = runtime
            .container(root)
            .expect("root container")
            .add(Box::new(
                ScalablePolygonFigure::new(polygon_bounds, template.clone())
                    .expect("valid scalable polygon")
                    .with_scale_mode(mode)
                    .with_alignment(horizontal, vertical)
                    .with_fill_color(color)
                    .with_stroke(INK, 5.0, LineJoin::Miter),
            ))
            .expect("valid scalable polygon");
        if index == 2 {
            runtime_mutated = Some(polygon);
        }
    }
    let runtime_mutated = runtime_mutated.expect("runtime mutation target");
    runtime
        .scalable_polygon(runtime_mutated)
        .expect("attached scalable polygon")
        .set_scale_mode(PolygonScaleMode::PreserveAspect)
        .expect("valid mode mutation");
    runtime
        .scalable_polygon(runtime_mutated)
        .expect("attached scalable polygon")
        .set_alignment(Alignment::End, Alignment::Start)
        .expect("valid alignment mutation");

    add_panel(
        &mut runtime,
        root,
        Rectangle::new(45.0, 325.0, 710.0, 190.0),
    );
    add_label(
        &mut runtime,
        root,
        "Same template after a wide bounds mutation",
        Rectangle::new(65.0, 340.0, 670.0, 24.0),
        "14px Inter Variable",
        INK,
    );
    let resized = runtime
        .container(root)
        .expect("root container")
        .add(Box::new(
            ScalablePolygonFigure::new(Rectangle::new(135.0, 380.0, 530.0, 105.0), template)
                .expect("valid scalable polygon")
                .with_scale_mode(PolygonScaleMode::PreserveAspect)
                .with_fill_color(RED)
                .with_stroke(INK, 5.0, LineJoin::Round),
        ))
        .expect("valid scalable polygon");
    runtime
        .figure(resized)
        .expect("attached scalable polygon")
        .set_bounds(Rectangle::new(95.0, 380.0, 610.0, 105.0))
        .expect("valid bounds mutation");
    runtime
}

fn text_flow_scene() -> Runtime {
    let (mut runtime, root) = scene_runtime(
        "Read-only text flow",
        "Paragraphs, inline fragments, CJK shaping, soft wrapping, and UTF-8-safe truncation.",
    );
    add_panel(
        &mut runtime,
        root,
        Rectangle::new(40.0, 105.0, 455.0, 430.0),
    );
    add_label(
        &mut runtime,
        root,
        "Soft wrap / two paragraphs",
        Rectangle::new(62.0, 122.0, 410.0, 28.0),
        "15px Inter Variable",
        BLUE,
    );
    let page = FlowPage::new(vec![
        FlowParagraph::new(vec![
            InlineTextFragment::new("Novadraw shapes inline fragments "),
            InlineTextFragment::new("as one continuous paragraph. "),
            InlineTextFragment::new("Narrow bounds create stable soft wraps."),
        ]),
        FlowParagraph::new(vec![
            InlineTextFragment::new("中文、English 与数字 12345 "),
            InlineTextFragment::new("共享同一套 shaping 和换行流程。"),
        ]),
    ]);
    runtime
        .container(root)
        .expect("root container")
        .add(Box::new(TextFlowFigure::new(
            Rectangle::new(65.0, 165.0, 405.0, 330.0),
            page,
        )))
        .expect("valid text flow");

    add_panel(
        &mut runtime,
        root,
        Rectangle::new(520.0, 105.0, 240.0, 205.0),
    );
    add_label(
        &mut runtime,
        root,
        "Truncate / 2 lines",
        Rectangle::new(538.0, 122.0, 204.0, 28.0),
        "15px Inter Variable",
        RED,
    );
    runtime
        .container(root)
        .expect("root container")
        .add(Box::new(
            TextFlowFigure::new(
                Rectangle::new(540.0, 165.0, 200.0, 100.0),
                FlowPage::from_text(
                    "第一段 mixed-script content is intentionally longer than the available two lines.",
                ),
            )
            .with_wrapping(FlowWrapping::Truncate { max_lines: 2 }),
        ))
        .expect("valid truncated text flow");

    add_panel(
        &mut runtime,
        root,
        Rectangle::new(520.0, 330.0, 240.0, 205.0),
    );
    add_label(
        &mut runtime,
        root,
        "No wrap",
        Rectangle::new(538.0, 347.0, 204.0, 28.0),
        "15px Inter Variable",
        GREEN,
    );
    runtime
        .container(root)
        .expect("root container")
        .add(Box::new(
            TextFlowFigure::new(
                Rectangle::new(540.0, 390.0, 200.0, 90.0),
                FlowPage::from_text("Single unwrapped line"),
            )
            .with_wrapping(FlowWrapping::NoWrap),
        ))
        .expect("valid no-wrap text flow");
    runtime
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advanced_figures_suite_builds_and_records_all_feature_scenes() {
        let mut suite = suite();
        assert_eq!(suite.scenes.len(), 4);

        for scene in &mut suite.scenes {
            let mut runtime = scene.build();
            assert!(
                runtime.tree().contents().is_some(),
                "missing root: {}",
                scene.id
            );
            assert!(
                !runtime.record_full_frame().commands().is_empty(),
                "empty frame: {}",
                scene.id
            );
        }
    }
}
