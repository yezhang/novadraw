use novadraw::render::{BackendCapabilities, BuiltinFont, RenderOutcome, SurfaceInfo};
use novadraw::{
    AccessibilityAction, AccessibilityNodeId, AccessibilityRole, AccessibilityUpdate,
    AccessibleFigure, ButtonFigure, Figure, FigureId, FigureTree, FramePreparation, MAX_TREE_DEPTH,
    Rectangle, RectangleFigure, Runtime, ToggleFigure,
};

struct AccessibleGroup;

impl Figure for AccessibleGroup {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 1.0, 1.0)
    }

    fn name(&self) -> &'static str {
        "AccessibleGroup"
    }

    fn accessible(&self) -> Option<&dyn AccessibleFigure> {
        Some(self)
    }
}

impl AccessibleFigure for AccessibleGroup {
    fn accessible_name(&self) -> Option<&str> {
        Some("group")
    }

    fn accessible_role(&self) -> AccessibilityRole {
        AccessibilityRole::Group
    }
}

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 320.0,
        logical_height: 200.0,
        pixel_width: 640,
        pixel_height: 400,
        scale_factor: 2.0,
    }
}

fn publish(runtime: &mut Runtime) {
    if let FramePreparation::Ready(submission) =
        runtime.prepare_submission(surface(), BackendCapabilities::FULL_FRAME_ONLY)
    {
        runtime.complete_submission(
            submission.session_id,
            submission.frame_id,
            RenderOutcome::Presented,
        );
    }
}

fn accessible_runtime() -> (Runtime, FigureId, FigureId) {
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 200.0)))
        .expect("valid Runtime mutation");
    let button = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            ButtonFigure::new("Apply").with_bounds(Rectangle::new(20.0, 30.0, 120.0, 40.0)),
        ))
        .expect("valid Runtime mutation");
    let toggle = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            ToggleFigure::new("Snap").with_bounds(Rectangle::new(160.0, 30.0, 120.0, 40.0)),
        ))
        .expect("valid Runtime mutation");
    (runtime, button, toggle)
}

#[test]
fn snapshot_promotes_accessible_children_through_decorative_figures() {
    let (mut runtime, button, toggle) = accessible_runtime();
    publish(&mut runtime);

    let updates = runtime.take_accessibility_updates();
    let AccessibilityUpdate::Snapshot(snapshot) = &updates[0] else {
        panic!("first accessibility publication must be a snapshot");
    };
    let root = AccessibilityNodeId::Root(runtime.tree().namespace());
    let root_node = snapshot.nodes.iter().find(|node| node.id == root).unwrap();
    assert_eq!(
        root_node.children,
        vec![
            AccessibilityNodeId::Figure(button),
            AccessibilityNodeId::Figure(toggle)
        ]
    );

    let button_node = snapshot
        .nodes
        .iter()
        .find(|node| node.id == AccessibilityNodeId::Figure(button))
        .unwrap();
    assert_eq!(button_node.parent, root);
    assert_eq!(button_node.name, "Apply");
    assert_eq!(button_node.role, AccessibilityRole::Button);
    assert_eq!(button_node.bounds, Rectangle::new(20.0, 30.0, 120.0, 40.0));
    assert!(button_node.state.enabled);
    assert!(button_node.state.focusable);
    assert_eq!(
        button_node.default_action,
        Some(AccessibilityAction::Default)
    );
}

#[test]
fn focus_and_default_actions_reuse_runtime_widget_transactions() {
    let (mut runtime, button, toggle) = accessible_runtime();
    publish(&mut runtime);
    runtime.take_accessibility_updates();

    assert!(
        runtime
            .perform_accessibility_action(
                AccessibilityNodeId::Figure(button),
                AccessibilityAction::Focus,
            )
            .unwrap()
    );
    assert!(
        runtime
            .perform_accessibility_action(
                AccessibilityNodeId::Figure(toggle),
                AccessibilityAction::Default,
            )
            .unwrap()
    );
    assert!(runtime.clickable_snapshot(toggle).unwrap().selected);

    publish(&mut runtime);
    let updates = runtime.take_accessibility_updates();
    let AccessibilityUpdate::Delta(delta) = &updates[0] else {
        panic!("semantic changes after the baseline must publish a delta");
    };
    assert_eq!(delta.focus, Some(AccessibilityNodeId::Figure(button)));
    let toggle_node = delta
        .upserts
        .iter()
        .find(|node| node.id == AccessibilityNodeId::Figure(toggle))
        .unwrap();
    assert!(toggle_node.state.selected);
}

#[test]
fn dispose_removes_accessibility_identity_in_the_next_delta() {
    let (mut runtime, button, _toggle) = accessible_runtime();
    publish(&mut runtime);
    runtime.take_accessibility_updates();

    let parent = runtime.tree().parent_id(button).unwrap();
    assert!(
        runtime
            .container(parent)
            .unwrap()
            .remove(button)
            .expect("valid Runtime mutation")
    );
    publish(&mut runtime);

    let updates = runtime.take_accessibility_updates();
    let AccessibilityUpdate::Delta(delta) = &updates[0] else {
        panic!("dispose after baseline must publish a delta");
    };
    assert!(delta.removed.contains(&AccessibilityNodeId::Figure(button)));
}

#[test]
fn accessibility_projection_supports_the_maximum_tree_depth() {
    let mut tree = FigureTree::new();
    let parent = {
        let mut builder = tree.builder();
        let mut parent = builder.set_contents(Box::new(AccessibleGroup));
        for _ in 1..MAX_TREE_DEPTH {
            parent = builder
                .add_child(parent, Box::new(AccessibleGroup))
                .expect("valid FigureTree construction");
        }
        parent
    };
    let mut runtime = Runtime::new(tree);

    publish(&mut runtime);

    let snapshot = runtime.accessibility_snapshot().unwrap();
    assert_eq!(snapshot.nodes.len(), MAX_TREE_DEPTH + 1);
    assert_eq!(
        snapshot.nodes.last().unwrap().id,
        AccessibilityNodeId::Figure(parent)
    );
}
