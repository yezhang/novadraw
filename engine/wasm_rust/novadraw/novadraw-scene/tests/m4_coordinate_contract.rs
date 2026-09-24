use std::sync::{Arc, Mutex};

use novadraw_core::Color;
use novadraw_geometry::{Point, Rectangle, Translatable};
use novadraw_render::{NdCanvas, command::LineCap, command::LineJoin};
use novadraw_scene::{
    Bounded, CoordinateListener, EventContext, EventDispatcher, Figure, FigureEvent,
    FigureEventHandler, FigureListener, FigureTree, InteractionState, LineBorder,
    ListenerDirective, MouseButton, MouseEvent, PendingMutations, RectangleFigure, Runtime,
    SceneDispatchContext, Shape, UpdateManager,
};

fn coordinate_root(x: f64, y: f64, width: f64, height: f64) -> RectangleFigure {
    RectangleFigure::new(x, y, width, height)
        .with_border(LineBorder::new(Color::BLACK, 1.0).with_insets(3.0, 5.0, 0.0, 0.0))
}

fn nested_coordinate_scene() -> (FigureTree, novadraw_scene::FigureId) {
    let mut graph = FigureTree::new();
    let contents = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 400.0)));
    let outer = graph
        .builder()
        .add_child(
            contents,
            Box::new(coordinate_root(100.0, 50.0, 300.0, 250.0)),
        )
        .expect("valid FigureTree construction");
    let inner =
        graph
            .builder()
            .add_child(
                outer,
                Box::new(RectangleFigure::new(20.0, 30.0, 180.0, 140.0).with_border(
                    LineBorder::new(Color::BLACK, 1.0).with_insets(7.0, 11.0, 0.0, 0.0),
                )),
            )
            .expect("valid FigureTree construction");
    let child = graph
        .builder()
        .add_child(
            inner,
            Box::new(RectangleFigure::new(10.0, 15.0, 60.0, 50.0)),
        )
        .expect("valid FigureTree construction");
    (graph, child)
}

#[test]
fn m4_point_roundtrips_across_nested_coordinate_roots_with_insets() {
    let (graph, child) = nested_coordinate_scene();
    let original = Point::new(15.0, 20.0);
    let mut point = original;

    point.transform(graph.local_to_surface_transform(child).unwrap());
    assert_eq!(point, Point::new(161.0, 125.0));

    point.transform(graph.surface_to_local_transform(child).unwrap());
    assert_eq!(point, original);
}

#[test]
fn m4_rectangle_roundtrip_preserves_extent_across_nested_coordinate_roots() {
    let (graph, child) = nested_coordinate_scene();
    let original = Rectangle::new(15.0, 20.0, 18.0, 12.0);
    let mut rect = original;

    rect.transform(graph.local_to_surface_transform(child).unwrap());
    assert_eq!(rect, Rectangle::new(161.0, 125.0, 18.0, 12.0));

    rect.transform(graph.surface_to_local_transform(child).unwrap());
    assert_eq!(rect, original);
}

#[test]
fn coordinate_transform_queries_reject_foreign_figures_without_mutating_geometry() {
    let (graph, _) = nested_coordinate_scene();
    let mut foreign = FigureTree::new();
    let foreign_figure = foreign
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));

    assert!(graph.local_to_parent_transform(foreign_figure).is_none());
    assert!(graph.parent_to_local_transform(foreign_figure).is_none());
    assert!(graph.local_to_surface_transform(foreign_figure).is_none());
    assert!(graph.surface_to_local_transform(foreign_figure).is_none());
    assert!(
        graph
            .child_content_to_surface_transform(foreign_figure)
            .is_none()
    );
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
}

impl Figure for RecordingFigure {
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

impl FigureEventHandler for RecordingFigure {
    fn on_mouse_pressed(&self, event: &MouseEvent, _ctx: &mut EventContext<'_>) -> bool {
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
    let mut graph = FigureTree::new();
    let contents = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 400.0)));
    let outer = graph
        .builder()
        .add_child(
            contents,
            Box::new(coordinate_root(100.0, 50.0, 300.0, 250.0)),
        )
        .expect("valid FigureTree construction");
    let inner =
        graph
            .builder()
            .add_child(
                outer,
                Box::new(RectangleFigure::new(20.0, 30.0, 180.0, 140.0).with_border(
                    LineBorder::new(Color::BLACK, 1.0).with_insets(7.0, 11.0, 0.0, 0.0),
                )),
            )
            .expect("valid FigureTree construction");
    let target = graph
        .builder()
        .add_child(
            inner,
            Box::new(RecordingFigure::new(
                Rectangle::new(10.0, 15.0, 60.0, 50.0),
                Arc::clone(&recorded),
            )),
        )
        .expect("valid FigureTree construction");
    let entry = Point::new(161.0, 125.0);

    assert_eq!(
        graph.find_mouse_event_target_at(entry.x(), entry.y()),
        Some(target)
    );

    let mut update_manager = UpdateManager::new();
    let mut interaction = InteractionState::default();
    let mut pending_mutations = PendingMutations::new();
    let mut dispatcher = EventDispatcher;
    let mut context = SceneDispatchContext::new(
        &mut graph,
        &mut interaction,
        &mut update_manager,
        &mut pending_mutations,
    );
    dispatcher.dispatch_mouse_pressed(&mut context, entry.x(), entry.y(), MouseButton::Left);

    assert_eq!(
        *recorded.lock().unwrap(),
        Some(RecordedMousePoint {
            target: Point::new(15.0, 20.0),
            entry,
        })
    );
}

struct CoordinateEventRecorder(Arc<Mutex<Vec<FigureEvent>>>);

impl FigureListener for CoordinateEventRecorder {
    fn figure_moved(&self, event: FigureEvent) -> ListenerDirective {
        self.0.lock().unwrap().push(event);
        ListenerDirective::Keep
    }
}

impl CoordinateListener for CoordinateEventRecorder {
    fn coordinate_system_changed(&self, event: FigureEvent) -> ListenerDirective {
        self.0.lock().unwrap().push(event);
        ListenerDirective::Keep
    }
}

#[test]
fn m4_coordinate_root_move_and_resize_is_one_atomic_bounds_change() {
    let mut graph = FigureTree::new();
    let contents = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 240.0)));
    let coordinate_root = graph
        .builder()
        .add_child(
            contents,
            Box::new(RectangleFigure::new(50.0, 40.0, 80.0, 60.0)),
        )
        .expect("valid FigureTree construction");
    let child = graph
        .builder()
        .add_child(
            coordinate_root,
            Box::new(RectangleFigure::new(10.0, 15.0, 20.0, 10.0)),
        )
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(graph);
    runtime.prepare_frame();
    let figure_events = Arc::new(Mutex::new(Vec::new()));
    runtime.add_figure_listener(Box::new(CoordinateEventRecorder(Arc::clone(
        &figure_events,
    ))));
    runtime.add_coordinate_listener(Box::new(CoordinateEventRecorder(Arc::clone(
        &figure_events,
    ))));
    assert!(
        runtime
            .set_bounds(coordinate_root, Rectangle::new(70.0, 55.0, 100.0, 70.0),)
            .unwrap()
    );

    assert_eq!(
        runtime.tree().figure_bounds(child),
        Some(Rectangle::new(10.0, 15.0, 20.0, 10.0))
    );
    let canvas = runtime.prepare_frame().unwrap();
    assert_eq!(
        *figure_events.lock().unwrap(),
        vec![
            FigureEvent::FigureMoved {
                figure_id: coordinate_root,
                old_bounds: Rectangle::new(50.0, 40.0, 80.0, 60.0),
                new_bounds: Rectangle::new(70.0, 55.0, 100.0, 70.0),
            },
            FigureEvent::CoordinateSystemChanged {
                figure_id: coordinate_root,
                old_bounds: Rectangle::new(50.0, 40.0, 80.0, 60.0),
                new_bounds: Rectangle::new(70.0, 55.0, 100.0, 70.0),
            },
        ]
    );

    assert_eq!(
        canvas.damage().union(),
        Some(Rectangle::new(50.0, 40.0, 120.0, 85.0))
    );
}
