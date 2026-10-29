use novadraw::figure::RectangleFigure;
use novadraw::geometry::Rectangle;
use novadraw::layout::LayoutError;
use novadraw::runtime::{Runtime, StableQueryError};
use novadraw::tree::{FigureId, FigureTree, FreeformError, TreeQueryError};

mod support;

fn assert_identity_queries(tree: &FigureTree, id: FigureId, error: TreeQueryError) {
    macro_rules! check {
        ($($query:ident),+ $(,)?) => {$(
            assert_eq!(tree.$query(id).err(), Some(error), stringify!($query));
        )+};
    }
    check!(
        node,
        figure_bounds,
        component_revision,
        child_order,
        parent_id,
        depth,
        is_attached,
        is_visible,
        is_enabled,
        is_opaque,
        insets,
        figure_style,
        resolved_style,
        border_is_effectively_opaque,
        is_effectively_visible,
        is_effectively_enabled,
        is_focusable,
        is_focus_traversable,
        can_request_focus,
        can_traverse_focus,
        is_connection_figure,
        connection_route_points,
        connection_stroke_color,
        point_list_points,
        point_list_stroke_style,
        child_clipping_strategy,
        is_valid,
        ancestor_ids,
        descendant_ids,
        layout_manager,
    );
    assert!(!tree.contains(id));
    assert_eq!(tree.viewport_handle(id).err(), Some(error));
    let capability_error = match error {
        TreeQueryError::ForeignFigure(id) => {
            novadraw::figure::CapabilityQueryError::ForeignRuntime(id)
        }
        TreeQueryError::UnknownFigure(id) => {
            novadraw::figure::CapabilityQueryError::UnknownOrDisposedFigure(id)
        }
    };
    assert_eq!(tree.scale_handle(id).err(), Some(capability_error));
    let shape_error = match error {
        TreeQueryError::ForeignFigure(id) => {
            novadraw::figure::ShapeMutationError::ForeignFigure(id)
        }
        TreeQueryError::UnknownFigure(id) => {
            novadraw::figure::ShapeMutationError::UnknownFigure(id)
        }
    };
    assert_eq!(tree.image_display_state(id).err(), Some(shape_error));
    assert_eq!(
        tree.layout_constraint::<Rectangle>(id),
        Err(LayoutError::Identity(error))
    );
    let freeform = match error {
        TreeQueryError::ForeignFigure(id) => FreeformError::ForeignFigure(id),
        TreeQueryError::UnknownFigure(id) => FreeformError::UnknownFigure(id),
    };
    assert_eq!(tree.freeform_extent(id), Err(freeform));
    assert_eq!(tree.freeform_extent_excluding(id, &[]), Err(freeform));
}

fn runtime() -> (Runtime, FigureId) {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::from_bounds(Rectangle::ZERO).expect("valid figure bounds"),
        ))
        .unwrap();
    (runtime, root)
}

#[test]
fn arbitrary_tree_facts_reject_foreign_and_retired_generation() {
    let (mut owner, root) = runtime();
    let retired = owner
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::from_bounds(Rectangle::ZERO).expect("valid figure bounds"),
        ))
        .unwrap();
    owner.dispose_subtree(retired).unwrap();
    let replacement = owner
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::from_bounds(Rectangle::ZERO).expect("valid figure bounds"),
        ))
        .unwrap();
    let (_other, foreign) = runtime();
    assert_ne!(retired, replacement);
    assert_identity_queries(
        owner.tree(),
        foreign,
        TreeQueryError::ForeignFigure(foreign),
    );
    assert_identity_queries(
        owner.tree(),
        retired,
        TreeQueryError::UnknownFigure(retired),
    );
    assert_eq!(owner.tree().figure_bounds(replacement), Ok(Rectangle::ZERO));
    for id in [foreign, retired] {
        let error = if id == foreign {
            TreeQueryError::ForeignFigure(id)
        } else {
            TreeQueryError::UnknownFigure(id)
        };
        assert_eq!(owner.tree().child_z_index(root, id), Err(error));
        assert_eq!(owner.tree().child_z_index(id, root), Err(error));
        assert_eq!(owner.tree().is_ancestor_of(root, id), Err(error));
        assert_eq!(owner.tree().is_ancestor_of(id, root), Err(error));
        // Equality must not bypass either identity check.
        assert_eq!(owner.tree().is_ancestor_of(id, id), Err(error));
    }
}

#[test]
fn valid_zero_absence_and_false_remain_distinct_from_identity_failure() {
    let (mut owner, root) = runtime();
    let leaf = owner
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::from_bounds(Rectangle::ZERO).expect("valid figure bounds"),
        ))
        .unwrap();
    let sibling = owner
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::from_bounds(Rectangle::ZERO).expect("valid figure bounds"),
        ))
        .unwrap();
    owner.figure(root).unwrap().set_visible(false).unwrap();
    owner.figure(root).unwrap().set_enabled(false).unwrap();
    let tree = owner.tree();
    let synthetic = tree.parent_id(root).unwrap().unwrap();
    assert_eq!(tree.parent_id(synthetic), Ok(None));
    assert_eq!(tree.child_order(leaf), Ok(vec![]));
    assert_eq!(tree.figure_bounds(leaf), Ok(Rectangle::ZERO));
    assert_eq!(tree.is_visible(leaf), Ok(true));
    assert_eq!(tree.is_effectively_visible(leaf), Ok(false));
    assert_eq!(tree.is_effectively_enabled(leaf), Ok(false));
    assert_eq!(tree.can_request_focus(leaf), Ok(false));
    assert_eq!(tree.can_traverse_focus(leaf), Ok(false));
    assert_eq!(tree.is_opaque(leaf), Ok(false));
    assert_eq!(tree.child_z_index(leaf, sibling), Ok(None));
    assert_eq!(tree.layout_constraint::<Rectangle>(leaf), Ok(None));
    assert!(tree.layout_manager(leaf).unwrap().is_none());
    assert_eq!(tree.connection_route_points(leaf), Ok(None));
    assert_eq!(tree.point_list_points(leaf), Ok(None));
    assert!(tree.viewport_handle(leaf).unwrap().is_none());
    assert!(tree.scale_handle(leaf).unwrap().is_none());
    // A borrowed node reads committed local values, independent of inherited visibility.
    assert!(tree.node(leaf).unwrap().state().is_visible());
}

#[test]
fn saved_viewport_handle_checks_its_owner_on_contents_queries() {
    let (mut owner, root) = runtime();
    let viewport = owner.viewports().add(root, Rectangle::ZERO).unwrap();
    assert_eq!(viewport.contents(owner.tree()), Ok(None));
    let (other, _) = runtime();
    assert_eq!(
        viewport.contents(other.tree()),
        Err(TreeQueryError::ForeignFigure(viewport.figure_id()))
    );
    owner.dispose_subtree(viewport.figure_id()).unwrap();
    assert_eq!(
        viewport.contents(owner.tree()),
        Err(TreeQueryError::UnknownFigure(viewport.figure_id()))
    );
}

#[test]
fn stable_query_revalidates_ids_and_owned_values_keep_their_original_epoch() {
    let (mut owner, root) = runtime();
    let leaf = owner
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(1.0, 2.0, 3.0, 4.0).expect("valid figure bounds"),
        ))
        .unwrap();
    assert!(matches!(
        owner.observations().stable_query(),
        Err(StableQueryError::NotStable { .. })
    ));
    owner.frames().stabilize_for_query().unwrap();
    support::prepare_next_submission(&mut owner).unwrap();
    let (_other, foreign) = runtime();
    let (epoch, saved) = {
        let observations = owner.observations();
        let stable = observations.stable_query().unwrap();
        assert_eq!(
            stable.is_attached(foreign),
            Err(TreeQueryError::ForeignFigure(foreign))
        );
        assert_eq!(
            stable.bounds(foreign),
            Err(TreeQueryError::ForeignFigure(foreign))
        );
        (stable.epoch(), stable.bounds(leaf).unwrap())
    };
    owner.dispose_subtree(leaf).unwrap();
    owner.frames().stabilize_for_query().unwrap();
    support::prepare_next_submission(&mut owner).unwrap();
    let observations = owner.observations();
    let stable = observations.stable_query().unwrap();
    assert!(stable.epoch() > epoch);
    assert_eq!(saved, Rectangle::new(1.0, 2.0, 3.0, 4.0));
    assert_eq!(
        stable.bounds(leaf),
        Err(TreeQueryError::UnknownFigure(leaf))
    );
    assert_eq!(
        stable.is_attached(leaf),
        Err(TreeQueryError::UnknownFigure(leaf))
    );
}
