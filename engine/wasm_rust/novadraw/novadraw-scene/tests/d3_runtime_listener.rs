use std::sync::{Arc, Mutex};

use novadraw_scene::{
    ActionEvent, ActionListener, AncestorEvent, AncestorListener, ClickableFigure,
    CoordinateListener, FigureEvent, FigureId, FigureListener, LayoutEvent, LayoutListener,
    ListenerDirective, MouseButton, PropertyChangeEvent, PropertyChangeListener, Rectangle,
    RectangleFigure, Runtime, UpdateEvent, UpdateListener, XYLayout,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ListenerCounts {
    update: usize,
    figure: usize,
    coordinate: usize,
    ancestor: usize,
    property: usize,
    action: usize,
    layout: usize,
}

struct TypedRecorder {
    counts: Arc<Mutex<ListenerCounts>>,
}

impl UpdateListener for TypedRecorder {
    fn on_update_event(&self, _event: UpdateEvent) -> ListenerDirective {
        self.counts.lock().unwrap().update += 1;
        ListenerDirective::Keep
    }

    fn on_figure_event(&self, _event: FigureEvent) -> ListenerDirective {
        ListenerDirective::Keep
    }

    fn on_notify(&self, _figure_id: FigureId) -> ListenerDirective {
        ListenerDirective::Keep
    }
}

impl FigureListener for TypedRecorder {
    fn figure_moved(&self, _event: FigureEvent) -> ListenerDirective {
        self.counts.lock().unwrap().figure += 1;
        ListenerDirective::Keep
    }
}

impl CoordinateListener for TypedRecorder {
    fn coordinate_system_changed(&self, _event: FigureEvent) -> ListenerDirective {
        self.counts.lock().unwrap().coordinate += 1;
        ListenerDirective::Keep
    }
}

impl AncestorListener for TypedRecorder {
    fn ancestor_changed(&self, _event: AncestorEvent) -> ListenerDirective {
        self.counts.lock().unwrap().ancestor += 1;
        ListenerDirective::Keep
    }
}

impl PropertyChangeListener for TypedRecorder {
    fn property_changed(&self, _event: &PropertyChangeEvent) -> ListenerDirective {
        self.counts.lock().unwrap().property += 1;
        ListenerDirective::Keep
    }
}

impl ActionListener for TypedRecorder {
    fn action_performed(&self, _event: ActionEvent) -> ListenerDirective {
        self.counts.lock().unwrap().action += 1;
        ListenerDirective::Keep
    }
}

impl LayoutListener for TypedRecorder {
    fn layout_changed(&self, _event: LayoutEvent) -> ListenerDirective {
        self.counts.lock().unwrap().layout += 1;
        ListenerDirective::Keep
    }
}

#[test]
fn runtime_exposes_all_listener_categories_with_one_removal_namespace() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)))
        .expect("valid Runtime mutation");
    let _ = runtime.prepare_frame();

    let counts = Arc::new(Mutex::new(ListenerCounts::default()));
    let update = runtime.add_update_listener(Box::new(TypedRecorder {
        counts: Arc::clone(&counts),
    }));
    let figure = runtime.add_figure_listener(Box::new(TypedRecorder {
        counts: Arc::clone(&counts),
    }));
    let coordinate = runtime.add_coordinate_listener(Box::new(TypedRecorder {
        counts: Arc::clone(&counts),
    }));
    let ancestor = runtime.add_ancestor_listener(Box::new(TypedRecorder {
        counts: Arc::clone(&counts),
    }));
    let property = runtime.add_property_listener(Box::new(TypedRecorder {
        counts: Arc::clone(&counts),
    }));
    let action = runtime.add_action_listener(Box::new(TypedRecorder {
        counts: Arc::clone(&counts),
    }));
    let layout = runtime.add_layout_listener(Box::new(TypedRecorder {
        counts: Arc::clone(&counts),
    }));

    let clickable = runtime
        .container(root)
        .unwrap()
        .add(Box::new(ClickableFigure::new(Rectangle::new(
            20.0, 20.0, 100.0, 40.0,
        ))))
        .expect("valid Runtime mutation");
    assert!(
        runtime
            .figure(root)
            .unwrap()
            .set_bounds(Rectangle::new(5.0, 5.0, 300.0, 200.0))
            .expect("valid Runtime mutation")
    );
    assert!(
        runtime
            .figure(clickable)
            .unwrap()
            .set_visible(false)
            .expect("valid Runtime mutation")
    );
    assert!(
        runtime
            .figure(clickable)
            .unwrap()
            .set_visible(true)
            .expect("valid Runtime mutation")
    );
    assert!(
        runtime
            .container(root)
            .unwrap()
            .set_layout_manager(Box::new(XYLayout::new()))
            .unwrap()
    );
    assert!(runtime.do_click(clickable).unwrap());
    let _ = runtime.prepare_frame();

    let observed = counts.lock().unwrap().clone();
    assert!(observed.update > 0);
    assert!(observed.figure > 0);
    assert!(observed.coordinate > 0);
    assert!(observed.ancestor > 0);
    assert!(observed.property > 0);
    assert!(observed.action > 0);
    assert!(observed.layout > 0);

    for id in [
        update, figure, coordinate, ancestor, property, action, layout,
    ] {
        assert!(runtime.remove_listener(id));
        assert!(!runtime.remove_listener(id));
    }

    assert!(
        runtime
            .figure(clickable)
            .unwrap()
            .set_visible(false)
            .expect("valid Runtime mutation")
    );
    let _ = runtime.prepare_frame();
    assert_eq!(*counts.lock().unwrap(), observed);
}

struct RemoveAfterFirstProperty {
    calls: Arc<Mutex<Vec<&'static str>>>,
    label: &'static str,
    directive: ListenerDirective,
}

impl PropertyChangeListener for RemoveAfterFirstProperty {
    fn property_changed(&self, _event: &PropertyChangeEvent) -> ListenerDirective {
        self.calls.lock().unwrap().push(self.label);
        self.directive
    }
}

#[test]
fn listener_self_removal_finishes_current_effect_and_skips_later_effects() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)))
        .expect("valid Runtime mutation");
    let child = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 20.0, 100.0, 40.0)))
        .expect("valid Runtime mutation");
    let _ = runtime.prepare_frame();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let removing = runtime.add_property_listener(Box::new(RemoveAfterFirstProperty {
        calls: Arc::clone(&calls),
        label: "remove",
        directive: ListenerDirective::Remove,
    }));
    let retaining = runtime.add_property_listener(Box::new(RemoveAfterFirstProperty {
        calls: Arc::clone(&calls),
        label: "keep",
        directive: ListenerDirective::Keep,
    }));

    assert!(
        runtime
            .figure(child)
            .unwrap()
            .set_visible(false)
            .expect("valid Runtime mutation")
    );
    assert!(
        runtime
            .figure(child)
            .unwrap()
            .set_visible(true)
            .expect("valid Runtime mutation")
    );
    let _ = runtime.prepare_frame();

    assert_eq!(*calls.lock().unwrap(), vec!["remove", "keep", "keep"]);
    assert!(!runtime.remove_listener(removing));
    assert!(runtime.remove_listener(retaining));
    assert!(!runtime.remove_listener(retaining));
}

#[test]
fn action_listener_can_remove_itself_after_the_current_action() {
    struct OnceAction(Arc<Mutex<usize>>);

    impl ActionListener for OnceAction {
        fn action_performed(&self, _event: ActionEvent) -> ListenerDirective {
            *self.0.lock().unwrap() += 1;
            ListenerDirective::Remove
        }
    }

    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)))
        .expect("valid Runtime mutation");
    let clickable = runtime
        .container(root)
        .unwrap()
        .add(Box::new(ClickableFigure::new(Rectangle::new(
            20.0, 20.0, 100.0, 40.0,
        ))))
        .expect("valid Runtime mutation");
    let _ = runtime.prepare_frame();

    let calls = Arc::new(Mutex::new(0));
    runtime.add_action_listener(Box::new(OnceAction(Arc::clone(&calls))));
    assert!(runtime.do_click(clickable).unwrap());
    assert!(runtime.do_click(clickable).unwrap());
    let _ = runtime.prepare_frame();

    assert_eq!(*calls.lock().unwrap(), 1);
}

#[test]
fn pointer_dispatch_remains_available_after_listener_api_completion() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)))
        .expect("valid Runtime mutation");
    let clickable = runtime
        .container(root)
        .unwrap()
        .add(Box::new(ClickableFigure::new(Rectangle::new(
            20.0, 20.0, 100.0, 40.0,
        ))))
        .expect("valid Runtime mutation");

    runtime.dispatch_mouse_pressed(30.0, 30.0, MouseButton::Left);
    runtime.dispatch_mouse_released(30.0, 30.0, MouseButton::Left);

    assert_eq!(runtime.interaction().focus_owner(), Some(clickable));
}
