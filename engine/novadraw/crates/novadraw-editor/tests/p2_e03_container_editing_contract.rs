mod support;

#[cfg(test)]
#[path = "support/container_allocations.rs"]
mod container_allocations;

use novadraw::{
    event::{KeyModifiers, MouseButton},
    figure::{ChildTransform, Figure, FigureContainer, RectangleFigure},
    geometry::{Affine2D, Dimension, Insets, Point, Rectangle},
};
use novadraw_editor::container::*;
use novadraw_editor::{
    Command, CommandError, EditPartBehavior, EditPartError, EditPartFactory, EditPolicy,
    EditorDomain, FeedbackVisual, GraphicalViewer, LayoutSlot, ModelAdapter, ModelEvent,
    ModelRevision, PartFactoryContext, PolicyError, PolicyHandlerBuilder, PolicyHost,
    PolicyInstallation, RequestKey, RequestMetadata, RequestProtocol, Tool, ToolContext,
    ToolDirective, ToolError, ToolMouseEvent, ViewerInputOutcome, VisualBuildContext,
    VisualUpdateContext,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    convert::Infallible,
    rc::Rc,
};

const ROOT: u32 = 1;
const LEFT: u32 = 2;
const RIGHT: u32 = 3;
const FIRST: u32 = 4;
const SECOND: u32 = 5;
const NESTED: u32 = 6;
const DESCENDANT: u32 = 7;
const LOCAL: u32 = 8;

#[derive(Clone, Debug, PartialEq)]
struct Node {
    bounds: Rectangle,
    children: Vec<u32>,
}

#[derive(Default)]
struct Trace {
    permissions: Vec<(u32, PermissionKind, Vec<u32>)>,
    build_count: usize,
    feedback_count: usize,
    reject: Option<(u32, PermissionKind)>,
    stale_header: Option<MovePlanHeader<u32>>,
    save_header: bool,
    ordered: bool,
    other_family_only: bool,
    reject_target: Option<u32>,
    duplicate_policy: bool,
    duplicate_reducer: bool,
    reducer_calls: usize,
    panic_cleanup: bool,
    projection_fail: bool,
    execute_fail: bool,
}

struct Model {
    nodes: HashMap<u32, Node>,
    revision: ModelRevision,
    events: Vec<ModelEvent<u32, ()>>,
    trace: Rc<RefCell<Trace>>,
    panes: HashMap<u32, Affine2D>,
    connections: Vec<novadraw_editor::ModelConnection<u32>>,
    child_reads: Cell<usize>,
}

impl Model {
    fn new() -> Self {
        let node = |bounds, children| Node { bounds, children };
        Self {
            nodes: HashMap::from([
                (
                    ROOT,
                    node(Rectangle::new(0.0, 0.0, 800.0, 600.0), vec![LEFT, RIGHT]),
                ),
                (
                    LEFT,
                    node(
                        Rectangle::new(50.0, 50.0, 230.0, 300.0),
                        vec![FIRST, SECOND, NESTED],
                    ),
                ),
                (
                    RIGHT,
                    node(Rectangle::new(350.0, 50.0, 230.0, 300.0), vec![LOCAL]),
                ),
                (FIRST, node(Rectangle::new(20.0, 20.0, 40.0, 30.0), vec![])),
                (
                    SECOND,
                    node(Rectangle::new(100.0, 30.0, 40.0, 30.0), vec![]),
                ),
                (
                    NESTED,
                    node(Rectangle::new(20.0, 150.0, 150.0, 100.0), vec![DESCENDANT]),
                ),
                (
                    DESCENDANT,
                    node(Rectangle::new(10.0, 10.0, 30.0, 20.0), vec![]),
                ),
                (
                    LOCAL,
                    node(Rectangle::new(150.0, 200.0, 40.0, 30.0), vec![]),
                ),
            ]),
            revision: ModelRevision::initial(),
            events: Vec::new(),
            trace: Rc::new(RefCell::new(Trace::default())),
            panes: HashMap::new(),
            connections: vec![],
            child_reads: Cell::new(0),
        }
    }

    fn publish(&mut self) {
        self.revision = self.revision.next().unwrap();
        self.events.push(ModelEvent::new(self.revision, ROOT, ()));
    }
}

impl ModelAdapter for Model {
    type ModelId = u32;
    type Event = ();
    type Error = Infallible;
    fn root(&self) -> u32 {
        ROOT
    }
    fn revision(&self) -> ModelRevision {
        self.revision
    }
    fn children(&self, model: u32) -> Result<Vec<u32>, Infallible> {
        self.child_reads.set(self.child_reads.get() + 1);
        Ok(self.nodes[&model].children.clone())
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<u32, ()>> {
        std::mem::take(&mut self.events)
    }
    fn connections(&self) -> Result<Vec<novadraw_editor::ModelConnection<u32>>, Infallible> {
        Ok(self.connections.clone())
    }
}

struct MoveFamily;
struct Protocol;

impl RequestProtocol<Model> for MoveFamily {
    type Payload = MoveCandidate<u32>;
    type Contribution = Plan;
    fn key() -> RequestKey<Self> {
        RequestKey::new("external-test-container-move")
    }
    fn reduce(
        mut plans: Vec<Plan>,
    ) -> Result<novadraw_editor::RequestResolution<Plan>, PolicyError> {
        let duplicate = if let Some(plan) = plans.first() {
            let mut trace = plan.trace.borrow_mut();
            trace.reducer_calls += 1;
            trace.duplicate_reducer.then(|| plan.clone())
        } else {
            None
        };
        if let Some(plan) = duplicate {
            plans.push(plan);
        }
        // An adversarial reducer might hide conflicting raw plans. The host limit runs first.
        if plans.len() > 1 && !plans[0].trace.borrow().duplicate_reducer {
            plans.truncate(1);
        }
        Ok(novadraw_editor::RequestResolution::all(plans))
    }
}

impl ContainerMoveProtocol<Model> for Protocol {
    type Payload = MoveCandidate<u32>;
    type Plan = Plan;
    type Request = MoveFamily;
    fn payload(candidate: &MoveCandidate<u32>) -> Self::Payload {
        candidate.clone()
    }
    fn source_geometry(
        model: &Model,
        source: &MoveSource<u32>,
    ) -> Result<ConstraintGeometry, ContainerEditError> {
        ConstraintGeometry::xy(model.nodes[&source.model()].bounds)
    }
}

#[derive(Clone)]
struct Plan {
    header: MovePlanHeader<u32>,
    candidate: MoveCandidate<u32>,
    trace: Rc<RefCell<Trace>>,
    insertion: Option<InsertionPosition<u32>>,
}

impl MovePlan<Model> for Plan {
    fn header(&self) -> &MovePlanHeader<u32> {
        &self.header
    }
    fn feedback(&self) -> Result<Vec<FeedbackVisual>, ContainerEditError> {
        self.trace.borrow_mut().feedback_count += 1;
        self.candidate
            .geometry()
            .iter()
            .map(|geometry| {
                // Application projects target-parent geometry before returning feedback.
                let bounds = geometry
                    .mapped(self.candidate.target_to_surface())?
                    .xy_bounds(GeometryTolerance::EXACT)?;
                Ok(feedback_visual(bounds, self.trace.clone()))
            })
            .collect()
    }
    fn build_command(
        self,
        model: &Model,
    ) -> Result<Option<Box<dyn Command<Model>>>, ContainerEditError> {
        self.trace.borrow_mut().build_count += 1;
        if model.revision != self.header.revision() {
            return Err(ContainerEditError::Invalidated);
        }
        let before = model.nodes.clone();
        let mut after = before.clone();
        let sources = self
            .header
            .sources()
            .iter()
            .map(|source| source.model())
            .collect::<Vec<_>>();
        let target = self.header.target_model();
        for node in after.values_mut() {
            node.children.retain(|id| !sources.contains(id));
        }
        let target_children = &mut after.get_mut(&target).unwrap().children;
        *target_children = ordered_insertion(
            target_children,
            &sources,
            self.insertion.unwrap_or(InsertionPosition::End),
        )?;
        for (source, geometry) in self.header.sources().iter().zip(self.candidate.geometry()) {
            after.get_mut(&source.model()).unwrap().bounds =
                geometry.xy_bounds(GeometryTolerance::EXACT)?;
        }
        if before == after {
            return Ok(None);
        }
        Ok(Some(Box::new(AtomicMove {
            before,
            after,
            baseline: model.revision,
        })))
    }
}

struct AtomicMove {
    before: HashMap<u32, Node>,
    after: HashMap<u32, Node>,
    baseline: ModelRevision,
}

impl Command<Model> for AtomicMove {
    fn label(&self) -> &str {
        "Atomic external container move"
    }
    fn execute(&mut self, model: &mut Model) -> Result<(), CommandError> {
        if model.trace.borrow().execute_fail {
            return Err(CommandError::operation(
                "external container execute refusal",
            ));
        }
        if model.revision != self.baseline || model.nodes != self.before {
            return Err(CommandError::operation(
                "model changed before container commit",
            ));
        }
        model.nodes = self.after.clone();
        model.publish();
        Ok(())
    }
    fn undo(&mut self, model: &mut Model) -> Result<(), CommandError> {
        if model.nodes != self.after {
            return Err(CommandError::operation("undo baseline changed"));
        }
        model.nodes = self.before.clone();
        model.publish();
        Ok(())
    }
    fn redo(&mut self, model: &mut Model) -> Result<(), CommandError> {
        if model.nodes != self.before {
            return Err(CommandError::operation("redo baseline changed"));
        }
        model.nodes = self.after.clone();
        model.publish();
        Ok(())
    }
}

struct ContainerPolicy;
struct OtherOnlyPolicy;
struct ExtraSlot;
impl novadraw_editor::PolicySlot for ExtraSlot {
    const NAME: &'static str = "external-container-extra";
}
impl EditPolicy<Model> for OtherOnlyPolicy {
    fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Model, Self>) {
        builder.handle::<OtherMoveFamily>(|_, host, metadata, payload, model| {
            ContainerPolicy.other_plan(host, metadata, payload, model)
        });
    }
}

impl EditPolicy<Model> for ContainerPolicy {
    fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Model, Self>) {
        builder.handle::<MoveFamily>(Self::plan);
        builder.handle::<CheckContainerPermission>(Self::permission);
        builder.handle::<OtherMoveFamily>(Self::other_plan);
        builder.handle::<CreateFamily>(Self::create);
    }
}

impl ContainerPolicy {
    fn plan(
        &mut self,
        host: PolicyHost<u32>,
        metadata: &RequestMetadata,
        candidate: &MoveCandidate<u32>,
        model: &Model,
    ) -> Result<Option<Plan>, PolicyError> {
        assert_eq!(host.part(), candidate.target());
        assert_eq!(
            metadata.source_parts(),
            candidate
                .batch()
                .sources()
                .iter()
                .map(|source| source.part())
                .collect::<Vec<_>>()
        );
        let mut trace = model.trace.borrow_mut();
        if trace.reject_target == Some(host.model()) {
            return Err(PolicyError::operation("explicit target refusal"));
        }
        if trace.other_family_only && host.model() == NESTED {
            return Ok(None);
        }
        let header = trace
            .stale_header
            .clone()
            .unwrap_or_else(|| MovePlanHeader::from_candidate(candidate));
        if trace.save_header {
            trace.stale_header = Some(MovePlanHeader::from_candidate(candidate));
            trace.save_header = false;
        }
        Ok(Some(Plan {
            header,
            candidate: candidate.clone(),
            trace: model.trace.clone(),
            insertion: (trace.ordered && host.model() == RIGHT).then_some(InsertionPosition::Start),
        }))
    }
    fn permission(
        &mut self,
        host: PolicyHost<u32>,
        metadata: &RequestMetadata,
        query: &PermissionQuery<u32>,
        model: &Model,
    ) -> Result<Option<PermissionDecision>, PolicyError> {
        assert_eq!(host.part(), query.host());
        assert!(query.is_for::<MoveFamily>() || query.is_for::<OtherMoveFamily>());
        assert_eq!(query.candidate(), query.move_candidate().id());
        assert_eq!(query.revision(), model.revision());
        assert_eq!(metadata.revision(), query.interaction_revision());
        assert_eq!(
            metadata.source_parts(),
            query
                .sources()
                .iter()
                .map(|source| source.part())
                .collect::<Vec<_>>()
        );
        let mut trace = model.trace.borrow_mut();
        trace.permissions.push((
            host.model(),
            query.kind(),
            query
                .sources()
                .iter()
                .map(|source| source.model())
                .collect(),
        ));
        Ok(Some(
            if trace.reject == Some((host.model(), query.kind())) {
                PermissionDecision::Reject("external parent refused".into())
            } else {
                PermissionDecision::Allow
            },
        ))
    }

    fn other_plan(
        &mut self,
        host: PolicyHost<u32>,
        metadata: &RequestMetadata,
        payload: &OtherPayload,
        model: &Model,
    ) -> Result<Option<OtherPlan>, PolicyError> {
        self.plan(host, metadata, &payload.0, model)
            .map(|plan| plan.map(OtherPlan))
    }

    fn create(
        &mut self,
        host: PolicyHost<u32>,
        metadata: &RequestMetadata,
        payload: &CreatePayload,
        model: &Model,
    ) -> Result<Option<CreatePlan>, PolicyError> {
        assert!(metadata.source_parts().is_empty());
        assert_eq!(host.part(), payload.candidate.target());
        if model.trace.borrow().reject_target == Some(host.model()) {
            return Err(PolicyError::operation("creation refused"));
        }
        Ok(Some(CreatePlan {
            header: CreationPlanHeader::from_candidate(&payload.candidate),
            id: payload.id,
            trace: model.trace.clone(),
        }))
    }
}

struct OtherMoveFamily;
struct OtherPayload(MoveCandidate<u32>);
struct OtherPlan(Plan);
struct OtherProtocol;

impl RequestProtocol<Model> for OtherMoveFamily {
    type Payload = OtherPayload;
    type Contribution = OtherPlan;
    fn key() -> RequestKey<Self> {
        RequestKey::new("external-ordered-move")
    }
}
impl ContainerMoveProtocol<Model> for OtherProtocol {
    type Payload = OtherPayload;
    type Plan = OtherPlan;
    type Request = OtherMoveFamily;
    fn payload(candidate: &MoveCandidate<u32>) -> OtherPayload {
        OtherPayload(candidate.clone())
    }
    fn source_geometry(
        model: &Model,
        source: &MoveSource<u32>,
    ) -> Result<ConstraintGeometry, ContainerEditError> {
        Protocol::source_geometry(model, source)
    }
}
impl MovePlan<Model> for OtherPlan {
    fn header(&self) -> &MovePlanHeader<u32> {
        self.0.header()
    }
    fn feedback(&self) -> Result<Vec<FeedbackVisual>, ContainerEditError> {
        self.0.feedback()
    }
    fn build_command(
        self,
        model: &Model,
    ) -> Result<Option<Box<dyn Command<Model>>>, ContainerEditError> {
        self.0.build_command(model)
    }
}

struct CreateFamily;
struct CreatePayload {
    candidate: CreationCandidate<u32>,
    id: u32,
}
struct Template(u32);
struct CreatePlan {
    header: CreationPlanHeader<u32>,
    id: u32,
    trace: Rc<RefCell<Trace>>,
}
impl RequestProtocol<Model> for CreateFamily {
    type Payload = CreatePayload;
    type Contribution = CreatePlan;
    fn key() -> RequestKey<Self> {
        RequestKey::new("external-parent-creation")
    }
}
impl ContainerCreationTemplate<Model> for Template {
    type Payload = CreatePayload;
    type Plan = CreatePlan;
    type Request = CreateFamily;
    fn default_size(&self) -> Dimension {
        Dimension::new(30.0, 20.0)
    }
    fn payload(&self, candidate: &CreationCandidate<u32>) -> CreatePayload {
        CreatePayload {
            candidate: candidate.clone(),
            id: self.0,
        }
    }
}
impl CreationPlan<Model> for CreatePlan {
    fn header(&self) -> &CreationPlanHeader<u32> {
        &self.header
    }
    fn feedback(&self) -> Result<Vec<FeedbackVisual>, ContainerEditError> {
        self.trace.borrow_mut().feedback_count += 1;
        let candidate = self.header.candidate();
        let bounds = ConstraintGeometry::xy(candidate.bounds())?
            .mapped(candidate.target_to_surface())?
            .xy_bounds(GeometryTolerance::EXACT)?;
        Ok(vec![feedback_visual(bounds, self.trace.clone())])
    }
    fn build_command(
        self,
        model: &Model,
    ) -> Result<Option<Box<dyn Command<Model>>>, ContainerEditError> {
        self.trace.borrow_mut().build_count += 1;
        let candidate = self.header.candidate();
        if candidate.revision() != model.revision || model.nodes.contains_key(&self.id) {
            return Err(ContainerEditError::Rejected(
                "creation identity conflict".into(),
            ));
        }
        let before = model.nodes.clone();
        let mut after = before.clone();
        after
            .get_mut(&candidate.target_model())
            .unwrap()
            .children
            .push(self.id);
        after.insert(
            self.id,
            Node {
                bounds: candidate.bounds(),
                children: vec![],
            },
        );
        Ok(Some(Box::new(AtomicMove {
            before,
            after,
            baseline: model.revision,
        })))
    }
}

struct Part;
impl EditPartBehavior<Model> for Part {
    fn create_figure(&mut self, model: &Model, id: u32) -> Result<Box<dyn Figure>, EditPartError> {
        if id == FIRST && model.trace.borrow().projection_fail {
            return Err(EditPartError::operation("external projection failed"));
        }
        Ok(Box::new(
            RectangleFigure::from_bounds(model.nodes[&id].bounds).unwrap(),
        ))
    }
    fn refresh_visuals(
        &mut self,
        model: &Model,
        id: u32,
        context: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        context.set_primary_bounds(model.nodes[&id].bounds)?;
        Ok(())
    }
    fn configure_visual(
        &mut self,
        model: &Model,
        id: u32,
        context: &mut VisualBuildContext<'_>,
    ) -> Result<(), EditPartError> {
        if let Some(transform) = model.panes.get(&id) {
            let bounds = model.nodes[&id].bounds;
            let pane = context.add_child(
                context.primary(),
                Box::new(Pane {
                    bounds: Rectangle::new(7.0, 11.0, bounds.width - 20.0, bounds.height - 20.0),
                    transform: *transform,
                }),
            )?;
            context.set_content_pane(pane)?;
        }
        Ok(())
    }
    fn create_policies(
        &mut self,
        model: &Model,
        id: u32,
    ) -> Result<Vec<PolicyInstallation<Model>>, EditPartError> {
        // The release delivery binary defines this feature; contract tests keep every handler.
        #[cfg(not(test))]
        if !cfg!(feature = "container-cost") {
            return Ok(Vec::new());
        }
        if [ROOT, LEFT, RIGHT, NESTED].contains(&id) || (1000..2000).contains(&id) {
            if model.trace.borrow().other_family_only && id == NESTED {
                return Ok(vec![
                    PolicyInstallation::new(LayoutSlot::key(), OtherOnlyPolicy)
                        .map_err(|error| EditPartError::operation(error.to_string()))?,
                ]);
            }
            let mut policies = vec![
                PolicyInstallation::new(LayoutSlot::key(), ContainerPolicy)
                    .map_err(|error| EditPartError::operation(error.to_string()))?,
            ];
            if model.trace.borrow().duplicate_policy && id == RIGHT {
                policies.push(
                    PolicyInstallation::new(
                        novadraw_editor::PolicyKey::<ExtraSlot>::new("external-container-extra"),
                        ContainerPolicy,
                    )
                    .map_err(|error| EditPartError::operation(error.to_string()))?,
                );
            }
            Ok(policies)
        } else {
            Ok(vec![])
        }
    }
}

struct Pane {
    bounds: Rectangle,
    transform: Affine2D,
}
impl Figure for Pane {
    fn name(&self) -> &'static str {
        "ExternalContainerPane"
    }
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn initial_insets(&self) -> Insets {
        Insets {
            top: 3.0,
            left: 5.0,
            bottom: 0.0,
            right: 0.0,
        }
    }
    fn register_capabilities(
        &self,
        out: &mut novadraw::figure::FigureCapabilityBuilder,
    ) -> Result<(), novadraw::figure::FigureCapabilityRegistrationError> {
        out.register(
            novadraw::figure::CONTAINER,
            novadraw::figure::ContainerCapability::of::<Self>(),
        )
    }
}
impl FigureContainer for Pane {
    fn child_transform(&self) -> ChildTransform {
        ChildTransform::from_affine(self.transform)
    }
}

struct FeedbackFigure {
    bounds: Rectangle,
    trace: Rc<RefCell<Trace>>,
}
impl Figure for FeedbackFigure {
    fn name(&self) -> &'static str {
        "ExternalContainerFeedback"
    }
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn register_capabilities(
        &self,
        out: &mut novadraw::figure::FigureCapabilityBuilder,
    ) -> Result<(), novadraw::figure::FigureCapabilityRegistrationError> {
        out.register(
            novadraw::figure::LIFECYCLE,
            novadraw::figure::LifecycleCapability::of::<Self>(),
        )
    }
}
impl novadraw::figure::FigureLifecycle for FeedbackFigure {
    fn on_detached(&mut self, _: novadraw::figure::FigureLifecycleContext) {
        assert!(
            !self.trace.borrow().panic_cleanup,
            "external feedback cleanup panic"
        );
    }
}
fn feedback_visual(bounds: Rectangle, trace: Rc<RefCell<Trace>>) -> FeedbackVisual {
    FeedbackVisual::unscaled(Box::new(FeedbackFigure { bounds, trace }))
}

type ProbeAction = Box<dyn FnMut(&mut ToolContext<'_, Model>) -> Result<(), ToolError>>;
struct Probe(ProbeAction);
impl Tool<Model> for Probe {
    fn key(&self) -> novadraw_editor::ErasedToolKey {
        novadraw_editor::ToolKey::<Self>::new("external-probe").erase()
    }
    fn activate(&mut self, ctx: &mut ToolContext<'_, Model>) -> Result<(), ToolError> {
        (self.0)(ctx)
    }
    fn cancel(&mut self, _: &mut ToolContext<'_, Model>) -> Result<(), ToolError> {
        Ok(())
    }
    fn mouse_pressed(
        &mut self,
        ctx: &mut ToolContext<'_, Model>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError> {
        Ok((ToolDirective::Continue, ctx.dispatch_pressed(event, false)?))
    }
    fn mouse_moved(
        &mut self,
        ctx: &mut ToolContext<'_, Model>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, novadraw::event::DispatchOutcome), ToolError> {
        Ok((
            ToolDirective::Continue,
            ctx.dispatch_moved(event.location()),
        ))
    }
    fn mouse_released(
        &mut self,
        ctx: &mut ToolContext<'_, Model>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, novadraw::event::DispatchOutcome), ToolError> {
        Ok((
            ToolDirective::Continue,
            ctx.dispatch_released(event.location(), event.button()),
        ))
    }
}

struct Factory;
impl EditPartFactory<Model> for Factory {
    fn create(
        &mut self,
        _: PartFactoryContext<u32>,
        _: &Model,
    ) -> Result<Box<dyn EditPartBehavior<Model>>, EditPartError> {
        Ok(Box::new(Part))
    }
    fn create_connection(
        &mut self,
        _: novadraw_editor::ConnectionPartFactoryContext<u32>,
        _: &Model,
    ) -> Result<Box<dyn EditPartBehavior<Model>>, EditPartError> {
        Ok(Box::new(EdgePart))
    }
}
struct EdgePart;
impl EditPartBehavior<Model> for EdgePart {
    fn create_figure(&mut self, _: &Model, _: u32) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(Box::new(novadraw::connection::ConnectionFigure::new()))
    }
}

fn editor() -> (GraphicalViewer<Model, Factory>, EditorDomain<Model>) {
    let mut viewer = GraphicalViewer::new(
        Model::new(),
        Factory,
        Rectangle::new(0.0, 0.0, 800.0, 600.0),
    )
    .unwrap();
    support::prepare_next_submission(&mut viewer);
    let mut domain = EditorDomain::new();
    domain
        .set_active_tool(&mut viewer, ContainerMoveTool::<Model, Protocol>::new())
        .unwrap();
    (viewer, domain)
}

/// Matched release consumer: initialize one Viewer, optionally execute container creation.
#[cfg(not(test))]
pub fn delivery_cost_probe<const CONTAINER: bool>() -> usize {
    let mut viewer = GraphicalViewer::new(
        Model::new(),
        Factory,
        Rectangle::new(0.0, 0.0, 800.0, 600.0),
    )
    .unwrap();
    support::prepare_next_submission(&mut viewer);
    let mut domain = EditorDomain::new();
    if CONTAINER {
        domain
            .set_active_tool(&mut viewer, ContainerCreationTool::new(Template(20)))
            .unwrap();
        drag(
            &mut viewer,
            &mut domain,
            Point::new(400.0, 120.0),
            Point::new(440.0, 160.0),
        );
        select(&mut viewer, &[FIRST]);
        domain
            .set_active_tool(&mut viewer, ContainerMoveTool::<Model, Protocol>::new())
            .unwrap();
        drag(
            &mut viewer,
            &mut domain,
            Point::new(80.0, 80.0),
            Point::new(400.0, 120.0),
        );
    }
    viewer.model().nodes.len() + domain.command_stack().undo_len()
}

fn select(viewer: &mut GraphicalViewer<Model, Factory>, ids: &[u32]) {
    viewer
        .set_selection(
            &ids.iter()
                .map(|id| viewer.part_for_model(*id).unwrap())
                .collect::<Vec<_>>(),
        )
        .unwrap();
}
fn drag(
    viewer: &mut GraphicalViewer<Model, Factory>,
    domain: &mut EditorDomain<Model>,
    start: Point,
    end: Point,
) {
    domain
        .pointer_pressed(viewer, start, MouseButton::Left, KeyModifiers::default())
        .unwrap();
    domain.pointer_moved(viewer, end).unwrap();
    domain
        .pointer_released(viewer, end, MouseButton::Left)
        .unwrap();
}

#[test]
fn cross_parent_move_is_atomic_and_rebuilds_selected_model_identities() {
    let (mut viewer, mut domain) = editor();
    select(&mut viewer, &[FIRST, SECOND]);
    let old_first = viewer.part_for_model(FIRST).unwrap();
    let old_second = viewer.part_for_model(SECOND).unwrap();
    let independent_focus = viewer.part_for_model(LOCAL).unwrap();
    viewer.set_focus(Some(independent_focus)).unwrap();
    drag(
        &mut viewer,
        &mut domain,
        Point::new(80.0, 80.0),
        Point::new(400.0, 120.0),
    );
    assert_eq!(
        viewer.model().nodes[&RIGHT].children,
        vec![LOCAL, FIRST, SECOND]
    );
    assert_eq!(viewer.model().nodes[&LEFT].children, vec![NESTED]);
    assert_eq!(
        viewer.model().nodes[&FIRST].bounds,
        Rectangle::new(40.0, 60.0, 40.0, 30.0)
    );
    assert_eq!(
        viewer.model().nodes[&SECOND].bounds,
        Rectangle::new(120.0, 70.0, 40.0, 30.0)
    );
    assert_ne!(viewer.part_for_model(FIRST), Some(old_first));
    assert_ne!(viewer.part_for_model(SECOND), Some(old_second));
    assert_eq!(
        viewer.selection().items(),
        [
            viewer.part_for_model(FIRST).unwrap(),
            viewer.part_for_model(SECOND).unwrap(),
        ]
    );
    assert_eq!(viewer.selection().focus(), Some(independent_focus));
    assert_eq!(domain.command_stack().undo_len(), 1);
    assert_eq!(viewer.model().trace.borrow().build_count, 1);
    // Each history operation captures the current selection, rather than restoring execute's selection.
    select(&mut viewer, &[SECOND, LOCAL]);
    domain.undo(&mut viewer).unwrap();
    assert_eq!(
        viewer.selection().items(),
        [
            viewer.part_for_model(SECOND).unwrap(),
            viewer.part_for_model(LOCAL).unwrap(),
        ]
    );
    select(&mut viewer, &[FIRST]);
    domain.redo(&mut viewer).unwrap();
    assert_eq!(
        viewer.selection().items(),
        [viewer.part_for_model(FIRST).unwrap()]
    );
}

#[test]
fn ancestor_dedup_preserves_descendant_primary_and_uses_covering_anchor() {
    let (mut viewer, mut domain) = editor();
    select(&mut viewer, &[NESTED, DESCENDANT]);
    drag(
        &mut viewer,
        &mut domain,
        Point::new(90.0, 220.0),
        Point::new(410.0, 120.0),
    );
    assert_eq!(viewer.model().nodes[&RIGHT].children, vec![LOCAL, NESTED]);
    assert_eq!(viewer.model().nodes[&NESTED].children, vec![DESCENDANT]);
    assert_eq!(
        viewer.selection().primary(),
        viewer.part_for_model(DESCENDANT)
    );
    assert!(
        viewer
            .model()
            .trace
            .borrow()
            .permissions
            .iter()
            .all(|(_, _, ids)| ids == &[NESTED])
    );
}

#[test]
fn mixed_local_and_incoming_sources_query_precise_parent_subbatches() {
    let (mut viewer, mut domain) = editor();
    select(&mut viewer, &[LOCAL, FIRST]);
    viewer.model().trace.borrow_mut().reject = Some((RIGHT, PermissionKind::Removal));
    drag(
        &mut viewer,
        &mut domain,
        Point::new(80.0, 80.0),
        Point::new(400.0, 120.0),
    );
    let trace = viewer.model().trace.borrow();
    assert!(
        trace
            .permissions
            .contains(&(RIGHT, PermissionKind::LocalModification, vec![LOCAL]))
    );
    assert!(
        trace
            .permissions
            .contains(&(LEFT, PermissionKind::Removal, vec![FIRST]))
    );
    assert!(
        !trace
            .permissions
            .iter()
            .any(|(host, kind, _)| *host == RIGHT && *kind == PermissionKind::Removal)
    );
    assert_eq!(domain.command_stack().undo_len(), 1);
}

#[test]
fn rejected_parent_permission_prevents_feedback_and_command_building() {
    let (mut viewer, mut domain) = editor();
    select(&mut viewer, &[FIRST]);
    let before = viewer.model().nodes.clone();
    viewer.model().trace.borrow_mut().reject = Some((LEFT, PermissionKind::Removal));
    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(80.0, 80.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    assert!(
        domain
            .pointer_moved(&mut viewer, Point::new(400.0, 120.0))
            .is_err()
    );
    assert!(!domain.has_active_gesture());
    assert_eq!(viewer.model().nodes, before);
    assert_eq!(domain.command_stack().undo_len(), 0);
    assert_eq!(viewer.model().trace.borrow().feedback_count, 0);
    assert_eq!(viewer.model().trace.borrow().build_count, 0);
}

#[test]
fn stale_header_cannot_reuse_previous_candidate_permission() {
    let (mut viewer, mut domain) = editor();
    select(&mut viewer, &[FIRST]);
    viewer.model().trace.borrow_mut().save_header = true;
    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(80.0, 80.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    // First preview saves a header; the next fresh computation must reject it.
    domain
        .pointer_moved(&mut viewer, Point::new(400.0, 120.0))
        .unwrap();
    assert!(
        domain
            .pointer_released(&mut viewer, Point::new(420.0, 130.0), MouseButton::Left)
            .is_err()
    );
    assert_eq!(domain.command_stack().undo_len(), 0);
    assert_eq!(viewer.model().trace.borrow().build_count, 0);
}

#[test]
fn same_tool_accepts_xy_to_ordered_and_an_independent_request_payload_plan() {
    for second_protocol in [false, true] {
        let (mut viewer, mut domain) = editor();
        viewer.model().trace.borrow_mut().ordered = true;
        if second_protocol {
            domain
                .set_active_tool(
                    &mut viewer,
                    ContainerMoveTool::<Model, OtherProtocol>::new(),
                )
                .unwrap();
        }
        select(&mut viewer, &[SECOND, FIRST]);
        drag(
            &mut viewer,
            &mut domain,
            Point::new(80.0, 80.0),
            Point::new(400.0, 120.0),
        );
        assert_eq!(
            viewer.model().nodes[&RIGHT].children,
            [SECOND, FIRST, LOCAL]
        );
        assert_eq!(domain.command_stack().undo_len(), 1);
        assert!(viewer.model().trace.borrow().permissions.contains(&(
            LEFT,
            PermissionKind::Removal,
            vec![SECOND, FIRST]
        )));
    }
}

#[test]
fn unsupported_inner_family_probes_parent_but_explicit_refusal_stops() {
    for refused in [false, true] {
        let model = Model::new();
        model.trace.borrow_mut().other_family_only = !refused;
        let mut viewer =
            GraphicalViewer::new(model, Factory, Rectangle::new(0.0, 0.0, 800.0, 600.0)).unwrap();
        support::prepare_next_submission(&mut viewer);
        let mut domain = EditorDomain::new();
        domain
            .set_active_tool(&mut viewer, ContainerMoveTool::<Model, Protocol>::new())
            .unwrap();
        let before = viewer.model().nodes.clone();
        if refused {
            viewer.model().trace.borrow_mut().reject_target = Some(NESTED);
        } else {
            viewer.model().trace.borrow_mut().other_family_only = true;
        }
        select(&mut viewer, &[FIRST]);
        domain
            .pointer_pressed(
                &mut viewer,
                Point::new(80.0, 80.0),
                MouseButton::Left,
                KeyModifiers::default(),
            )
            .unwrap();
        let result =
            domain.pointer_released(&mut viewer, Point::new(100.0, 260.0), MouseButton::Left);
        if refused {
            assert!(result.is_err());
            assert_eq!(viewer.model().nodes, before);
            assert_eq!(domain.command_stack().undo_len(), 0);
        } else {
            result.unwrap();
            assert_eq!(
                viewer.model().nodes[&LEFT].children,
                [SECOND, NESTED, FIRST]
            );
            assert_eq!(domain.command_stack().undo_len(), 1);
        }
    }
}

#[test]
fn raw_and_reduced_duplicate_plans_fail_before_feedback_or_permissions() {
    for raw in [false, true] {
        let model = Model::new();
        model.trace.borrow_mut().duplicate_policy = raw;
        model.trace.borrow_mut().duplicate_reducer = !raw;
        let mut viewer =
            GraphicalViewer::new(model, Factory, Rectangle::new(0.0, 0.0, 800.0, 600.0)).unwrap();
        support::prepare_next_submission(&mut viewer);
        let mut domain = EditorDomain::new();
        domain
            .set_active_tool(&mut viewer, ContainerMoveTool::<Model, Protocol>::new())
            .unwrap();
        select(&mut viewer, &[FIRST]);
        domain
            .pointer_pressed(
                &mut viewer,
                Point::new(80.0, 80.0),
                MouseButton::Left,
                Default::default(),
            )
            .unwrap();
        assert!(
            domain
                .pointer_released(&mut viewer, Point::new(400.0, 120.0), MouseButton::Left)
                .is_err()
        );
        let trace = viewer.model().trace.borrow();
        assert_eq!(trace.reducer_calls, usize::from(!raw));
        assert!(trace.permissions.is_empty());
        assert_eq!(trace.feedback_count, 0);
        assert_eq!(trace.build_count, 0);
        assert_eq!(domain.command_stack().undo_len(), 0);
    }
}

#[test]
fn multi_original_parents_query_each_removal_and_programmatic_noop_adds_no_history() {
    let (mut viewer, mut domain) = editor();
    let first = viewer.part_for_model(FIRST).unwrap();
    let descendant = viewer.part_for_model(DESCENDANT).unwrap();
    domain
        .set_active_tool(
            &mut viewer,
            ContainerMoveTool::<Model, Protocol>::from_sources(
                vec![descendant, first],
                Some(first),
                Point::new(80.0, 80.0),
                novadraw_editor::InteractionRevision::initial(),
            ),
        )
        .unwrap();
    domain
        .pointer_released(&mut viewer, Point::new(400.0, 120.0), MouseButton::Left)
        .unwrap();
    let trace = viewer.model().trace.borrow();
    assert!(
        trace
            .permissions
            .contains(&(NESTED, PermissionKind::Removal, vec![DESCENDANT]))
    );
    assert!(
        trace
            .permissions
            .contains(&(LEFT, PermissionKind::Removal, vec![FIRST]))
    );
    assert_eq!(
        viewer.model().nodes[&RIGHT].children,
        [LOCAL, DESCENDANT, FIRST]
    );
    drop(trace);
    let (mut viewer, mut domain) = editor();
    let nested = viewer.part_for_model(NESTED).unwrap();
    domain
        .set_active_tool(
            &mut viewer,
            ContainerMoveTool::<Model, Protocol>::from_sources(
                vec![nested],
                None,
                Point::new(100.0, 260.0),
                novadraw_editor::InteractionRevision::initial(),
            ),
        )
        .unwrap();
    domain
        .pointer_released(&mut viewer, Point::new(100.0, 260.0), MouseButton::Left)
        .unwrap();
    assert_eq!(domain.command_stack().undo_len(), 0);
}

#[test]
fn retired_acquired_feedback_rejects_cleanup_before_build_for_both_tools() {
    for creating in [false, true] {
        let (mut viewer, mut domain) = editor();
        let before = viewer.model().nodes.clone();
        let revision = viewer.model().revision;
        let start = if creating {
            domain
                .set_active_tool(&mut viewer, ContainerCreationTool::new(Template(20)))
                .unwrap();
            Point::new(400.0, 120.0)
        } else {
            select(&mut viewer, &[FIRST]);
            Point::new(80.0, 80.0)
        };
        let pointer = Point::new(440.0, 160.0);
        domain
            .pointer_pressed(&mut viewer, start, MouseButton::Left, Default::default())
            .unwrap();
        domain.pointer_moved(&mut viewer, pointer).unwrap();
        let layer = viewer.root_layers().feedback();
        let acquired = viewer.runtime().tree().child_order(layer).unwrap();
        let feedback = acquired
            .into_iter()
            .find(|figure| viewer.visual_owner(*figure).is_some())
            .expect("the real tool acquired feedback");
        viewer.remove_overlay_visual(feedback).unwrap();
        let error = domain
            .pointer_released(&mut viewer, pointer, MouseButton::Left)
            .unwrap_err();
        assert!(error.to_string().contains("Figure is not owned Overlay"));
        assert_eq!(viewer.model().trace.borrow().build_count, 0);
        assert_eq!(domain.command_stack().undo_len(), 0);
        assert_eq!(viewer.model().nodes, before);
        assert_eq!(viewer.model().revision, revision);
        // Admission refused before disposal; the unfinished tool retains its gesture.
        assert!(domain.has_active_gesture());
        assert!(!viewer.is_faulted());
    }
}

#[test]
fn feedback_cleanup_panic_prevents_building_and_faults_viewer_for_both_tools() {
    for creating in [false, true] {
        let (mut viewer, mut domain) = editor();
        let start = if creating {
            domain
                .set_active_tool(&mut viewer, ContainerCreationTool::new(Template(20)))
                .unwrap();
            Point::new(400.0, 120.0)
        } else {
            select(&mut viewer, &[FIRST]);
            Point::new(80.0, 80.0)
        };
        domain
            .pointer_pressed(&mut viewer, start, MouseButton::Left, Default::default())
            .unwrap();
        domain
            .pointer_moved(&mut viewer, Point::new(440.0, 160.0))
            .unwrap();
        viewer.model().trace.borrow_mut().panic_cleanup = true;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            domain.pointer_released(&mut viewer, Point::new(440.0, 160.0), MouseButton::Left)
        }));
        assert!(panic.is_err());
        assert!(viewer.is_faulted());
        assert_eq!(domain.command_stack().undo_len(), 0);
        assert_eq!(viewer.model().trace.borrow().build_count, 0);
    }
}

#[test]
fn failed_model_execute_preserves_model_revision_and_history() {
    let (mut viewer, mut domain) = editor();
    select(&mut viewer, &[FIRST]);
    let before = viewer.model().nodes.clone();
    let revision = viewer.model().revision;
    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(80.0, 80.0),
            MouseButton::Left,
            Default::default(),
        )
        .unwrap();
    domain
        .pointer_moved(&mut viewer, Point::new(400.0, 120.0))
        .unwrap();
    viewer.model().trace.borrow_mut().execute_fail = true;
    assert!(
        domain
            .pointer_released(&mut viewer, Point::new(400.0, 120.0), MouseButton::Left,)
            .is_err()
    );
    assert_eq!(viewer.model().nodes, before);
    assert_eq!(viewer.model().revision, revision);
    assert_eq!(domain.command_stack().undo_len(), 0);
    assert!(!domain.has_active_gesture());
    assert!(!viewer.is_faulted());
}

#[test]
fn successful_model_and_history_survive_failed_reparent_projection() {
    let (mut viewer, mut domain) = editor();
    select(&mut viewer, &[FIRST]);
    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(80.0, 80.0),
            MouseButton::Left,
            Default::default(),
        )
        .unwrap();
    domain
        .pointer_moved(&mut viewer, Point::new(400.0, 120.0))
        .unwrap();
    viewer.model().trace.borrow_mut().projection_fail = true;
    assert!(
        domain
            .pointer_released(&mut viewer, Point::new(400.0, 120.0), MouseButton::Left)
            .is_err()
    );
    assert!(viewer.is_faulted());
    assert_eq!(viewer.model().nodes[&RIGHT].children, [LOCAL, FIRST]);
    assert_eq!(domain.command_stack().undo_len(), 1);
    assert_eq!(viewer.model().trace.borrow().build_count, 1);
}

#[test]
fn mixed_connection_selection_and_descendant_primary_survive_reparent_and_current_history_snapshots()
 {
    const EDGE: u32 = 30;
    let mut model = Model::new();
    model
        .connections
        .push(novadraw_editor::ModelConnection::new(
            EDGE, DESCENDANT, LOCAL,
        ));
    let mut viewer =
        GraphicalViewer::new(model, Factory, Rectangle::new(0.0, 0.0, 800.0, 600.0)).unwrap();
    support::prepare_next_submission(&mut viewer);
    let mut domain = EditorDomain::new();
    select(&mut viewer, &[NESTED, EDGE, DESCENDANT]);
    let focus = viewer.part_for_model(LOCAL).unwrap();
    viewer.set_focus(Some(focus)).unwrap();
    domain
        .set_active_tool(&mut viewer, ContainerMoveTool::<Model, Protocol>::new())
        .unwrap();
    drag(
        &mut viewer,
        &mut domain,
        Point::new(90.0, 220.0),
        Point::new(410.0, 120.0),
    );
    let selected_models = |viewer: &GraphicalViewer<Model, Factory>| {
        viewer
            .selection()
            .items()
            .iter()
            .map(|part| viewer.parts().get(*part).unwrap().model_id().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(selected_models(&viewer), [NESTED, EDGE, DESCENDANT]);
    assert_eq!(
        viewer.selection().primary(),
        viewer.part_for_model(DESCENDANT)
    );
    assert_eq!(viewer.selection().focus(), Some(focus));
    assert!(
        viewer
            .model()
            .trace
            .borrow()
            .permissions
            .iter()
            .all(|(_, _, sources)| sources == &[NESTED])
    );
    select(&mut viewer, &[EDGE, NESTED]);
    domain.undo(&mut viewer).unwrap();
    assert_eq!(selected_models(&viewer), [EDGE, NESTED]);
    select(&mut viewer, &[DESCENDANT, EDGE]);
    domain.redo(&mut viewer).unwrap();
    assert_eq!(selected_models(&viewer), [DESCENDANT, EDGE]);
}

#[test]
fn stationary_pointer_after_zoom_recomputes_shared_anchor_and_fixed_creation_origin() {
    let (mut viewer, mut domain) = editor();
    let first = viewer.part_for_model(FIRST).unwrap();
    let second = viewer.part_for_model(SECOND).unwrap();
    let right = viewer.part_for_model(RIGHT).unwrap();
    let saved_move = Rc::new(RefCell::new(None));
    let saved_creation = Rc::new(RefCell::new(None));
    let move_out = saved_move.clone();
    let creation_out = saved_creation.clone();
    let pointer = Point::new(400.0, 120.0);
    let start = Point::new(80.0, 80.0);
    domain
        .set_active_tool(
            &mut viewer,
            Probe(Box::new(move |ctx| {
                let revision = novadraw_editor::InteractionRevision::initial();
                *move_out.borrow_mut() = Some(ctx.begin_container_move::<Protocol>(
                    &[second, first],
                    Some(first),
                    start,
                    revision,
                )?);
                *creation_out.borrow_mut() =
                    Some(ctx.begin_container_creation(pointer, Default::default())?);
                Ok(())
            })),
        )
        .unwrap();
    viewer.set_viewport_scale_at(1.5, None).unwrap();
    domain
        .set_active_tool(
            &mut viewer,
            Probe(Box::new(move |ctx| {
                let revision = novadraw_editor::InteractionRevision::initial();
                let saved = saved_move.borrow();
                let batch = saved.as_ref().unwrap();
                let candidate = ctx.container_move_candidate(batch, right, pointer, revision)?;
                let anchor_parent = batch.grab_to_parent().transform_point(batch.grab_point());
                let anchor_surface = ctx
                    .child_content_to_surface(batch.anchor().parent())?
                    .transform_point(anchor_parent);
                assert_eq!(
                    candidate.translation().transform_point(anchor_surface),
                    pointer
                );
                let xy = candidate.xy_constraints(GeometryTolerance::EXACT)?;
                assert_eq!(
                    Point::new(xy[0].x - xy[1].x, xy[0].y - xy[1].y),
                    Point::new(80.0, 10.0)
                );
                let saved = saved_creation.borrow();
                let creation = ctx.container_creation_candidate(
                    saved.as_ref().unwrap(),
                    right,
                    pointer,
                    Dimension::new(30.0, 20.0),
                    true,
                    revision,
                )?;
                let current_origin = ctx
                    .child_content_to_surface(ctx.contents())?
                    .transform_point(saved.as_ref().unwrap().point());
                let inverse = ctx.surface_to_child_content(right)?.unwrap();
                let origin = inverse.transform_point(current_origin);
                let end = inverse.transform_point(pointer);
                assert_eq!(
                    creation.bounds(),
                    Rectangle::new(
                        origin.x().min(end.x()),
                        origin.y().min(end.y()),
                        (end.x() - origin.x()).abs(),
                        (end.y() - origin.y()).abs()
                    )
                );
                assert_ne!(current_origin, pointer);
                Ok(())
            })),
        )
        .unwrap();
}

#[test]
#[ignore = "release-mode performance evidence; run explicitly"]
fn container_scaling_samples_include_multiple_parents_and_large_source_subtrees() {
    use container_allocations::measure;
    const SAMPLES: usize = 32;
    for (label, parent_count, descendants) in [
        ("parents16", 16, 0),
        ("parents64", 64, 0),
        ("subtree32", 1, 32),
        ("subtree2048", 1, 2048),
    ] {
        let mut model = Model::new();
        let mut sources = Vec::new();
        for index in 0..parent_count {
            let parent = 1000 + index;
            let source = 2000 + index;
            model.nodes.get_mut(&LEFT).unwrap().children.push(parent);
            model.nodes.insert(
                parent,
                Node {
                    bounds: Rectangle::new(0.0, 0.0, 230.0, 300.0),
                    children: vec![source],
                },
            );
            model.nodes.insert(
                source,
                Node {
                    bounds: Rectangle::new(20.0, 20.0, 40.0, 30.0),
                    children: vec![],
                },
            );
            sources.push(source);
        }
        for index in 0..descendants {
            let id = 3000 + index;
            model.nodes.get_mut(&sources[0]).unwrap().children.push(id);
            model.nodes.insert(
                id,
                Node {
                    bounds: Rectangle::new(0.0, 0.0, 1.0, 1.0),
                    children: vec![],
                },
            );
        }
        let nodes = model.nodes.len();
        let mut viewer =
            GraphicalViewer::new(model, Factory, Rectangle::new(0.0, 0.0, 800.0, 600.0)).unwrap();
        support::prepare_next_submission(&mut viewer);
        let parts = sources
            .iter()
            .map(|id| viewer.part_for_model(*id).unwrap())
            .collect::<Vec<_>>();
        let target = viewer.part_for_model(RIGHT).unwrap();
        let source_parent = viewer.parts().parent(parts[0]).unwrap();
        let start = viewer
            .child_content_to_surface(source_parent)
            .unwrap()
            .transform_point(Point::new(30.0, 30.0));
        let source_hit = start;
        let mut domain = EditorDomain::new();
        domain
            .set_active_tool(
                &mut viewer,
                Probe(Box::new(move |ctx| {
                    let revision = novadraw_editor::InteractionRevision::initial();
                    let mut observations: [Vec<(u128, usize, usize)>; 5] =
                        std::array::from_fn(|_| Vec::with_capacity(SAMPLES));
                    let child_reads = ctx.model().child_reads.get();
                    for _ in 0..SAMPLES {
                        let (batch, observation) = measure(|| {
                            ctx.begin_container_move::<Protocol>(&parts, None, start, revision)
                                .unwrap()
                        });
                        observations[0].push(observation);
                        let (hit, observation) =
                            measure(|| ctx.container_target_at(source_hit, &parts).unwrap());
                        assert!(!parts.contains(&hit));
                        observations[1].push(observation);
                        let (candidate, observation) = measure(|| {
                            ctx.container_move_candidate(
                                &batch,
                                target,
                                Point::new(400.0, 120.0),
                                revision,
                            )
                            .unwrap()
                        });
                        observations[2].push(observation);
                        ctx.model().trace.borrow_mut().permissions.clear();
                        let (checked, observation) = measure(|| {
                            ctx.resolve_container_move::<Protocol>(&candidate)
                                .unwrap()
                                .unwrap()
                        });
                        assert_eq!(
                            ctx.model().trace.borrow().permissions.len(),
                            parent_count as usize
                        );
                        observations[3].push(observation);
                        let (_, observation) = measure(|| {
                            let feedback = ctx.show_container_move_feedback(&checked).unwrap();
                            ctx.remove_overlay_visuals(&feedback).unwrap();
                        });
                        observations[4].push(observation);
                    }
                    assert_eq!(ctx.model().child_reads.get(), child_reads);
                    for (stage, samples) in [
                        "begin",
                        "source_exclusion_hit",
                        "candidate",
                        "policy",
                        "feedback",
                    ]
                    .into_iter()
                    .zip(observations)
                    {
                        print_scaling(label, nodes, parent_count, descendants, stage, samples);
                    }
                    Ok(())
                })),
            )
            .unwrap();
        let mut commits = Vec::with_capacity(SAMPLES);
        let mut syncs = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let parts = sources
                .iter()
                .map(|id| viewer.part_for_model(*id).unwrap())
                .collect();
            domain
                .set_active_tool(
                    &mut viewer,
                    ContainerMoveTool::<Model, Protocol>::from_sources(
                        parts,
                        None,
                        start,
                        novadraw_editor::InteractionRevision::initial(),
                    ),
                )
                .unwrap();
            let (_, observation) = measure(|| {
                domain
                    .pointer_released(&mut viewer, Point::new(400.0, 120.0), MouseButton::Left)
                    .unwrap()
            });
            commits.push(observation);
            domain.undo(&mut viewer).unwrap();
            let reads = viewer.model().child_reads.get();
            let (_, observation) = measure(|| viewer.synchronize_model(Model::publish).unwrap());
            assert_eq!(viewer.model().child_reads.get() - reads, nodes);
            syncs.push(observation);
        }
        print_scaling(
            label,
            nodes,
            parent_count,
            descendants,
            "full_commit",
            commits,
        );
        print_scaling(
            label,
            nodes,
            parent_count,
            descendants,
            "revision_sync",
            syncs,
        );
    }
}

#[cfg(test)]
fn print_scaling(
    label: &str,
    nodes: usize,
    parents: u32,
    descendants: u32,
    stage: &str,
    samples: Vec<(u128, usize, usize)>,
) {
    let mut time = samples.iter().map(|value| value.0).collect::<Vec<_>>();
    let mut calls = samples.iter().map(|value| value.1).collect::<Vec<_>>();
    let mut bytes = samples.iter().map(|value| value.2).collect::<Vec<_>>();
    time.sort_unstable();
    calls.sort_unstable();
    bytes.sort_unstable();
    let mid = samples.len() / 2;
    let tail = samples.len() * 95 / 100;
    println!(
        "container_scaling scenario={label} nodes={nodes} parents={parents} descendants={descendants} stage={stage} samples={} p50_ns={} p95_ns={} alloc_p50={} alloc_p95={} bytes_p50={} bytes_p95={}",
        samples.len(),
        time[mid],
        time[tail],
        calls[mid],
        calls[tail],
        bytes[mid],
        bytes[tail]
    );
}

#[test]
#[ignore = "release-mode performance evidence; run explicitly"]
fn container_cost_samples_include_sync_hit_geometry_policy_feedback_and_commit() {
    use std::time::Instant;
    const SAMPLES: usize = 64;
    for (label, width, depth, selected_count) in [
        ("small", 8, 2, 2),
        ("wide", 512, 2, 32),
        ("deep", 8, 128, 2),
    ] {
        let mut model = Model::new();
        let mut parent = LEFT;
        for level in 0..depth {
            let id = 1000 + level;
            model.nodes.get_mut(&parent).unwrap().children.push(id);
            model.nodes.insert(
                id,
                Node {
                    bounds: Rectangle::new(0.0, 0.0, 230.0, 300.0),
                    children: vec![],
                },
            );
            parent = id;
        }
        let sources = (0..width).map(|index| 2000 + index).collect::<Vec<_>>();
        for (index, id) in sources.iter().enumerate() {
            model.nodes.insert(
                *id,
                Node {
                    bounds: Rectangle::new(
                        (index % 8) as f64 * 20.0,
                        (index / 8) as f64 * 20.0,
                        10.0,
                        10.0,
                    ),
                    children: vec![],
                },
            );
        }
        model
            .nodes
            .get_mut(&parent)
            .unwrap()
            .children
            .extend(&sources);
        let mut viewer =
            GraphicalViewer::new(model, Factory, Rectangle::new(0.0, 0.0, 800.0, 600.0)).unwrap();
        support::prepare_next_submission(&mut viewer);
        let parts = sources[..selected_count]
            .iter()
            .map(|id| viewer.part_for_model(*id).unwrap())
            .collect::<Vec<_>>();
        let target = viewer.part_for_model(RIGHT).unwrap();
        let parent_part = viewer.part_for_model(parent).unwrap();
        let start = viewer
            .child_content_to_surface(parent_part)
            .unwrap()
            .transform_point(Point::new(5.0, 5.0));
        let count = viewer.model().nodes.len();
        let mut domain = EditorDomain::new();
        domain.set_active_tool(&mut viewer, Probe(Box::new(move |ctx| {
            let revision = novadraw_editor::InteractionRevision::initial();
            let batch = ctx.begin_container_move::<Protocol>(&parts, None, start, revision)?;
            let mut timings: [Vec<u128>; 5] = std::array::from_fn(|_| Vec::new());
            for index in 0..SAMPLES {
                let pointer = Point::new(400.0 + (index % 3) as f64, 120.0);
                let began = Instant::now();
                ctx.synchronize_model()?;
                timings[0].push(began.elapsed().as_nanos());
                let began = Instant::now();
                ctx.container_target_at(pointer, &parts)?;
                timings[1].push(began.elapsed().as_nanos());
                let began = Instant::now();
                let candidate = ctx.container_move_candidate(&batch, target, pointer, revision)?;
                timings[2].push(began.elapsed().as_nanos());
                let began = Instant::now();
                let checked = ctx.resolve_container_move::<Protocol>(&candidate)?.unwrap();
                timings[3].push(began.elapsed().as_nanos());
                let began = Instant::now();
                let visuals = ctx.show_container_move_feedback(&checked)?;
                ctx.remove_overlay_visuals(&visuals)?;
                timings[4].push(began.elapsed().as_nanos());
            }
            for (stage, mut samples) in ["sync", "hit", "geometry", "policy", "feedback"].into_iter().zip(timings) {
                samples.sort_unstable();
                println!("container_cost scenario={label} nodes={count} k={selected_count} depth={depth} stage={stage} p50_ns={} p95_ns={}",
                    samples[SAMPLES / 2], samples[SAMPLES * 95 / 100]);
            }
            Ok(())
        }))).unwrap();
        let mut commits = Vec::new();
        let mut synchronizations = Vec::new();
        let mut ordered = Vec::new();
        let mut sync_reads = Vec::new();
        for _ in 0..SAMPLES {
            let parts = sources[..selected_count]
                .iter()
                .map(|id| viewer.part_for_model(*id).unwrap())
                .collect();
            domain
                .set_active_tool(
                    &mut viewer,
                    ContainerMoveTool::<Model, Protocol>::from_sources(
                        parts,
                        None,
                        start,
                        novadraw_editor::InteractionRevision::initial(),
                    ),
                )
                .unwrap();
            let began = Instant::now();
            domain
                .pointer_released(&mut viewer, Point::new(400.0, 120.0), MouseButton::Left)
                .unwrap();
            commits.push(began.elapsed().as_nanos());
            domain.undo(&mut viewer).unwrap();
            let reads = viewer.model().child_reads.get();
            let began = Instant::now();
            viewer.synchronize_model(Model::publish).unwrap();
            synchronizations.push(began.elapsed().as_nanos());
            sync_reads.push(viewer.model().child_reads.get() - reads);
            let began = Instant::now();
            let result =
                ordered_insertion(&sources, &sources[..selected_count], InsertionPosition::End)
                    .unwrap();
            std::hint::black_box(result);
            ordered.push(began.elapsed().as_nanos());
        }
        for (stage, mut samples) in [
            ("full_commit", commits),
            ("revision_sync", synchronizations),
            ("ordered", ordered),
        ] {
            samples.sort_unstable();
            println!(
                "container_cost scenario={label} nodes={count} k={selected_count} depth={depth} stage={stage} p50_ns={} p95_ns={}",
                samples[SAMPLES / 2],
                samples[SAMPLES * 95 / 100]
            );
        }
        println!(
            "container_cost scenario={label} samples={SAMPLES} sync_child_reads_min={} sync_child_reads_max={}",
            sync_reads.iter().min().unwrap(),
            sync_reads.iter().max().unwrap()
        );
    }
}

#[test]
fn parent_creation_click_drag_and_drop_share_template_and_atomic_history() {
    for mode in 0..3 {
        let (mut viewer, mut domain) = editor();
        let id = 20;
        let expected = match mode {
            0 => {
                domain
                    .set_active_tool(&mut viewer, ContainerCreationTool::new(Template(id)))
                    .unwrap();
                drag(
                    &mut viewer,
                    &mut domain,
                    Point::new(400.0, 120.0),
                    Point::new(400.0, 120.0),
                );
                Rectangle::new(50.0, 70.0, 30.0, 20.0)
            }
            1 => {
                domain
                    .set_active_tool(&mut viewer, ContainerCreationTool::new(Template(id)))
                    .unwrap();
                drag(
                    &mut viewer,
                    &mut domain,
                    Point::new(400.0, 120.0),
                    Point::new(450.0, 160.0),
                );
                Rectangle::new(50.0, 70.0, 50.0, 40.0)
            }
            _ => {
                domain
                    .drop_into_container(
                        &mut viewer,
                        &Template(id),
                        Point::new(400.0, 120.0),
                        Point::new(5.0, 7.0),
                        Default::default(),
                    )
                    .unwrap();
                Rectangle::new(45.0, 63.0, 30.0, 20.0)
            }
        };
        assert_eq!(viewer.model().nodes[&RIGHT].children, [LOCAL, id]);
        assert_eq!(viewer.model().nodes[&id].bounds, expected);
        assert!(viewer.model().trace.borrow().permissions.is_empty());
        assert_eq!(viewer.model().trace.borrow().build_count, 1);
        assert_eq!(domain.command_stack().undo_len(), 1);
        domain.undo(&mut viewer).unwrap();
        assert!(!viewer.model().nodes.contains_key(&id));
        domain.redo(&mut viewer).unwrap();
        assert_eq!(viewer.model().nodes[&id].bounds, expected);
    }
}

#[test]
fn creation_rejection_and_external_change_leave_no_model_or_history() {
    for change_model in [false, true] {
        let (mut viewer, mut domain) = editor();
        domain
            .set_active_tool(&mut viewer, ContainerCreationTool::new(Template(20)))
            .unwrap();
        domain
            .pointer_pressed(
                &mut viewer,
                Point::new(400.0, 120.0),
                MouseButton::Left,
                KeyModifiers::default(),
            )
            .unwrap();
        if change_model {
            viewer.synchronize_model(|model| model.publish()).unwrap();
        } else {
            viewer.model().trace.borrow_mut().reject_target = Some(RIGHT);
        }
        let result =
            domain.pointer_released(&mut viewer, Point::new(440.0, 160.0), MouseButton::Left);
        assert_eq!(result.is_err(), !change_model);
        assert!(!domain.has_active_gesture());
        assert!(!viewer.model().nodes.contains_key(&20));
        assert_eq!(viewer.model().trace.borrow().build_count, 0);
        assert_eq!(domain.command_stack().undo_len(), 0);
    }
}

#[test]
fn checked_services_reject_empty_duplicate_foreign_retired_and_cycle_inputs_as_whole_batches() {
    let (mut viewer, _) = editor();
    let (foreign_viewer, _) = editor();
    let first = viewer.part_for_model(FIRST).unwrap();
    let nested = viewer.part_for_model(NESTED).unwrap();
    let descendant = viewer.part_for_model(DESCENDANT).unwrap();
    let foreign = foreign_viewer.part_for_model(SECOND).unwrap();
    let revision = novadraw_editor::InteractionRevision::initial();
    let before = viewer.model().nodes.clone();
    let mut domain = EditorDomain::new();
    domain
        .set_active_tool(
            &mut viewer,
            Probe(Box::new(move |ctx| {
                for sources in [
                    vec![],
                    vec![first, first],
                    vec![first, foreign],
                    vec![ctx.part_tree().root()],
                ] {
                    assert!(
                        ctx.begin_container_move::<Protocol>(
                            &sources,
                            None,
                            Point::new(80.0, 80.0),
                            revision
                        )
                        .is_err()
                    );
                }
                let batch = ctx.begin_container_move::<Protocol>(
                    &[nested],
                    None,
                    Point::new(90.0, 220.0),
                    revision,
                )?;
                for target in [nested, descendant, foreign] {
                    assert!(
                        ctx.container_move_candidate(
                            &batch,
                            target,
                            Point::new(100.0, 240.0),
                            revision
                        )
                        .is_err()
                    );
                }
                Ok(())
            })),
        )
        .unwrap();
    assert_eq!(viewer.model().nodes, before);
    viewer
        .synchronize_model(|model| {
            model
                .nodes
                .get_mut(&LEFT)
                .unwrap()
                .children
                .retain(|id| *id != FIRST);
            model.nodes.remove(&FIRST);
            model.publish();
        })
        .unwrap();
    domain
        .set_active_tool(
            &mut viewer,
            Probe(Box::new(move |ctx| {
                assert!(
                    ctx.begin_container_move::<Protocol>(&[first], None, Point::ZERO, revision)
                        .is_err()
                );
                Ok(())
            })),
        )
        .unwrap();
    assert_eq!(domain.command_stack().undo_len(), 0);
}

#[test]
fn actual_content_panes_insets_and_heterogeneous_maps_share_one_surface_displacement() {
    for (left_transform, right_transform) in [
        (Affine2D::from_scale(2.0, 0.5), Affine2D::IDENTITY),
        (
            Affine2D::new(0.0, 1.0, -1.0, 0.0, 180.0, 0.0),
            Affine2D::new(0.0, 1.0, -1.0, 0.0, 180.0, 0.0),
        ),
        (Affine2D::from_scale(-1.0, 1.0), Affine2D::IDENTITY),
    ] {
        let mut model = Model::new();
        model.panes.insert(LEFT, left_transform);
        model.panes.insert(RIGHT, right_transform);
        let mut viewer =
            GraphicalViewer::new(model, Factory, Rectangle::new(0.0, 0.0, 800.0, 600.0)).unwrap();
        support::prepare_next_submission(&mut viewer);
        let first = viewer.part_for_model(FIRST).unwrap();
        let second = viewer.part_for_model(LOCAL).unwrap();
        let left = viewer.part_for_model(LEFT).unwrap();
        let right = viewer.part_for_model(RIGHT).unwrap();
        let left_map = viewer.child_content_to_surface(left).unwrap();
        let right_map = viewer.child_content_to_surface(right).unwrap();
        assert_eq!(
            left_map,
            Affine2D::from_translation(62.0, 64.0) * left_transform
        );
        assert_eq!(
            right_map,
            Affine2D::from_translation(362.0, 64.0) * right_transform
        );
        let start = left_map.transform_point(Point::new(30.0, 30.0));
        let end = right_map.transform_point(Point::new(60.0, 70.0));
        let revision = novadraw_editor::InteractionRevision::initial();
        let mut domain = EditorDomain::new();
        domain
            .set_active_tool(
                &mut viewer,
                Probe(Box::new(move |ctx| {
                    let batch = ctx.begin_container_move::<Protocol>(
                        &[second, first],
                        Some(first),
                        start,
                        revision,
                    )?;
                    let candidate = ctx.container_move_candidate(&batch, right, end, revision)?;
                    assert_eq!(batch.anchor().model(), FIRST);
                    let displacement = Point::new(end.x() - start.x(), end.y() - start.y());
                    assert_eq!(
                        candidate.translation(),
                        Affine2D::from_translation(displacement.x(), displacement.y())
                    );
                    for (source, geometry) in batch.sources().iter().zip(candidate.geometry()) {
                        let map = ctx.child_content_to_surface(source.parent())?;
                        let composite = right_map.checked_inverse().unwrap().unwrap()
                            * candidate.translation()
                            * map;
                        let expected =
                            ConstraintGeometry::xy(ctx.model().nodes[&source.model()].bounds)?
                                .mapped(composite)?;
                        assert_eq!(geometry, &expected);
                    }
                    assert_eq!(candidate.xy_constraints(GeometryTolerance::EXACT)?.len(), 2);
                    assert!(
                        ctx.resolve_container_move::<Protocol>(&candidate)?
                            .is_some()
                    );
                    Ok(())
                })),
            )
            .unwrap();
        assert_eq!(domain.command_stack().undo_len(), 0);
    }
}

#[test]
fn held_candidate_cannot_submit_after_viewport_geometry_changes() {
    let (mut viewer, mut domain) = editor();
    let first = viewer.part_for_model(FIRST).unwrap();
    let right = viewer.part_for_model(RIGHT).unwrap();
    let saved = Rc::new(RefCell::new(None));
    let out = saved.clone();
    domain
        .set_active_tool(
            &mut viewer,
            Probe(Box::new(move |ctx| {
                let revision = novadraw_editor::InteractionRevision::initial();
                let batch = ctx.begin_container_move::<Protocol>(
                    &[first],
                    None,
                    Point::new(80.0, 80.0),
                    revision,
                )?;
                *out.borrow_mut() = Some(ctx.container_move_candidate(
                    &batch,
                    right,
                    Point::new(400.0, 120.0),
                    revision,
                )?);
                Ok(())
            })),
        )
        .unwrap();
    viewer.set_viewport_scale_at(2.0, None).unwrap();
    domain
        .set_active_tool(
            &mut viewer,
            Probe(Box::new(move |ctx| {
                assert!(matches!(
                    ctx.resolve_container_move::<Protocol>(saved.borrow().as_ref().unwrap()),
                    Err(ToolError::Container(ContainerEditError::Invalidated))
                ));
                Ok(())
            })),
        )
        .unwrap();
    assert_eq!(viewer.model().trace.borrow().feedback_count, 0);
    assert_eq!(viewer.model().trace.borrow().build_count, 0);
}

#[test]
fn nested_click_and_drop_keep_default_size_in_scaled_parent_units() {
    let mut model = Model::new();
    model.panes.insert(RIGHT, Affine2D::from_uniform_scale(2.0));
    let mut viewer =
        GraphicalViewer::new(model, Factory, Rectangle::new(0.0, 0.0, 800.0, 600.0)).unwrap();
    support::prepare_next_submission(&mut viewer);
    let mut domain = EditorDomain::new();
    let right = viewer.part_for_model(RIGHT).unwrap();
    let pointer = viewer
        .child_content_to_surface(right)
        .unwrap()
        .transform_point(Point::new(30.0, 40.0));
    domain
        .set_active_tool(&mut viewer, ContainerCreationTool::new(Template(20)))
        .unwrap();
    drag(&mut viewer, &mut domain, pointer, pointer);
    assert_eq!(
        viewer.model().nodes[&20].bounds,
        Rectangle::new(30.0, 40.0, 30.0, 20.0)
    );
    domain
        .drop_into_container(
            &mut viewer,
            &Template(21),
            pointer,
            Point::new(5.0, 7.0),
            Default::default(),
        )
        .unwrap();
    assert_eq!(
        viewer.model().nodes[&21].bounds,
        Rectangle::new(25.0, 33.0, 30.0, 20.0)
    );
}

#[test]
fn stationary_multi_selection_survives_dual_axis_autoexpose_scroll_and_resize() {
    let mut viewer = GraphicalViewer::new(
        Model::new(),
        Factory,
        Rectangle::new(0.0, 0.0, 300.0, 200.0),
    )
    .unwrap();
    support::prepare_next_submission(&mut viewer);
    let mut domain = EditorDomain::new();
    select(&mut viewer, &[SECOND, FIRST]);
    domain
        .set_active_tool(&mut viewer, ContainerMoveTool::<Model, Protocol>::new())
        .unwrap();
    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(80.0, 80.0),
            MouseButton::Left,
            Default::default(),
        )
        .unwrap();
    let pointer = Point::new(294.0, 194.0);
    domain.pointer_moved(&mut viewer, pointer).unwrap();
    let origin = viewer.viewport_origin().unwrap();
    let tick = domain
        .autoexpose_tick(&mut viewer, std::time::Duration::from_millis(16))
        .unwrap();
    let scrolled = viewer.viewport_origin().unwrap();
    assert!(tick.scrolled());
    assert!(scrolled.x() > origin.x() && scrolled.y() > origin.y());
    domain
        .set_viewport_origin(&mut viewer, Point::new(40.0, 30.0))
        .unwrap();
    viewer
        .drive()
        .resize_logical_viewport(320.0, 220.0)
        .unwrap();
    domain.refresh_after_viewport_change(&mut viewer).unwrap();
    assert!(domain.has_active_gesture());
    domain
        .pointer_released(&mut viewer, pointer, MouseButton::Left)
        .unwrap();
    let first = viewer.model().nodes[&FIRST].bounds;
    let second = viewer.model().nodes[&SECOND].bounds;
    assert_eq!(second.x - first.x, 80.0);
    assert_eq!(second.y - first.y, 10.0);
    assert_eq!(domain.command_stack().undo_len(), 1);
    assert_eq!(viewer.model().trace.borrow().build_count, 1);
}

#[test]
fn stationary_creation_recomputes_after_dual_axis_autoexpose_scroll_zoom_and_resize() {
    let mut viewer = GraphicalViewer::new(
        Model::new(),
        Factory,
        Rectangle::new(0.0, 0.0, 300.0, 200.0),
    )
    .unwrap();
    support::prepare_next_submission(&mut viewer);
    let mut domain = EditorDomain::new();
    domain
        .set_active_tool(&mut viewer, ContainerCreationTool::new(Template(20)))
        .unwrap();
    let start = Point::new(80.0, 80.0);
    let contents = viewer.contents();
    let start_in_contents = viewer
        .surface_to_child_content(contents)
        .unwrap()
        .unwrap()
        .transform_point(start);
    domain
        .pointer_pressed(&mut viewer, start, MouseButton::Left, Default::default())
        .unwrap();
    let pointer = Point::new(294.0, 194.0);
    domain.pointer_moved(&mut viewer, pointer).unwrap();
    let origin = viewer.viewport_origin().unwrap();
    assert!(
        domain
            .autoexpose_tick(&mut viewer, std::time::Duration::from_millis(16))
            .unwrap()
            .scrolled()
    );
    let exposed = viewer.viewport_origin().unwrap();
    assert!(exposed.x() > origin.x() && exposed.y() > origin.y());
    domain
        .set_viewport_origin(&mut viewer, Point::new(40.0, 30.0))
        .unwrap();
    viewer.set_viewport_scale_at(1.5, None).unwrap();
    viewer
        .drive()
        .resize_logical_viewport(320.0, 220.0)
        .unwrap();
    domain.refresh_after_viewport_change(&mut viewer).unwrap();
    assert!(domain.has_active_gesture());
    let target = viewer.container_target_at(pointer, &[]).unwrap();
    let target_model = viewer.parts().get(target).unwrap().model_id().unwrap();
    let start_surface = viewer
        .child_content_to_surface(contents)
        .unwrap()
        .transform_point(start_in_contents);
    let inverse = viewer.surface_to_child_content(target).unwrap().unwrap();
    let a = inverse.transform_point(start_surface);
    let b = inverse.transform_point(pointer);
    let expected = Rectangle::new(
        a.x().min(b.x()),
        a.y().min(b.y()),
        (b.x() - a.x()).abs(),
        (b.y() - a.y()).abs(),
    );
    assert_ne!(start_surface, start);
    domain
        .pointer_released(&mut viewer, pointer, MouseButton::Left)
        .unwrap();
    assert_eq!(viewer.model().nodes[&20].bounds, expected);
    assert!(viewer.model().nodes[&target_model].children.contains(&20));
    assert_eq!(viewer.model().trace.borrow().build_count, 1);
    assert_eq!(domain.command_stack().undo_len(), 1);
    assert!(viewer.model().trace.borrow().permissions.is_empty());
    domain.undo(&mut viewer).unwrap();
    assert!(!viewer.model().nodes.contains_key(&20));
    domain.redo(&mut viewer).unwrap();
    assert_eq!(viewer.model().nodes[&20].bounds, expected);
}

#[test]
fn source_subtree_and_feedback_are_excluded_from_container_hits() {
    let (mut viewer, mut domain) = editor();
    let nested = viewer.part_for_model(NESTED).unwrap();
    let descendant = viewer.part_for_model(DESCENDANT).unwrap();
    let left = viewer.part_for_model(LEFT).unwrap();
    let pointer = viewer
        .child_content_to_surface(nested)
        .unwrap()
        .transform_point(Point::new(20.0, 20.0));
    assert_eq!(
        viewer.container_target_at(pointer, &[]).unwrap(),
        descendant
    );
    assert_eq!(
        viewer.container_target_at(pointer, &[nested]).unwrap(),
        left
    );
    domain
        .set_active_tool(
            &mut viewer,
            Probe(Box::new(move |ctx| {
                let visuals = ctx.attach_feedback(
                    left,
                    vec![FeedbackVisual::unscaled(Box::new(
                        RectangleFigure::new(pointer.x() - 5.0, pointer.y() - 5.0, 10.0, 10.0)
                            .unwrap(),
                    ))],
                )?;
                assert_eq!(ctx.container_target_at(pointer, &[nested])?, left);
                ctx.remove_overlay_visuals(&visuals)?;
                Ok(())
            })),
        )
        .unwrap();
    assert_eq!(domain.command_stack().undo_len(), 0);
}

#[test]
fn cancellation_tool_switch_and_external_deletion_never_submit_an_old_gesture() {
    for creation in [false, true] {
        for cancellation in 0..3 {
            let (mut viewer, mut domain) = editor();
            select(&mut viewer, &[FIRST]);
            if creation {
                domain
                    .set_active_tool(&mut viewer, ContainerCreationTool::new(Template(20)))
                    .unwrap();
            }
            domain
                .pointer_pressed(
                    &mut viewer,
                    Point::new(80.0, 80.0),
                    MouseButton::Left,
                    KeyModifiers::default(),
                )
                .unwrap();
            domain
                .pointer_moved(&mut viewer, Point::new(400.0, 120.0))
                .unwrap();
            assert!(domain.has_active_gesture());
            match cancellation {
                // Host focus/capture loss uses this same explicit cancellation entry.
                0 => domain.cancel_tool(&mut viewer).unwrap(),
                1 => domain
                    .set_active_tool(&mut viewer, novadraw_editor::SelectionTool::new())
                    .unwrap(),
                _ => {
                    viewer
                        .synchronize_model(|model| {
                            model
                                .nodes
                                .get_mut(&LEFT)
                                .unwrap()
                                .children
                                .retain(|id| *id != FIRST);
                            model.nodes.remove(&FIRST);
                            model.publish();
                        })
                        .unwrap();
                    domain
                        .pointer_moved(&mut viewer, Point::new(410.0, 120.0))
                        .unwrap();
                    assert!(viewer.part_for_model(FIRST).is_none());
                    assert!(viewer.selection().items().is_empty());
                }
            }
            assert!(!domain.has_active_gesture());
            let before = viewer.model().nodes.clone();
            domain
                .pointer_released(&mut viewer, Point::new(410.0, 120.0), MouseButton::Left)
                .unwrap();
            assert_eq!(viewer.model().nodes, before);
            assert_eq!(viewer.model().trace.borrow().build_count, 0);
            assert_eq!(domain.command_stack().undo_len(), 0);
        }
    }
}

#[test]
fn external_revision_or_relevant_policy_invalidation_cancels_the_whole_gesture() {
    for change_model in [false, true] {
        let (mut viewer, mut domain) = editor();
        select(&mut viewer, &[FIRST]);
        domain
            .pointer_pressed(
                &mut viewer,
                Point::new(80.0, 80.0),
                MouseButton::Left,
                KeyModifiers::default(),
            )
            .unwrap();
        domain
            .pointer_moved(&mut viewer, Point::new(400.0, 120.0))
            .unwrap();
        if change_model {
            viewer.synchronize_model(|model| model.publish()).unwrap();
        } else {
            viewer
                .invalidate_container_policy(viewer.part_for_model(RIGHT).unwrap())
                .unwrap();
        }
        domain
            .pointer_moved(&mut viewer, Point::new(410.0, 120.0))
            .unwrap();
        assert!(!domain.has_active_gesture());
        domain
            .pointer_released(&mut viewer, Point::new(410.0, 120.0), MouseButton::Left)
            .unwrap();
        assert_eq!(domain.command_stack().undo_len(), 0);
        assert_eq!(viewer.model().trace.borrow().build_count, 0);
    }
}

#[test]
fn release_recomputes_the_latest_pointer_and_builds_once() {
    let (mut viewer, mut domain) = editor();
    select(&mut viewer, &[FIRST]);
    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(80.0, 80.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    domain
        .pointer_moved(&mut viewer, Point::new(400.0, 120.0))
        .unwrap();
    domain
        .pointer_released(&mut viewer, Point::new(440.0, 160.0), MouseButton::Left)
        .unwrap();
    assert_eq!(
        viewer.model().nodes[&FIRST].bounds,
        Rectangle::new(80.0, 100.0, 40.0, 30.0)
    );
    assert_eq!(viewer.model().trace.borrow().build_count, 1);
}

#[test]
fn ordered_insertion_uses_operation_order_and_rejects_source_references() {
    assert_eq!(
        ordered_insertion(&[1, 2, 3, 4], &[4, 2], InsertionPosition::Before(3)).unwrap(),
        vec![1, 4, 2, 3]
    );
    assert_eq!(
        ordered_insertion(&[1, 2, 3, 4], &[4, 2], InsertionPosition::After(3)).unwrap(),
        vec![1, 3, 4, 2]
    );
    assert_eq!(
        ordered_insertion(&[], &[4, 2], InsertionPosition::Start).unwrap(),
        vec![4, 2]
    );
    assert!(ordered_insertion(&[1, 2], &[2], InsertionPosition::Before(2)).is_err());
    assert!(ordered_insertion(&[1, 2], &[2], InsertionPosition::After(5)).is_err());
    assert!(ordered_insertion(&[1, 2], &[2, 2], InsertionPosition::End).is_err());
}

#[test]
fn xy_geometry_rejects_skew_and_overflow_but_accepts_reflection_and_zero_size() {
    let geometry = ConstraintGeometry::xy(Rectangle::new(10.0, 20.0, 30.0, 40.0)).unwrap();
    let swapped = geometry
        .mapped(Affine2D::new(0.0, 1.0, -1.0, 0.0, 0.0, 0.0))
        .unwrap();
    assert_eq!(
        swapped.xy_bounds(GeometryTolerance::EXACT).unwrap(),
        Rectangle::new(-60.0, 10.0, 40.0, 30.0)
    );
    assert!(
        geometry
            .mapped(Affine2D::new(1.0, 0.0, 0.5, 1.0, 0.0, 0.0))
            .unwrap()
            .xy_bounds(GeometryTolerance::EXACT)
            .is_err()
    );
    assert!(
        geometry
            .mapped(Affine2D::from_uniform_scale(f64::MAX))
            .is_err()
    );
    let zero = ConstraintGeometry::xy(Rectangle::new(10.0, 20.0, 0.0, 0.0)).unwrap();
    assert!(zero.local_to_parent().checked_inverse().unwrap().is_some());
    assert!(
        Affine2D::from_uniform_scale(1e-200)
            .checked_inverse()
            .unwrap()
            .is_some()
    );
    assert!(
        Affine2D::from_scale(0.0, 1.0)
            .checked_inverse()
            .unwrap()
            .is_none()
    );
    assert!(Affine2D::from_scale(1e-320, 1.0).checked_inverse().is_err());
}
