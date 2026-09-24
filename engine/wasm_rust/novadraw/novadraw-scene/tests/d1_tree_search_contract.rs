use novadraw_scene::{ExclusionSearch, FigureTree, RectangleFigure, TreeSearch, TreeSearchContext};

struct NamedFigureSearch {
    name: &'static str,
}

impl TreeSearch for NamedFigureSearch {
    fn accept(&mut self, candidate: TreeSearchContext<'_>) -> bool {
        candidate.figure().name() == self.name
    }
}

#[test]
fn public_search_strategy_filters_hit_test_without_changing_geometry_order() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    let child = tree
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(10.0, 10.0, 40.0, 40.0)))
        .expect("valid FigureTree construction");

    let hit = tree.hit_test_with(
        (20.0, 20.0),
        &mut NamedFigureSearch {
            name: "RectangleFigure",
        },
    );

    assert_eq!(hit.map(|(target, _)| target), Some(child));
}

#[test]
fn public_exclusion_search_prunes_by_figure_id() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    let child = tree
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(10.0, 10.0, 40.0, 40.0)))
        .expect("valid FigureTree construction");
    let mut search = ExclusionSearch::new([child]);

    assert_eq!(
        tree.hit_test_with((20.0, 20.0), &mut search)
            .map(|(target, _)| target),
        Some(root)
    );
    assert_eq!(tree.ancestor_ids(child), Some(vec![root]));
    assert_eq!(tree.descendant_ids(root), Some(vec![child]));
    assert!(tree.is_ancestor_of(root, child));
}
