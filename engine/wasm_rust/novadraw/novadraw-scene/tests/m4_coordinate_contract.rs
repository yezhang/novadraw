use std::sync::{Arc, Mutex};

use novadraw_core::Color;
use novadraw_geometry::{Point, Rectangle};
use novadraw_render::{NdCanvas, command::LineCap, command::LineJoin};
use novadraw_scene::{
    BasicEventDispatcher, Bounded, EventDispatcher, FigureEvent, FigureGraph, LineBorder,
    MouseButton, MouseEvent, NotificationEffect, NovadrawContext, PendingMutations,
    RectangleFigure, SceneDispatchContext, SceneUpdateManager, Shape, Updatable,
};

fn coordinate_root(x: f64, y: f64, width: f64, height: f64) -> RectangleFigure {
    RectangleFigure::new(x, y, width, height)
        .with_local_coordinates(true)
        .with_border(LineBorder::new(Color::BLACK, 1.0).with_insets(3.0, 5.0, 0.0, 0.0))
}

fn nested_coordinate_scene() -> (FigureGraph, novadraw_scene::BlockId) {
    let mut graph = FigureGraph::new();
    let contents = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 400.0)));
    let outer = graph.add_child_to(
        contents,
        Box::new(coordinate_root(100.0, 50.0, 300.0, 250.0)),
    );
    let inner = graph.add_child_to(
        outer,
        Box::new(
            RectangleFigure::new(20.0, 30.0, 180.0, 140.0)
                .with_local_coordinates(true)
                .with_border(LineBorder::new(Color::BLACK, 1.0).with_insets(7.0, 11.0, 0.0, 0.0)),
        ),
    );
    let child = graph.add_child_to(
        inner,
        Box::new(RectangleFigure::new(10.0, 15.0, 60.0, 50.0)),
    );
    (graph, child)
}

#[test]
fn m4_point_roundtrips_across_nested_coordinate_roots_with_insets() {
    let (graph, child) = nested_coordinate_scene();
    let original = Point::new(25.0, 35.0);
    let mut point = original;

    graph.translate_to_absolute_mut(child, &mut point);
    assert_eq!(point, Point::new(161.0, 125.0));

    graph.translate_to_relative(child, &mut point);
    assert_eq!(point, original);
}

#[test]
fn m4_rectangle_roundtrip_preserves_extent_across_nested_coordinate_roots() {
    let (graph, child) = nested_coordinate_scene();
    let original = Rectangle::new(25.0, 35.0, 18.0, 12.0);
    let mut rect = original;

    graph.translate_to_absolute_mut(child, &mut rect);
    assert_eq!(rect, Rectangle::new(161.0, 125.0, 18.0, 12.0));

    graph.translate_to_relative(child, &mut rect);
    assert_eq!(rect, original);
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RecordedMousePoint {
    target: Point,
    entry: Point,
}

#[derive(Clone, Debug)]
struct RecordingFigure {
    bounds: Rectangle,
    recorded: Arc<Mutex<Option<RecordedMousePoint>>>,
}

impl RecordingFigure {
    fn new(bounds: Rectangle, recorded: Arc<Mutex<Option<RecordedMousePoint>>>) -> Self {
        Self { bounds, recorded }
    }
}

impl Bounded for RecordingFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "RecordingFigure"
    }
}

impl Updatable for RecordingFigure {
    fn validate(&mut self) {}
}

impl Shape for RecordingFigure {
    fn fill_enabled(&self) -> bool {
        false
    }

    fn outline_enabled(&self) -> bool {
        false
    }

    fn stroke_width(&self) -> f64 {
        0.0
    }

    fn stroke_color(&self) -> Option<Color> {
        None
    }

    fn fill_color(&self) -> Option<Color> {
        None
    }

    fn line_cap(&self) -> LineCap {
        LineCap::default()
    }

    fn line_join(&self) -> LineJoin {
        LineJoin::default()
    }

    fn fill_shape(&self, _gc: &mut NdCanvas) {}

    fn outline_shape(&self, _gc: &mut NdCanvas) {}

    fn wants_mouse_events(&self) -> bool {
        true
    }

    fn on_mouse_pressed(&self, event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        *self.recorded.lock().unwrap() = Some(RecordedMousePoint {
            target: Point::new(event.x, event.y),
            entry: event.entry_point(),
        });
        true
    }
}

#[test]
fn m4_hit_test_and_mouse_callback_share_the_same_target_coordinate_domain() {
    let recorded = Arc::new(Mutex::new(None));
    let mut graph = FigureGraph::new();
    let contents = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 400.0)));
    let outer = graph.add_child_to(
        contents,
        Box::new(coordinate_root(100.0, 50.0, 300.0, 250.0)),
    );
    let inner = graph.add_child_to(
        outer,
        Box::new(
            RectangleFigure::new(20.0, 30.0, 180.0, 140.0)
                .with_local_coordinates(true)
                .with_border(LineBorder::new(Color::BLACK, 1.0).with_insets(7.0, 11.0, 0.0, 0.0)),
        ),
    );
    let target = graph.add_child_to(
        inner,
        Box::new(RecordingFigure::new(
            Rectangle::new(10.0, 15.0, 60.0, 50.0),
            Arc::clone(&recorded),
        )),
    );
    let entry = Point::new(161.0, 125.0);

    assert_eq!(
        graph.find_mouse_event_target_at(entry.x(), entry.y()),
        Some(target)
    );

    let mut update_manager = SceneUpdateManager::new();
    let mut pending_mutations = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;
    let mut context =
        SceneDispatchContext::new(&mut graph, &mut update_manager, &mut pending_mutations);
    dispatcher.dispatch_mouse_pressed(&mut context, entry.x(), entry.y(), MouseButton::Left);

    assert_eq!(
        *recorded.lock().unwrap(),
        Some(RecordedMousePoint {
            target: Point::new(25.0, 35.0),
            entry,
        })
    );
}

#[test]
fn m4_coordinate_root_move_and_resize_is_one_atomic_bounds_change() {
    let mut graph = FigureGraph::new();
    let contents = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 240.0)));
    let coordinate_root = graph.add_child_to(
        contents,
        Box::new(RectangleFigure::new(50.0, 40.0, 80.0, 60.0).with_local_coordinates(true)),
    );
    let child = graph.add_child_to(
        coordinate_root,
        Box::new(RectangleFigure::new(10.0, 15.0, 20.0, 10.0)),
    );
    graph.drain_notification_effects();
    let mut update_manager = SceneUpdateManager::new();

    assert!(graph.set_bounds_with_update(
        &mut update_manager,
        coordinate_root,
        70.0,
        55.0,
        100.0,
        70.0,
    ));

    assert_eq!(
        graph.figure_bounds(child),
        Some(Rectangle::new(10.0, 15.0, 20.0, 10.0))
    );
    let figure_events: Vec<_> = graph
        .notification_effects()
        .iter()
        .filter_map(|effect| match effect {
            NotificationEffect::EmitFigure(event) => Some(*event),
            _ => None,
        })
        .collect();
    assert_eq!(
        figure_events,
        vec![
            FigureEvent::FigureMoved {
                block_id: coordinate_root,
                old_bounds: Rectangle::new(50.0, 40.0, 80.0, 60.0),
                new_bounds: Rectangle::new(70.0, 55.0, 100.0, 70.0),
            },
            FigureEvent::CoordinateSystemChanged {
                block_id: coordinate_root,
                old_bounds: Rectangle::new(50.0, 40.0, 80.0, 60.0),
                new_bounds: Rectangle::new(70.0, 55.0, 100.0, 70.0),
            },
        ]
    );

    let canvas = graph.perform_update(&mut update_manager);
    assert_eq!(
        canvas.damage().union(),
        Some(Rectangle::new(50.0, 40.0, 120.0, 85.0))
    );
}
