use novadraw::figure::{
    ACCESSIBILITY, AccessibilityCapability, AccessibleFigure, ButtonFigure,
    FigureCapabilityBuilder, FigureCapabilityRegistrationError, ToggleFigure,
};
use novadraw::render::{BackendCapabilities, BuiltinFont, RenderDisposition, SurfaceInfo};
use novadraw::runtime::{
    AccessibilityAction, AccessibilityNodeId, AccessibilityRole, AccessibilityUpdate,
};
use novadraw::tree::MAX_TREE_DEPTH;
use novadraw::{
    figure::Figure, figure::RectangleFigure, geometry::Rectangle, runtime::FramePreparation,
    runtime::Runtime, tree::FigureId, tree::FigureTree,
};

struct AccessibleGroup;

impl Figure for AccessibleGroup {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 1.0, 1.0)
    }

    fn name(&self) -> &'static str {
        "AccessibleGroup"
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(ACCESSIBILITY, AccessibilityCapability::of::<Self>())
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
    if let FramePreparation::Ready(submission) = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::FULL_FRAME_ONLY)
    {
        runtime
            .frames()
            .complete_submission(
                submission.session_id,
                submission.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");
    }
}

fn accessible_runtime() -> (Runtime, FigureId, FigureId) {
    let mut runtime = Runtime::empty();
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 200.0)))
        .expect("valid Runtime mutation");
    let button = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            ButtonFigure::new("Apply").with_bounds(Rectangle::new(20.0, 30.0, 120.0, 40.0)),
        ))
        .expect("valid Runtime mutation");
    let toggle = runtime
        .figure(root)
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

    let updates = runtime.accessibility().take_updates();
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
    runtime.accessibility().take_updates();

    assert!(
        runtime
            .accessibility()
            .perform_action(
                AccessibilityNodeId::Figure(button),
                AccessibilityAction::Focus,
            )
            .unwrap()
    );
    assert!(
        runtime
            .accessibility()
            .perform_action(
                AccessibilityNodeId::Figure(toggle),
                AccessibilityAction::Default,
            )
            .unwrap()
    );
    assert!(runtime.input().clickable_snapshot(toggle).unwrap().selected);

    publish(&mut runtime);
    let updates = runtime.accessibility().take_updates();
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
    runtime.accessibility().take_updates();

    let parent = runtime.tree().parent_id(button).unwrap();
    assert!(
        runtime
            .figure(parent)
            .unwrap()
            .remove(button)
            .expect("valid Runtime mutation")
    );
    publish(&mut runtime);

    let updates = runtime.accessibility().take_updates();
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
        let mut parent = builder
            .set_contents(Box::new(AccessibleGroup))
            .expect("valid FigureTree construction");
        for _ in 1..MAX_TREE_DEPTH {
            parent = builder
                .add_child(parent, Box::new(AccessibleGroup))
                .expect("valid FigureTree construction");
        }
        parent
    };
    let mut runtime = Runtime::new(tree);

    publish(&mut runtime);

    let snapshot = runtime.accessibility().snapshot().unwrap().clone();
    assert_eq!(snapshot.nodes.len(), MAX_TREE_DEPTH + 1);
    assert_eq!(
        snapshot.nodes.last().unwrap().id,
        AccessibilityNodeId::Figure(parent)
    );
}
