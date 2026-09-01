use std::sync::{Arc, Mutex};

use novadraw_core::Color;
use novadraw_geometry::{Point, Rectangle, Transform, Translatable};
use novadraw_render::command::RenderCommandKind;
use novadraw_scene::{
    BasicEventDispatcher, Bounded, DefaultRangeModel, EventDispatcher, Figure, FigureEventHandler,
    FigureGraph, GesturePhase, GestureSessionId, InteractionState, KeyModifiers, LineBorder,
    MouseButton, PendingMutations, RangeChange, RangeListener, RangeModel, RangeModelError,
    RangeProperty, RectangleFigure, ScaleError, SceneDispatchContext, SceneUpdateManager,
    ScrollBarVisibility, ScrollDeltaKind, Updatable, ViewportFigure, WheelEvent, ZoomError,
    ZoomEvent, ZoomManager,
};

struct RecordingRangeListener {
    changes: Arc<Mutex<Vec<RangeChange>>>,
}

impl RangeListener for RecordingRangeListener {
    fn range_changed(&self, change: RangeChange) {
        self.changes.lock().unwrap().push(change);
    }
}

#[test]
fn range_model_clamps_extent_and_value_atomically() {
    let model = DefaultRangeModel::new(0.0, 20.0, 100.0).unwrap();
    model.set_value(90.0).unwrap();
    assert_eq!(model.value(), 80.0);

    let changes = model.set_all(10.0, 200.0, 60.0).unwrap();

    assert_eq!(model.minimum(), 10.0);
    assert_eq!(model.maximum(), 60.0);
    assert_eq!(model.extent(), 50.0);
    assert_eq!(model.value(), 10.0);
    assert!(!model.is_enabled());
    assert_eq!(
        changes
            .changes()
            .iter()
            .map(|change| change.property)
            .collect::<Vec<_>>(),
        vec![
            RangeProperty::Maximum,
            RangeProperty::Extent,
            RangeProperty::Minimum,
            RangeProperty::Value,
        ]
    );
}

#[test]
fn range_model_rejects_invalid_input_without_partial_state() {
    let model = DefaultRangeModel::new(0.0, 20.0, 100.0).unwrap();
    let original = model.snapshot();

    assert_eq!(
        model.set_all(100.0, 10.0, 20.0),
        Err(RangeModelError::InvalidBounds)
    );
    assert_eq!(
        model.set_value(f64::NAN),
        Err(RangeModelError::NonFiniteValue)
    );
    assert_eq!(model.snapshot(), original);
}

#[test]
fn range_model_listener_observes_changes_until_removed() {
    let model = DefaultRangeModel::default();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let listener_id = model.add_listener(Arc::new(RecordingRangeListener {
        changes: Arc::clone(&recorded),
    }));

    model.set_value(30.0).unwrap();
    assert!(model.remove_listener(listener_id));
    model.set_value(40.0).unwrap();

    assert_eq!(
        *recorded.lock().unwrap(),
        vec![RangeChange {
            property: RangeProperty::Value,
            old_value: 0.0,
            new_value: 30.0,
        }]
    );
}

#[test]
fn viewport_handle_owns_contents_and_derives_ranges_from_layout() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let viewport = graph
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 300.0, 200.0))
        .unwrap();
    let mut update_manager = SceneUpdateManager::new();
    let contents = viewport
        .set_contents(
            &mut graph,
            &mut update_manager,
            Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 450.0)),
        )
        .unwrap();

    graph.revalidate(viewport.block_id());

    assert_eq!(viewport.contents(&graph), Some(contents));
    assert_eq!(viewport.horizontal_range().extent, 300.0);
    assert_eq!(viewport.horizontal_range().maximum, 600.0);
    assert_eq!(viewport.vertical_range().extent, 200.0);
    assert_eq!(viewport.vertical_range().maximum, 450.0);
    assert_eq!(
        graph.figure_bounds(contents),
        Some(Rectangle::new(0.0, 0.0, 600.0, 450.0))
    );
}

#[test]
fn viewport_handle_replaces_contents_without_leaving_two_children() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let viewport = graph
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 300.0, 200.0))
        .unwrap();
    let mut update_manager = SceneUpdateManager::new();
    let old_contents = viewport
        .set_contents(
            &mut graph,
            &mut update_manager,
            Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 450.0)),
        )
        .unwrap();
    let new_contents = viewport
        .set_contents(
            &mut graph,
            &mut update_manager,
            Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)),
        )
        .unwrap();

    assert_eq!(viewport.contents(&graph), Some(new_contents));
    assert_eq!(graph.parent_id(old_contents), None);
    assert_eq!(
        graph.child_order(viewport.block_id()),
        Some(vec![new_contents])
    );
}

#[test]
fn viewport_handle_scroll_clamps_and_repaints_the_viewport() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let viewport = graph
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 300.0, 200.0))
        .unwrap();
    let mut update_manager = SceneUpdateManager::new();
    viewport
        .set_contents(
            &mut graph,
            &mut update_manager,
            Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 450.0)),
        )
        .unwrap();
    graph.revalidate(viewport.block_id());
    graph.drain_notification_effects();
    update_manager.clear();

    assert!(
        viewport
            .set_view_location(&mut graph, &mut update_manager, 500.0, 400.0)
            .unwrap()
    );

    assert_eq!(
        viewport.view_location(),
        novadraw_geometry::Point::new(300.0, 250.0)
    );
    assert!(update_manager.has_pending_repaint());
    assert!(graph.notification_effects().iter().any(|effect| {
        matches!(
            effect,
            novadraw_scene::NotificationEffect::EmitProperty(event)
                if event.block_id == viewport.block_id() && event.property == "viewLocation"
        )
    }));
}

#[test]
fn viewport_track_width_uses_available_width_until_content_minimum() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let viewport = graph
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 300.0, 200.0))
        .unwrap();
    let mut update_manager = SceneUpdateManager::new();
    let contents = viewport
        .set_contents(
            &mut graph,
            &mut update_manager,
            Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 450.0)),
        )
        .unwrap();
    graph.set_minimum_size(contents, Some((180.0, 120.0)));
    viewport
        .set_tracks_width(&mut graph, &mut update_manager, true)
        .unwrap();

    graph.revalidate(viewport.block_id());

    assert_eq!(graph.figure_bounds(contents).unwrap().width, 300.0);
    assert_eq!(viewport.horizontal_range().maximum, 300.0);
    assert!(!viewport.horizontal_range().is_enabled());
}

#[test]
fn scalable_layered_pane_composes_with_viewport_parent_transform() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let viewport = graph
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 300.0, 200.0))
        .unwrap();
    let scalable = graph
        .add_scalable_layered_pane_to(viewport.block_id(), Rectangle::new(0.0, 0.0, 600.0, 400.0))
        .unwrap();
    let child_color = Color::hex("#d7263d");
    let child = graph.add_child_to(
        scalable.block_id(),
        Box::new(RectangleFigure::new_with_color(
            20.0,
            30.0,
            40.0,
            20.0,
            child_color,
        )),
    );
    let mut update_manager = SceneUpdateManager::new();

    assert!(
        scalable
            .set_scale(&mut graph, &mut update_manager, 2.0)
            .unwrap()
    );
    let mut point = Point::new(0.0, 0.0);
    graph.translate_to_absolute_mut(child, &mut point);

    assert_eq!(point, Point::new(140.0, 140.0));
    let canvas = graph.render();
    let mut transform = Transform::IDENTITY;
    let mut stack = Vec::new();
    let mut projected_child = None;
    for command in canvas.commands() {
        match command.kind {
            RenderCommandKind::PushState => stack.push(transform),
            RenderCommandKind::RestoreState => {
                transform = *stack.last().expect("restore requires saved state");
            }
            RenderCommandKind::PopState => {
                transform = stack.pop().expect("pop requires saved state");
            }
            RenderCommandKind::ConcatTransform { matrix } => {
                transform = transform.post_concat(matrix);
            }
            RenderCommandKind::SetTransform { matrix } => transform = matrix,
            RenderCommandKind::ResetTransform => transform = Transform::IDENTITY,
            RenderCommandKind::FillRect { rect, color } if color == child_color => {
                let mut bounds = Rectangle::new(
                    rect[0].x,
                    rect[0].y,
                    rect[1].x - rect[0].x,
                    rect[1].y - rect[0].y,
                );
                bounds.transform(transform);
                projected_child = Some(bounds);
            }
            _ => {}
        }
    }
    assert_eq!(
        projected_child,
        Some(Rectangle::new(140.0, 140.0, 80.0, 40.0))
    );
    assert_eq!(
        graph.figure_bounds(scalable.block_id()),
        Some(Rectangle::new(0.0, 0.0, 600.0, 400.0))
    );
    assert!(update_manager.has_pending_repaint());
    graph.perform_update(&mut update_manager);
    assert_eq!(
        graph.figure_bounds(scalable.block_id()),
        Some(Rectangle::new(0.0, 0.0, 1200.0, 800.0))
    );
}

#[test]
fn scalable_layered_pane_rejects_invalid_scale_without_state_change() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let scalable = graph
        .add_scalable_layered_pane_to(root, Rectangle::new(0.0, 0.0, 600.0, 400.0))
        .unwrap();
    let mut update_manager = SceneUpdateManager::new();

    assert_eq!(
        scalable.set_scale(&mut graph, &mut update_manager, 0.0),
        Err(ScaleError::InvalidScale)
    );
    assert_eq!(scalable.scale(), 1.0);
    assert!(!update_manager.has_pending_repaint());
}

#[test]
fn scalable_projects_explicit_unscaled_preferred_size_through_scale() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let viewport = graph
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 300.0, 200.0))
        .unwrap();
    let scalable = graph
        .add_scalable_layered_pane_to(viewport.block_id(), Rectangle::new(0.0, 0.0, 600.0, 400.0))
        .unwrap();
    assert!(graph.set_preferred_size(scalable.block_id(), Some((500.0, 300.0))));
    let mut update_manager = SceneUpdateManager::new();

    scalable
        .set_scale(&mut graph, &mut update_manager, 2.0)
        .unwrap();
    graph.perform_update(&mut update_manager);

    assert_eq!(
        graph.figure_bounds(scalable.block_id()),
        Some(Rectangle::new(0.0, 0.0, 1000.0, 600.0))
    );
}

#[derive(Clone)]
struct WheelIgnoringFigure {
    bounds: Rectangle,
}

impl Bounded for WheelIgnoringFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "WheelIgnoringFigure"
    }
}

impl Updatable for WheelIgnoringFigure {
    fn validate(&mut self) {}
}

impl Figure for WheelIgnoringFigure {
    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl FigureEventHandler for WheelIgnoringFigure {}

fn large_scroll_pane_scene() -> (
    FigureGraph,
    novadraw_scene::ScrollPaneHandle,
    SceneUpdateManager,
) {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let pane = graph
        .add_scroll_pane_to(root, Rectangle::new(100.0, 80.0, 320.0, 220.0))
        .unwrap();
    let mut update_manager = SceneUpdateManager::new();
    let contents = pane
        .set_contents(
            &mut graph,
            &mut update_manager,
            Box::new(RectangleFigure::new(0.0, 0.0, 640.0, 480.0)),
        )
        .unwrap();
    graph.add_child_to(
        contents,
        Box::new(WheelIgnoringFigure {
            bounds: Rectangle::new(10.0, 10.0, 100.0, 80.0),
        }),
    );
    graph.revalidate(pane.pane_id());
    (graph, pane, update_manager)
}

#[test]
fn scroll_pane_automatic_policy_reserves_both_scroll_bars() {
    let (graph, pane, _) = large_scroll_pane_scene();

    assert!(graph.is_visible(pane.horizontal_scroll_bar()));
    assert!(graph.is_visible(pane.vertical_scroll_bar()));
    assert_eq!(
        graph.figure_bounds(pane.viewport().block_id()),
        Some(Rectangle::new(0.0, 0.0, 306.0, 206.0))
    );
    assert_eq!(pane.viewport().horizontal_range().extent, 306.0);
    assert_eq!(pane.viewport().vertical_range().extent, 206.0);
}

#[test]
fn scroll_pane_visibility_policy_controls_layout() {
    let (mut graph, pane, mut update_manager) = large_scroll_pane_scene();

    pane.set_scroll_bar_visibility(
        &mut graph,
        &mut update_manager,
        ScrollBarVisibility::Never,
        ScrollBarVisibility::Always,
    )
    .unwrap();
    graph.revalidate(pane.pane_id());

    assert!(!graph.is_visible(pane.horizontal_scroll_bar()));
    assert!(graph.is_visible(pane.vertical_scroll_bar()));
    assert_eq!(
        graph.figure_bounds(pane.viewport().block_id()),
        Some(Rectangle::new(0.0, 0.0, 306.0, 220.0))
    );
}

#[test]
fn scroll_pane_resize_recomputes_automatic_visibility_and_range_extent() {
    let (mut graph, pane, mut update_manager) = large_scroll_pane_scene();

    graph.set_bounds_with_update(
        &mut update_manager,
        pane.pane_id(),
        100.0,
        80.0,
        900.0,
        700.0,
    );
    graph.perform_update(&mut update_manager);

    assert!(!graph.is_visible(pane.horizontal_scroll_bar()));
    assert!(!graph.is_visible(pane.vertical_scroll_bar()));
    assert_eq!(pane.viewport().horizontal_range().extent, 900.0);
    assert_eq!(pane.viewport().vertical_range().extent, 700.0);
}

#[test]
fn unhandled_wheel_uses_nearest_scroll_pane_fallback() {
    let (mut graph, pane, mut update_manager) = large_scroll_pane_scene();
    let mut interaction = InteractionState::default();
    let mut pending = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;
    {
        let mut context = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
        dispatcher.dispatch_mouse_wheel(&mut context, 120.0, 100.0, 0.0, -1.0);
    }

    assert_eq!(pane.viewport().view_location().y(), 24.0);
    assert!(
        update_manager
            .notification_effects()
            .iter()
            .any(|effect| matches!(
                effect,
                novadraw_scene::NotificationEffect::EmitFigure(
                    novadraw_scene::FigureEvent::CoordinateSystemChanged { block_id, .. }
                ) if *block_id == pane.viewport().block_id()
            ))
    );
}

#[test]
fn touchpad_pixel_scroll_uses_logical_distance_without_line_multiplier() {
    let (mut graph, pane, mut update_manager) = large_scroll_pane_scene();
    let mut interaction = InteractionState::default();
    let mut pending = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;
    {
        let mut context = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
        dispatcher.dispatch_scroll(
            &mut context,
            WheelEvent::with_details(
                120.0,
                100.0,
                0.0,
                -7.5,
                ScrollDeltaKind::LogicalPixels,
                GesturePhase::Impulse,
                KeyModifiers::default(),
                GestureSessionId::IMPULSE,
            ),
        );
    }

    assert_eq!(pane.viewport().view_location().y(), 7.5);
}

#[test]
fn pinch_zoom_keeps_content_point_under_the_entry_anchor() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let pane = graph
        .add_scroll_pane_to(root, Rectangle::new(100.0, 80.0, 320.0, 220.0))
        .unwrap();
    let scalable = graph
        .add_scalable_layered_pane_to(
            pane.viewport().block_id(),
            Rectangle::new(0.0, 0.0, 640.0, 480.0),
        )
        .unwrap();
    let child = graph.add_child_to(
        scalable.block_id(),
        Box::new(WheelIgnoringFigure {
            bounds: Rectangle::new(0.0, 0.0, 640.0, 480.0),
        }),
    );
    graph.revalidate(pane.pane_id());
    let mut update_manager = SceneUpdateManager::new();
    let mut interaction = InteractionState::default();
    let mut pending = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;

    {
        let mut context = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
        dispatcher.dispatch_zoom(
            &mut context,
            ZoomEvent::new(
                150.0,
                130.0,
                2.0,
                GesturePhase::Impulse,
                KeyModifiers::default(),
                GestureSessionId::IMPULSE,
            ),
        );
    }

    assert_eq!(scalable.scale(), 2.0);
    assert_eq!(pane.viewport().view_location(), Point::new(50.0, 50.0));
    let mut anchored_content_point = Point::new(50.0, 50.0);
    graph.translate_to_absolute_mut(child, &mut anchored_content_point);
    assert_eq!(anchored_content_point, Point::new(150.0, 130.0));
}

#[test]
fn zoomed_canvas_remains_reachable_at_every_scroll_range_edge() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let pane = graph
        .add_scroll_pane_to(root, Rectangle::new(100.0, 80.0, 320.0, 220.0))
        .unwrap();
    let scalable = graph
        .add_scalable_layered_pane_to(
            pane.viewport().block_id(),
            Rectangle::new(0.0, 0.0, 640.0, 480.0),
        )
        .unwrap();
    let content = graph.add_child_to(
        scalable.block_id(),
        Box::new(WheelIgnoringFigure {
            bounds: Rectangle::new(0.0, 0.0, 640.0, 480.0),
        }),
    );
    graph.revalidate(pane.pane_id());
    let mut update_manager = SceneUpdateManager::new();
    let mut interaction = InteractionState::default();
    let mut pending = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;

    {
        let mut context = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
        dispatcher.dispatch_zoom(
            &mut context,
            ZoomEvent::new(
                250.0,
                180.0,
                2.0,
                GesturePhase::Impulse,
                KeyModifiers::default(),
                GestureSessionId::IMPULSE,
            ),
        );
    }
    // The next input event must already observe the scaled range; waiting for a
    // render/validation frame makes a pinch followed by pan use stale limits.
    assert_eq!(pane.viewport().horizontal_range().maximum, 1280.0);
    assert_eq!(pane.viewport().vertical_range().maximum, 960.0);

    {
        let mut context = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
        dispatcher.dispatch_scroll(
            &mut context,
            WheelEvent::with_details(
                250.0,
                180.0,
                10_000.0,
                10_000.0,
                ScrollDeltaKind::LogicalPixels,
                GesturePhase::Impulse,
                KeyModifiers::default(),
                GestureSessionId::IMPULSE,
            ),
        );
    }
    assert_eq!(pane.viewport().view_location(), Point::new(0.0, 0.0));
    let mut top_left = Point::new(0.0, 0.0);
    graph.translate_to_absolute_mut(content, &mut top_left);
    assert_eq!(top_left, Point::new(100.0, 80.0));

    {
        let mut context = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
        dispatcher.dispatch_scroll(
            &mut context,
            WheelEvent::with_details(
                250.0,
                180.0,
                -10_000.0,
                -10_000.0,
                ScrollDeltaKind::LogicalPixels,
                GesturePhase::Impulse,
                KeyModifiers::default(),
                GestureSessionId::IMPULSE,
            ),
        );
    }
    let horizontal = pane.viewport().horizontal_range();
    let vertical = pane.viewport().vertical_range();
    assert_eq!(
        pane.viewport().view_location(),
        Point::new(
            horizontal.maximum - horizontal.extent,
            vertical.maximum - vertical.extent
        )
    );
    let mut bottom_right = Point::new(640.0, 480.0);
    graph.translate_to_absolute_mut(content, &mut bottom_right);
    assert_eq!(
        bottom_right,
        Point::new(100.0 + horizontal.extent, 80.0 + vertical.extent)
    );

    graph.perform_update(&mut update_manager);
    assert_eq!(pane.viewport().horizontal_range(), horizontal);
    assert_eq!(pane.viewport().vertical_range(), vertical);
}

#[test]
fn zoom_out_layout_does_not_corrupt_the_unscaled_preferred_extent() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let pane = graph
        .add_scroll_pane_to(root, Rectangle::new(100.0, 80.0, 320.0, 220.0))
        .unwrap();
    let scalable = graph
        .add_scalable_layered_pane_to(
            pane.viewport().block_id(),
            Rectangle::new(0.0, 0.0, 400.0, 300.0),
        )
        .unwrap();
    let content = graph.add_child_to(
        scalable.block_id(),
        Box::new(WheelIgnoringFigure {
            bounds: Rectangle::new(0.0, 0.0, 400.0, 300.0),
        }),
    );
    graph.revalidate(pane.pane_id());
    let mut update_manager = SceneUpdateManager::new();
    let mut interaction = InteractionState::default();
    let mut pending = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;

    for factor in [0.5, 4.0] {
        {
            let mut context = SceneDispatchContext::new(
                &mut graph,
                &mut interaction,
                &mut update_manager,
                &mut pending,
            );
            dispatcher.dispatch_zoom(
                &mut context,
                ZoomEvent::new(
                    100.0,
                    80.0,
                    factor,
                    GesturePhase::Impulse,
                    KeyModifiers::default(),
                    GestureSessionId::IMPULSE,
                ),
            );
        }
        graph.perform_update(&mut update_manager);
        if factor == 0.5 {
            let horizontal = pane.viewport().horizontal_range();
            let vertical = pane.viewport().vertical_range();
            let scaled_width = 400.0 * scalable.scale();
            let scaled_height = 300.0 * scalable.scale();
            let mut top_left = Point::new(0.0, 0.0);
            let mut bottom_right = Point::new(400.0, 300.0);
            graph.translate_to_absolute_mut(content, &mut top_left);
            graph.translate_to_absolute_mut(content, &mut bottom_right);
            assert_eq!(top_left, Point::new(100.0, 80.0));
            assert_eq!(
                bottom_right,
                Point::new(top_left.x() + scaled_width, top_left.y() + scaled_height)
            );
            assert!(bottom_right.x() <= 100.0 + horizontal.extent);
            assert!(bottom_right.y() <= 80.0 + vertical.extent);
        }
    }

    assert_eq!(scalable.scale(), 2.0);
    assert_eq!(pane.viewport().horizontal_range().maximum, 800.0);
    assert_eq!(pane.viewport().vertical_range().maximum, 600.0);
}

#[test]
fn vertical_scroll_bar_step_updates_shared_viewport_model() {
    let (mut graph, pane, mut update_manager) = large_scroll_pane_scene();
    let bounds = graph.figure_bounds(pane.vertical_scroll_bar()).unwrap();
    let mut point = Point::new(bounds.width / 2.0, bounds.height - 2.0);
    graph.translate_to_absolute_mut(pane.vertical_scroll_bar(), &mut point);
    let mut interaction = InteractionState::default();
    let mut pending = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;
    let mut context = SceneDispatchContext::new(
        &mut graph,
        &mut interaction,
        &mut update_manager,
        &mut pending,
    );
    dispatcher.dispatch_mouse_pressed(&mut context, point.x(), point.y(), MouseButton::Left);
    dispatcher.dispatch_mouse_released(&mut context, point.x(), point.y(), MouseButton::Left);

    assert_eq!(pane.viewport().view_location().y(), 24.0);
}

#[test]
fn vertical_scroll_bar_thumb_drag_updates_shared_viewport_model() {
    let (mut graph, pane, mut update_manager) = large_scroll_pane_scene();
    let bounds = graph.figure_bounds(pane.vertical_scroll_bar()).unwrap();
    let mut start = Point::new(bounds.width / 2.0, 20.0);
    let mut end = Point::new(start.x(), start.y() + 50.0);
    graph.translate_to_absolute_mut(pane.vertical_scroll_bar(), &mut start);
    graph.translate_to_absolute_mut(pane.vertical_scroll_bar(), &mut end);
    let mut interaction = InteractionState::default();
    let mut pending = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;
    let mut context = SceneDispatchContext::new(
        &mut graph,
        &mut interaction,
        &mut update_manager,
        &mut pending,
    );

    dispatcher.dispatch_mouse_pressed(&mut context, start.x(), start.y(), MouseButton::Left);
    dispatcher.dispatch_mouse_moved(&mut context, end.x(), end.y());
    dispatcher.dispatch_mouse_released(&mut context, end.x(), end.y(), MouseButton::Left);

    assert!(pane.viewport().view_location().y() > 0.0);
}

#[test]
fn zoom_manager_owns_zoom_limits_and_default_center_policy() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let viewport = graph
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 300.0, 200.0))
        .unwrap();
    let scalable = graph
        .add_scalable_layered_pane_to(viewport.block_id(), Rectangle::new(0.0, 0.0, 600.0, 400.0))
        .unwrap();
    graph.revalidate(viewport.block_id());
    let mut update_manager = SceneUpdateManager::new();
    let manager = ZoomManager::new(scalable.clone(), viewport.clone());

    assert!(
        manager
            .set_zoom(&mut graph, &mut update_manager, 2.0)
            .unwrap()
    );
    assert_eq!(manager.zoom(), 2.0);
    assert_eq!(viewport.view_location(), Point::new(150.0, 100.0));
    assert_eq!(viewport.horizontal_range().maximum, 1200.0);
    assert_eq!(viewport.vertical_range().maximum, 800.0);

    assert!(
        manager
            .set_zoom(&mut graph, &mut update_manager, 0.01)
            .unwrap()
    );
    assert_eq!(manager.zoom(), 0.5);
    assert!(
        manager
            .set_zoom(&mut graph, &mut update_manager, 100.0)
            .unwrap()
    );
    assert_eq!(manager.zoom(), 4.0);
    assert!(manager.fit_all(&mut graph, &mut update_manager).unwrap());
    assert_eq!(manager.zoom(), 0.5);
    assert_eq!(viewport.view_location(), Point::new(0.0, 0.0));

    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            manager.set_zoom(&mut graph, &mut update_manager, invalid),
            Err(ZoomError::InvalidZoom)
        );
        assert_eq!(manager.zoom(), 0.5);
    }
}

#[test]
fn zoom_manager_uses_configured_levels_for_step_zoom() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let viewport = graph
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 300.0, 200.0))
        .unwrap();
    let scalable = graph
        .add_scalable_layered_pane_to(viewport.block_id(), Rectangle::new(0.0, 0.0, 600.0, 400.0))
        .unwrap();
    graph.revalidate(viewport.block_id());
    let mut update_manager = SceneUpdateManager::new();
    let mut manager = ZoomManager::new(scalable, viewport);
    manager.set_zoom_levels(vec![0.25, 1.0, 2.0]).unwrap();

    assert!(
        manager
            .set_zoom(&mut graph, &mut update_manager, 0.01)
            .unwrap()
    );
    assert_eq!(manager.zoom(), 0.25);
    assert!(manager.zoom_in(&mut graph, &mut update_manager).unwrap());
    assert_eq!(manager.zoom(), 1.0);
    assert!(manager.zoom_out(&mut graph, &mut update_manager).unwrap());
    assert_eq!(manager.zoom(), 0.25);
    assert_eq!(
        manager.set_zoom_levels(vec![1.0, 0.5]),
        Err(ZoomError::InvalidZoomLevels)
    );
}

#[test]
fn viewport_border_insets_define_child_transform_and_client_extent() {
    let viewport = ViewportFigure::new(100.0, 50.0, 300.0, 200.0)
        .with_origin(20.0, 30.0)
        .with_border(LineBorder::new(Color::BLACK, 1.0).with_insets(10.0, 10.0, 10.0, 10.0));

    let mut content_origin = Rectangle::new(20.0, 30.0, 1.0, 1.0);
    viewport.child_transform().apply_to(&mut content_origin);

    assert_eq!(content_origin, Rectangle::new(10.0, 10.0, 1.0, 1.0));
    assert_eq!(
        viewport.client_area(),
        Rectangle::new(10.0, 10.0, 280.0, 180.0)
    );
}

#[test]
fn viewport_rejects_a_second_contents_child_atomically() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 400.0)));
    let viewport = graph.add_child_to(
        root,
        Box::new(ViewportFigure::new(20.0, 20.0, 300.0, 200.0)),
    );
    let first = graph.try_add_child_to(
        viewport,
        Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 400.0)),
    );
    assert!(first.is_ok());

    let second = graph.try_add_child_to(
        viewport,
        Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)),
    );

    assert!(second.is_err());
    assert_eq!(graph.child_order(viewport).unwrap().len(), 1);
}
