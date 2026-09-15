use std::{collections::HashMap, convert::Infallible};

use novadraw_editor::{
    ChangeBoundsRequest, Command, CommandError, CreateRequest, CreationType, DeleteRequest,
    EditPartBehavior, EditPartError, EditPartFactory, EditPolicy, EditorDomain, EditorDomainError,
    EditorRequest, FeedbackVisual, GraphicalViewer, HandleRole, InteractionRevision, ModelAdapter,
    ModelEvent, ModelRevision, PartFactoryContext, PolicyError, PolicyHost, PolicyInstallation,
    PolicyRole, RequestModifiers, ResizeDirection, ViewerError, VisualUpdateContext,
};
use novadraw_geometry::{Dimension, Point, Rectangle, Vec2};
use novadraw_scene::{Figure, KeyModifiers, MouseButton, RectangleFigure, RootFigure};

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

struct DiagramFactory;

impl EditPartFactory<DiagramModel> for DiagramFactory {
    fn create(
        &mut self,
        _context: PartFactoryContext<NodeId>,
        _model: &DiagramModel,
    ) -> Result<Box<dyn EditPartBehavior<DiagramModel>>, EditPartError> {
        Ok(Box::new(DiagramPart))
    }
}

fn viewer() -> GraphicalViewer<DiagramModel, DiagramFactory> {
    GraphicalViewer::new(
        DiagramModel::new(),
        DiagramFactory,
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
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
