use novadraw::{
    event::{EventContext, FigureEventHandler, MouseButton, MouseEvent},
    figure::{
        CursorIcon, Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError,
        FigureStyle, INPUT, InputCapability, RectangleFigure,
    },
    geometry::Rectangle,
    graphics::Color,
    runtime::Runtime,
    tree::FigureId,
};
use std::sync::{Arc, Mutex};

struct StyleTrigger {
    target: FigureId,
    styles: Vec<FigureStyle>,
    observed: Arc<Mutex<Option<FigureStyle>>>,
}

impl Figure for StyleTrigger {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 20.0, 20.0)
    }
    fn name(&self) -> &'static str {
        "ExternalStyleTrigger"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}

impl FigureEventHandler for StyleTrigger {
    fn on_mouse_pressed(&self, _: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        for style in &self.styles {
            context
                .effects()
                .set_style_later(self.target, style.clone());
        }
        // Deferred effects must not change the immutable scene inside a callback.
        *self.observed.lock().unwrap() = Some(context.scene().figure_style(self.target).unwrap());
        true
    }
}

fn fixture(styles: Vec<FigureStyle>) -> (Runtime, FigureId, Arc<Mutex<Option<FigureStyle>>>) {
    let mut runtime = Runtime::empty();
    let target = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)))
        .unwrap();
    let observed = Arc::new(Mutex::new(None));
    runtime
        .figure(target)
        .unwrap()
        .add(Box::new(StyleTrigger {
            target,
            styles,
            observed: observed.clone(),
        }))
        .unwrap();
    (runtime, target, observed)
}

#[test]
fn deferred_style_uses_runtime_validation_and_does_not_write_during_callback() {
    let style = FigureStyle {
        foreground: Some(Color::WHITE),
        background: Some(Color::BLACK),
        cursor: Some(CursorIcon::NorthSouthResize),
        ..FigureStyle::default()
    };
    let (mut runtime, target, observed) = fixture(vec![style.clone()]);
    let before = runtime.tree().figure_style(target).unwrap().clone();
    runtime
        .input()
        .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
    assert_eq!(*observed.lock().unwrap(), Some(before));
    assert_eq!(runtime.tree().figure_style(target).unwrap(), &style);
    assert_eq!(runtime.focus().cursor_icon(), CursorIcon::NorthSouthResize);
    assert!(runtime.take_deferred_mutation_errors().is_empty());
}

#[test]
fn ordered_replacements_clear_local_values_and_resume_inheritance() {
    let (mut runtime, target, _) = fixture(vec![
        FigureStyle {
            foreground: Some(Color::WHITE),
            cursor: Some(CursorIcon::Crosshair),
            ..FigureStyle::default()
        },
        FigureStyle::default(),
    ]);
    runtime
        .input()
        .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
    assert_eq!(
        runtime.tree().figure_style(target).unwrap(),
        &FigureStyle::default()
    );
    assert_eq!(runtime.focus().cursor_icon(), CursorIcon::Default);
    assert!(runtime.take_deferred_mutation_errors().is_empty());
}

#[test]
fn invalid_alpha_rejects_sync_and_deferred_style_without_changing_previous_value() {
    for alpha in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let style = FigureStyle {
            alpha: Some(alpha),
            ..FigureStyle::default()
        };
        let (mut runtime, target, _) = fixture(vec![style.clone()]);
        let before = runtime.tree().figure_style(target).unwrap().clone();
        assert!(runtime.figure(target).unwrap().set_style(style).is_err());
        assert_eq!(runtime.tree().figure_style(target).unwrap(), &before);
        runtime
            .input()
            .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
        assert_eq!(runtime.tree().figure_style(target).unwrap(), &before);
        assert_eq!(runtime.take_deferred_mutation_errors().len(), 1);
        assert!(!runtime.is_faulted());
    }
}

#[test]
fn foreign_targets_are_reported_without_mutating_the_local_tree() {
    let (foreign, target, _) = fixture(Vec::new());
    let mut runtime = Runtime::empty();
    let local = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .unwrap();
    // A Figure with no scene query can request a checked foreign-target write.
    struct ForeignTrigger(FigureId);
    impl Figure for ForeignTrigger {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 20.0, 20.0)
        }
        fn name(&self) -> &'static str {
            "ForeignStyleTrigger"
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(INPUT, InputCapability::of::<Self>())
        }
    }
    impl FigureEventHandler for ForeignTrigger {
        fn on_mouse_pressed(&self, _: &MouseEvent, context: &mut EventContext<'_>) -> bool {
            context.effects().set_style_later(
                self.0,
                FigureStyle {
                    foreground: Some(Color::WHITE),
                    ..FigureStyle::default()
                },
            );
            true
        }
    }
    runtime
        .figure(local)
        .unwrap()
        .add(Box::new(ForeignTrigger(target)))
        .unwrap();
    let before = runtime.tree().figure_style(local).unwrap().clone();
    runtime
        .input()
        .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
    assert_eq!(runtime.tree().figure_style(local).unwrap(), &before);
    assert_eq!(runtime.take_deferred_mutation_errors().len(), 1);
    assert!(!runtime.is_faulted());
    drop(foreign);
}
