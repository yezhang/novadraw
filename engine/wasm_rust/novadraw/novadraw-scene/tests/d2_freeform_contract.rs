use novadraw_render::RenderCommandKind;
use novadraw_scene::{
    FREEFORM_EXTENT_PROPERTY, FigureTree, FreeformConstraint, FreeformConstraintError,
    FreeformError, FreeformLayerFigure, FreeformLayout, LayerKey, LayerPlacement, LayoutError,
    ListenerDirective, MeasureConstraints, MouseLocationZoomScrollPolicy, Point,
    PropertyChangeEvent, PropertyChangeListener, PropertyValue, Rectangle, RectangleFigure,
    Runtime, ScaleHandle, ViewportHandle, XYConstraint, ZoomManager,
};
use std::sync::{Arc, Mutex};

struct PropertyRecorder(Arc<Mutex<Vec<PropertyChangeEvent>>>);

impl PropertyChangeListener for PropertyRecorder {
    fn property_changed(&self, event: &PropertyChangeEvent) -> ListenerDirective {
        self.0.lock().unwrap().push(event.clone());
        ListenerDirective::Keep
    }
}

fn scalable_freeform_viewport(
    layer_bounds: Rectangle,
) -> (
    Runtime,
    ViewportHandle,
    ScaleHandle,
    novadraw_scene::FigureId,
) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let viewport = tree
        .builder()
        .add_viewport_to(root, Rectangle::new(0.0, 0.0, 300.0, 200.0))
        .unwrap();
    let scalable = tree
        .builder()
        .add_scalable_freeform_layered_pane_to(
            viewport.figure_id(),
            Rectangle::new(0.0, 0.0, 300.0, 200.0),
        )
        .unwrap();
    let mut runtime = Runtime::new(tree);
    let layer = runtime
        .layered_pane(scalable.figure_id())
        .unwrap()
        .add_layer(
            Box::new(FreeformLayerFigure::new(0.0, 0.0, 300.0, 200.0)),
            LayerKey::new("content").unwrap(),
            LayerPlacement::Last,
        )
        .unwrap();
    let content = runtime
        .container(layer)
        .unwrap()
        .add(Box::new(RectangleFigure::new(
            layer_bounds.x,
            layer_bounds.y,
            layer_bounds.width,
            layer_bounds.height,
        )))
        .expect("valid Runtime mutation");
    runtime.prepare_frame();
    (runtime, viewport, scalable, content)
}

#[test]
fn freeform_query_distinguishes_unknown_non_freeform_and_unvalidated() {
    let mut tree = FigureTree::new();
    let ordinary = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
    assert_eq!(
        tree.freeform_extent(ordinary),
        Err(FreeformError::NotFreeform(ordinary))
    );
    assert_eq!(
        tree.freeform_extent(novadraw_scene::FigureId::null()),
        Err(FreeformError::UnknownFigure(
            novadraw_scene::FigureId::null()
        ))
    );

    let freeform = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 10.0, 10.0)));
    assert_eq!(
        tree.freeform_extent(freeform),
        Err(FreeformError::Unvalidated(freeform))
    );
}

#[test]
fn empty_freeform_extent_is_zero() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 100.0, 80.0)));

    tree.builder()
        .validate_subtree(host)
        .expect("valid FigureTree construction");

    assert_eq!(tree.freeform_extent(host), Ok(Rectangle::ZERO));
}

#[test]
fn freeform_extent_unions_positive_and_negative_child_bounds() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 100.0, 80.0)));
    tree.builder()
        .add_child(
            host,
            Box::new(RectangleFigure::new(-40.0, -20.0, 10.0, 15.0)),
        )
        .expect("valid FigureTree construction");
    tree.builder()
        .add_child(host, Box::new(RectangleFigure::new(20.0, 30.0, 25.0, 10.0)))
        .expect("valid FigureTree construction");

    tree.builder()
        .validate_subtree(host)
        .expect("valid FigureTree construction");

    assert_eq!(
        tree.freeform_extent(host),
        Ok(Rectangle::new(-40.0, -20.0, 85.0, 60.0))
    );
}

#[test]
fn nested_freeform_uses_derived_extent_not_presentation_bounds() {
    let mut tree = FigureTree::new();
    let outer = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 100.0, 80.0)));
    let inner = tree
        .builder()
        .add_child(
            outer,
            Box::new(FreeformLayerFigure::new(100.0, 50.0, 500.0, 400.0)),
        )
        .expect("valid FigureTree construction");
    tree.builder()
        .add_child(
            inner,
            Box::new(RectangleFigure::new(-20.0, -10.0, 30.0, 20.0)),
        )
        .expect("valid FigureTree construction");

    tree.builder()
        .validate_subtree(outer)
        .expect("valid FigureTree construction");

    assert_eq!(
        tree.freeform_extent(inner),
        Ok(Rectangle::new(-20.0, -10.0, 30.0, 20.0))
    );
    assert_eq!(
        tree.freeform_extent(outer),
        Ok(Rectangle::new(80.0, 40.0, 30.0, 20.0))
    );
}

#[test]
fn child_move_keeps_old_stable_extent_until_revalidation() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 100.0, 80.0)));
    let child = tree
        .builder()
        .add_child(
            host,
            Box::new(RectangleFigure::new(-10.0, -5.0, 20.0, 10.0)),
        )
        .expect("valid FigureTree construction");
    tree.builder()
        .validate_subtree(host)
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    runtime.prepare_frame().expect("initial full frame");
    let events = Arc::new(Mutex::new(Vec::new()));
    runtime.add_property_listener(Box::new(PropertyRecorder(Arc::clone(&events))));
    runtime
        .figure(child)
        .unwrap()
        .set_bounds(Rectangle::new(30.0, 40.0, 20.0, 10.0))
        .unwrap();

    assert_eq!(
        runtime.freeform_extent(host),
        Ok(Rectangle::new(-10.0, -5.0, 20.0, 10.0))
    );
    assert!(
        runtime
            .tree()
            .node(host)
            .unwrap()
            .layout_state()
            .freeform_state()
            .unwrap()
            .is_dirty()
    );

    runtime.prepare_frame();

    assert_eq!(
        runtime.freeform_extent(host),
        Ok(Rectangle::new(30.0, 40.0, 20.0, 10.0))
    );
    let extent_events: Vec<_> = events
        .lock()
        .unwrap()
        .iter()
        .filter(|event| event.property == FREEFORM_EXTENT_PROPERTY)
        .cloned()
        .collect();
    assert_eq!(extent_events.len(), 1);
    assert_eq!(
        extent_events[0].old_value,
        PropertyValue::Rectangle(Rectangle::new(-10.0, -5.0, 20.0, 10.0))
    );
    assert_eq!(
        extent_events[0].new_value,
        PropertyValue::Rectangle(Rectangle::new(30.0, 40.0, 20.0, 10.0))
    );
}

#[test]
fn visibility_does_not_change_freeform_extent() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 100.0, 80.0)));
    let child = tree
        .builder()
        .add_child(
            host,
            Box::new(RectangleFigure::new(-25.0, -15.0, 20.0, 10.0)),
        )
        .expect("valid FigureTree construction");
    tree.builder()
        .set_visible(child, false)
        .expect("valid FigureTree construction");

    tree.builder()
        .validate_subtree(host)
        .expect("valid FigureTree construction");

    assert_eq!(
        tree.freeform_extent(host),
        Ok(Rectangle::new(-25.0, -15.0, 20.0, 10.0))
    );
}

#[test]
fn overflow_visible_allows_hits_outside_freeform_border_box() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 20.0, 20.0)));
    let child = tree
        .builder()
        .add_child(
            host,
            Box::new(RectangleFigure::new(-40.0, -30.0, 15.0, 10.0)),
        )
        .expect("valid FigureTree construction");

    assert_eq!(tree.hit_test_simple((-35.0, -25.0)), Some(child));
}

#[test]
fn normal_ancestor_still_clips_freeform_overflow() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 80.0)));
    let freeform = tree
        .builder()
        .add_child(
            root,
            Box::new(FreeformLayerFigure::new(10.0, 10.0, 20.0, 20.0)),
        )
        .expect("valid FigureTree construction");
    tree.builder()
        .add_child(
            freeform,
            Box::new(RectangleFigure::new(120.0, 0.0, 15.0, 10.0)),
        )
        .expect("valid FigureTree construction");

    assert_eq!(tree.hit_test_simple((135.0, 15.0)), None);
}

#[test]
fn overflow_visible_skips_host_clip_during_rendering() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 100.0, 80.0)));
    tree.builder()
        .add_child(
            host,
            Box::new(RectangleFigure::new(-40.0, -30.0, 15.0, 10.0)),
        )
        .expect("valid FigureTree construction");

    let canvas = Runtime::new(tree).record_full_frame();
    let has_host_clip = canvas.commands().iter().any(|command| {
        matches!(
            command.kind,
            RenderCommandKind::Clip { rect }
                if rect[0].x == 0.0
                    && rect[0].y == 0.0
                    && rect[1].x == 100.0
                    && rect[1].y == 80.0
        )
    });

    assert!(!has_host_clip);
}

#[test]
fn viewport_does_not_clip_freeform_contents_to_presentation_bounds() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 160.0)));
    let viewport = tree
        .builder()
        .add_viewport_to(root, Rectangle::new(0.0, 0.0, 100.0, 80.0))
        .unwrap();
    let freeform = tree
        .builder()
        .add_child(
            viewport.figure_id(),
            Box::new(FreeformLayerFigure::new(0.0, 0.0, 100.0, 80.0)),
        )
        .unwrap();
    tree.builder()
        .add_child(
            freeform,
            Box::new(RectangleFigure::new(-20.0, -10.0, 30.0, 20.0)),
        )
        .expect("valid FigureTree construction");
    tree.builder()
        .validate_subtree(viewport.figure_id())
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    runtime
        .viewport(viewport.figure_id())
        .unwrap()
        .set_view_location(-20.0, -10.0)
        .unwrap();

    let viewport_sized_clips = runtime
        .record_full_frame()
        .commands()
        .iter()
        .filter(|command| {
            matches!(
                command.kind,
                RenderCommandKind::Clip { rect }
                    if rect[0].x == 0.0
                        && rect[0].y == 0.0
                        && rect[1].x == 100.0
                        && rect[1].y == 80.0
            )
        })
        .count();

    assert_eq!(viewport_sized_clips, 2);
}

#[test]
fn overflow_visible_damage_is_not_clipped_to_host_bounds() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 20.0, 20.0)));
    let child = tree
        .builder()
        .add_child(
            host,
            Box::new(RectangleFigure::new(-40.0, -30.0, 15.0, 10.0)),
        )
        .expect("valid FigureTree construction");
    tree.builder()
        .validate_subtree(host)
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    runtime.prepare_frame().expect("initial full frame");
    runtime.figure(child).unwrap().repaint(None).unwrap();
    let canvas = runtime.prepare_frame().expect("queued repaint");

    assert_eq!(
        canvas.damage().union(),
        Some(Rectangle::new(-40.0, -30.0, 15.0, 10.0))
    );
}

#[test]
fn moving_overflow_visible_child_damages_old_and_new_surface_regions() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 150.0)));
    let host = tree
        .builder()
        .add_child(
            root,
            Box::new(FreeformLayerFigure::new(20.0, 20.0, 20.0, 20.0)),
        )
        .expect("valid FigureTree construction");
    let child = tree
        .builder()
        .add_child(host, Box::new(RectangleFigure::new(60.0, 0.0, 10.0, 10.0)))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    runtime.prepare_frame();
    assert!(
        runtime
            .figure(child)
            .unwrap()
            .set_bounds(Rectangle::new(80.0, 0.0, 10.0, 10.0))
            .unwrap()
    );
    let canvas = runtime.prepare_frame().unwrap();

    assert_eq!(
        canvas.damage().regions(),
        &[
            Rectangle::new(80.0, 20.0, 10.0, 10.0),
            Rectangle::new(100.0, 20.0, 10.0, 10.0),
        ]
    );
}

#[test]
fn freeform_constraint_rejects_invalid_values_and_accepts_explicit_zero() {
    assert_eq!(
        FreeformConstraint::at(Point::new(f64::NAN, 0.0)),
        Err(FreeformConstraintError::NonFiniteOrigin)
    );
    assert_eq!(
        FreeformConstraint::new(Point::new(0.0, 0.0), Some(-1.0), None),
        Err(FreeformConstraintError::InvalidWidth)
    );
    assert_eq!(
        FreeformConstraint::new(Point::new(0.0, 0.0), None, Some(f64::INFINITY)),
        Err(FreeformConstraintError::InvalidHeight)
    );
    assert_eq!(
        FreeformConstraint::fixed(Point::new(-5.0, -6.0), 0.0, 0.0)
            .unwrap()
            .width(),
        Some(0.0)
    );
}

#[test]
fn freeform_layout_preserves_negative_origin_and_uses_intrinsic_fallback() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 100.0, 80.0)));
    let child = tree
        .builder()
        .add_child(host, Box::new(RectangleFigure::new(1.0, 2.0, 3.0, 4.0)))
        .expect("valid FigureTree construction");
    tree.builder()
        .set_layout_manager(host, Box::new(FreeformLayout::new()))
        .expect("valid FigureTree construction");
    tree.builder()
        .set_preferred_size(child, Some((40.0, 50.0)))
        .expect("valid FigureTree construction");
    tree.builder()
        .set_layout_constraint(
            child,
            FreeformConstraint::new(Point::new(-30.0, -20.0), None, Some(0.0)).unwrap(),
        )
        .expect("valid FigureTree construction");

    assert_eq!(
        tree.preferred_measurement(host, MeasureConstraints::UNBOUNDED)
            .map(|measurement| measurement.size().into()),
        Some((40.0, 20.0))
    );
    assert_eq!(
        tree.freeform_extent(host),
        Err(FreeformError::Unvalidated(host))
    );

    tree.builder()
        .validate_subtree(host)
        .expect("valid FigureTree construction");

    assert_eq!(
        tree.figure_bounds(child),
        Some(Rectangle::new(-30.0, -20.0, 40.0, 0.0))
    );
    assert_eq!(
        tree.freeform_extent(host),
        Ok(Rectangle::new(-30.0, -20.0, 40.0, 0.0))
    );
}

#[test]
fn builder_rejects_invalid_constraint_type_before_layout() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 100.0, 80.0)));
    let first = tree
        .builder()
        .add_child(host, Box::new(RectangleFigure::new(1.0, 2.0, 3.0, 4.0)))
        .expect("valid FigureTree construction");
    let second = tree
        .builder()
        .add_child(host, Box::new(RectangleFigure::new(5.0, 6.0, 7.0, 8.0)))
        .expect("valid FigureTree construction");
    tree.builder()
        .set_layout_manager(host, Box::new(FreeformLayout::new()))
        .expect("valid FigureTree construction");
    tree.builder()
        .set_layout_constraint(
            first,
            FreeformConstraint::fixed(Point::new(20.0, 30.0), 40.0, 50.0).unwrap(),
        )
        .expect("valid FigureTree construction");
    let error = tree
        .builder()
        .set_layout_constraint(second, XYConstraint::at_size(60.0, 70.0, 80.0, 90.0))
        .unwrap_err();

    assert!(matches!(
        error,
        LayoutError::ConstraintTypeMismatch {
            container,
            child,
            ..
        } if container == host && child == second
    ));
    assert_eq!(
        tree.figure_bounds(first),
        Some(Rectangle::new(1.0, 2.0, 3.0, 4.0))
    );
}

#[test]
fn freeform_viewport_range_uses_unscaled_content_domain() {
    let (runtime, viewport, scalable, _) =
        scalable_freeform_viewport(Rectangle::new(-100.0, -50.0, 500.0, 300.0));

    assert_eq!(
        runtime.tree().freeform_extent(scalable.figure_id()),
        Ok(Rectangle::new(-100.0, -50.0, 500.0, 300.0))
    );
    assert_eq!(
        runtime.tree().figure_bounds(scalable.figure_id()),
        Some(Rectangle::new(0.0, 0.0, 300.0, 200.0))
    );
    assert_eq!(viewport.horizontal_range().minimum, -100.0);
    assert_eq!(viewport.horizontal_range().maximum, 400.0);
    assert_eq!(viewport.horizontal_range().extent, 300.0);
    assert_eq!(viewport.vertical_range().minimum, -50.0);
    assert_eq!(viewport.vertical_range().maximum, 250.0);
    assert_eq!(viewport.vertical_range().extent, 200.0);
}

#[test]
fn freeform_zoom_preserves_anchor_and_keeps_range_unscaled() {
    let (mut runtime, viewport, scalable, _) =
        scalable_freeform_viewport(Rectangle::new(-100.0, -50.0, 500.0, 300.0));
    let mut zoom = ZoomManager::new(scalable.clone(), viewport.clone());
    zoom.set_scroll_policy(Arc::new(MouseLocationZoomScrollPolicy));

    assert!(
        runtime
            .zoom(&zoom)
            .unwrap()
            .set_zoom_at(2.0, Some(Point::new(60.0, 40.0)))
            .unwrap()
    );

    assert_eq!(viewport.view_location(), Point::new(30.0, 20.0));
    assert_eq!(viewport.horizontal_range().minimum, -100.0);
    assert_eq!(viewport.horizontal_range().maximum, 400.0);
    assert_eq!(viewport.horizontal_range().extent, 150.0);
    assert_eq!(viewport.vertical_range().minimum, -50.0);
    assert_eq!(viewport.vertical_range().maximum, 250.0);
    assert_eq!(viewport.vertical_range().extent, 100.0);
    assert_eq!(
        runtime.tree().figure_bounds(scalable.figure_id()),
        Some(Rectangle::new(0.0, 0.0, 150.0, 100.0))
    );
    assert_eq!((60.0 - viewport.view_location().x()) * zoom.zoom(), 60.0);
    assert_eq!((40.0 - viewport.view_location().y()) * zoom.zoom(), 40.0);
}

#[test]
fn freeform_fit_uses_derived_extent_instead_of_presentation_bounds() {
    let (mut runtime, viewport, scalable, _) =
        scalable_freeform_viewport(Rectangle::new(-100.0, -50.0, 500.0, 300.0));
    let zoom = ZoomManager::new(scalable, viewport.clone());

    assert!(runtime.zoom(&zoom).unwrap().fit_all().unwrap());

    assert_eq!(zoom.zoom(), 0.6);
    assert_eq!(viewport.view_location(), Point::new(-100.0, -50.0));
}

#[test]
fn freeform_extent_shrink_clamps_origin_and_repaints_viewport() {
    let (mut runtime, viewport, _, layer) =
        scalable_freeform_viewport(Rectangle::new(-100.0, -50.0, 1000.0, 600.0));
    runtime
        .viewport(viewport.figure_id())
        .unwrap()
        .set_view_location(600.0, 350.0)
        .unwrap();
    runtime.prepare_frame();

    assert!(
        runtime
            .figure(layer)
            .unwrap()
            .set_bounds(Rectangle::new(-20.0, -10.0, 100.0, 80.0))
            .expect("valid Runtime mutation")
    );
    let canvas = runtime.prepare_frame().unwrap();

    assert_eq!(viewport.view_location(), Point::new(0.0, 0.0));
    assert!(canvas.damage().union().is_some());
}
