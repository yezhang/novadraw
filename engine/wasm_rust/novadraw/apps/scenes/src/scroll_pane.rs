use novadraw::{Color, FigureTree, Rectangle, RectangleFigure, ScrollBarVisibility, ZoomManager};

use crate::{DemoSuite, SceneSpec, ValidationKind};

pub const WINDOW_WIDTH: f64 = 800.0;
pub const WINDOW_HEIGHT: f64 = 600.0;
pub const PANE_X: f64 = 120.0;
pub const PANE_Y: f64 = 90.0;
pub const PANE_WIDTH: f64 = 480.0;
pub const PANE_HEIGHT: f64 = 340.0;
pub const LARGE_CONTENT_WIDTH: f64 = 860.0;
pub const LARGE_CONTENT_HEIGHT: f64 = 640.0;
const SMALL_CONTENT_WIDTH: f64 = 240.0;
const SMALL_CONTENT_HEIGHT: f64 = 160.0;
const GRID_COLUMNS: usize = 8;
const GRID_ROWS: usize = 6;
const TILE_WIDTH: f64 = 82.0;
const TILE_HEIGHT: f64 = 68.0;
const TILE_GAP: f64 = 12.0;
const INITIAL_SCROLL_X: f64 = 90.0;
const INITIAL_SCROLL_Y: f64 = 70.0;
pub const DEMO_SCALE: f64 = 1.5;

fn color(hex: &str) -> Color {
    Color::hex(hex)
}

pub fn base_scene() -> (FigureTree, novadraw::FigureId) {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            color("#eeeeee"),
        )));
    (graph, root)
}

fn add_grid(graph: &mut FigureTree, parent: novadraw::FigureId) {
    for row in 0..GRID_ROWS {
        for column in 0..GRID_COLUMNS {
            let fill = match (row + column) % 4 {
                0 => color("#2f80ed"),
                1 => color("#27ae60"),
                2 => color("#f2994a"),
                _ => color("#9b51e0"),
            };
            graph
                .builder()
                .add_child(
                    parent,
                    Box::new(RectangleFigure::new_with_color(
                        TILE_GAP + column as f64 * (TILE_WIDTH + TILE_GAP),
                        TILE_GAP + row as f64 * (TILE_HEIGHT + TILE_GAP),
                        TILE_WIDTH,
                        TILE_HEIGHT,
                        fill,
                    )),
                )
                .expect("valid FigureTree construction");
        }
    }
}

fn scene_with_policy(
    content_width: f64,
    content_height: f64,
    horizontal: ScrollBarVisibility,
    vertical: ScrollBarVisibility,
    initial_scroll: Option<(f64, f64)>,
) -> FigureTree {
    let (mut graph, root) = base_scene();
    let pane = graph
        .builder()
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .expect("attach scroll pane");
    graph
        .builder()
        .set_scroll_bar_visibility(&pane, horizontal, vertical)
        .expect("set scrollbar visibility");
    let contents = graph
        .builder()
        .set_scroll_pane_contents(
            &pane,
            Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                content_width,
                content_height,
                Color::WHITE,
            )),
        )
        .expect("set scroll pane contents");
    add_grid(&mut graph, contents);
    graph
        .builder()
        .validate_subtree(pane.pane_id())
        .expect("valid FigureTree construction");
    if let Some((x, y)) = initial_scroll {
        graph
            .builder()
            .scroll_pane_to(&pane, x, y)
            .expect("set initial scroll");
    }
    graph
}

fn automatic_scene() -> FigureTree {
    scene_with_policy(
        LARGE_CONTENT_WIDTH,
        LARGE_CONTENT_HEIGHT,
        ScrollBarVisibility::Automatic,
        ScrollBarVisibility::Automatic,
        None,
    )
}

fn scrolled_scene() -> FigureTree {
    scene_with_policy(
        LARGE_CONTENT_WIDTH,
        LARGE_CONTENT_HEIGHT,
        ScrollBarVisibility::Automatic,
        ScrollBarVisibility::Automatic,
        Some((INITIAL_SCROLL_X, INITIAL_SCROLL_Y)),
    )
}

fn hidden_bars_scene() -> FigureTree {
    scene_with_policy(
        SMALL_CONTENT_WIDTH,
        SMALL_CONTENT_HEIGHT,
        ScrollBarVisibility::Automatic,
        ScrollBarVisibility::Automatic,
        None,
    )
}

fn scalable_scene() -> FigureTree {
    let (mut graph, root) = base_scene();
    let pane = graph
        .builder()
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .expect("attach scroll pane");
    let scalable = graph
        .builder()
        .add_scalable_layered_pane_to(
            pane.viewport().figure_id(),
            Rectangle::new(0.0, 0.0, LARGE_CONTENT_WIDTH, LARGE_CONTENT_HEIGHT),
        )
        .expect("attach scalable pane");
    add_grid(&mut graph, scalable.figure_id());
    let zoom = ZoomManager::new(scalable, pane.viewport().clone());
    graph
        .builder()
        .set_zoom(&zoom, DEMO_SCALE)
        .expect("set demo zoom");
    graph
        .builder()
        .set_view_location(pane.viewport().figure_id(), 0.0, 0.0)
        .expect("reset demo view location");
    graph
        .builder()
        .validate_subtree(pane.pane_id())
        .expect("valid FigureTree construction");
    graph
}

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "scroll-pane",
        "Scroll pane",
        vec![
            SceneSpec::new(
                "automatic-scrollbars",
                "automatic_scrollbars",
                size,
                ValidationKind::Interactive,
                automatic_scene,
            ),
            SceneSpec::new(
                "scrolled-content",
                "scrolled_content",
                size,
                ValidationKind::Interactive,
                scrolled_scene,
            ),
            SceneSpec::new(
                "automatic-hidden",
                "automatic_hidden",
                size,
                ValidationKind::Interactive,
                hidden_bars_scene,
            ),
            SceneSpec::new(
                "scalable-content",
                "scalable_content",
                size,
                ValidationKind::Interactive,
                scalable_scene,
            ),
        ],
    )
}
