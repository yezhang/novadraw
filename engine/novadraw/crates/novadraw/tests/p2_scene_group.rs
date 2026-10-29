use novadraw::{
    event::{EventContext, FigureEventHandler, MouseButton, MouseEvent},
    figure::{
        Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, INPUT, InputCapability,
        RectangleFigure,
    },
    geometry::Rectangle,
    runtime::{
        ComponentUpdateError, FigureComponentContext, FigureComponentUpdate, FigureUpdateEffect,
        FigureUpdateEffectContext, PreparedFigureUpdate, Runtime, RuntimeMutationError,
    },
    scene::{
        SCENE_SOURCE_VIEW, SOURCE_LOCAL_CAPTURE, SceneAdmissionError, SceneBindingGroup,
        SceneError, SceneSourceId, SceneViewDependencyKey, SceneViewDependencyPolicy,
        SourceLocalCapture,
    },
    tree::{FigureId, GraphMutationError},
};

fn participant(group: &SceneBindingGroup) -> (Runtime, FigureId, SceneSourceId) {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    runtime.scenes().join_group(group.clone()).unwrap();
    let source = runtime.scenes().register_source(root).unwrap();
    (runtime, root, source)
}

fn cycle_error() -> SceneError {
    SceneError::Admission(SceneAdmissionError::CyclicSceneViewBinding)
}

#[test]
fn hidden_zero_size_rect_outside_and_pending_views_are_structural_members() {
    let group = SceneBindingGroup::default();
    let (mut a, root_a, source_a) = participant(&group);
    let (mut b, root_b, source_b) = participant(&group);
    let view = a
        .figure(root_a)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(1000.0, 1000.0, 0.0, 0.0).expect("valid figure bounds"),
        ))
        .unwrap();
    a.figure(view).unwrap().set_visible(false).unwrap();
    a.scenes()
        .bind_view_dependency(view, SCENE_SOURCE_VIEW, source_b)
        .unwrap();
    let revision = group.graph_revision().unwrap();
    assert_eq!(
        b.scenes()
            .bind_view_dependency(root_b, SCENE_SOURCE_VIEW, source_a),
        Err(cycle_error())
    );
    assert_eq!(group.graph_revision().unwrap(), revision);
    a.figure(view).unwrap().set_visible(true).unwrap();
    a.figure(view)
        .unwrap()
        .set_bounds(Rectangle::new(1.0, 1.0, 20.0, 20.0))
        .unwrap();
    assert_eq!(group.graph_revision().unwrap(), revision);
    let (subscription, _, _) = a
        .scenes()
        .subscribe(
            source_a,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture::default(),
        )
        .unwrap();
    assert_eq!(
        a.scenes()
            .capture(subscription)
            .unwrap()
            .dependencies()
            .unwrap()
            .direct_sources,
        vec![source_b]
    );
}

#[test]
fn direct_reparent_rejects_cycle_before_changing_parent_or_graph() {
    let group = SceneBindingGroup::default();
    let (mut a, root_a, source_a) = participant(&group);
    let (mut b, root_b, source_b) = participant(&group);
    let outside = a
        .figure(root_a)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(0.0, 0.0, 50.0, 50.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let inside = a
        .figure(root_a)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(0.0, 0.0, 50.0, 50.0).expect("valid figure bounds"),
        ))
        .unwrap();
    // The registered source is a strict subtree; outside is excluded structurally.
    a.scenes().remove_source(source_a).unwrap();
    let source_a = a.scenes().register_source(inside).unwrap();
    let view = a
        .figure(outside)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(0.0, 0.0, 5.0, 5.0).expect("valid figure bounds"),
        ))
        .unwrap();
    a.scenes()
        .bind_view_dependency(view, SCENE_SOURCE_VIEW, source_b)
        .unwrap();
    b.scenes()
        .bind_view_dependency(root_b, SCENE_SOURCE_VIEW, source_a)
        .unwrap();
    let revision = group.graph_revision().unwrap();
    assert_eq!(
        a.figure(view).unwrap().reparent(inside),
        Err(RuntimeMutationError::Graph(
            GraphMutationError::SceneAdmission(SceneAdmissionError::CyclicSceneViewBinding)
        ))
    );
    assert_eq!(a.tree().parent_id(view).unwrap(), Some(outside));
    assert_eq!(group.graph_revision().unwrap(), revision);
}

struct Probe {
    value: u64,
}
impl Figure for Probe {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 5.0, 5.0)
    }
    fn name(&self) -> &'static str {
        "ExternalSceneProbe"
    }
}

struct MoveEffect {
    parent: FigureId,
}
impl FigureUpdateEffect for MoveEffect {
    fn prepare(
        self: Box<Self>,
        context: &mut FigureUpdateEffectContext<'_>,
    ) -> Result<(), RuntimeMutationError> {
        context.reparent(context.figure_id(), self.parent)
    }
}
struct MoveProbe {
    parent: FigureId,
}
impl FigureComponentUpdate for MoveProbe {
    type Figure = Probe;
    type Prepared = u64;
    type Error = ();
    fn prepare(
        self,
        target: &Probe,
        _: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<u64>, ()> {
        Ok(
            PreparedFigureUpdate::paint(target.value + 1).with_effect(MoveEffect {
                parent: self.parent,
            }),
        )
    }
    fn commit(value: u64, target: &mut Probe) {
        target.value = value;
    }
}

#[test]
fn typed_component_reparent_is_rejected_before_component_revision_changes() {
    let group = SceneBindingGroup::default();
    let (mut a, root_a, source_a) = participant(&group);
    let (mut b, root_b, source_b) = participant(&group);
    let inside = a
        .figure(root_a)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(0.0, 0.0, 50.0, 50.0).expect("valid figure bounds"),
        ))
        .unwrap();
    a.scenes().remove_source(source_a).unwrap();
    let source_a = a.scenes().register_source(inside).unwrap();
    let view = a
        .figure(root_a)
        .unwrap()
        .add(Box::new(Probe { value: 0 }))
        .unwrap();
    a.scenes()
        .bind_view_dependency(view, SCENE_SOURCE_VIEW, source_b)
        .unwrap();
    b.scenes()
        .bind_view_dependency(root_b, SCENE_SOURCE_VIEW, source_a)
        .unwrap();
    let revision = group.graph_revision().unwrap();
    assert_eq!(
        a.figure(view)
            .unwrap()
            .update_component(MoveProbe { parent: inside }),
        Err(ComponentUpdateError::Effect(RuntimeMutationError::Graph(
            GraphMutationError::SceneAdmission(SceneAdmissionError::CyclicSceneViewBinding)
        )))
    );
    assert_eq!(a.tree().parent_id(view).unwrap(), Some(root_a));
    assert_eq!(group.graph_revision().unwrap(), revision);
    // A subsequent legal update must still see the original component value.
    let receipt = a
        .figure(view)
        .unwrap()
        .update_component(MoveProbe { parent: root_a })
        .unwrap();
    assert_eq!(receipt.previous_revision, 0);
    assert_eq!(receipt.revision, 1);
}

const EXTERNAL_VIEW: SceneViewDependencyKey<ExternalConfig> =
    SceneViewDependencyKey::new("external.consumer", "view");
struct ExternalConfig(SceneSourceId);
struct ExternalDependency;
impl SceneViewDependencyPolicy<ExternalConfig> for ExternalDependency {
    fn source(&self, payload: &ExternalConfig) -> Result<SceneSourceId, SceneError> {
        Ok(payload.0)
    }
}

#[test]
fn external_dependency_uses_the_same_cycle_and_foreign_source_errors() {
    let group = SceneBindingGroup::default();
    let (mut a, root_a, source_a) = participant(&group);
    let (mut b, root_b, source_b) = participant(&group);
    a.scenes()
        .register_view_dependency_policy(EXTERNAL_VIEW, ExternalDependency)
        .unwrap();
    a.scenes()
        .bind_view_dependency(root_a, EXTERNAL_VIEW, ExternalConfig(source_b))
        .unwrap();
    assert_eq!(
        b.scenes()
            .bind_view_dependency(root_b, SCENE_SOURCE_VIEW, source_a),
        Err(cycle_error())
    );
    let foreign = SceneBindingGroup::default();
    let (_, _, source_foreign) = participant(&foreign);
    assert_eq!(
        a.scenes()
            .bind_view_dependency(root_a, EXTERNAL_VIEW, ExternalConfig(source_foreign)),
        Err(SceneError::Admission(
            SceneAdmissionError::UnknownSceneMember
        ))
    );
}

#[test]
fn removing_or_dropping_a_source_clears_outgoing_edges_without_reusing_identity() {
    let group = SceneBindingGroup::default();
    let (mut a, root_a, source_a) = participant(&group);
    let (mut b, root_b, source_b) = participant(&group);
    let binding = a
        .scenes()
        .bind_view_dependency(root_a, SCENE_SOURCE_VIEW, source_b)
        .unwrap();
    a.scenes().unbind_view_dependency(binding).unwrap();
    b.scenes()
        .bind_view_dependency(root_b, SCENE_SOURCE_VIEW, source_a)
        .unwrap();
    drop(b);
    assert_eq!(
        a.scenes()
            .bind_view_dependency(root_a, SCENE_SOURCE_VIEW, source_b),
        Err(SceneError::Admission(
            SceneAdmissionError::UnknownSceneMember
        ))
    );
}

struct DeferredView {
    parent: FigureId,
}
impl Figure for DeferredView {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 10.0, 10.0)
    }
    fn name(&self) -> &'static str {
        "ExternalDeferredView"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}
impl FigureEventHandler for DeferredView {
    fn on_mouse_pressed(&self, _: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let figure = context.target_id();
        context.effects().reparent_later(figure, self.parent);
        true
    }
}

#[test]
fn callback_deferred_reparent_uses_runtime_admission_and_does_not_retry_forever() {
    let group = SceneBindingGroup::default();
    let (mut a, root_a, source_a) = participant(&group);
    let (mut b, root_b, source_b) = participant(&group);
    let inside = a
        .figure(root_a)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(50.0, 50.0, 20.0, 20.0).expect("valid figure bounds"),
        ))
        .unwrap();
    a.scenes().remove_source(source_a).unwrap();
    let source_a = a.scenes().register_source(inside).unwrap();
    let view = a
        .figure(root_a)
        .unwrap()
        .add(Box::new(DeferredView { parent: inside }))
        .unwrap();
    a.scenes()
        .bind_view_dependency(view, SCENE_SOURCE_VIEW, source_b)
        .unwrap();
    b.scenes()
        .bind_view_dependency(root_b, SCENE_SOURCE_VIEW, source_a)
        .unwrap();
    a.frames().stabilize_for_query().unwrap();
    let revision = group.graph_revision().unwrap();
    a.input()
        .dispatch_mouse_pressed(2.0, 2.0, MouseButton::Left);
    assert_eq!(a.tree().parent_id(view).unwrap(), Some(root_a));
    assert_eq!(
        a.take_deferred_mutation_errors(),
        vec![RuntimeMutationError::Graph(
            GraphMutationError::SceneAdmission(SceneAdmissionError::CyclicSceneViewBinding)
        )]
    );
    a.frames().stabilize_for_query().unwrap();
    assert!(a.take_deferred_mutation_errors().is_empty());
    assert_eq!(group.graph_revision().unwrap(), revision);
}

#[test]
fn three_producers_reject_an_indirect_cycle() {
    let group = SceneBindingGroup::default();
    let (mut a, root_a, source_a) = participant(&group);
    let (mut b, root_b, source_b) = participant(&group);
    let (mut c, root_c, source_c) = participant(&group);
    a.scenes()
        .bind_view_dependency(root_a, SCENE_SOURCE_VIEW, source_b)
        .unwrap();
    b.scenes()
        .bind_view_dependency(root_b, SCENE_SOURCE_VIEW, source_c)
        .unwrap();
    let revision = group.graph_revision().unwrap();
    assert_eq!(
        c.scenes()
            .bind_view_dependency(root_c, SCENE_SOURCE_VIEW, source_a),
        Err(cycle_error())
    );
    assert_eq!(group.graph_revision().unwrap(), revision);
}

#[test]
fn subtree_replacement_commits_once_and_retires_structural_members() {
    let group = SceneBindingGroup::default();
    let (mut a, root_a, source_a) = participant(&group);
    let (_b, _, source_b) = participant(&group);
    a.scenes()
        .bind_view_dependency(root_a, SCENE_SOURCE_VIEW, source_b)
        .unwrap();
    let (subscription, _, handoff) = a
        .scenes()
        .subscribe(
            source_a,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture::default(),
        )
        .unwrap();
    a.scenes().capture(subscription).unwrap();
    let revision = group.graph_revision().unwrap();
    let new_root = a
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 30.0, 30.0).expect("valid figure bounds"),
        ))
        .unwrap();
    assert!(!a.tree().contains(root_a));
    assert!(a.tree().contains(new_root));
    assert_eq!(group.graph_revision().unwrap(), revision + 1);
    assert!(matches!(
        handoff.take().unwrap(),
        Some(novadraw::scene::SceneEvent::Closed(_))
    ));
}

#[test]
fn metadata_changes_refresh_evidence_without_changing_drawing_identity_or_polling_unrelated_members()
 {
    let group = SceneBindingGroup::default();
    let (mut a, root_a, source_a) = participant(&group);
    let (mut b, root_b, source_b) = participant(&group);
    let (_c, _, source_c) = participant(&group);
    let (subscription, _, _) = a
        .scenes()
        .subscribe(
            source_a,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture::default(),
        )
        .unwrap();
    let first = a.scenes().capture(subscription).unwrap();
    assert!(!a.scenes().needs_capture(subscription).unwrap());
    a.scenes()
        .bind_view_dependency(root_a, SCENE_SOURCE_VIEW, source_b)
        .unwrap();
    assert!(a.scenes().needs_capture(subscription).unwrap());
    let second = a.scenes().capture(subscription).unwrap();
    assert_eq!(first.stable_source_epoch(), second.stable_source_epoch());
    assert_eq!(first, second);
    assert_ne!(first.dependencies(), second.dependencies());
    assert!(!a.scenes().needs_capture(subscription).unwrap());
    b.scenes()
        .bind_view_dependency(root_b, SCENE_SOURCE_VIEW, source_c)
        .unwrap();
    assert!(!a.scenes().needs_capture(subscription).unwrap());
}
