use slotmap::Key;

use novadraw_render::RenderCommandKind;
use novadraw_scene::{
    FREEFORM_EXTENT_PROPERTY, FigureTree, FreeformError, FreeformLayerFigure, NotificationEffect,
    PropertyValue, Rectangle, RectangleFigure, UpdateManager,
};

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

    tree.revalidate(host);

    assert_eq!(tree.freeform_extent(host), Ok(Rectangle::ZERO));
}

#[test]
fn freeform_extent_unions_positive_and_negative_child_bounds() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 100.0, 80.0)));
    tree.builder().add_child_to(
        host,
        Box::new(RectangleFigure::new(-40.0, -20.0, 10.0, 15.0)),
    );
    tree.builder()
        .add_child_to(host, Box::new(RectangleFigure::new(20.0, 30.0, 25.0, 10.0)));

    tree.revalidate(host);

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
    let inner = tree.builder().add_child_to(
        outer,
        Box::new(FreeformLayerFigure::new(100.0, 50.0, 500.0, 400.0)),
    );
    tree.builder().add_child_to(
        inner,
        Box::new(RectangleFigure::new(-20.0, -10.0, 30.0, 20.0)),
    );

    tree.revalidate(outer);

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
    let child = tree.builder().add_child_to(
        host,
        Box::new(RectangleFigure::new(-10.0, -5.0, 20.0, 10.0)),
    );
    tree.revalidate(host);
    tree.drain_notification_effects();

    tree.set_bounds(child, 30.0, 40.0, 20.0, 10.0);

    assert_eq!(
        tree.freeform_extent(host),
        Ok(Rectangle::new(-10.0, -5.0, 20.0, 10.0))
    );
    assert!(
        tree.get_block(host)
            .unwrap()
            .layout_state()
            .freeform_state()
            .unwrap()
            .is_dirty()
    );

    tree.revalidate(host);

    assert_eq!(
        tree.freeform_extent(host),
        Ok(Rectangle::new(30.0, 40.0, 20.0, 10.0))
    );
    let extent_events: Vec<_> = tree
        .drain_notification_effects()
        .into_iter()
        .filter_map(|effect| match effect {
            NotificationEffect::EmitProperty(event)
                if event.property == FREEFORM_EXTENT_PROPERTY =>
            {
                Some(event)
            }
            _ => None,
        })
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
    let child = tree.builder().add_child_to(
        host,
        Box::new(RectangleFigure::new(-25.0, -15.0, 20.0, 10.0)),
    );
    tree.set_visible(child, false);

    tree.revalidate(host);

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
    let child = tree.builder().add_child_to(
        host,
        Box::new(RectangleFigure::new(-40.0, -30.0, 15.0, 10.0)),
    );

    assert_eq!(tree.hit_test_simple((-35.0, -25.0)), Some(child));
}

#[test]
fn normal_ancestor_still_clips_freeform_overflow() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 80.0)));
    let freeform = tree.builder().add_child_to(
        root,
        Box::new(FreeformLayerFigure::new(10.0, 10.0, 20.0, 20.0)),
    );
    tree.builder().add_child_to(
        freeform,
        Box::new(RectangleFigure::new(120.0, 0.0, 15.0, 10.0)),
    );

    assert_eq!(tree.hit_test_simple((135.0, 15.0)), None);
}

#[test]
fn overflow_visible_skips_host_clip_during_rendering() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 100.0, 80.0)));
    tree.builder().add_child_to(
        host,
        Box::new(RectangleFigure::new(-40.0, -30.0, 15.0, 10.0)),
    );

    let canvas = tree.render();
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
fn overflow_visible_damage_is_not_clipped_to_host_bounds() {
    let mut tree = FigureTree::new();
    let host = tree
        .builder()
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 20.0, 20.0)));
    let child = tree.builder().add_child_to(
        host,
        Box::new(RectangleFigure::new(-40.0, -30.0, 15.0, 10.0)),
    );
    tree.revalidate(host);
    let mut updates = UpdateManager::new();
    tree.repaint(&mut updates, child, None);

    let canvas = tree.perform_update(&mut updates);

    assert_eq!(
        canvas.damage().union(),
        Some(Rectangle::new(-40.0, -30.0, 15.0, 10.0))
    );
}
