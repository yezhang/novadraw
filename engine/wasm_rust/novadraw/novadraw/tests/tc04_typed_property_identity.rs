use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use novadraw::animation::{
    AnimationBehavior, AnimationBehaviorContext, AnimationFact, AnimationLifecycle,
    AnimationTrigger,
};
use novadraw::event::{
    DiscretePropertyValue, EventContext, FigureEventHandler, ListenerDirective, MouseButton,
    MouseEvent, PropertyChangeEvent, PropertyChangeListener, PropertyKey, PropertyValue,
    PropertyValueType,
};
use novadraw::figure::{
    FigureCapabilityBuilder, FigureCapabilityRegistrationError, INPUT, InputCapability,
};
use novadraw::{Figure, Rectangle, Runtime};

const MODE: PropertyKey<Mode> = PropertyKey::new("example.widget", "mode");
const SAME_NAME_OTHER_NAMESPACE: PropertyKey<Mode> =
    PropertyKey::new("example.other-widget", "mode");
const SAME_IDENTITY_OTHER_TYPE: PropertyKey<f64> = PropertyKey::new("example.widget", "mode");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Idle,
    Armed,
    Active,
}

impl PropertyValueType for Mode {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::Text("mode".into())
    }
}

impl DiscretePropertyValue for Mode {}

struct PropertyFigure {
    mode: Mutex<Mode>,
}

impl Figure for PropertyFigure {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 80.0, 40.0)
    }

    fn name(&self) -> &'static str {
        "PropertyFigure"
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}

impl FigureEventHandler for PropertyFigure {
    fn wants_mouse_events(&self) -> bool {
        true
    }

    fn on_mouse_pressed(&self, _event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let (old, new) = {
            let mut mode = self.mode.lock().unwrap();
            let old = *mode;
            let new = match old {
                Mode::Idle => Mode::Armed,
                Mode::Armed => Mode::Active,
                Mode::Active => Mode::Active,
            };
            *mode = new;
            (old, new)
        };
        context.emit_property_change(context.target_id(), MODE, old, new);
        context.emit_property_change(context.target_id(), SAME_NAME_OTHER_NAMESPACE, old, new);
        true
    }
}

#[test]
fn lifecycle_visibility_mapping_uses_the_standard_typed_key() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(PropertyFigure {
            mode: Mutex::new(Mode::Idle),
        }))
        .unwrap();
    runtime.record_full_frame();

    let shown = Arc::new(AtomicUsize::new(0));
    let hidden = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&shown);
    runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::Lifecycle(AnimationLifecycle::Shown),
                move |_: AnimationBehaviorContext<'_>| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(None)
                },
            )
            .scoped_to(figure),
        )
        .unwrap();
    let calls = Arc::clone(&hidden);
    runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::Lifecycle(AnimationLifecycle::Hidden),
                move |_: AnimationBehaviorContext<'_>| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(None)
                },
            )
            .scoped_to(figure),
        )
        .unwrap();

    runtime.figure(figure).unwrap().set_visible(false).unwrap();
    runtime.record_full_frame();
    runtime.figure(figure).unwrap().set_visible(true).unwrap();
    runtime.record_full_frame();

    assert_eq!(hidden.load(Ordering::SeqCst), 1);
    assert_eq!(shown.load(Ordering::SeqCst), 1);
}

#[derive(Clone)]
struct PropertyRecorder(Arc<Mutex<Vec<PropertyChangeEvent>>>);

impl PropertyChangeListener for PropertyRecorder {
    fn property_changed(&self, event: &PropertyChangeEvent) -> ListenerDirective {
        self.0.lock().unwrap().push(event.clone());
        ListenerDirective::Keep
    }
}

#[test]
fn typed_keys_drive_matching_coalescing_and_diagnostics_without_string_identity() {
    assert_ne!(MODE.erase(), SAME_IDENTITY_OTHER_TYPE.erase());

    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(PropertyFigure {
            mode: Mutex::new(Mode::Idle),
        }))
        .unwrap();
    runtime.record_full_frame();

    let property_calls = Arc::new(AtomicUsize::new(0));
    let state_calls = Arc::new(AtomicUsize::new(0));
    let transaction_facts = Arc::new(Mutex::new(Vec::new()));

    let calls = Arc::clone(&property_calls);
    runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::property_changed(MODE),
                move |_: AnimationBehaviorContext<'_>| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(None)
                },
            )
            .scoped_to(figure),
        )
        .unwrap();

    let calls = Arc::clone(&state_calls);
    runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::state_changed(MODE),
                move |_: AnimationBehaviorContext<'_>| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(None)
                },
            )
            .scoped_to(figure),
        )
        .unwrap();

    let facts = Arc::clone(&transaction_facts);
    runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::Transaction,
                move |context: AnimationBehaviorContext<'_>| {
                    facts.lock().unwrap().extend_from_slice(context.facts());
                    Ok(None)
                },
            )
            .scoped_to(figure),
        )
        .unwrap();

    let observed = Arc::new(Mutex::new(Vec::new()));
    runtime.add_property_listener(Box::new(PropertyRecorder(Arc::clone(&observed))));

    runtime.dispatch_mouse_pressed(10.0, 10.0, MouseButton::Left);
    runtime.dispatch_mouse_released(10.0, 10.0, MouseButton::Left);
    runtime.dispatch_mouse_pressed(10.0, 10.0, MouseButton::Left);
    runtime.record_full_frame();

    assert_eq!(property_calls.load(Ordering::SeqCst), 1);
    assert_eq!(state_calls.load(Ordering::SeqCst), 1);

    let facts = transaction_facts.lock().unwrap();
    assert_eq!(facts.len(), 2);
    assert!(facts.iter().any(|fact| matches!(
        fact,
        AnimationFact::PropertyChanged(event)
            if event.property().is(MODE)
                && event.old_value() == &PropertyValue::Text("mode".into())
                && event.new_value() == &PropertyValue::Text("mode".into())
    )));
    assert!(facts.iter().any(|fact| matches!(
        fact,
        AnimationFact::PropertyChanged(event)
            if event.property().is(SAME_NAME_OTHER_NAMESPACE)
    )));

    let observed = observed.lock().unwrap();
    assert_eq!(observed.len(), 4);
    assert_eq!(observed[0].property().name(), "mode");
    assert_eq!(observed[0].property().namespace(), "example.widget");
    assert!(observed[0].property().is(MODE));
    assert_eq!(observed[0].old_value(), &PropertyValue::Text("mode".into()));
    assert_eq!(observed[0].new_value(), &PropertyValue::Text("mode".into()));
}
