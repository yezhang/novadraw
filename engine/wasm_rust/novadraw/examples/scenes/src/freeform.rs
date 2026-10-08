use novadraw::container::{
    FreeformLayerFigure, LayerKey, LayerPlacement, ScaleHandle, ScrollBarVisibility,
    ScrollPaneHandle, ZoomManager,
};
use novadraw::{Color, FigureId, FigureTree, Rectangle, RectangleFigure, Runtime};

use crate::{DemoSuite, SceneSpec, ValidationKind};

pub const WINDOW_WIDTH: f64 = 800.0;
pub const WINDOW_HEIGHT: f64 = 600.0;
pub const PANE_X: f64 = 120.0;
pub const PANE_Y: f64 = 90.0;
pub const PANE_WIDTH: f64 = 480.0;
pub const PANE_HEIGHT: f64 = 340.0;
pub const CONTENT_MIN_X: f64 = -220.0;
pub const CONTENT_MIN_Y: f64 = -160.0;
pub const CONTENT_MAX_X: f64 = 780.0;
pub const CONTENT_MAX_Y: f64 = 580.0;
pub const OVERLAP_X: f64 = 80.0;
pub const OVERLAP_Y: f64 = 70.0;
pub const DEMO_SCALE: f64 = 1.5;

const LAYER_WIDTH: f64 = 480.0;
const LAYER_HEIGHT: f64 = 340.0;
const GRID_STEP: f64 = 100.0;
const GRID_LINE_WIDTH: f64 = 2.0;

pub struct FreeformDemo {
    pub runtime: Runtime,
    pub pane: ScrollPaneHandle,
    pub scalable: ScaleHandle,
    pub content_layer: FigureId,
    pub overlay_layer: FigureId,
    pub lower_overlap: FigureId,
    pub upper_overlap: FigureId,
}

fn key(value: &str) -> LayerKey {
    LayerKey::new(value).expect("demo layer key must be valid")
}

pub fn build_demo(scale: f64, view_location: (f64, f64)) -> FreeformDemo {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            Color::from_hex("#eeeeee").expect("valid color literal"),
        )))
        .expect("valid FigureTree construction");
    let pane = graph
        .builder()
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .expect("attach freeform scroll pane");
    let scalable = graph
        .builder()
        .add_scalable_freeform_layered_pane_to(
            pane.viewport().figure_id(),
            Rectangle::new(0.0, 0.0, LAYER_WIDTH, LAYER_HEIGHT),
        )
        .expect("attach scalable freeform pane");
    graph
        .builder()
        .set_scroll_bar_visibility(
            &pane,
            ScrollBarVisibility::Always,
            ScrollBarVisibility::Always,
        )
        .expect("set freeform scrollbar visibility");

    let mut runtime = Runtime::new(graph);
    let (content_layer, overlay_layer) = {
        let mut layers = runtime
            .layered_pane(scalable.figure_id())
            .expect("scalable freeform pane must be registered");
        let content = layers
            .add_layer(
                Box::new(FreeformLayerFigure::new(
                    0.0,
                    0.0,
                    LAYER_WIDTH,
                    LAYER_HEIGHT,
                )),
                key("content"),
                LayerPlacement::Last,
            )
            .expect("add content layer");
        let overlay = layers
            .add_layer(
                Box::new(FreeformLayerFigure::new(
                    0.0,
                    0.0,
                    LAYER_WIDTH,
                    LAYER_HEIGHT,
                )),
                key("overlay"),
                LayerPlacement::Last,
            )
            .expect("add overlay layer");
        (content, overlay)
    };

    let mut x = CONTENT_MIN_X;
    while x < CONTENT_MAX_X {
        runtime
            .container(content_layer)
            .unwrap()
            .add(Box::new(RectangleFigure::new_with_color(
                x,
                CONTENT_MIN_Y,
                GRID_LINE_WIDTH,
                CONTENT_MAX_Y - CONTENT_MIN_Y,
                Color::from_hex("#dfe6ee").expect("valid color literal"),
            )))
            .expect("valid Runtime mutation");
        x += GRID_STEP;
    }
    runtime
        .container(content_layer)
        .unwrap()
        .add(Box::new(RectangleFigure::new_with_color(
            CONTENT_MAX_X - GRID_LINE_WIDTH,
            CONTENT_MIN_Y,
            GRID_LINE_WIDTH,
            CONTENT_MAX_Y - CONTENT_MIN_Y,
            Color::from_hex("#dfe6ee").expect("valid color literal"),
        )))
        .expect("valid Runtime mutation");

    let mut y = CONTENT_MIN_Y;
    while y < CONTENT_MAX_Y {
        runtime
            .container(content_layer)
            .unwrap()
            .add(Box::new(RectangleFigure::new_with_color(
                CONTENT_MIN_X,
                y,
                CONTENT_MAX_X - CONTENT_MIN_X,
                GRID_LINE_WIDTH,
                Color::from_hex("#dfe6ee").expect("valid color literal"),
            )))
            .expect("valid Runtime mutation");
        y += GRID_STEP;
    }
    runtime
        .container(content_layer)
        .unwrap()
        .add(Box::new(RectangleFigure::new_with_color(
            CONTENT_MIN_X,
            CONTENT_MAX_Y - GRID_LINE_WIDTH,
            CONTENT_MAX_X - CONTENT_MIN_X,
            GRID_LINE_WIDTH,
            Color::from_hex("#dfe6ee").expect("valid color literal"),
        )))
        .expect("valid Runtime mutation");

    for (bounds, fill) in [
        (
            Rectangle::new(CONTENT_MIN_X, CONTENT_MIN_Y, 100.0, 80.0),
            Color::from_hex("#2f80ed").expect("valid color literal"),
        ),
        (
            Rectangle::new(CONTENT_MAX_X - 100.0, CONTENT_MIN_Y, 100.0, 80.0),
            Color::from_hex("#27ae60").expect("valid color literal"),
        ),
        (
            Rectangle::new(CONTENT_MIN_X, CONTENT_MAX_Y - 80.0, 100.0, 80.0),
            Color::from_hex("#f2994a").expect("valid color literal"),
        ),
        (
            Rectangle::new(CONTENT_MAX_X - 100.0, CONTENT_MAX_Y - 80.0, 100.0, 80.0),
            Color::from_hex("#9b51e0").expect("valid color literal"),
        ),
    ] {
        runtime
            .container(content_layer)
            .unwrap()
            .add(Box::new(RectangleFigure::new_with_color(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                fill,
            )))
            .expect("valid Runtime mutation");
    }
    let lower_overlap = runtime
        .container(content_layer)
        .unwrap()
        .add(Box::new(RectangleFigure::new_with_color(
            OVERLAP_X,
            OVERLAP_Y,
            180.0,
            130.0,
            Color::from_hex("#eb5757").expect("valid color literal"),
        )))
        .expect("valid Runtime mutation");
    let upper_overlap = runtime
        .container(overlay_layer)
        .unwrap()
        .add(Box::new(RectangleFigure::new_with_color(
            OVERLAP_X + 45.0,
            OVERLAP_Y + 35.0,
            180.0,
            130.0,
            Color::from_hex("#56ccf2").expect("valid color literal"),
        )))
        .expect("valid Runtime mutation");

    runtime
        .prepare_frame()
        .expect("stabilize freeform demo before viewport configuration");
    let zoom = ZoomManager::new(scalable.clone(), pane.viewport().clone());
    if scale != 1.0 {
        runtime
            .zoom(&zoom)
            .expect("zoom targets must remain attached")
            .set_zoom_at(scale, None)
            .expect("set freeform demo scale");
    }
    runtime
        .viewport(pane.viewport().figure_id())
        .expect("scroll pane viewport must remain attached")
        .set_view_location(view_location.0, view_location.1)
        .expect("set freeform demo view location");
    let _ = runtime.prepare_frame();

    FreeformDemo {
        runtime,
        pane,
        scalable,
        content_layer,
        overlay_layer,
        lower_overlap,
        upper_overlap,
    }
}

fn layer_order_scene() -> Runtime {
    build_demo(1.0, (0.0, 0.0)).runtime
}

fn negative_origin_scene() -> Runtime {
    build_demo(1.0, (CONTENT_MIN_X, CONTENT_MIN_Y)).runtime
}

fn positive_extent_scene() -> Runtime {
    build_demo(1.0, (CONTENT_MAX_X, CONTENT_MAX_Y)).runtime
}

fn zoomed_scene() -> Runtime {
    build_demo(DEMO_SCALE, (-40.0, -30.0)).runtime
}

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "layer-freeform",
        "Layer / Freeform",
        vec![
            SceneSpec::runtime(
                "layer-order",
                "layer_order",
                size,
                ValidationKind::Interactive,
                layer_order_scene,
            ),
            SceneSpec::runtime(
                "negative-origin",
                "negative_origin",
                size,
                ValidationKind::Interactive,
                negative_origin_scene,
            ),
            SceneSpec::runtime(
                "positive-extent",
                "positive_extent",
                size,
                ValidationKind::Interactive,
                positive_extent_scene,
            ),
            SceneSpec::runtime(
                "zoomed-freeform",
                "zoomed_freeform",
                size,
                ValidationKind::Interactive,
                zoomed_scene,
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use novadraw::Point;
    use novadraw::container::MouseLocationZoomScrollPolicy;
    use novadraw::event::{GesturePhase, GestureSessionId, KeyModifiers, ZoomEvent};

    use super::*;

    #[test]
    fn shared_scene_preserves_extent_range_and_layer_hit_order() {
        let demo = build_demo(1.0, (0.0, 0.0));
        assert_eq!(
            demo.runtime
                .tree()
                .freeform_extent(demo.scalable.figure_id()),
            Ok(Rectangle::new(
                CONTENT_MIN_X,
                CONTENT_MIN_Y,
                CONTENT_MAX_X - CONTENT_MIN_X,
                CONTENT_MAX_Y - CONTENT_MIN_Y,
            ))
        );
        assert_eq!(
            demo.pane.viewport().horizontal_range().minimum,
            CONTENT_MIN_X
        );
        assert_eq!(
            demo.pane.viewport().horizontal_range().maximum,
            CONTENT_MAX_X
        );
        assert_eq!(demo.pane.viewport().vertical_range().minimum, CONTENT_MIN_Y);
        assert_eq!(demo.pane.viewport().vertical_range().maximum, CONTENT_MAX_Y);

        let overlap = Point::new(PANE_X + 150.0, PANE_Y + 120.0);
        assert_eq!(
            demo.runtime
                .tree()
                .hit_test_simple((overlap.x(), overlap.y())),
            Some(demo.upper_overlap)
        );
        let empty = Point::new(PANE_X + 20.0, PANE_Y + 20.0);
        let target = demo.runtime.tree().hit_test_simple((empty.x(), empty.y()));
        assert_ne!(target, Some(demo.content_layer));
        assert_ne!(target, Some(demo.overlay_layer));
    }

    #[test]
    fn shared_scene_reaches_four_edges_and_preserves_zoom_anchor() {
        let mut demo = build_demo(1.0, (0.0, 0.0));
        let viewport = demo.pane.viewport().clone();

        demo.runtime
            .viewport(viewport.figure_id())
            .unwrap()
            .set_view_location(CONTENT_MIN_X, CONTENT_MIN_Y)
            .unwrap();
        assert_eq!(
            viewport.view_location(),
            Point::new(CONTENT_MIN_X, CONTENT_MIN_Y)
        );
        demo.runtime
            .viewport(viewport.figure_id())
            .unwrap()
            .set_view_location(f64::MAX, f64::MAX)
            .unwrap();
        let horizontal = viewport.horizontal_range();
        let vertical = viewport.vertical_range();
        assert_eq!(
            viewport.view_location(),
            Point::new(
                horizontal.maximum - horizontal.extent,
                vertical.maximum - vertical.extent,
            )
        );

        demo.runtime
            .viewport(viewport.figure_id())
            .unwrap()
            .set_view_location(0.0, 0.0)
            .unwrap();
        let mut zoom = ZoomManager::new(demo.scalable, viewport.clone());
        zoom.set_scroll_policy(Arc::new(MouseLocationZoomScrollPolicy));
        demo.runtime
            .zoom(&zoom)
            .unwrap()
            .set_zoom_at(2.0, Some(Point::new(60.0, 40.0)))
            .unwrap();
        assert_eq!(viewport.view_location(), Point::new(30.0, 20.0));
        assert_eq!(viewport.horizontal_range().maximum, CONTENT_MAX_X);
        assert_eq!(viewport.vertical_range().maximum, CONTENT_MAX_Y);
    }

    #[test]
    fn blank_viewport_area_routes_zoom_to_direct_scalable_contents() {
        let mut demo = build_demo(1.0, (0.0, 0.0));
        let scalable = demo.scalable.clone();

        demo.runtime.dispatch_zoom(ZoomEvent::new(
            PANE_X + 20.0,
            PANE_Y + 20.0,
            2.0,
            GesturePhase::Impulse,
            KeyModifiers::default(),
            GestureSessionId::IMPULSE,
        ));

        assert_eq!(scalable.scale(), 2.0);
    }
}
