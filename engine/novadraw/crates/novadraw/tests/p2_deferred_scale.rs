use novadraw::{
    container::ScalableLayeredPaneFigure,
    event::{EventContext, FigureEventHandler, MouseButton, MouseEvent},
    figure::{
        Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, INPUT, InputCapability,
    },
    geometry::Rectangle,
    runtime::Runtime,
    tree::FigureId,
};

struct ScaleTrigger {
    target: FigureId,
    value: f64,
}

impl Figure for ScaleTrigger {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 20.0, 20.0)
    }
    fn name(&self) -> &'static str {
        "ExternalScaleTrigger"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}

impl FigureEventHandler for ScaleTrigger {
    fn on_mouse_pressed(&self, _: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        context.effects().set_scale_later(self.target, self.value);
        true
    }
}

fn fixture(value: f64) -> (Runtime, FigureId) {
    let mut runtime = Runtime::empty();
    let target = runtime
        .set_contents(Box::new(ScalableLayeredPaneFigure::new(
            0.0, 0.0, 200.0, 200.0,
        )))
        .unwrap();
    runtime
        .figure(target)
        .unwrap()
        .add(Box::new(ScaleTrigger { target, value }))
        .unwrap();
    (runtime, target)
}

#[test]
fn external_callback_uses_the_same_scale_contract_as_the_runtime_facade() {
    let (mut runtime, target) = fixture(2.5);
    runtime
        .input()
        .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
    assert_eq!(
        runtime.viewports().scale_handle(target).unwrap().scale(),
        2.5
    );
    assert!(runtime.take_deferred_mutation_errors().is_empty());
}

#[test]
fn rejected_deferred_scale_preserves_the_previous_value() {
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let (mut runtime, target) = fixture(value);
        runtime
            .input()
            .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
        assert_eq!(
            runtime.viewports().scale_handle(target).unwrap().scale(),
            1.0
        );
        assert_eq!(runtime.take_deferred_mutation_errors().len(), 1);
    }
}
