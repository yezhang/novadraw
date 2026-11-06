use novadraw::{
    figure::{
        FigureMeasurement, MeasureConstraints, PreparationInputError, PreparationInputErrorKind,
        PreparationInputKey, PreparationInputPayload, PreparationInputRegistry, PreparationScene,
        RectangleFigure,
    },
    geometry::{Dimension, Rectangle},
    layout::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery},
    render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
    runtime::{FramePreparation, FramePreparationError, Runtime},
    tree::FigureId,
};
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

#[derive(Eq, PartialEq)]
struct ModelValue(u64);
impl PreparationInputPayload for ModelValue {
    fn retained_units(&self) -> u64 {
        0
    }
}
const MODEL: PreparationInputKey<ModelValue> = PreparationInputKey::new("consumer.model");

fn registry(value: &Arc<AtomicU64>) -> PreparationInputRegistry {
    let value = value.clone();
    let mut inputs = PreparationInputRegistry::new();
    inputs
        .register(MODEL, move |_: FigureId, _: &PreparationScene<'_>| {
            Ok(ModelValue(value.load(Ordering::Relaxed)))
        })
        .unwrap();
    inputs
}

struct ModelLayout {
    inputs: PreparationInputRegistry,
    value: Arc<AtomicU64>,
    mutate: Arc<AtomicBool>,
    ignore_error: bool,
}
impl LayoutManager for ModelLayout {
    fn preferred_measurement(
        &self,
        owner: FigureId,
        _: MeasureConstraints,
        query: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        let value = query.input(&self.inputs, MODEL, owner)?;
        Ok(FigureMeasurement::new(
            value.value(MODEL).unwrap().0 as f64,
            10.0,
            None,
        ))
    }
    fn minimum_size(
        &self,
        owner: FigureId,
        constraints: MeasureConstraints,
        query: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(self
            .preferred_measurement(owner, constraints, query)?
            .size())
    }
    fn layout(
        &mut self,
        owner: FigureId,
        query: &LayoutSceneQuery<'_>,
        output: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let first = query.input(&self.inputs, MODEL, owner);
        if self.ignore_error {
            let _ = first;
            return Ok(());
        }
        let first = first?;
        if self.mutate.swap(false, Ordering::Relaxed) {
            self.value.fetch_add(1, Ordering::Relaxed);
            // A discarded failed read must not make this candidate publishable.
            let _ = query.input(&self.inputs, MODEL, owner);
        }
        for (child, _) in query.children(owner)? {
            output.set_child_bounds(
                child,
                Rectangle::new(0.0, 0.0, first.value(MODEL).unwrap().0 as f64, 10.0),
            );
        }
        Ok(())
    }
}

fn fixture(
    inputs: PreparationInputRegistry,
    value: Arc<AtomicU64>,
    mutate: Arc<AtomicBool>,
    ignore_error: bool,
) -> (Runtime, FigureId, FigureId) {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
        ))
        .unwrap();
    let child = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap(),
        ))
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_layout_manager(Box::new(ModelLayout {
            inputs,
            value,
            mutate,
            ignore_error,
        }))
        .unwrap();
    (runtime, root, child)
}

fn frame(runtime: &mut Runtime) -> Result<(), FramePreparationError> {
    match runtime.frames().prepare_submission(
        SurfaceInfo {
            logical_width: 100.0,
            logical_height: 100.0,
            pixel_width: 100,
            pixel_height: 100,
            scale_factor: 1.0,
        },
        BackendCapabilities::RETAINED_PARTIAL,
    ) {
        FramePreparation::Ready(submission) => {
            let _ = runtime
                .frames()
                .complete_submission(
                    submission.session_id,
                    submission.frame_id,
                    Ok(RenderDisposition::Presented),
                )
                .unwrap();
            Ok(())
        }
        FramePreparation::Error(error) => Err(error),
        other => panic!("{other:?}"),
    }
}

fn input_error<'a>(error: &'a (dyn Error + 'static)) -> &'a PreparationInputError {
    let mut current = error;
    loop {
        if let Some(input) = current.downcast_ref::<PreparationInputError>() {
            return input;
        }
        current = current.source().expect("original input error chain");
    }
}

#[test]
fn ordinary_layout_records_private_inputs_and_refreshes_between_frames() {
    let value = Arc::new(AtomicU64::new(1));
    let (mut runtime, _, child) = fixture(
        registry(&value),
        value.clone(),
        Arc::new(AtomicBool::new(false)),
        false,
    );
    frame(&mut runtime).unwrap();
    assert_eq!(runtime.tree().figure_bounds(child).unwrap().width, 1.0);
    value.store(2, Ordering::Relaxed);
    // No Figure mutation accompanies this model update.
    frame(&mut runtime).unwrap();
    assert_eq!(runtime.tree().figure_bounds(child).unwrap().width, 2.0);
}

#[test]
fn ignored_conflict_rejects_output_and_preserves_the_first_owned_value() {
    let value = Arc::new(AtomicU64::new(1));
    let armed = Arc::new(AtomicBool::new(false));
    let (mut runtime, root, child) = fixture(registry(&value), value.clone(), armed.clone(), false);
    frame(&mut runtime).unwrap();
    let old = runtime.tree().figure_bounds(child).unwrap();
    armed.store(true, Ordering::Relaxed);
    runtime
        .figure(root)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, 101.0, 100.0))
        .unwrap();
    let error = frame(&mut runtime).unwrap_err();
    let input = input_error(&error);
    assert_eq!(input.source, Some(root));
    match &input.kind {
        PreparationInputErrorKind::Changed { first, current } => {
            assert_eq!(first.value(MODEL).unwrap().0, 1);
            assert_eq!(current.value(MODEL).unwrap().0, 2);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(runtime.tree().figure_bounds(child).unwrap(), old);
    frame(&mut runtime).unwrap();
    assert_eq!(runtime.tree().figure_bounds(child).unwrap().width, 2.0);
}

#[test]
fn ignored_unknown_registration_is_still_a_frame_error() {
    let (mut runtime, root, _) = fixture(
        PreparationInputRegistry::new(),
        Arc::new(AtomicU64::new(1)),
        Arc::new(AtomicBool::new(false)),
        true,
    );
    let error = frame(&mut runtime).unwrap_err();
    let input = input_error(&error);
    assert_eq!(input.source, Some(root));
    assert_eq!(input.kind, PreparationInputErrorKind::UnknownRegistration);
}

struct MeasuringParent;
impl LayoutManager for MeasuringParent {
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        Ok(FigureMeasurement::new(100.0, 100.0, None))
    }
    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(Dimension::ZERO)
    }
    fn layout(
        &mut self,
        owner: FigureId,
        query: &LayoutSceneQuery<'_>,
        output: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        for (child, _) in query.children(owner)? {
            let preferred = query.preferred_measurement(child, MeasureConstraints::UNBOUNDED)?;
            let minimum = query.minimum_size(child, MeasureConstraints::UNBOUNDED)?;
            assert_eq!(preferred.size(), minimum);
            output.set_child_bounds(
                child,
                Rectangle::new(0.0, 0.0, preferred.width, preferred.height),
            );
        }
        Ok(())
    }
}

#[test]
fn ordinary_nested_preferred_and_minimum_use_the_runtime_input_session() {
    let value = Arc::new(AtomicU64::new(3));
    let (mut runtime, root, child) = fixture(
        registry(&value),
        value.clone(),
        Arc::new(AtomicBool::new(false)),
        false,
    );
    // Move the private-model manager down one level. No preparation producer exists.
    runtime
        .figure(child)
        .unwrap()
        .set_layout_manager(Box::new(ModelLayout {
            inputs: registry(&value),
            value: value.clone(),
            mutate: Arc::new(AtomicBool::new(false)),
            ignore_error: false,
        }))
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_layout_manager(Box::new(MeasuringParent))
        .unwrap();
    frame(&mut runtime).unwrap();
    assert_eq!(runtime.tree().figure_bounds(child).unwrap().width, 3.0);
    value.store(4, Ordering::Relaxed);
    frame(&mut runtime).unwrap();
    assert_eq!(runtime.tree().figure_bounds(child).unwrap().width, 4.0);
}

#[test]
fn layout_input_reader_observes_inherited_style_in_each_immutable_session() {
    let mut inputs = PreparationInputRegistry::new();
    inputs
        .register(MODEL, |source, scene: &PreparationScene<'_>| {
            Ok(ModelValue(scene.style(source)?.font.size() as u64))
        })
        .unwrap();
    let value = Arc::new(AtomicU64::new(1));
    let (mut runtime, root, child) = fixture(
        inputs.clone(),
        value.clone(),
        Arc::new(AtomicBool::new(false)),
        false,
    );
    runtime
        .figure(child)
        .unwrap()
        .set_layout_manager(Box::new(ModelLayout {
            inputs,
            value,
            mutate: Arc::new(AtomicBool::new(false)),
            ignore_error: false,
        }))
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_layout_manager(Box::new(MeasuringParent))
        .unwrap();
    for size in [20, 30] {
        runtime
            .figure(root)
            .unwrap()
            .set_style(novadraw::figure::FigureStyle {
                font: Some(
                    novadraw::text::FontDescriptor::parse(&format!("{size}px Inter Variable"))
                        .unwrap(),
                ),
                ..Default::default()
            })
            .unwrap();
        frame(&mut runtime).unwrap();
        assert_eq!(
            runtime.tree().figure_bounds(child).unwrap().width,
            f64::from(size)
        );
    }
}

#[test]
fn sessionless_measurement_refuses_to_call_a_private_reader() {
    let value = Arc::new(AtomicU64::new(1));
    let (runtime, root, _) = fixture(
        registry(&value),
        value,
        Arc::new(AtomicBool::new(false)),
        false,
    );
    assert!(matches!(
        runtime.tree().preferred_measurement(root, MeasureConstraints::UNBOUNDED),
        Err(LayoutError::InputSessionUnavailable { source }) if source == root
    ));
}

struct ReadsBeforeChild {
    inputs: PreparationInputRegistry,
}
impl LayoutManager for ReadsBeforeChild {
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        Ok(FigureMeasurement::new(100.0, 100.0, None))
    }
    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(Dimension::ZERO)
    }
    fn layout(
        &mut self,
        owner: FigureId,
        query: &LayoutSceneQuery<'_>,
        output: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let first = query.input(&self.inputs, MODEL, owner)?;
        for (child, _) in query.children(owner)? {
            let _ = query.minimum_size(child, MeasureConstraints::UNBOUNDED)?;
            output.set_child_bounds(
                child,
                Rectangle::new(0.0, 0.0, first.value(MODEL).unwrap().0 as f64, 10.0),
            );
        }
        Ok(())
    }
}

struct MutatingMinimum {
    value: Arc<AtomicU64>,
    armed: Arc<AtomicBool>,
}
impl LayoutManager for MutatingMinimum {
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        Ok(FigureMeasurement::new(10.0, 10.0, None))
    }
    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        if self.armed.swap(false, Ordering::Relaxed) {
            self.value.fetch_add(1, Ordering::Relaxed);
        }
        Ok(Dimension::new(10.0, 10.0))
    }
    fn layout(
        &mut self,
        _: FigureId,
        _: &LayoutSceneQuery<'_>,
        _: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        Ok(())
    }
}

#[test]
fn child_measurement_callback_cannot_rebase_an_earlier_layout_input() {
    let value = Arc::new(AtomicU64::new(1));
    let armed = Arc::new(AtomicBool::new(false));
    let (mut runtime, root, child) = fixture(registry(&value), value.clone(), armed.clone(), false);
    runtime
        .figure(root)
        .unwrap()
        .set_layout_manager(Box::new(ReadsBeforeChild {
            inputs: registry(&value),
        }))
        .unwrap();
    runtime
        .figure(child)
        .unwrap()
        .set_layout_manager(Box::new(MutatingMinimum {
            value,
            armed: armed.clone(),
        }))
        .unwrap();
    frame(&mut runtime).unwrap();
    let old = runtime.tree().figure_bounds(child).unwrap();
    armed.store(true, Ordering::Relaxed);
    runtime
        .figure(root)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, 101.0, 100.0))
        .unwrap();
    let error = frame(&mut runtime).unwrap_err();
    let input = input_error(&error);
    assert_eq!(input.checkpoint, "layout consumption");
    assert!(matches!(
        input.kind,
        PreparationInputErrorKind::Changed { .. }
    ));
    assert_eq!(runtime.tree().figure_bounds(child).unwrap(), old);
    frame(&mut runtime).unwrap();
    assert_eq!(runtime.tree().figure_bounds(child).unwrap().width, 2.0);
}

#[test]
fn ignored_reader_budget_failure_prevents_publication() {
    let mut inputs = PreparationInputRegistry::new();
    inputs
        .register(MODEL, |_: FigureId, scene: &PreparationScene<'_>| {
            scene
                .budget()
                .allocate("external layout reader", u64::MAX)?;
            Ok(ModelValue(1))
        })
        .unwrap();
    let (mut runtime, _, _) = fixture(
        inputs,
        Arc::new(AtomicU64::new(1)),
        Arc::new(AtomicBool::new(false)),
        true,
    );
    let error = frame(&mut runtime).unwrap_err();
    let mut source: &(dyn Error + 'static) = &error;
    loop {
        if let Some(budget) = source.downcast_ref::<novadraw::figure::PreparationBudgetError>() {
            assert_eq!(budget.stage, "external layout reader");
            break;
        }
        source = source.source().expect("original budget error chain");
    }
}

#[test]
fn ignored_first_style_allocation_failure_prevents_publication() {
    let mut inputs = PreparationInputRegistry::new();
    inputs
        .register(MODEL, |source: FigureId, scene: &PreparationScene<'_>| {
            let budget = scene.budget();
            budget.allocate(
                "external reader fills allocation",
                budget.limit().allocation - budget.used().allocation,
            )?;
            let _ = scene.style(source);
            Ok(ModelValue(1))
        })
        .unwrap();
    let (mut runtime, _, _) = fixture(
        inputs,
        Arc::new(AtomicU64::new(1)),
        Arc::new(AtomicBool::new(false)),
        true,
    );
    let error = frame(&mut runtime).unwrap_err();
    let mut source: &(dyn Error + 'static) = &error;
    loop {
        if let Some(budget) = source.downcast_ref::<novadraw::figure::PreparationBudgetError>() {
            assert_eq!(budget.stage, "preparation input styles");
            break;
        }
        source = source.source().expect("original style budget error chain");
    }
}

struct RetiringLayout {
    inputs: Option<PreparationInputRegistry>,
}
impl LayoutManager for RetiringLayout {
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        Ok(FigureMeasurement::default())
    }
    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(Dimension::ZERO)
    }
    fn layout(
        &mut self,
        owner: FigureId,
        query: &LayoutSceneQuery<'_>,
        _: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        query.input(self.inputs.as_ref().unwrap(), MODEL, owner)?;
        self.inputs.take();
        Ok(())
    }
}

#[test]
fn layout_proof_keeps_a_weak_registration_and_reports_retirement() {
    let value = Arc::new(AtomicU64::new(1));
    let (mut runtime, root, _) = fixture(
        registry(&value),
        value.clone(),
        Arc::new(AtomicBool::new(false)),
        false,
    );
    runtime
        .figure(root)
        .unwrap()
        .set_layout_manager(Box::new(RetiringLayout {
            inputs: Some(registry(&value)),
        }))
        .unwrap();
    let error = frame(&mut runtime).unwrap_err();
    let input = input_error(&error);
    assert_eq!(input.source, Some(root));
    assert_eq!(input.kind, PreparationInputErrorKind::RetiredRegistration);
}

#[test]
fn ordinary_layout_reader_preserves_its_original_error_when_ignored() {
    let mut inputs = PreparationInputRegistry::new();
    inputs
        .register(MODEL, |_: FigureId, _: &PreparationScene<'_>| {
            Err(novadraw::text::TextError::InvalidPreparedScene(
                "external reader rejected model".into(),
            ))
        })
        .unwrap();
    let (mut runtime, _, _) = fixture(
        inputs,
        Arc::new(AtomicU64::new(1)),
        Arc::new(AtomicBool::new(false)),
        true,
    );
    let error = frame(&mut runtime).unwrap_err();
    let mut source: &(dyn Error + 'static) = &error;
    loop {
        if let Some(text) = source.downcast_ref::<novadraw::text::TextError>() {
            assert_eq!(
                text,
                &novadraw::text::TextError::InvalidPreparedScene(
                    "external reader rejected model".into()
                )
            );
            break;
        }
        source = source.source().expect("original reader error chain");
    }
}

#[test]
fn same_private_query_works_when_an_unrelated_preparation_figure_exists() {
    let value = Arc::new(AtomicU64::new(1));
    let (mut runtime, root, child) = fixture(
        registry(&value),
        value.clone(),
        Arc::new(AtomicBool::new(false)),
        false,
    );
    runtime
        .resources()
        .register_builtin_font(novadraw::render::BuiltinFont::Inter)
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            novadraw::figure::TextFlowFigure::new(
                Rectangle::new(0.0, 0.0, 10.0, 10.0),
                novadraw::figure::FlowPage::from_text("sample"),
            )
            .unwrap(),
        ))
        .unwrap();
    frame(&mut runtime).unwrap();
    assert_eq!(runtime.tree().figure_bounds(child).unwrap().width, 1.0);
    value.store(2, Ordering::Relaxed);
    frame(&mut runtime).unwrap();
    assert_eq!(runtime.tree().figure_bounds(child).unwrap().width, 2.0);
}

mod local_requests {
    use super::*;
    use novadraw::{
        event::PropertyKey,
        event::{EventContext, FigureEventHandler, MouseButton, MouseEvent},
        figure::{
            CONTAINER, ChildPolicy, ContainerCapability, Figure, FigureCapabilityBuilder,
            FigureCapabilityRegistrationError, FigureContainer, FigureLifecycle, INPUT,
            InputCapability, LIFECYCLE, LifecycleCapability,
        },
        layout::{
            LayoutDependencyDeclaration, LayoutDependencyRegistrationError, LayoutRevalidationGuard,
        },
    };
    use std::sync::atomic::AtomicUsize;

    const VALUE: PropertyKey<f64> = PropertyKey::new("external.local-layout", "value");

    struct Source {
        inputs: PreparationInputRegistry,
        model: Arc<AtomicU64>,
        width: Arc<AtomicU64>,
        ordinary: Arc<AtomicBool>,
        lifecycle_mutation: Arc<AtomicBool>,
        focus_lost: Arc<AtomicUsize>,
    }
    impl Figure for Source {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 100.0)
        }
        fn name(&self) -> &'static str {
            "ExternalLocalLayoutSource"
        }
        fn initial_focusable(&self) -> bool {
            true
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(CONTAINER, ContainerCapability::of::<Self>())?;
            out.register(LIFECYCLE, LifecycleCapability::of::<Self>())?;
            out.register(INPUT, InputCapability::of::<Self>())
        }
    }
    impl FigureContainer for Source {
        fn child_policy(&self) -> ChildPolicy {
            ChildPolicy::Multiple
        }
    }
    impl FigureLifecycle for Source {
        fn validate(&mut self, _: Rectangle) {
            if self.lifecycle_mutation.swap(false, Ordering::Relaxed) {
                self.model.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    impl FigureEventHandler for Source {
        fn on_mouse_pressed(&self, _: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
            let first = ModelValue(self.model.load(Ordering::Relaxed));
            let old = self.width.swap(20, Ordering::Relaxed);
            let guard =
                LayoutRevalidationGuard::new(&self.inputs, MODEL, ctx.target_id(), first).unwrap();
            ctx.effects()
                .revalidate_layout_for_property(VALUE, old as f64, 20.0, guard);
            if self.ordinary.load(Ordering::Relaxed) {
                ctx.effects().invalidate();
            }
            true
        }
        fn on_focus_lost(
            &self,
            _: &novadraw::event::FocusEvent,
            ctx: &mut EventContext<'_>,
        ) -> bool {
            self.focus_lost.fetch_add(1, Ordering::Relaxed);
            let guard = LayoutRevalidationGuard::new(
                &self.inputs,
                MODEL,
                ctx.target_id(),
                ModelValue(self.model.load(Ordering::Relaxed)),
            )
            .unwrap();
            ctx.effects()
                .revalidate_layout_for_property(VALUE, 20.0, 20.0, guard);
            false
        }
    }

    struct Parent {
        independent: bool,
        calls: Arc<AtomicUsize>,
    }
    impl LayoutManager for Parent {
        fn register_child_layout_dependencies(
            &self,
            out: &mut LayoutDependencyDeclaration,
        ) -> Result<(), LayoutDependencyRegistrationError> {
            if self.independent {
                out.independent_child_property(VALUE)?;
            }
            Ok(())
        }
        fn preferred_measurement(
            &self,
            _: FigureId,
            _: MeasureConstraints,
            _: &LayoutSceneQuery<'_>,
        ) -> Result<FigureMeasurement, LayoutError> {
            Ok(FigureMeasurement::new(100.0, 100.0, None))
        }
        fn minimum_size(
            &self,
            _: FigureId,
            _: MeasureConstraints,
            _: &LayoutSceneQuery<'_>,
        ) -> Result<Dimension, LayoutError> {
            Ok(Dimension::ZERO)
        }
        fn layout(
            &mut self,
            owner: FigureId,
            query: &LayoutSceneQuery<'_>,
            output: &mut LayoutOutput,
        ) -> Result<(), LayoutError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            for (child, _) in query.children(owner)? {
                output.set_child_bounds(child, Rectangle::new(0.0, 0.0, 100.0, 100.0));
            }
            Ok(())
        }
    }

    struct Internal {
        width: Arc<AtomicU64>,
        model: Arc<AtomicU64>,
        mutate: Arc<AtomicBool>,
        visibility: Arc<AtomicBool>,
    }
    impl LayoutManager for Internal {
        fn preferred_measurement(
            &self,
            _: FigureId,
            _: MeasureConstraints,
            _: &LayoutSceneQuery<'_>,
        ) -> Result<FigureMeasurement, LayoutError> {
            Ok(FigureMeasurement::new(100.0, 100.0, None))
        }
        fn minimum_size(
            &self,
            _: FigureId,
            _: MeasureConstraints,
            _: &LayoutSceneQuery<'_>,
        ) -> Result<Dimension, LayoutError> {
            Ok(Dimension::ZERO)
        }
        fn layout(
            &mut self,
            owner: FigureId,
            query: &LayoutSceneQuery<'_>,
            output: &mut LayoutOutput,
        ) -> Result<(), LayoutError> {
            // This manager does not read MODEL itself. The request guard must
            // still protect publication across this external callback.
            if self.mutate.swap(false, Ordering::Relaxed) {
                self.model.fetch_add(1, Ordering::Relaxed);
            }
            for (child, _) in query.children(owner)? {
                output.set_child_bounds(
                    child,
                    Rectangle::new(0.0, 0.0, self.width.load(Ordering::Relaxed) as f64, 10.0),
                );
                if self.visibility.load(Ordering::Relaxed) {
                    output.set_child_visible(child, false);
                }
            }
            Ok(())
        }
    }

    struct Fixture {
        runtime: Runtime,
        root: FigureId,
        source: FigureId,
        child: FigureId,
        calls: Arc<AtomicUsize>,
        model: Arc<AtomicU64>,
        mutate: Arc<AtomicBool>,
        ordinary: Arc<AtomicBool>,
        visibility: Arc<AtomicBool>,
        lifecycle_mutation: Arc<AtomicBool>,
        focus_lost: Arc<AtomicUsize>,
    }
    impl Fixture {
        fn new(independent: bool) -> Self {
            let mut runtime = Runtime::empty();
            let root = runtime
                .set_contents(Box::new(
                    RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
                ))
                .unwrap();
            let model = Arc::new(AtomicU64::new(1));
            let width = Arc::new(AtomicU64::new(10));
            let ordinary = Arc::new(AtomicBool::new(false));
            let mutate = Arc::new(AtomicBool::new(false));
            let visibility = Arc::new(AtomicBool::new(false));
            let lifecycle_mutation = Arc::new(AtomicBool::new(false));
            let focus_lost = Arc::new(AtomicUsize::new(0));
            let source = runtime
                .figure(root)
                .unwrap()
                .add(Box::new(Source {
                    inputs: registry(&model),
                    model: model.clone(),
                    width: width.clone(),
                    ordinary: ordinary.clone(),
                    lifecycle_mutation: lifecycle_mutation.clone(),
                    focus_lost: focus_lost.clone(),
                }))
                .unwrap();
            let child = runtime
                .figure(source)
                .unwrap()
                .add(Box::new(
                    RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap(),
                ))
                .unwrap();
            let calls = Arc::new(AtomicUsize::new(0));
            runtime
                .figure(root)
                .unwrap()
                .set_layout_manager(Box::new(Parent {
                    independent,
                    calls: calls.clone(),
                }))
                .unwrap();
            runtime
                .figure(source)
                .unwrap()
                .set_layout_manager(Box::new(Internal {
                    width,
                    model: model.clone(),
                    mutate: mutate.clone(),
                    visibility: visibility.clone(),
                }))
                .unwrap();
            frame(&mut runtime).unwrap();
            calls.store(0, Ordering::Relaxed);
            Self {
                runtime,
                root,
                source,
                child,
                calls,
                model,
                mutate,
                ordinary,
                visibility,
                lifecycle_mutation,
                focus_lost,
            }
        }
        fn press(&mut self) {
            self.runtime
                .input()
                .dispatch_mouse_pressed(50.0, 50.0, MouseButton::Left);
            assert!(self.runtime.take_deferred_mutation_errors().is_empty());
        }
    }

    #[test]
    fn public_event_request_skips_only_explicitly_independent_ancestors() {
        for independent in [false, true] {
            let mut f = Fixture::new(independent);
            f.press();
            frame(&mut f.runtime).unwrap();
            assert_eq!(f.runtime.tree().figure_bounds(f.child).unwrap().width, 20.0);
            assert_eq!(f.calls.load(Ordering::Relaxed) == 0, independent);
        }
    }

    #[test]
    fn explicit_event_invalidation_and_visibility_output_keep_full_propagation() {
        for visibility in [false, true] {
            let mut f = Fixture::new(true);
            f.ordinary.store(!visibility, Ordering::Relaxed);
            f.visibility.store(visibility, Ordering::Relaxed);
            f.press();
            frame(&mut f.runtime).unwrap();
            assert!(f.calls.load(Ordering::Relaxed) > 0);
        }
    }

    #[test]
    fn changed_non_target_input_before_execution_upgrades_the_request() {
        let mut f = Fixture::new(true);
        f.press();
        f.model.store(2, Ordering::Relaxed);
        frame(&mut f.runtime).unwrap();
        assert!(f.calls.load(Ordering::Relaxed) > 0);
        assert_eq!(f.runtime.tree().figure_bounds(f.child).unwrap().width, 20.0);
    }

    #[test]
    fn external_callback_conflict_rolls_back_geometry_and_retries_full_work() {
        let mut f = Fixture::new(true);
        let old = f.runtime.tree().figure_bounds(f.child).unwrap();
        f.press();
        f.mutate.store(true, Ordering::Relaxed);
        let error = frame(&mut f.runtime).unwrap_err();
        let input = input_error(&error);
        assert_eq!(input.source, Some(f.source));
        match &input.kind {
            PreparationInputErrorKind::Changed { first, current } => {
                assert_eq!(first.value(MODEL).unwrap().0, 1);
                assert_eq!(current.value(MODEL).unwrap().0, 2);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(f.runtime.tree().figure_bounds(f.child).unwrap(), old);
        assert_eq!(f.calls.load(Ordering::Relaxed), 0);
        frame(&mut f.runtime).unwrap();
        assert_eq!(f.runtime.tree().figure_bounds(f.child).unwrap().width, 20.0);
        assert!(f.calls.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn public_manager_replacement_cannot_keep_a_queued_local_scope() {
        let mut f = Fixture::new(true);
        f.press();
        f.runtime
            .figure(f.root)
            .unwrap()
            .set_layout_manager(Box::new(Parent {
                independent: false,
                calls: f.calls.clone(),
            }))
            .unwrap();
        frame(&mut f.runtime).unwrap();
        assert!(f.calls.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn reparent_after_event_uses_the_current_ancestor_chain() {
        let mut f = Fixture::new(true);
        let new_parent = f
            .runtime
            .figure(f.root)
            .unwrap()
            .add(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        f.runtime
            .figure(new_parent)
            .unwrap()
            .set_layout_manager(Box::new(Parent {
                independent: false,
                calls: calls.clone(),
            }))
            .unwrap();
        frame(&mut f.runtime).unwrap();
        calls.store(0, Ordering::Relaxed);
        f.press();
        assert!(
            f.runtime
                .figure(f.source)
                .unwrap()
                .reparent(new_parent)
                .unwrap()
        );
        frame(&mut f.runtime).unwrap();
        assert!(calls.load(Ordering::Relaxed) > 0);
        assert_eq!(
            f.runtime.tree().parent_id(f.source).unwrap(),
            Some(new_parent)
        );
        assert_eq!(f.runtime.tree().figure_bounds(f.child).unwrap().width, 20.0);
    }

    #[test]
    fn source_lifecycle_conflict_rolls_back_applied_internal_geometry() {
        let mut f = Fixture::new(true);
        let old = f.runtime.tree().figure_bounds(f.child).unwrap();
        f.press();
        f.lifecycle_mutation.store(true, Ordering::Relaxed);
        let error = frame(&mut f.runtime).unwrap_err();
        assert_eq!(input_error(&error).checkpoint, "local layout return");
        assert_eq!(f.runtime.tree().figure_bounds(f.child).unwrap(), old);
        frame(&mut f.runtime).unwrap();
        assert_eq!(f.runtime.tree().figure_bounds(f.child).unwrap().width, 20.0);
        assert!(f.calls.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn retired_focus_callback_cannot_revive_a_local_request() {
        let mut f = Fixture::new(true);
        f.press();
        f.runtime.figure(f.root).unwrap().remove(f.source).unwrap();
        assert_eq!(f.focus_lost.load(Ordering::Relaxed), 1);
        assert!(f.runtime.tree().node(f.source).is_err());
        assert!(f.runtime.take_deferred_mutation_errors().is_empty());
        f.runtime
            .figure(f.root)
            .unwrap()
            .set_bounds(Rectangle::new(0.0, 0.0, 101.0, 100.0))
            .unwrap();
        frame(&mut f.runtime).unwrap();
        assert!(f.runtime.tree().node(f.source).is_err());
    }

    #[test]
    fn builder_late_callback_checks_parent_measurement_of_visible_and_hidden_children() {
        for hidden in [false, true] {
            let mut tree = novadraw::tree::FigureTree::new();
            let model = Arc::new(AtomicU64::new(3));
            let mutation = Arc::new(AtomicBool::new(true));
            let root = tree
                .builder()
                .set_contents(Box::new(Source {
                    inputs: registry(&model),
                    model: model.clone(),
                    width: Arc::new(AtomicU64::new(10)),
                    ordinary: Arc::new(AtomicBool::new(false)),
                    lifecycle_mutation: mutation,
                    focus_lost: Arc::new(AtomicUsize::new(0)),
                }))
                .unwrap();
            let child = tree
                .builder()
                .add_child(
                    root,
                    Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap()),
                )
                .unwrap();
            tree.builder()
                .set_layout_manager(
                    child,
                    Box::new(ModelLayout {
                        inputs: registry(&model),
                        value: model.clone(),
                        mutate: Arc::new(AtomicBool::new(false)),
                        ignore_error: false,
                    }),
                )
                .unwrap();
            tree.builder()
                .set_layout_manager(root, Box::new(MeasuringParent))
                .unwrap();
            tree.builder().set_visible(child, !hidden).unwrap();
            let old = tree.figure_bounds(child).unwrap();
            let error = tree.builder().validate_subtree(root).unwrap_err();
            assert_eq!(input_error(&error).source, Some(child));
            assert_eq!(model.load(Ordering::Relaxed), 4);
            assert_eq!(tree.figure_bounds(child).unwrap(), old);
            tree.builder().validate_subtree(root).unwrap();
            assert_eq!(tree.figure_bounds(child).unwrap().width, 4.0);
        }
    }
}
