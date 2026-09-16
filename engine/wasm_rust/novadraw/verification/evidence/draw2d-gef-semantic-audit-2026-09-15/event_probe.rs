//! Standalone audit reproduction; records actual behavior without changing tests.
use std::{cell::RefCell, rc::Rc};
use novadraw_scene::{
    EventContext, Figure, FigureEventHandler, FigureTree, MouseEvent, MouseEventKind,
    Rectangle, RectangleFigure, Runtime,
};

struct InteractiveParent(Rc<RefCell<Vec<MouseEventKind>>>);

impl Figure for InteractiveParent {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 300.0, 300.0)
    }

    fn name(&self) -> &'static str {
        "AuditInteractiveParent"
    }

    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl FigureEventHandler for InteractiveParent {
    fn on_mouse_entered(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
        self.0.borrow_mut().push(event.kind);
        false
    }

    fn on_mouse_exited(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
        self.0.borrow_mut().push(event.kind);
        false
    }

    fn on_mouse_moved(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
        self.0.borrow_mut().push(event.kind);
        false
    }
}

fn main() {
    let mut tree = FigureTree::new();
    let root = tree.builder().set_contents(Box::new(RectangleFigure::new(
        0.0, 0.0, 300.0, 300.0,
    )));
    let child = tree.builder().add_child_to(
        root, Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );
    let mut runtime = Runtime::new(tree);
    runtime.set_enabled(child, false);
    println!(
        "disabled_visible_generic_hit: expected_child=true actual_child={} actual_parent={}",
        runtime.tree().hit_test_simple((20.0, 20.0)) == Some(child),
        runtime.tree().hit_test_simple((20.0, 20.0)) == Some(root),
    );

    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = FigureTree::new();
    let parent = tree.builder().set_contents(Box::new(InteractiveParent(events.clone())));
    tree.builder().add_child_to(
        parent, Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
    );
    let mut runtime = Runtime::new(tree);
    let result = runtime.dispatch_mouse_moved(20.0, 20.0);
    println!("interactive_parent_target: {}", result.target() == Some(parent));
    println!("enter_over_noninteractive_child: expected=[Entered, Moved] actual={:?}", events.borrow());
    events.borrow_mut().clear();
    runtime.dispatch_mouse_moved(100.0, 100.0);
    println!("same_mouse_target_child_to_parent: expected=[Moved] actual={:?}", events.borrow());
}
