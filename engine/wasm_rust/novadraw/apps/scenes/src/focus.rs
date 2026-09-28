use novadraw::{Color, Figure, FigureId, FigureTree, Rectangle, RectangleFigure};

pub const FOCUS_TRAVERSAL_SCENE_TITLE: &str = "Focus traversal tree";

#[derive(Debug, Clone, Copy)]
pub struct FocusProbeSpec {
    pub label: &'static str,
    pub bounds: Rectangle,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct FocusTraversalSceneIds {
    pub root: FigureId,
    pub a: FigureId,
    pub group: FigureId,
    pub b: FigureId,
    pub skip: FigureId,
    pub c: FigureId,
    pub d: FigureId,
}

pub fn build_focus_traversal_scene(
    mut make_probe: impl FnMut(FocusProbeSpec) -> Box<dyn Figure>,
) -> (FigureTree, FocusTraversalSceneIds) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            800.0,
            500.0,
            Color::from_hex("#eef1f4").expect("valid color literal"),
        )));
    let a = tree
        .builder()
        .add_child(
            root,
            make_probe(FocusProbeSpec {
                label: "A · 1",
                bounds: Rectangle::new(50.0, 80.0, 130.0, 90.0),
                enabled: true,
            }),
        )
        .expect("valid FigureTree construction");
    let group = tree
        .builder()
        .add_child(
            root,
            Box::new(RectangleFigure::new_with_color(
                220.0,
                45.0,
                360.0,
                250.0,
                Color::from_hex("#dfe6e9").expect("valid color literal"),
            )),
        )
        .expect("valid FigureTree construction");
    let b = tree
        .builder()
        .add_child(
            group,
            make_probe(FocusProbeSpec {
                label: "B · 2",
                bounds: Rectangle::new(30.0, 70.0, 90.0, 80.0),
                enabled: true,
            }),
        )
        .expect("valid FigureTree construction");
    let skip = tree
        .builder()
        .add_child(
            group,
            make_probe(FocusProbeSpec {
                label: "Skip",
                bounds: Rectangle::new(135.0, 70.0, 90.0, 80.0),
                enabled: false,
            }),
        )
        .expect("valid FigureTree construction");
    let c = tree
        .builder()
        .add_child(
            group,
            make_probe(FocusProbeSpec {
                label: "C · 3",
                bounds: Rectangle::new(240.0, 70.0, 90.0, 80.0),
                enabled: true,
            }),
        )
        .expect("valid FigureTree construction");
    let d = tree
        .builder()
        .add_child(
            root,
            make_probe(FocusProbeSpec {
                label: "D · 4",
                bounds: Rectangle::new(620.0, 80.0, 130.0, 90.0),
                enabled: true,
            }),
        )
        .expect("valid FigureTree construction");

    for id in [a, b, c, d] {
        tree.builder()
            .set_focusable(id, true)
            .expect("valid FigureTree construction");
        tree.builder()
            .set_focus_traversable(id, true)
            .expect("valid FigureTree construction");
    }
    tree.builder()
        .set_focus_traversable(skip, true)
        .expect("valid FigureTree construction");
    tree.builder()
        .set_enabled(skip, false)
        .expect("valid FigureTree construction");

    (
        tree,
        FocusTraversalSceneIds {
            root,
            a,
            group,
            b,
            skip,
            c,
            d,
        },
    )
}

#[cfg(test)]
mod tests {
    use novadraw::event::{FocusTraversalDirection, FocusTraversalPolicy, TreeOrderFocusTraversal};

    use super::*;

    #[test]
    fn shared_focus_scene_has_stable_nested_traversal_order() {
        let (tree, ids) = build_focus_traversal_scene(|spec| {
            Box::new(RectangleFigure::new(
                spec.bounds.x,
                spec.bounds.y,
                spec.bounds.width,
                spec.bounds.height,
            ))
        });
        let mut policy = TreeOrderFocusTraversal;

        assert_eq!(
            tree.child_order(ids.root),
            Some(vec![ids.a, ids.group, ids.d])
        );
        assert_eq!(
            tree.child_order(ids.group),
            Some(vec![ids.b, ids.skip, ids.c])
        );
        assert!(!tree.is_focus_traversable(ids.group));
        assert!(tree.is_focus_traversable(ids.skip));
        assert!(!tree.is_effectively_enabled(ids.skip));

        let mut current = None;
        for expected in [ids.a, ids.b, ids.c, ids.d] {
            current = policy.traverse(&tree, ids.root, current, FocusTraversalDirection::Forward);
            assert_eq!(current, Some(expected));
        }
        assert_eq!(
            policy.traverse(&tree, ids.root, current, FocusTraversalDirection::Forward,),
            None
        );

        current = None;
        for expected in [ids.d, ids.c, ids.b, ids.a] {
            current = policy.traverse(&tree, ids.root, current, FocusTraversalDirection::Backward);
            assert_eq!(current, Some(expected));
        }
        assert_eq!(
            policy.traverse(&tree, ids.root, current, FocusTraversalDirection::Backward,),
            None
        );
    }
}
