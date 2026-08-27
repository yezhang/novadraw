use std::sync::Arc;

use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use slotmap::Key;

use crate::{
    BlockId, BorderConstraint, BorderRegion, FigureGraph, GraphMutationError, MAX_TREE_DEPTH,
    PendingMutations, RectangleFigure, ScalableLayeredPaneFigure, SceneUpdateManager,
    ViewportFigure, XYConstraint, XYLayout,
    mutation::{PendingMutation, PendingMutationKind},
};

fn new_scene() -> (FigureGraph, SceneUpdateManager) {
    (FigureGraph::new(), SceneUpdateManager::new())
}

#[test]
fn test_add_child_marks_layout_invalid() {
    let (mut scene, _) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.validate();
    scene.add_child_to(
        container_id,
        Box::new(RectangleFigure::new(0.0, 0.0, 50.0, 50.0)),
    );
    assert!(!scene.is_layout_valid());
}

#[test]
fn test_mark_invalid_adds_to_queue() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    assert!(!update_manager.is_update_queued());
    scene.mark_invalid(&mut update_manager, container_id);
    assert!(update_manager.is_update_queued());
    assert!(update_manager.has_pending_layout());
}

#[test]
fn test_repaint_adds_dirty_region() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    assert!(!update_manager.has_pending_repaint());
    scene.repaint(&mut update_manager, container_id, None);
    assert!(update_manager.has_pending_repaint());
    assert_eq!(update_manager.dirty_count(), 1);
}

#[test]
fn test_repaint_uses_specified_rect() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let partial_rect = Rectangle::new(10.0, 10.0, 50.0, 50.0);
    scene.repaint(&mut update_manager, container_id, Some(partial_rect));
    let damage = update_manager.compute_damage();
    assert_eq!(damage, partial_rect);
}

#[test]
fn test_multiple_repaints_merge_regions() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.repaint(
        &mut update_manager,
        container_id,
        Some(Rectangle::new(0.0, 0.0, 50.0, 50.0)),
    );
    scene.repaint(
        &mut update_manager,
        container_id,
        Some(Rectangle::new(100.0, 100.0, 50.0, 50.0)),
    );
    let damage = update_manager.compute_damage();
    assert_eq!(damage.x, 0.0);
    assert_eq!(damage.y, 0.0);
    assert_eq!(damage.width, 150.0);
    assert_eq!(damage.height, 150.0);
}

#[test]
fn test_invisible_block_no_dirty_region() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.blocks.get_mut(container_id).unwrap().is_visible = false;
    scene.repaint(&mut update_manager, container_id, None);
    assert!(!update_manager.has_pending_repaint());
}

#[test]
fn test_effective_visibility_follows_parent_chain() {
    let (mut scene, _) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let child_id = scene.add_child_to(
        parent_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );

    assert!(scene.is_visible(child_id));
    assert!(scene.is_effectively_visible(child_id));

    assert!(scene.set_visible(parent_id, false));
    assert!(scene.is_visible(child_id));
    assert!(!scene.is_effectively_visible(child_id));
    assert!(!scene.set_visible(parent_id, false));

    assert!(scene.set_visible(parent_id, true));
    assert!(scene.is_effectively_visible(child_id));
}

#[test]
fn test_effective_enabled_follows_parent_chain() {
    let (mut scene, _) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let child_id = scene.add_child_to(
        parent_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );

    assert!(scene.is_enabled(child_id));
    assert!(scene.is_effectively_enabled(child_id));

    assert!(scene.set_enabled(parent_id, false));
    assert!(scene.is_enabled(child_id));
    assert!(!scene.is_effectively_enabled(child_id));
    assert!(!scene.set_enabled(parent_id, false));

    assert!(scene.set_enabled(parent_id, true));
    assert!(scene.is_effectively_enabled(child_id));
}

#[test]
fn test_repaint_skips_effectively_invisible_child() {
    let (mut scene, mut update_manager) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let child_id = scene.add_child_to(
        parent_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );

    scene.set_visible(parent_id, false);
    scene.repaint(&mut update_manager, child_id, None);

    assert!(!update_manager.has_pending_repaint());
}

#[test]
fn test_hidden_parent_skips_child_validation_but_drains_queue() {
    let (mut scene, mut update_manager) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let child_id = scene.add_child_to(
        parent_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );

    scene.set_visible(parent_id, false);
    scene.mark_invalid(&mut update_manager, child_id);
    scene.perform_update(&mut update_manager);

    assert!(!update_manager.has_pending_layout());
    assert!(!update_manager.is_update_queued());
    assert!(!scene.get_block(child_id).unwrap().is_valid);
}

#[test]
fn test_disabled_parent_skips_child_validation_but_drains_queue() {
    let (mut scene, mut update_manager) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let child_id = scene.add_child_to(
        parent_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );

    scene.set_enabled(parent_id, false);
    scene.mark_invalid(&mut update_manager, child_id);
    scene.perform_update(&mut update_manager);

    assert!(!update_manager.has_pending_layout());
    assert!(!update_manager.is_update_queued());
    assert!(!scene.get_block(child_id).unwrap().is_valid);
}

#[test]
fn test_showing_hidden_subtree_requeues_deferred_validation() {
    let (mut scene, mut update_manager) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let child_id = scene.add_child_to(
        parent_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );
    scene.set_visible(parent_id, false);
    scene.mark_invalid(&mut update_manager, child_id);
    scene.perform_update(&mut update_manager);
    assert!(!scene.is_valid(child_id));

    assert!(scene.set_visible_with_update(&mut update_manager, parent_id, true));
    scene.perform_update(&mut update_manager);

    assert!(scene.is_valid(parent_id));
    assert!(scene.is_valid(child_id));
    assert!(!update_manager.is_update_queued());
}

#[test]
fn test_enabling_disabled_subtree_requeues_deferred_validation() {
    let (mut scene, mut update_manager) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let child_id = scene.add_child_to(
        parent_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );
    scene.set_enabled(parent_id, false);
    scene.mark_invalid(&mut update_manager, child_id);
    scene.perform_update(&mut update_manager);
    assert!(!scene.is_valid(child_id));

    assert!(scene.set_enabled_with_update(&mut update_manager, parent_id, true));
    scene.perform_update(&mut update_manager);

    assert!(scene.is_valid(parent_id));
    assert!(scene.is_valid(child_id));
    assert!(!update_manager.is_update_queued());
}

#[test]
fn test_perform_update_two_phase() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.mark_invalid(&mut update_manager, container_id);
    scene.repaint(&mut update_manager, container_id, None);
    let canvas = scene.perform_update(&mut update_manager);
    assert!(!update_manager.has_pending_layout());
    assert!(!update_manager.has_pending_repaint());
    assert!(!update_manager.is_update_queued());
    assert!(!canvas.commands().is_empty());
}

#[test]
fn test_perform_update_marks_layout_valid() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.mark_invalid(&mut update_manager, container_id);
    scene.perform_update(&mut update_manager);
    assert!(!update_manager.has_pending_layout());
    assert!(scene.is_layout_valid());
}

#[test]
fn test_clear_updates() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.mark_invalid(&mut update_manager, container_id);
    scene.repaint(&mut update_manager, container_id, None);
    assert!(update_manager.is_update_queued());
    update_manager.clear();
    assert!(!update_manager.is_update_queued());
}

#[test]
fn test_revalidate_flow() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.validate();
    scene.mark_invalid(&mut update_manager, container_id);
    scene.perform_update(&mut update_manager);
    assert!(scene.is_layout_valid());
}

#[test]
fn test_repaint_all() {
    let (mut scene, mut update_manager) = new_scene();
    scene.repaint_all(&mut update_manager);
    assert!(!update_manager.has_pending_repaint());
    scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.repaint_all(&mut update_manager);
    assert!(update_manager.has_pending_repaint());
}

#[test]
fn test_empty_rect_ignored() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.repaint(
        &mut update_manager,
        container_id,
        Some(Rectangle::new(0.0, 0.0, 0.0, 100.0)),
    );
    assert!(!update_manager.has_pending_repaint());
    scene.repaint(
        &mut update_manager,
        container_id,
        Some(Rectangle::new(0.0, 0.0, 100.0, 0.0)),
    );
    assert!(!update_manager.has_pending_repaint());
    scene.repaint(
        &mut update_manager,
        container_id,
        Some(Rectangle::new(-10.0, -10.0, 100.0, 100.0)),
    );
    assert!(update_manager.has_pending_repaint());
}

#[test]
fn test_multiple_blocks_independent_dirty_regions() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let child1_id = scene.add_child_to(
        container_id,
        Box::new(RectangleFigure::new(0.0, 0.0, 50.0, 50.0)),
    );
    let child2_id = scene.add_child_to(
        container_id,
        Box::new(RectangleFigure::new(100.0, 0.0, 50.0, 50.0)),
    );
    scene.repaint(
        &mut update_manager,
        child1_id,
        Some(Rectangle::new(0.0, 0.0, 25.0, 25.0)),
    );
    scene.repaint(
        &mut update_manager,
        child2_id,
        Some(Rectangle::new(100.0, 0.0, 25.0, 25.0)),
    );
    assert_eq!(update_manager.dirty_count(), 2);
    let damage = update_manager.compute_damage();
    assert_eq!(damage.x, 0.0);
    assert_eq!(damage.width, 125.0);
}

#[test]
fn test_mark_invalid_with_repaint() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.mark_invalid(&mut update_manager, container_id);
    scene.repaint(&mut update_manager, container_id, None);
    assert!(update_manager.has_pending_layout());
    assert!(update_manager.has_pending_repaint());
    assert!(update_manager.is_update_queued());
    scene.perform_update(&mut update_manager);
    assert!(!update_manager.has_pending_layout());
    assert!(!update_manager.has_pending_repaint());
    assert!(!update_manager.is_update_queued());
}

#[test]
fn test_add_child_triggers_updates() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    assert!(!update_manager.is_update_queued());
    scene.add_child(
        &mut update_manager,
        container_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );
    assert!(update_manager.has_pending_layout());
    assert!(update_manager.has_pending_repaint());
    assert!(update_manager.is_update_queued());
}

#[test]
fn test_add_child_under_coordinate_root_repair_uses_child_coordinate_domain() {
    let (mut scene, mut update_manager) = new_scene();
    let contents_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
    let coordinate_root_id = scene.add_child_to(
        contents_id,
        Box::new(
            RectangleFigure::new_with_color(100.0, 50.0, 200.0, 150.0, Color::WHITE)
                .with_local_coordinates(true),
        ),
    );

    scene.add_child(
        &mut update_manager,
        coordinate_root_id,
        Box::new(RectangleFigure::new(20.0, 30.0, 40.0, 40.0)),
    );

    let canvas = scene.perform_update(&mut update_manager);
    assert_eq!(
        canvas.damage().union(),
        Some(Rectangle::new(120.0, 80.0, 40.0, 40.0))
    );
}

#[test]
fn test_add_child_under_viewport_repair_uses_content_transform() {
    let (mut scene, mut update_manager) = new_scene();
    let contents_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
    let viewport_id = scene.add_child_to(
        contents_id,
        Box::new(ViewportFigure::new(100.0, 50.0, 200.0, 100.0).with_origin(40.0, 20.0)),
    );
    let scalable_id = scene.add_child_to(
        viewport_id,
        Box::new(ScalableLayeredPaneFigure::new(0.0, 0.0, 400.0, 300.0).with_scale(2.0)),
    );

    scene.add_child(
        &mut update_manager,
        scalable_id,
        Box::new(RectangleFigure::new(30.0, 20.0, 40.0, 40.0)),
    );

    let canvas = scene.perform_update(&mut update_manager);
    assert_eq!(
        canvas.damage().union(),
        Some(Rectangle::new(120.0, 70.0, 80.0, 80.0))
    );
}

#[test]
fn test_layout_repositions_descendants_via_set_bounds_protocol() {
    let (mut scene, mut update_manager) = new_scene();
    let contents_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
    let container_id = scene.add_child_to(
        contents_id,
        Box::new(RectangleFigure::new(50.0, 50.0, 200.0, 150.0)),
    );
    let child_id = scene.add_child_to(
        container_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 40.0, 40.0)),
    );
    let grandchild_id = scene.add_child_to(
        child_id,
        Box::new(RectangleFigure::new(15.0, 15.0, 10.0, 10.0)),
    );

    scene.set_block_layout_manager(container_id, Arc::new(XYLayout::new()));
    scene.set_constraint(child_id, Rectangle::new(30.0, 40.0, 40.0, 40.0));
    scene.mark_invalid(&mut update_manager, container_id);
    scene.perform_update(&mut update_manager);

    let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();
    let grandchild_bounds = scene.blocks.get(grandchild_id).unwrap().figure_bounds();
    assert_eq!(child_bounds, Rectangle::new(80.0, 90.0, 40.0, 40.0));
    assert_eq!(grandchild_bounds, Rectangle::new(85.0, 95.0, 10.0, 10.0));
}

#[test]
fn test_layout_constraint_is_owned_by_parent_and_typed() {
    let (mut scene, _) = new_scene();
    let parent = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let child = scene.add_child_to(parent, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));

    assert!(scene.set_constraint(child, XYConstraint::at_size(10.0, 20.0, 30.0, 40.0)));
    assert_eq!(
        scene.get_constraint::<XYConstraint>(child),
        Some(&XYConstraint::at_size(10.0, 20.0, 30.0, 40.0))
    );
    assert!(scene.get_constraint::<BorderConstraint>(child).is_none());
}

#[test]
fn test_reparent_removes_constraint_owned_by_old_parent() {
    let (mut scene, mut update_manager) = new_scene();
    let root = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)));
    let left = scene.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    let right = scene.add_child_to(
        root,
        Box::new(RectangleFigure::new(150.0, 0.0, 100.0, 100.0)),
    );
    let child = scene.add_child_to(left, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
    assert!(scene.set_constraint(child, BorderConstraint::with_size(BorderRegion::West, 20.0)));

    let changed = scene.apply_reparent_mutation(
        &mut update_manager,
        PendingMutationKind::Reparent {
            child,
            new_parent: right,
        },
    );

    assert!(changed);
    assert!(scene.get_constraint::<BorderConstraint>(child).is_none());
}

#[test]
fn test_layout_uses_local_client_area_for_coordinate_root_container() {
    let (mut scene, mut update_manager) = new_scene();
    let contents_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
    let container_id = scene.add_child_to(
        contents_id,
        Box::new(
            RectangleFigure::new_with_color(100.0, 50.0, 200.0, 150.0, Color::WHITE)
                .with_local_coordinates(true),
        ),
    );
    let child_id = scene.add_child_to(
        container_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 40.0, 40.0)),
    );

    scene.set_block_layout_manager(container_id, Arc::new(XYLayout::new()));
    scene.set_constraint(child_id, Rectangle::new(20.0, 30.0, 40.0, 40.0));
    scene.mark_invalid(&mut update_manager, container_id);
    scene.perform_update(&mut update_manager);

    let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();
    assert_eq!(child_bounds, Rectangle::new(20.0, 30.0, 40.0, 40.0));
}

#[test]
fn test_validation_promotes_queued_child_to_highest_invalid_ancestor() {
    let (mut scene, mut update_manager) = new_scene();
    let root = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
    let container = scene.add_child_to(
        root,
        Box::new(RectangleFigure::new(50.0, 50.0, 200.0, 150.0)),
    );
    let child = scene.add_child_to(
        container,
        Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)),
    );
    scene.set_block_layout_manager(container, Arc::new(XYLayout::new()));
    assert!(scene.set_constraint(child, XYConstraint::at_size(10.0, 20.0, 30.0, 40.0)));
    scene.mark_invalid(&mut update_manager, root);
    scene.perform_update(&mut update_manager);

    assert!(scene.set_constraint(child, XYConstraint::at_size(30.0, 40.0, 50.0, 60.0)));
    update_manager.add_invalid_figure(child);
    let canvas = scene.perform_update(&mut update_manager);

    assert_eq!(
        scene.figure_bounds(child),
        Some(Rectangle::new(80.0, 90.0, 50.0, 60.0))
    );
    assert!(scene.is_valid(root));
    assert!(scene.is_valid(container));
    assert!(scene.is_valid(child));
    assert_eq!(
        canvas.damage().union(),
        Some(Rectangle::new(60.0, 70.0, 70.0, 80.0))
    );
}

#[test]
fn test_explicit_size_overrides_take_precedence() {
    let (mut scene, _) = new_scene();
    let block = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 30.0, 40.0)));

    assert_eq!(scene.preferred_size(block, -1.0, -1.0), Some((30.0, 40.0)));
    assert_eq!(scene.minimum_size(block, -1.0, -1.0), Some((30.0, 40.0)));
    let maximum = scene.maximum_size(block).expect("default maximum size");
    assert!(maximum.0 > 30.0);
    assert!(maximum.1 > 40.0);

    assert!(scene.set_preferred_size(block, Some((50.0, 60.0))));
    assert!(scene.set_minimum_size(block, Some((10.0, 20.0))));
    assert!(scene.set_maximum_size(block, Some((100.0, 120.0))));
    assert_eq!(scene.preferred_size(block, -1.0, -1.0), Some((50.0, 60.0)));
    assert_eq!(scene.minimum_size(block, -1.0, -1.0), Some((10.0, 20.0)));
    assert_eq!(scene.maximum_size(block), Some((100.0, 120.0)));
}

#[test]
fn test_perform_update_clears_dirty_regions() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.repaint(
        &mut update_manager,
        container_id,
        Some(Rectangle::new(10.0, 10.0, 50.0, 50.0)),
    );
    assert!(update_manager.has_pending_repaint());
    scene.perform_update(&mut update_manager);
    assert!(!update_manager.has_pending_repaint());
}

#[test]
fn test_perform_update_uses_damage_region() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.repaint(
        &mut update_manager,
        container_id,
        Some(Rectangle::new(10.0, 10.0, 50.0, 50.0)),
    );
    let render_ctx = scene.perform_update(&mut update_manager);
    assert!(!render_ctx.commands().is_empty());
}

#[test]
fn test_coordinate_root_move_repairs_old_and_new_parent_regions() {
    let (mut scene, mut update_manager) = new_scene();
    let contents_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 240.0)));
    let coordinate_root_id = scene.add_child_to(
        contents_id,
        Box::new(RectangleFigure::new(50.0, 40.0, 80.0, 60.0).with_local_coordinates(true)),
    );
    let child_id = scene.add_child_to(
        coordinate_root_id,
        Box::new(RectangleFigure::new(10.0, 15.0, 20.0, 10.0)),
    );

    assert!(scene.set_bounds_with_update(
        &mut update_manager,
        coordinate_root_id,
        70.0,
        55.0,
        80.0,
        60.0,
    ));

    assert_eq!(
        scene.get_block(child_id).unwrap().figure_bounds(),
        Rectangle::new(10.0, 15.0, 20.0, 10.0)
    );

    let canvas = scene.perform_update(&mut update_manager);
    assert_eq!(
        canvas.damage().union(),
        Some(Rectangle::new(50.0, 40.0, 100.0, 75.0))
    );
}

#[test]
fn test_batch_construction_no_updates() {
    let (mut scene, update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.add_child_to(
        container_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );
    scene.add_child_to(
        container_id,
        Box::new(RectangleFigure::new(100.0, 10.0, 50.0, 50.0)),
    );
    assert!(!update_manager.is_update_queued());
}

#[test]
fn test_batch_then_manual_update() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.add_child_to(
        container_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );
    scene.mark_invalid(&mut update_manager, container_id);
    scene.repaint(&mut update_manager, container_id, None);
    assert!(update_manager.is_update_queued());
    scene.perform_update(&mut update_manager);
    assert!(!update_manager.is_update_queued());
}

#[test]
fn test_mark_invalid_updates_block_validity() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.validate();
    scene.mark_invalid(&mut update_manager, container_id);
    assert!(!scene.get_block(container_id).unwrap().is_valid);
    assert!(!scene.is_layout_valid());
}

#[test]
fn test_hidden_block_skips_validation_but_drains_queue() {
    let (mut scene, mut update_manager) = new_scene();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    scene.validate();
    scene.blocks.get_mut(container_id).unwrap().is_visible = false;

    scene.mark_invalid(&mut update_manager, container_id);
    scene.perform_update(&mut update_manager);

    assert!(!update_manager.has_pending_layout());
    assert!(!update_manager.is_update_queued());
    assert!(!scene.get_block(container_id).unwrap().is_valid);
}

#[test]
fn test_interaction_state_accessors() {
    let mut scene = FigureGraph::new();
    let container_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    assert_eq!(scene.mouse_target(), None);
    assert_eq!(scene.focus_owner(), None);
    assert_eq!(scene.captured(), None);
    assert!(!scene.is_hovered(container_id));
    assert!(!scene.is_pressed(container_id));
    scene.set_mouse_target(Some(container_id));
    scene.set_focus_owner(Some(container_id));
    scene.set_captured(Some(container_id));
    scene.set_hovered(container_id, true);
    scene.set_pressed(container_id, true);
    assert_eq!(scene.mouse_target(), Some(container_id));
    assert_eq!(scene.focus_owner(), Some(container_id));
    assert_eq!(scene.captured(), Some(container_id));
    assert!(scene.is_hovered(container_id));
    assert!(scene.is_pressed(container_id));
}

#[test]
fn test_apply_pending_add_child_figure_allocates_and_attaches_child() {
    let (mut scene, mut update_manager) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let mut pending_mutations = PendingMutations::new();
    pending_mutations.enqueue(PendingMutation::add_child_figure(
        parent_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    ));

    assert!(scene.apply_pending_mutations(&mut update_manager, pending_mutations.drain(),));

    assert_eq!(scene.get_block(parent_id).unwrap().children_count(), 1);
    assert!(update_manager.has_pending_layout());
    assert!(update_manager.has_pending_repaint());
}

#[test]
fn test_apply_pending_add_child_with_invalid_parent_has_no_side_effect() {
    let (mut scene, mut update_manager) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let block_count = scene.blocks.len();
    let uuid_count = scene.uuid_map.len();
    let mut pending_mutations = PendingMutations::new();
    pending_mutations.enqueue(PendingMutation::add_child_figure(
        BlockId::null(),
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    ));

    assert!(!scene.apply_pending_mutations(&mut update_manager, pending_mutations.drain()));

    assert_eq!(scene.get_block(parent_id).unwrap().children_count(), 0);
    assert_eq!(scene.blocks.len(), block_count);
    assert_eq!(scene.uuid_map.len(), uuid_count);
}

#[test]
fn test_direct_add_child_to_invalid_parent_has_no_side_effect() {
    let (mut scene, _) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let block_count = scene.blocks.len();
    let uuid_count = scene.uuid_map.len();

    let child_id = scene.add_child_to(
        BlockId::null(),
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );

    assert_eq!(child_id, BlockId::null());
    assert_eq!(scene.get_block(parent_id).unwrap().children_count(), 0);
    assert_eq!(scene.blocks.len(), block_count);
    assert_eq!(scene.uuid_map.len(), uuid_count);
}

#[test]
fn test_try_add_child_to_invalid_parent_returns_error_without_side_effect() {
    let (mut scene, _) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let block_count = scene.blocks.len();
    let uuid_count = scene.uuid_map.len();

    let child_id = scene.try_add_child_to(
        BlockId::null(),
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );

    assert_eq!(child_id, Err(GraphMutationError::ParentNotFound));
    assert_eq!(scene.get_block(parent_id).unwrap().children_count(), 0);
    assert_eq!(scene.blocks.len(), block_count);
    assert_eq!(scene.uuid_map.len(), uuid_count);
}

#[test]
fn test_tree_depth_limit_accepts_boundary_and_rejects_next_level_atomically() {
    let mut scene = FigureGraph::new();
    let mut parent = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)));

    for expected_depth in 2..=MAX_TREE_DEPTH {
        parent = scene
            .try_add_child_to(parent, Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)))
            .expect("depth at the configured boundary must be accepted");
        assert_eq!(scene.block_depth(parent), Some(expected_depth));
    }

    let canvas = scene.render();
    assert!(canvas.damage().is_full());
    assert!(!canvas.commands().is_empty());

    let block_count = scene.blocks.len();
    let uuid_count = scene.uuid_map.len();
    let child_count = scene.get_block(parent).unwrap().children_count();
    let effects = scene.notification_effects().to_vec();

    let result = scene.try_add_child_to(parent, Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)));

    assert_eq!(
        result,
        Err(GraphMutationError::DepthLimitExceeded {
            limit: MAX_TREE_DEPTH
        })
    );
    assert_eq!(scene.blocks.len(), block_count);
    assert_eq!(scene.uuid_map.len(), uuid_count);
    assert_eq!(
        scene.get_block(parent).unwrap().children_count(),
        child_count
    );
    assert_eq!(scene.notification_effects(), effects);
}

#[test]
fn test_reparent_rejects_subtree_that_would_exceed_depth_limit_without_side_effects() {
    let (mut scene, mut update_manager) = new_scene();
    let contents = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)));
    let subtree = scene.add_child_to(contents, Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)));
    let subtree_child =
        scene.add_child_to(subtree, Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)));

    let mut deep_parent = contents;
    for _ in 2..MAX_TREE_DEPTH {
        deep_parent = scene.add_child_to(
            deep_parent,
            Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)),
        );
    }
    assert_eq!(scene.block_depth(deep_parent), Some(MAX_TREE_DEPTH - 1));

    let old_parent = scene.get_block(subtree).unwrap().parent;
    let old_subtree_depth = scene.block_depth(subtree);
    let old_child_depth = scene.block_depth(subtree_child);
    let changed = scene.apply_reparent_mutation(
        &mut update_manager,
        PendingMutationKind::Reparent {
            child: subtree,
            new_parent: deep_parent,
        },
    );

    assert!(!changed);
    assert_eq!(scene.get_block(subtree).unwrap().parent, old_parent);
    assert_eq!(scene.block_depth(subtree), old_subtree_depth);
    assert_eq!(scene.block_depth(subtree_child), old_child_depth);
    assert!(!update_manager.is_update_queued());
}

#[test]
fn test_direct_add_child_with_update_manager_invalid_parent_has_no_side_effect() {
    let (mut scene, mut update_manager) = new_scene();
    let _parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let block_count = scene.blocks.len();
    let uuid_count = scene.uuid_map.len();

    let child_id = scene.add_child(
        &mut update_manager,
        BlockId::null(),
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );

    assert_eq!(child_id, BlockId::null());
    assert_eq!(scene.blocks.len(), block_count);
    assert_eq!(scene.uuid_map.len(), uuid_count);
    assert!(!update_manager.is_update_queued());
    assert!(!update_manager.has_pending_layout());
    assert!(!update_manager.has_pending_repaint());
}

#[test]
fn test_apply_pending_remove_child_clears_interaction_state() {
    let (mut scene, mut update_manager) = new_scene();
    let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let child_id = scene.add_child_to(
        parent_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );
    scene.set_mouse_target(Some(child_id));
    scene.set_cursor_target(Some(child_id));
    scene.set_hover_source(Some(child_id));
    scene.set_focus_owner(Some(child_id));
    scene.set_captured(Some(child_id));
    scene.set_selected(Some(child_id));

    let mut pending_mutations = PendingMutations::new();
    pending_mutations.enqueue(PendingMutation::remove_child(parent_id, child_id));
    assert!(scene.apply_pending_mutations(&mut update_manager, pending_mutations.drain()));

    assert_eq!(scene.get_block(child_id).unwrap().parent, None);
    assert!(
        !scene
            .get_block(parent_id)
            .unwrap()
            .children
            .contains(&child_id)
    );
    assert_eq!(scene.mouse_target(), None);
    assert_eq!(scene.cursor_target(), None);
    assert_eq!(scene.hover_source(), None);
    assert_eq!(scene.focus_owner(), None);
    assert_eq!(scene.captured(), None);
    assert_eq!(scene.selected_block(), None);
}

#[test]
fn test_apply_pending_remove_child_with_wrong_parent_has_no_side_effects() {
    let (mut scene, mut update_manager) = new_scene();
    let root_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 400.0)));
    let left_id = scene.add_child_to(
        root_id,
        Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)),
    );
    let right_id = scene.add_child_to(
        root_id,
        Box::new(RectangleFigure::new(200.0, 0.0, 100.0, 100.0)),
    );
    let child_id = scene.add_child_to(
        left_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)),
    );
    scene.set_mouse_target(Some(child_id));
    scene.set_focus_owner(Some(child_id));
    scene.set_captured(Some(child_id));

    let mut pending_mutations = PendingMutations::new();
    pending_mutations.enqueue(PendingMutation::remove_child(right_id, child_id));

    assert!(!scene.apply_pending_mutations(&mut update_manager, pending_mutations.drain()));
    assert_eq!(scene.get_block(child_id).unwrap().parent, Some(left_id));
    assert!(
        scene
            .get_block(left_id)
            .unwrap()
            .children
            .contains(&child_id)
    );
    assert_eq!(scene.mouse_target(), Some(child_id));
    assert_eq!(scene.focus_owner(), Some(child_id));
    assert_eq!(scene.captured(), Some(child_id));
    assert!(!update_manager.is_update_queued());
    assert_eq!(update_manager.invalid_count(), 0);
    assert_eq!(update_manager.dirty_count(), 0);
}

#[test]
fn test_apply_pending_reparent_moves_child_between_containers() {
    let (mut scene, mut update_manager) = new_scene();
    let root_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 400.0)));
    let left_id = scene.add_child_to(
        root_id,
        Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)),
    );
    let right_id = scene.add_child_to(
        root_id,
        Box::new(RectangleFigure::new(200.0, 0.0, 100.0, 100.0)),
    );
    let child_id = scene.add_child_to(
        left_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)),
    );

    let mut pending_mutations = PendingMutations::new();
    pending_mutations.enqueue(PendingMutation::reparent(child_id, right_id));
    assert!(scene.apply_pending_mutations(&mut update_manager, pending_mutations.drain()));

    assert_eq!(scene.get_block(child_id).unwrap().parent, Some(right_id));
    assert!(
        !scene
            .get_block(left_id)
            .unwrap()
            .children
            .contains(&child_id)
    );
    assert!(
        scene
            .get_block(right_id)
            .unwrap()
            .children
            .contains(&child_id)
    );
}

#[test]
fn test_apply_pending_reparent_with_duplicate_new_parent_entry_has_no_side_effects() {
    let (mut scene, mut update_manager) = new_scene();
    let root_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 400.0)));
    let left_id = scene.add_child_to(
        root_id,
        Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)),
    );
    let right_id = scene.add_child_to(
        root_id,
        Box::new(RectangleFigure::new(200.0, 0.0, 100.0, 100.0)),
    );
    let child_id = scene.add_child_to(
        left_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)),
    );
    scene
        .blocks
        .get_mut(right_id)
        .unwrap()
        .children
        .push(child_id);

    let mut pending_mutations = PendingMutations::new();
    pending_mutations.enqueue(PendingMutation::reparent(child_id, right_id));

    assert!(!scene.apply_pending_mutations(&mut update_manager, pending_mutations.drain()));
    assert_eq!(scene.get_block(child_id).unwrap().parent, Some(left_id));
    assert!(
        scene
            .get_block(left_id)
            .unwrap()
            .children
            .contains(&child_id)
    );
    assert_eq!(
        scene
            .get_block(right_id)
            .unwrap()
            .children
            .iter()
            .filter(|&&id| id == child_id)
            .count(),
        1
    );
    assert!(!update_manager.is_update_queued());
    assert_eq!(update_manager.invalid_count(), 0);
    assert_eq!(update_manager.dirty_count(), 0);
}

#[test]
fn test_apply_pending_reparent_to_invalid_parent_keeps_original_tree() {
    let (mut scene, mut update_manager) = new_scene();
    let root_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 400.0)));
    let left_id = scene.add_child_to(
        root_id,
        Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)),
    );
    let child_id = scene.add_child_to(
        left_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)),
    );
    let mut pending_mutations = PendingMutations::new();
    pending_mutations.enqueue(PendingMutation::reparent(child_id, BlockId::null()));

    assert!(!scene.apply_pending_mutations(&mut update_manager, pending_mutations.drain()));

    assert_eq!(scene.get_block(child_id).unwrap().parent, Some(left_id));
    assert!(
        scene
            .get_block(left_id)
            .unwrap()
            .children
            .contains(&child_id)
    );
}

#[test]
fn test_apply_pending_reparent_to_self_keeps_original_tree() {
    let (mut scene, mut update_manager) = new_scene();
    let root_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 400.0)));
    let child_id = scene.add_child_to(
        root_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)),
    );
    let mut pending_mutations = PendingMutations::new();
    pending_mutations.enqueue(PendingMutation::reparent(child_id, child_id));

    assert!(!scene.apply_pending_mutations(&mut update_manager, pending_mutations.drain()));

    assert_eq!(scene.get_block(child_id).unwrap().parent, Some(root_id));
    assert!(
        scene
            .get_block(root_id)
            .unwrap()
            .children
            .contains(&child_id)
    );
}

#[test]
fn test_apply_pending_reparent_to_descendant_keeps_original_tree() {
    let (mut scene, mut update_manager) = new_scene();
    let root_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 400.0)));
    let parent_id = scene.add_child_to(
        root_id,
        Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)),
    );
    let child_id = scene.add_child_to(
        parent_id,
        Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)),
    );
    let grandchild_id = scene.add_child_to(
        child_id,
        Box::new(RectangleFigure::new(12.0, 12.0, 8.0, 8.0)),
    );
    let mut pending_mutations = PendingMutations::new();
    pending_mutations.enqueue(PendingMutation::reparent(child_id, grandchild_id));

    assert!(!scene.apply_pending_mutations(&mut update_manager, pending_mutations.drain()));

    assert_eq!(scene.get_block(child_id).unwrap().parent, Some(parent_id));
    assert_eq!(
        scene.get_block(grandchild_id).unwrap().parent,
        Some(child_id)
    );
    assert!(
        scene
            .get_block(parent_id)
            .unwrap()
            .children
            .contains(&child_id)
    );
    assert!(
        scene
            .get_block(child_id)
            .unwrap()
            .children
            .contains(&grandchild_id)
    );
}
