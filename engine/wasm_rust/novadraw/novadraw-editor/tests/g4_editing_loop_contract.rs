use std::{
    cell::RefCell,
    collections::HashMap,
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    time::Duration,
};

use novadraw::geometry::{Dimension, Point, Rectangle, Vec2};
use novadraw::{Figure, KeyModifiers, MouseButton, RectangleFigure, RootFigure};
use novadraw_editor::{
    ChangeBoundsRequest, Command, CommandError, CreateRequest, CreationType, DeleteRequest,
    EditPartBehavior, EditPartError, EditPartFactory, EditPolicy, EditorDomain, EditorDomainError,
    EditorRequest, FeedbackVisual, GraphicalViewer, HandleRole, InteractionRevision, ModelAdapter,
    ModelEvent, ModelRevision, PartFactoryContext, PolicyError, PolicyHost, PolicyInstallation,
    PolicyRole, RequestModifiers, ResizeDirection, ViewerError, VisualUpdateContext,
};

const ROOT: NodeId = NodeId(1);
const FIRST: NodeId = NodeId(2);
const SECOND: NodeId = NodeId(3);
const MIN_NODE_SIZE: f64 = 20.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct NodeId(u64);

#[derive(Clone, Copy, Debug)]
struct Node {
    bounds: Rectangle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagramEvent {
    Changed,
}

struct DiagramModel {
    revision: ModelRevision,
    nodes: HashMap<NodeId, Node>,
    children: Vec<NodeId>,
    events: Vec<ModelEvent<NodeId, DiagramEvent>>,
    next_id: u64,
}

impl DiagramModel {
    fn new() -> Self {
        Self {
            revision: ModelRevision::initial(),
            nodes: HashMap::from([
                (
                    ROOT,
                    Node {
                        bounds: Rectangle::new(0.0, 0.0, 600.0, 400.0),
                    },
                ),
                (
                    FIRST,
                    Node {
                        bounds: Rectangle::new(50.0, 60.0, 100.0, 80.0),
                    },
                ),
                (
                    SECOND,
                    Node {
                        bounds: Rectangle::new(220.0, 80.0, 120.0, 90.0),
                    },
                ),
            ]),
            children: vec![FIRST, SECOND],
            events: Vec::new(),
            next_id: 4,
        }
    }

    fn publish(&mut self, subject: NodeId) {
        self.revision = self.revision.next().unwrap();
        self.events.push(ModelEvent::new(
            self.revision,
            subject,
            DiagramEvent::Changed,
        ));
    }

    fn set_bounds(&mut self, id: NodeId, bounds: Rectangle) {
        self.nodes.get_mut(&id).unwrap().bounds = bounds;
        self.publish(id);
    }

    fn insert(&mut self, id: NodeId, node: Node, index: usize) {
        assert!(self.nodes.insert(id, node).is_none());
        self.children.insert(index, id);
        self.publish(ROOT);
    }

    fn remove(&mut self, id: NodeId) -> (Node, usize) {
        let index = self
            .children
            .iter()
            .position(|candidate| *candidate == id)
            .unwrap();
        self.children.remove(index);
        let node = self.nodes.remove(&id).unwrap();
        self.publish(ROOT);
        (node, index)
    }
}

impl ModelAdapter for DiagramModel {
    type ModelId = NodeId;
    type Event = DiagramEvent;
    type Error = Infallible;

    fn root(&self) -> Self::ModelId {
        ROOT
    }

    fn revision(&self) -> ModelRevision {
        self.revision
    }

    fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
        Ok(if model == ROOT {
            self.children.clone()
        } else {
            Vec::new()
        })
    }

    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
        std::mem::take(&mut self.events)
    }
}

struct SetBoundsCommand {
    id: NodeId,
    before: Rectangle,
    after: Rectangle,
}

impl Command<DiagramModel> for SetBoundsCommand {
    fn label(&self) -> &str {
        "Set bounds"
    }

    fn can_execute(&self, _model: &DiagramModel) -> bool {
        self.after.width >= MIN_NODE_SIZE && self.after.height >= MIN_NODE_SIZE
    }

    fn execute(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        model.set_bounds(self.id, self.after);
        Ok(())
    }

    fn undo(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        model.set_bounds(self.id, self.before);
        Ok(())
    }
}

struct CreateNodeCommand {
    id: NodeId,
    node: Node,
    index: usize,
}

impl Command<DiagramModel> for CreateNodeCommand {
    fn label(&self) -> &str {
        "Create node"
    }

    fn execute(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        model.insert(self.id, self.node, self.index);
        Ok(())
    }

    fn undo(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        model.remove(self.id);
        Ok(())
    }
}

struct DeleteNodeCommand {
    id: NodeId,
    removed: Option<(Node, usize)>,
}

impl Command<DiagramModel> for DeleteNodeCommand {
    fn label(&self) -> &str {
        "Delete node"
    }

    fn execute(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        self.removed = Some(model.remove(self.id));
        Ok(())
    }

    fn undo(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        let (node, index) = self
            .removed
            .take()
            .ok_or_else(|| CommandError::state_unknown("deleted node snapshot is missing"))?;
        model.insert(self.id, node, index);
        Ok(())
    }

    fn redo(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        self.execute(model)
    }
}

struct NodePolicy;

impl EditPolicy<DiagramModel> for NodePolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(
            request,
            EditorRequest::ChangeBounds(_) | EditorRequest::Delete(_)
        )
    }

    fn command(
        &mut self,
        host: PolicyHost<NodeId>,
        request: &EditorRequest,
        model: &DiagramModel,
    ) -> Result<Option<Box<dyn Command<DiagramModel>>>, PolicyError> {
        match request {
            EditorRequest::ChangeBounds(request) => {
                let before = model.nodes[&host.model()].bounds;
                Ok(Some(Box::new(SetBoundsCommand {
                    id: host.model(),
                    before,
                    after: request.transformed_bounds(before),
                })))
            }
            EditorRequest::Delete(_) => {
                if host.parent_model() != Some(ROOT) {
                    return Err(PolicyError::operation("only root children are deletable"));
                }
                Ok(Some(Box::new(DeleteNodeCommand {
                    id: host.model(),
                    removed: None,
                })))
            }
            EditorRequest::Create(_)
            | EditorRequest::CreateConnection(_)
            | EditorRequest::ReconnectConnection(_)
            | EditorRequest::Bendpoint(_) => Ok(None),
        }
    }

    fn feedback(
        &mut self,
        host: PolicyHost<NodeId>,
        request: &EditorRequest,
        model: &DiagramModel,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        let EditorRequest::ChangeBounds(request) = request else {
            return Ok(Vec::new());
        };
        let bounds = request.transformed_bounds(model.nodes[&host.model()].bounds);
        let figure = RectangleFigure::new(bounds.x, bounds.y, bounds.width, bounds.height);
        Ok(vec![FeedbackVisual::scaled(Box::new(figure))])
    }
}

struct CanvasPolicy;

impl EditPolicy<DiagramModel> for CanvasPolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(request, EditorRequest::Create(_))
    }

    fn command(
        &mut self,
        host: PolicyHost<NodeId>,
        request: &EditorRequest,
        model: &DiagramModel,
    ) -> Result<Option<Box<dyn Command<DiagramModel>>>, PolicyError> {
        let EditorRequest::Create(request) = request else {
            return Ok(None);
        };
        if host.model() != ROOT || request.creation_type().as_str() != "node" {
            return Err(PolicyError::operation(
                "unsupported creation target or type",
            ));
        }
        Ok(Some(Box::new(CreateNodeCommand {
            id: NodeId(model.next_id),
            node: Node {
                bounds: request.bounds(),
            },
            index: model.children.len(),
        })))
    }
}

struct DiagramPart;

impl EditPartBehavior<DiagramModel> for DiagramPart {
    fn create_figure(
        &mut self,
        model: &DiagramModel,
        model_id: NodeId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        let bounds = model.nodes[&model_id].bounds;
        Ok(if model_id == ROOT {
            Box::new(RootFigure::new(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
            ))
        } else {
            Box::new(RectangleFigure::new(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
            ))
        })
    }

    fn create_policies(
        &mut self,
        _model: &DiagramModel,
        model_id: NodeId,
    ) -> Result<Vec<PolicyInstallation<DiagramModel>>, EditPartError> {
        Ok(if model_id == ROOT {
            vec![(PolicyRole::Layout, Box::new(CanvasPolicy))]
        } else {
            vec![(PolicyRole::Component, Box::new(NodePolicy))]
        })
    }

    fn refresh_visuals(
        &mut self,
        model: &DiagramModel,
        model_id: NodeId,
        context: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        context.set_primary_bounds(model.nodes[&model_id].bounds)?;
        Ok(())
    }
}

#[derive(Default)]
struct PolicyRoutingState {
    target: Option<novadraw_editor::EditPartId>,
    command_hosts: Vec<NodeId>,
    feedback_hosts: Vec<NodeId>,
    panic_command: bool,
    panic_feedback: bool,
}

struct RoutingPolicy {
    state: Rc<RefCell<PolicyRoutingState>>,
}

impl EditPolicy<DiagramModel> for RoutingPolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(request, EditorRequest::Delete(_))
    }

    fn target(
        &self,
        host: PolicyHost<NodeId>,
        request: &EditorRequest,
    ) -> Option<novadraw_editor::EditPartId> {
        self.understands(request)
            .then(|| self.state.borrow().target.unwrap_or(host.part()))
    }

    fn command(
        &mut self,
        host: PolicyHost<NodeId>,
        _request: &EditorRequest,
        _model: &DiagramModel,
    ) -> Result<Option<Box<dyn Command<DiagramModel>>>, PolicyError> {
        assert!(
            !self.state.borrow().panic_command,
            "policy command extension panicked"
        );
        self.state.borrow_mut().command_hosts.push(host.model());
        Ok(None)
    }

    fn feedback(
        &mut self,
        host: PolicyHost<NodeId>,
        _request: &EditorRequest,
        model: &DiagramModel,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        assert!(
            !self.state.borrow().panic_feedback,
            "policy feedback extension panicked"
        );
        self.state.borrow_mut().feedback_hosts.push(host.model());
        Ok(vec![FeedbackVisual::scaled(Box::new(
            RectangleFigure::from_bounds(model.nodes[&host.model()].bounds),
        ))])
    }
}

#[derive(Default)]
struct DiagramFactory {
    policy_routing: Option<Rc<RefCell<PolicyRoutingState>>>,
}

impl EditPartFactory<DiagramModel> for DiagramFactory {
    fn create(
        &mut self,
        _context: PartFactoryContext<NodeId>,
        _model: &DiagramModel,
    ) -> Result<Box<dyn EditPartBehavior<DiagramModel>>, EditPartError> {
        Ok(if let Some(state) = &self.policy_routing {
            Box::new(RoutingPart {
                state: Rc::clone(state),
            })
        } else {
            Box::new(DiagramPart)
        })
    }
}

fn viewer() -> GraphicalViewer<DiagramModel, DiagramFactory> {
    viewer_with_bounds(Rectangle::new(0.0, 0.0, 640.0, 480.0))
}

fn viewer_with_bounds(bounds: Rectangle) -> GraphicalViewer<DiagramModel, DiagramFactory> {
    GraphicalViewer::new(DiagramModel::new(), DiagramFactory::default(), bounds).unwrap()
}

struct RoutingPart {
    state: Rc<RefCell<PolicyRoutingState>>,
}

impl EditPartBehavior<DiagramModel> for RoutingPart {
    fn create_figure(
        &mut self,
        model: &DiagramModel,
        model_id: NodeId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        let bounds = model.nodes[&model_id].bounds;
        Ok(Box::new(RectangleFigure::from_bounds(bounds)))
    }

    fn create_policies(
        &mut self,
        _model: &DiagramModel,
        _model_id: NodeId,
    ) -> Result<Vec<PolicyInstallation<DiagramModel>>, EditPartError> {
        Ok(vec![(
            PolicyRole::Component,
            Box::new(RoutingPolicy {
                state: Rc::clone(&self.state),
            }),
        )])
    }
}

fn routing_viewer(
    state: Rc<RefCell<PolicyRoutingState>>,
) -> GraphicalViewer<DiagramModel, DiagramFactory> {
    GraphicalViewer::new(
        DiagramModel::new(),
        DiagramFactory {
            policy_routing: Some(state),
        },
        Rectangle::new(0.0, 0.0, 640.0, 480.0),
    )
    .unwrap()
}

#[test]
fn typed_change_bounds_request_preserves_operation_and_transforms_bounds() {
    let request = ChangeBoundsRequest::resizing(
        Vec::new(),
        Point::new(120.0, 140.0),
        Vec2::new(5.0, 7.0),
        Dimension::new(20.0, 30.0),
        ResizeDirection::SouthEast,
        RequestModifiers::default(),
        InteractionRevision::initial(),
    );

    assert_eq!(
        request.transformed_bounds(Rectangle::new(10.0, 20.0, 100.0, 80.0)),
        Rectangle::new(15.0, 27.0, 120.0, 110.0)
    );
    assert_eq!(request.revision(), InteractionRevision::initial());
}

#[test]
fn domain_executes_compound_move_and_refreshes_then_undoes_and_redoes() {
    let mut viewer = viewer();
    let first = viewer.part_for_model(FIRST).unwrap();
    let second = viewer.part_for_model(SECOND).unwrap();
    let mut domain = EditorDomain::new();
    let request = EditorRequest::ChangeBounds(ChangeBoundsRequest::moving(
        vec![first, second],
        Point::new(30.0, 40.0),
        Vec2::new(30.0, 40.0),
        RequestModifiers::default(),
        domain.next_interaction_revision().unwrap(),
    ));

    domain.execute_request(&mut viewer, &request).unwrap();
    assert_eq!(
        viewer.model().nodes[&FIRST].bounds,
        Rectangle::new(80.0, 100.0, 100.0, 80.0)
    );
    assert_eq!(
        viewer.part_bounds_in_surface(first).unwrap(),
        Rectangle::new(80.0, 100.0, 100.0, 80.0)
    );

    domain.undo(&mut viewer).unwrap();
    assert_eq!(
        viewer.model().nodes[&FIRST].bounds,
        Rectangle::new(50.0, 60.0, 100.0, 80.0)
    );
    domain.redo(&mut viewer).unwrap();
    assert_eq!(
        viewer.model().nodes[&SECOND].bounds,
        Rectangle::new(250.0, 120.0, 120.0, 90.0)
    );
}

#[test]
fn create_and_delete_round_trip_through_policy_command_and_projection() {
    let mut viewer = viewer();
    let mut domain = EditorDomain::new();
    let create = EditorRequest::Create(CreateRequest::new(
        viewer.contents(),
        CreationType::new("node").unwrap(),
        Rectangle::new(360.0, 220.0, 90.0, 70.0),
        RequestModifiers::default(),
        domain.next_interaction_revision().unwrap(),
    ));

    domain.execute_request(&mut viewer, &create).unwrap();
    let created = NodeId(4);
    let created_part = viewer.part_for_model(created).unwrap();
    assert_eq!(
        viewer.part_bounds_in_surface(created_part).unwrap(),
        Rectangle::new(360.0, 220.0, 90.0, 70.0)
    );

    let delete = EditorRequest::Delete(DeleteRequest::new(
        vec![created_part],
        domain.next_interaction_revision().unwrap(),
    ));
    domain.execute_request(&mut viewer, &delete).unwrap();
    assert!(viewer.part_for_model(created).is_none());
    domain.undo(&mut viewer).unwrap();
    assert!(viewer.part_for_model(created).is_some());
}

#[test]
fn selection_tool_moves_selected_parts_and_commits_after_feedback_cleanup() {
    let mut viewer = viewer();
    let mut domain = EditorDomain::new();
    let press = domain
        .pointer_pressed(
            &mut viewer,
            Point::new(70.0, 80.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    assert!(press.selection().is_some());
    assert!(domain.has_active_gesture());

    domain
        .pointer_moved(&mut viewer, Point::new(110.0, 115.0))
        .unwrap();
    let release = domain
        .pointer_released(&mut viewer, Point::new(110.0, 115.0), MouseButton::Left)
        .unwrap();

    assert!(release.command_executed());
    assert!(!domain.has_active_gesture());
    assert_eq!(
        viewer.model().nodes[&FIRST].bounds,
        Rectangle::new(90.0, 95.0, 100.0, 80.0)
    );
    assert!(matches!(
        viewer.target_at(110.0, 115.0),
        novadraw_editor::ViewerTarget::Part(_)
    ));
}

#[test]
fn autoexpose_scrolls_without_pointer_motion_and_preserves_model_delta() {
    let mut viewer = viewer_with_bounds(Rectangle::new(0.0, 0.0, 320.0, 240.0));
    viewer.runtime_mut().prepare_frame().unwrap();
    let mut domain = EditorDomain::new();
    let start = Point::new(70.0, 80.0);
    let edge = Point::new(315.0, 100.0);

    domain
        .pointer_pressed(
            &mut viewer,
            start,
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    domain.pointer_moved(&mut viewer, edge).unwrap();
    assert!(domain.autoexpose_requested());

    let revision_before = viewer.model().revision;
    let content_before = viewer.model_point_from_surface(edge).unwrap();
    let tick = domain
        .autoexpose_tick(&mut viewer, Duration::from_millis(30))
        .unwrap();
    assert!(tick.scrolled());
    assert!(tick.continue_requested());
    assert_eq!(viewer.model().revision, revision_before);
    assert!(viewer.viewport_origin().unwrap().x() > 0.0);

    let content_after = viewer.model_point_from_surface(edge).unwrap();
    assert!(content_after.x() > content_before.x());
    let release = domain
        .pointer_released(&mut viewer, edge, MouseButton::Left)
        .unwrap();
    assert!(release.command_executed());
    assert!(!domain.autoexpose_requested());
    let expected_x = 50.0 + content_after.x() - start.x();
    assert!((viewer.model().nodes[&FIRST].bounds.x - expected_x).abs() <= f64::EPSILON);

    domain.undo(&mut viewer).unwrap();
    assert_eq!(
        viewer.model().nodes[&FIRST].bounds,
        Rectangle::new(50.0, 60.0, 100.0, 80.0)
    );
}

#[test]
fn autoexpose_corner_scrolls_both_available_viewport_axes() {
    let mut viewer = viewer_with_bounds(Rectangle::new(0.0, 0.0, 320.0, 240.0));
    viewer.runtime_mut().prepare_frame().unwrap();
    let mut domain = EditorDomain::new();
    let start = Point::new(70.0, 80.0);
    let corner = Point::new(315.0, 235.0);

    domain
        .pointer_pressed(
            &mut viewer,
            start,
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    domain.pointer_moved(&mut viewer, corner).unwrap();
    let before = viewer.viewport_origin().unwrap();
    let tick = domain
        .autoexpose_tick(&mut viewer, Duration::from_millis(30))
        .unwrap();
    let after = viewer.viewport_origin().unwrap();

    assert!(tick.scrolled());
    assert!(after.x() > before.x());
    assert!(after.y() > before.y());
}

#[test]
fn transient_feedback_expands_freeform_range_before_edge_scroll() {
    let mut viewer = viewer();
    viewer.runtime_mut().prepare_frame().unwrap();
    let mut domain = EditorDomain::new();
    let start = Point::new(70.0, 80.0);
    let corner = Point::new(635.0, 475.0);

    domain
        .pointer_pressed(
            &mut viewer,
            start,
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    domain.pointer_moved(&mut viewer, corner).unwrap();
    assert_eq!(viewer.viewport_origin().unwrap(), Point::new(0.0, 0.0));

    let tick = domain
        .autoexpose_tick(&mut viewer, Duration::from_millis(30))
        .unwrap();
    let origin = viewer.viewport_origin().unwrap();

    assert!(tick.scrolled());
    assert!(origin.x() > 0.0);
    assert!(origin.y() > 0.0);
    let feedback_extent = viewer
        .runtime()
        .freeform_extent(viewer.root_layers().scaled_feedback())
        .unwrap();
    assert!(feedback_extent.x + feedback_extent.width > 640.0);
    assert!(feedback_extent.y + feedback_extent.height > 480.0);

    domain.pointer_moved(&mut viewer, start).unwrap();
    let returned_origin = viewer.viewport_origin().unwrap();
    assert!(returned_origin.x() <= origin.x());
    assert!(returned_origin.y() <= origin.y());
}

#[test]
fn zoom_during_drag_preserves_the_content_point_under_the_pointer() {
    let mut viewer = viewer();
    viewer.runtime_mut().prepare_frame().unwrap();
    let mut domain = EditorDomain::new();
    let start = Point::new(70.0, 80.0);
    let current = Point::new(110.0, 115.0);

    domain
        .pointer_pressed(
            &mut viewer,
            start,
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    domain.pointer_moved(&mut viewer, current).unwrap();
    let before_zoom = viewer.model_point_from_surface(current).unwrap();
    assert!(
        domain
            .set_viewport_scale_at(&mut viewer, 2.0, Some(current))
            .unwrap()
    );
    let after_zoom = viewer.model_point_from_surface(current).unwrap();
    assert!((after_zoom.x() - before_zoom.x()).abs() <= f64::EPSILON);
    assert!((after_zoom.y() - before_zoom.y()).abs() <= f64::EPSILON);

    domain
        .pointer_released(&mut viewer, current, MouseButton::Left)
        .unwrap();
    assert_eq!(
        viewer.model().nodes[&FIRST].bounds,
        Rectangle::new(90.0, 95.0, 100.0, 80.0)
    );
}

#[test]
fn resize_handle_commits_resize_request_without_changing_selection() {
    let mut viewer = viewer();
    let first = viewer.part_for_model(FIRST).unwrap();
    viewer.replace_selection(first).unwrap();
    let (_, handle) = viewer
        .add_handle_visual_with_role(
            first,
            HandleRole::Resize(ResizeDirection::SouthEast),
            Box::new(RectangleFigure::new(145.0, 135.0, 10.0, 10.0)),
        )
        .unwrap();
    assert_eq!(
        viewer.visual_owner(handle).unwrap().owner_part(),
        Some(first)
    );
    let mut domain = EditorDomain::new();

    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(150.0, 140.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    domain
        .pointer_moved(&mut viewer, Point::new(180.0, 160.0))
        .unwrap();
    domain
        .pointer_released(&mut viewer, Point::new(180.0, 160.0), MouseButton::Left)
        .unwrap();

    assert_eq!(viewer.selection().items(), &[first]);
    assert_eq!(
        viewer.model().nodes[&FIRST].bounds,
        Rectangle::new(50.0, 60.0, 130.0, 100.0)
    );
}

#[test]
fn selection_tool_preserves_multi_selection_for_drag_and_collapses_on_click() {
    let mut viewer = viewer();
    let first = viewer.part_for_model(FIRST).unwrap();
    let second = viewer.part_for_model(SECOND).unwrap();
    viewer.replace_selection(first).unwrap();
    viewer.append_selection(second).unwrap();
    let mut domain = EditorDomain::new();

    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(240.0, 100.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    assert_eq!(viewer.selection().items(), &[first, second]);
    domain
        .pointer_moved(&mut viewer, Point::new(260.0, 120.0))
        .unwrap();
    domain
        .pointer_released(&mut viewer, Point::new(260.0, 120.0), MouseButton::Left)
        .unwrap();
    assert_eq!(
        viewer.model().nodes[&FIRST].bounds,
        Rectangle::new(70.0, 80.0, 100.0, 80.0)
    );
    assert_eq!(
        viewer.model().nodes[&SECOND].bounds,
        Rectangle::new(240.0, 100.0, 120.0, 90.0)
    );

    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(80.0, 90.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    domain
        .pointer_released(&mut viewer, Point::new(80.0, 90.0), MouseButton::Left)
        .unwrap();
    assert_eq!(viewer.selection().items(), &[first]);
}

#[test]
fn policy_rejection_is_distinct_from_no_command_contribution() {
    let mut viewer = viewer();
    let mut domain = EditorDomain::new();
    let rejected = EditorRequest::Create(CreateRequest::new(
        viewer.contents(),
        CreationType::new("unsupported").unwrap(),
        Rectangle::new(10.0, 10.0, 50.0, 50.0),
        RequestModifiers::default(),
        domain.next_interaction_revision().unwrap(),
    ));
    assert!(matches!(
        domain.execute_request(&mut viewer, &rejected),
        Err(EditorDomainError::Viewer(ViewerError::Policy(_)))
    ));

    let no_contribution = EditorRequest::Create(CreateRequest::new(
        viewer.part_for_model(FIRST).unwrap(),
        CreationType::new("node").unwrap(),
        Rectangle::new(10.0, 10.0, 50.0, 50.0),
        RequestModifiers::default(),
        domain.next_interaction_revision().unwrap(),
    ));
    assert!(matches!(
        domain.execute_request(&mut viewer, &no_contribution),
        Err(EditorDomainError::NoCommand)
    ));
}

#[test]
fn policy_target_routes_command_and_feedback_to_the_resolved_part_once() {
    let state = Rc::new(RefCell::new(PolicyRoutingState::default()));
    let mut viewer = routing_viewer(Rc::clone(&state));
    let first = viewer.part_for_model(FIRST).unwrap();
    let second = viewer.part_for_model(SECOND).unwrap();
    state.borrow_mut().target = Some(second);
    let request = EditorRequest::Delete(DeleteRequest::new(
        vec![first, second],
        InteractionRevision::initial(),
    ));

    assert!(viewer.command_for_request(&request).unwrap().is_none());
    assert_eq!(state.borrow().command_hosts, vec![SECOND]);

    let feedback = viewer.show_feedback_for_request(&request).unwrap();
    assert_eq!(state.borrow().feedback_hosts, vec![SECOND]);
    assert_eq!(feedback.len(), 1);
    assert_eq!(
        viewer.visual_owner(feedback[0]).unwrap().owner_part(),
        Some(second)
    );
}

#[test]
fn policy_command_panic_faults_viewer_and_rejects_follow_up_operations() {
    let state = Rc::new(RefCell::new(PolicyRoutingState {
        panic_command: true,
        ..PolicyRoutingState::default()
    }));
    let mut viewer = routing_viewer(state);
    let source = viewer.part_for_model(FIRST).unwrap();
    let request = EditorRequest::Delete(DeleteRequest::new(
        vec![source],
        InteractionRevision::initial(),
    ));

    let panic = catch_unwind(AssertUnwindSafe(|| viewer.command_for_request(&request)));
    assert!(panic.is_err());
    assert!(viewer.is_faulted());
    assert!(matches!(viewer.model_mut(), Err(ViewerError::Faulted)));
    assert!(matches!(
        viewer.command_for_request(&request),
        Err(ViewerError::Faulted)
    ));
    assert!(matches!(
        viewer.show_feedback_for_request(&request),
        Err(ViewerError::Faulted)
    ));
}

#[test]
fn policy_feedback_panic_faults_viewer_before_attaching_feedback() {
    let state = Rc::new(RefCell::new(PolicyRoutingState {
        panic_feedback: true,
        ..PolicyRoutingState::default()
    }));
    let mut viewer = routing_viewer(state);
    let source = viewer.part_for_model(FIRST).unwrap();
    let request = EditorRequest::Delete(DeleteRequest::new(
        vec![source],
        InteractionRevision::initial(),
    ));

    let panic = catch_unwind(AssertUnwindSafe(|| {
        viewer.show_feedback_for_request(&request)
    }));
    assert!(panic.is_err());
    assert!(viewer.is_faulted());
    assert!(matches!(
        viewer.show_feedback_for_request(&request),
        Err(ViewerError::Faulted)
    ));
}

#[test]
fn policy_target_rejects_foreign_and_stale_parts() {
    let state = Rc::new(RefCell::new(PolicyRoutingState::default()));
    let mut viewer = routing_viewer(Rc::clone(&state));
    let source = viewer.part_for_model(FIRST).unwrap();
    let request = EditorRequest::Delete(DeleteRequest::new(
        vec![source],
        InteractionRevision::initial(),
    ));

    let foreign_state = Rc::new(RefCell::new(PolicyRoutingState::default()));
    let foreign_viewer = routing_viewer(foreign_state);
    let foreign = foreign_viewer.part_for_model(SECOND).unwrap();
    state.borrow_mut().target = Some(foreign);
    assert!(matches!(
        viewer.command_for_request(&request),
        Err(ViewerError::InvalidPart(part)) if part == foreign
    ));

    let stale = viewer.part_for_model(SECOND).unwrap();
    viewer.model_mut().unwrap().remove(SECOND);
    viewer.refresh().unwrap();
    state.borrow_mut().target = Some(stale);
    assert!(matches!(
        viewer.show_feedback_for_request(&request),
        Err(ViewerError::InvalidPart(part)) if part == stale
    ));
}
