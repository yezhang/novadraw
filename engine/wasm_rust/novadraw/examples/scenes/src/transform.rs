//! M4 坐标域与变换闭环验证。

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use novadraw::event::{EventContext, FigureEventHandler, MouseEvent};
use novadraw::figure::{Bounded, Shape, border::LineBorder};
use novadraw::geometry::Translatable;
use novadraw::graphics::{LineCap, LineJoin};
use novadraw::{Color, Figure, NdCanvas, Point, Rectangle, RectangleFigure};

use crate::{DemoSuite, SceneSpec};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;
const BACKGROUND: Color = Color::rgba(0.933, 0.933, 0.933, 1.0);
const OUTER_COLOR: Color = Color::rgba(0.36, 0.61, 0.84, 1.0);
const INNER_COLOR: Color = Color::rgba(0.95, 0.67, 0.24, 1.0);
const CHILD_COLOR: Color = Color::rgba(0.33, 0.73, 0.53, 1.0);
const TARGET_COLOR: Color = Color::rgba(0.86, 0.32, 0.32, 1.0);
const OLD_BOUNDS_COLOR: Color = Color::rgba(0.55, 0.55, 0.55, 1.0);
const SELECTED_BORDER_COLOR: Color = Color::rgba(1.0, 0.84, 0.0, 1.0);
const BORDER_WIDTH: f64 = 3.0;

fn background() -> RectangleFigure {
    RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, BACKGROUND)
}

fn coordinate_root(x: f64, y: f64, width: f64, height: f64, color: Color) -> RectangleFigure {
    RectangleFigure::new_with_color(x, y, width, height, color)
        .with_border(LineBorder::new(Color::BLACK, BORDER_WIDTH).with_insets(8.0, 12.0, 8.0, 12.0))
}

fn add_nested_roots(
    scene: &mut novadraw::FigureTree,
    contents: novadraw::FigureId,
) -> (novadraw::FigureId, novadraw::FigureId) {
    let outer = scene
        .builder()
        .add_child(
            contents,
            Box::new(coordinate_root(120.0, 90.0, 520.0, 400.0, OUTER_COLOR)),
        )
        .expect("valid FigureTree construction");
    let inner = scene
        .builder()
        .add_child(
            outer,
            Box::new(coordinate_root(70.0, 60.0, 330.0, 240.0, INNER_COLOR)),
        )
        .expect("valid FigureTree construction");
    (outer, inner)
}

fn create_nested_coordinate_roots() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let contents = scene.builder().set_contents(Box::new(background()));
    let (_, inner) = add_nested_roots(&mut scene, contents);
    scene
        .builder()
        .add_child(
            inner,
            Box::new(RectangleFigure::new_with_color(
                45.0,
                40.0,
                150.0,
                100.0,
                CHILD_COLOR,
            )),
        )
        .expect("valid FigureTree construction");
    scene
}

fn create_coordinate_roundtrip_overlay() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let contents = scene.builder().set_contents(Box::new(background()));
    let (_, inner) = add_nested_roots(&mut scene, contents);
    let local_bounds = Rectangle::new(45.0, 40.0, 150.0, 100.0);
    let child = scene
        .builder()
        .add_child(
            inner,
            Box::new(
                RectangleFigure::from_bounds(local_bounds).with_stroke(Color::WHITE, BORDER_WIDTH),
            ),
        )
        .expect("valid FigureTree construction");

    let mut absolute_bounds = Rectangle::new(0.0, 0.0, local_bounds.width, local_bounds.height);
    absolute_bounds.transform(scene.local_to_surface_transform(child).unwrap());
    let mut roundtrip = absolute_bounds;
    roundtrip.transform(scene.surface_to_local_transform(child).unwrap());
    assert_eq!(
        roundtrip,
        Rectangle::new(0.0, 0.0, local_bounds.width, local_bounds.height)
    );

    scene
        .builder()
        .add_child(
            contents,
            Box::new(
                RectangleFigure::new_with_color(
                    absolute_bounds.x,
                    absolute_bounds.y,
                    absolute_bounds.width,
                    absolute_bounds.height,
                    Color::rgba(0.0, 0.0, 0.0, 0.0),
                )
                .with_stroke(TARGET_COLOR, BORDER_WIDTH),
            ),
        )
        .expect("valid FigureTree construction");
    scene
}

fn create_coordinate_root_move() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let contents = scene.builder().set_contents(Box::new(background()));
    scene
        .builder()
        .add_child(
            contents,
            Box::new(
                RectangleFigure::new_with_color(
                    120.0,
                    100.0,
                    300.0,
                    230.0,
                    Color::rgba(0.0, 0.0, 0.0, 0.0),
                )
                .with_stroke(OLD_BOUNDS_COLOR, BORDER_WIDTH),
            ),
        )
        .expect("valid FigureTree construction");
    let coordinate_root = scene
        .builder()
        .add_child(
            contents,
            Box::new(coordinate_root(120.0, 100.0, 300.0, 230.0, OUTER_COLOR)),
        )
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(
            coordinate_root,
            Box::new(RectangleFigure::new_with_color(
                35.0,
                35.0,
                120.0,
                80.0,
                CHILD_COLOR,
            )),
        )
        .expect("valid FigureTree construction");

    scene
        .builder()
        .set_bounds(coordinate_root, Rectangle::new(330.0, 220.0, 340.0, 250.0))
        .expect("valid FigureTree construction");
    scene
}

#[derive(Clone)]
struct TargetDomainFigure {
    bounds: Rectangle,
    selected: Arc<AtomicBool>,
}

impl Bounded for TargetDomainFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "TargetDomainFigure"
    }
}

impl Figure for TargetDomainFigure {
    fn initial_bounds(&self) -> Rectangle {
        Bounded::bounds(self)
    }

    fn name(&self) -> &'static str {
        Bounded::name(self)
    }

    fn paint_figure(&self, gc: &mut NdCanvas) {
        Shape::paint_figure(self, gc);
    }

    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl Shape for TargetDomainFigure {
    fn stroke_color(&self) -> Option<Color> {
        Some(Color::WHITE)
    }

    fn stroke_width(&self) -> f64 {
        BORDER_WIDTH
    }

    fn fill_color(&self) -> Option<Color> {
        Some(TARGET_COLOR)
    }

    fn line_cap(&self) -> LineCap {
        LineCap::default()
    }

    fn line_join(&self) -> LineJoin {
        LineJoin::default()
    }

    fn fill_shape(&self, gc: &mut NdCanvas) {
        let bounds = self.bounds;
        gc.fill_rect_with_color(0.0, 0.0, bounds.width, bounds.height, TARGET_COLOR);
    }

    fn outline_shape(&self, gc: &mut NdCanvas) {
        let bounds = self.bounds;
        let color = if self.selected.load(Ordering::Acquire) {
            SELECTED_BORDER_COLOR
        } else {
            Color::WHITE
        };
        gc.stroke_rect_with_style(
            0.0,
            0.0,
            bounds.width,
            bounds.height,
            color,
            BORDER_WIDTH,
            LineCap::default(),
            LineJoin::default(),
        );
    }
}

impl FigureEventHandler for TargetDomainFigure {
    fn on_mouse_pressed(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        let point = Point::new(event.x, event.y);
        let local_bounds = Rectangle::new(0.0, 0.0, self.bounds.width, self.bounds.height);
        if local_bounds.contains(point) {
            self.selected.store(true, Ordering::Release);
            ctx.repaint(None);
            return true;
        }
        false
    }
}

fn create_event_point_reduction() -> novadraw::FigureTree {
    create_event_point_reduction_with_state().0
}

fn create_event_point_reduction_with_state()
-> (novadraw::FigureTree, Arc<AtomicBool>, novadraw::FigureId) {
    let mut scene = novadraw::FigureTree::new();
    let contents = scene.builder().set_contents(Box::new(background()));
    let (_, inner) = add_nested_roots(&mut scene, contents);
    let selected = Arc::new(AtomicBool::new(false));
    let target = scene
        .builder()
        .add_child(
            inner,
            Box::new(TargetDomainFigure {
                bounds: Rectangle::new(55.0, 50.0, 180.0, 110.0),
                selected: Arc::clone(&selected),
            }),
        )
        .expect("valid FigureTree construction");
    (scene, selected, target)
}

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "transform",
        "Transforms",
        vec![
            SceneSpec::visual(
                "nested-coordinate-roots",
                "nested_coordinate_roots",
                size,
                create_nested_coordinate_roots,
            ),
            SceneSpec::visual(
                "coordinate-roundtrip-overlay",
                "coordinate_roundtrip_overlay",
                size,
                create_coordinate_roundtrip_overlay,
            ),
            SceneSpec::visual(
                "coordinate-root-move",
                "coordinate_root_move",
                size,
                create_coordinate_root_move,
            ),
            SceneSpec::visual(
                "event-point-reduction",
                "event_point_reduction",
                size,
                create_event_point_reduction,
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use novadraw::Runtime;
    use novadraw::event::MouseButton;

    use super::*;

    #[test]
    fn target_domain_click_selects_the_figure_after_parent_chain_reduction() {
        let (tree, selected, target) = create_event_point_reduction_with_state();
        let entry = tree
            .local_to_surface_transform(target)
            .unwrap()
            .transform_point(Point::new(20.0, 20.0));
        let mut runtime = Runtime::new(tree);

        runtime.dispatch_mouse_pressed(entry.x(), entry.y(), MouseButton::Left);

        assert!(selected.load(Ordering::Acquire));
    }
}
