use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex},
};

use novadraw_editor::{
    EditPartBehavior, EditPartError, EditPartFactory, EditPartId, GraphicalViewer, ModelAdapter,
    ModelEvent, ModelRevision, PartFactoryContext, ViewerError, VisualBuildContext,
    VisualUpdateContext,
};
use novadraw_geometry::Rectangle;
use novadraw_scene::{Figure, FigureId, RectangleFigure, RootFigure};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct NodeId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagramEvent {
    Changed,
}

struct DiagramModel {
    revision: ModelRevision,
    root: NodeId,
    children: HashMap<NodeId, Vec<NodeId>>,
    bounds: HashMap<NodeId, Rectangle>,
    events: Vec<ModelEvent<NodeId, DiagramEvent>>,
}

impl DiagramModel {
    fn initial() -> Self {
        Self {
            revision: ModelRevision::initial(),
            root: NodeId(1),
            children: HashMap::from([
                (NodeId(1), vec![NodeId(2), NodeId(3)]),
                (NodeId(2), vec![NodeId(4)]),
            ]),
            bounds: HashMap::from([
                (NodeId(1), Rectangle::new(0.0, 0.0, 400.0, 300.0)),
                (NodeId(2), Rectangle::new(10.0, 20.0, 100.0, 80.0)),
                (NodeId(3), Rectangle::new(140.0, 20.0, 100.0, 80.0)),
                (NodeId(4), Rectangle::new(5.0, 5.0, 40.0, 30.0)),
            ]),
            events: Vec::new(),
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
}

impl ModelAdapter for DiagramModel {
    type Error = Infallible;
    type Event = DiagramEvent;
    type ModelId = NodeId;

    fn root(&self) -> Self::ModelId {
        self.root
    }

    fn revision(&self) -> ModelRevision {
        self.revision
    }

    fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
        Ok(self.children.get(&model).cloned().unwrap_or_default())
    }

    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
        std::mem::take(&mut self.events)
    }
}

struct RectanglePart {
    lifecycle: Arc<Mutex<Vec<String>>>,
}

impl EditPartBehavior<DiagramModel> for RectanglePart {
    fn create_figure(
        &mut self,
        model: &DiagramModel,
        model_id: NodeId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        let bounds = model.bounds[&model_id];
        Ok(Box::new(RectangleFigure::from_bounds(bounds)))
    }

    fn configure_visual(
        &mut self,
        model: &DiagramModel,
        model_id: NodeId,
        context: &mut VisualBuildContext<'_>,
    ) -> Result<(), EditPartError> {
        if model_id == NodeId(2) {
            let bounds = model.bounds[&model_id];
            let pane = context.add_child(
                context.primary(),
                Box::new(RootFigure::new(0.0, 0.0, bounds.width, bounds.height)),
            )?;
            context.set_content_pane(pane)?;
        }
        Ok(())
    }

    fn refresh_visuals(
        &mut self,
        model: &DiagramModel,
        model_id: NodeId,
        context: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        context.set_primary_bounds(model.bounds[&model_id])?;
        Ok(())
    }

    fn activate(&mut self, _model: &DiagramModel, model_id: NodeId) -> Result<(), EditPartError> {
        self.lifecycle
            .lock()
            .unwrap()
            .push(format!("activate:{}", model_id.0));
        Ok(())
    }

    fn deactivate(&mut self, _model: &DiagramModel, model_id: NodeId) {
        self.lifecycle
            .lock()
            .unwrap()
            .push(format!("deactivate:{}", model_id.0));
    }
}

struct RectangleFactory {
    lifecycle: Arc<Mutex<Vec<String>>>,
}

impl EditPartFactory<DiagramModel> for RectangleFactory {
    fn create(
        &mut self,
        context: PartFactoryContext<NodeId>,
        _model: &DiagramModel,
    ) -> Result<Box<dyn EditPartBehavior<DiagramModel>>, EditPartError> {
        if context.model_id() == NodeId(1) {
            assert_eq!(context.parent_model(), None);
        }
        Ok(Box::new(RectanglePart {
            lifecycle: Arc::clone(&self.lifecycle),
        }))
    }
}

struct FailingChildFactory {
    lifecycle: Arc<Mutex<Vec<String>>>,
    fail_on: NodeId,
}

impl EditPartFactory<DiagramModel> for FailingChildFactory {
    fn create(
        &mut self,
        context: PartFactoryContext<NodeId>,
        _model: &DiagramModel,
    ) -> Result<Box<dyn EditPartBehavior<DiagramModel>>, EditPartError> {
        if context.model_id() == self.fail_on {
            return Err(EditPartError::operation("child creation failed"));
        }
        Ok(Box::new(RectanglePart {
            lifecycle: Arc::clone(&self.lifecycle),
        }))
    }
}

fn viewer() -> (
    GraphicalViewer<DiagramModel, RectangleFactory>,
    Arc<Mutex<Vec<String>>>,
) {
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let factory = RectangleFactory {
        lifecycle: Arc::clone(&lifecycle),
    };
    let viewer = GraphicalViewer::new(
        DiagramModel::initial(),
        factory,
        Rectangle::new(0.0, 0.0, 800.0, 600.0),
    )
    .unwrap();
    (viewer, lifecycle)
}

#[test]
fn viewer_projects_model_tree_and_registers_model_and_visual_identity() {
    let (viewer, lifecycle) = viewer();
    let contents = viewer.contents();
    let node_two = viewer.part_for_model(NodeId(2)).unwrap();
    let node_four = viewer.part_for_model(NodeId(4)).unwrap();

    assert_eq!(viewer.parts().parent(node_two), Some(contents));
    assert_eq!(viewer.parts().parent(node_four), Some(node_two));
    assert_eq!(
        viewer.parts().children(contents).unwrap(),
        &[node_two, viewer.part_for_model(NodeId(3)).unwrap()]
    );

    let primary = viewer.parts().get(node_two).unwrap().primary_figure();
    let pane = viewer.parts().get(node_two).unwrap().content_pane();
    assert_ne!(primary, pane);
    assert_eq!(viewer.part_for_visual(primary), Some(node_two));
    assert_eq!(viewer.part_for_visual(pane), Some(node_two));
    assert_eq!(
        viewer.part_for_visual_or_ancestor(viewer.parts().get(node_four).unwrap().primary_figure()),
        Some(node_four)
    );
    assert_eq!(
        lifecycle.lock().unwrap().as_slice(),
        ["activate:1", "activate:2", "activate:4", "activate:3"]
    );
}

#[test]
fn part_ids_are_namespaced_and_rejected_by_another_viewer() {
    let (left, _) = viewer();
    let (right, _) = viewer();
    let foreign = left.part_for_model(NodeId(2)).unwrap();

    assert_ne!(left.namespace(), right.namespace());
    assert!(right.parts().get(foreign).is_none());
    assert!(right.part_for_model(NodeId(2)).is_some());
}

#[test]
fn refresh_reuses_reorders_adds_and_removes_parts_with_lifecycle_cleanup() {
    let (mut viewer, lifecycle) = viewer();
    let old_two = viewer.part_for_model(NodeId(2)).unwrap();
    let old_three = viewer.part_for_model(NodeId(3)).unwrap();
    let removed_visual = viewer.parts().get(old_two).unwrap().primary_figure();

    viewer
        .model_mut()
        .children
        .insert(NodeId(1), vec![NodeId(3), NodeId(5)]);
    viewer
        .model_mut()
        .bounds
        .insert(NodeId(5), Rectangle::new(20.0, 140.0, 80.0, 60.0));
    viewer.model_mut().publish(NodeId(1));
    viewer.refresh().unwrap();

    let new_three = viewer.part_for_model(NodeId(3)).unwrap();
    let new_five = viewer.part_for_model(NodeId(5)).unwrap();
    assert_eq!(old_three, new_three);
    assert_eq!(
        viewer.parts().children(viewer.contents()).unwrap(),
        &[new_three, new_five]
    );
    assert!(viewer.parts().get(old_two).is_none());
    assert_eq!(viewer.part_for_visual(removed_visual), None);
    assert!(!viewer.runtime().tree().is_attached(removed_visual));
    assert_eq!(
        lifecycle.lock().unwrap().as_slice(),
        [
            "activate:1",
            "activate:2",
            "activate:4",
            "activate:3",
            "deactivate:2",
            "deactivate:4",
            "activate:5",
        ]
    );
}

#[test]
fn revision_gap_faults_viewer_before_projection_changes() {
    let (mut viewer, _) = viewer();
    let children_before = viewer.parts().children(viewer.contents()).unwrap().to_vec();
    viewer.model_mut().revision = ModelRevision::new(3).unwrap();
    viewer.model_mut().events.push(ModelEvent::new(
        ModelRevision::new(3).unwrap(),
        NodeId(1),
        DiagramEvent::Changed,
    ));

    assert!(matches!(
        viewer.refresh(),
        Err(ViewerError::RevisionGap { .. })
    ));
    assert!(viewer.is_faulted());
    assert_eq!(
        viewer.parts().children(viewer.contents()).unwrap(),
        children_before
    );
}

#[test]
fn one_revision_may_carry_multiple_ordered_events() {
    let (mut viewer, _) = viewer();
    let revision = viewer.model().revision.next().unwrap();
    let updated = Rectangle::new(160.0, 40.0, 120.0, 90.0);
    viewer.model_mut().revision = revision;
    viewer.model_mut().bounds.insert(NodeId(3), updated);
    viewer.model_mut().events.extend([
        ModelEvent::new(revision, NodeId(1), DiagramEvent::Changed),
        ModelEvent::new(revision, NodeId(3), DiagramEvent::Changed),
    ]);

    assert!(viewer.refresh().unwrap());
    assert_eq!(viewer.applied_revision(), revision);
    let figure = viewer
        .parts()
        .get(viewer.part_for_model(NodeId(3)).unwrap())
        .unwrap()
        .primary_figure();
    assert_eq!(viewer.runtime().tree().figure_bounds(figure), Some(updated));
}

#[test]
fn initial_projection_absorbs_events_already_represented_by_the_snapshot() {
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let mut model = DiagramModel::initial();
    model.events.push(ModelEvent::new(
        ModelRevision::initial(),
        NodeId(1),
        DiagramEvent::Changed,
    ));
    let mut viewer = GraphicalViewer::new(
        model,
        RectangleFactory { lifecycle },
        Rectangle::new(0.0, 0.0, 800.0, 600.0),
    )
    .unwrap();

    assert!(!viewer.refresh().unwrap());
    assert_eq!(viewer.applied_revision(), ModelRevision::initial());
}

#[test]
fn stale_notification_faults_viewer_without_changing_projection() {
    let (mut viewer, _) = viewer();
    let children_before = viewer.parts().children(viewer.contents()).unwrap().to_vec();
    viewer.model_mut().events.push(ModelEvent::new(
        ModelRevision::initial(),
        NodeId(1),
        DiagramEvent::Changed,
    ));

    assert!(matches!(
        viewer.refresh(),
        Err(ViewerError::StaleRevision { .. })
    ));
    assert!(viewer.is_faulted());
    assert_eq!(
        viewer.parts().children(viewer.contents()).unwrap(),
        children_before
    );
}

#[test]
fn removing_and_readding_a_model_rebuilds_runtime_identity() {
    let (mut viewer, _) = viewer();
    let old_part = viewer.part_for_model(NodeId(2)).unwrap();
    let old_figure = viewer.parts().get(old_part).unwrap().primary_figure();

    viewer
        .model_mut()
        .children
        .insert(NodeId(1), vec![NodeId(3)]);
    viewer.model_mut().publish(NodeId(1));
    viewer.refresh().unwrap();
    viewer
        .model_mut()
        .children
        .insert(NodeId(1), vec![NodeId(3), NodeId(2)]);
    viewer.model_mut().publish(NodeId(1));
    viewer.refresh().unwrap();

    let new_part = viewer.part_for_model(NodeId(2)).unwrap();
    let new_figure = viewer.parts().get(new_part).unwrap().primary_figure();
    assert_ne!(new_part, old_part);
    assert_ne!(new_figure, old_figure);
    assert!(viewer.parts().get(old_part).is_none());
    assert!(!viewer.runtime().tree().is_attached(old_figure));
}

#[test]
fn reparenting_is_order_independent_and_rebuilds_the_moved_part() {
    let (mut viewer, _) = viewer();
    let old_part = viewer.part_for_model(NodeId(4)).unwrap();
    viewer
        .model_mut()
        .children
        .insert(NodeId(1), vec![NodeId(3), NodeId(2)]);
    viewer.model_mut().children.insert(NodeId(2), Vec::new());
    viewer
        .model_mut()
        .children
        .insert(NodeId(3), vec![NodeId(4)]);
    viewer.model_mut().publish(NodeId(1));

    viewer.refresh().unwrap();

    let new_part = viewer.part_for_model(NodeId(4)).unwrap();
    assert_ne!(new_part, old_part);
    assert_eq!(
        viewer.parts().parent(new_part),
        viewer.part_for_model(NodeId(3))
    );
}

#[test]
fn duplicate_model_identity_is_rejected_during_initial_projection() {
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let mut model = DiagramModel::initial();
    model.children.insert(NodeId(1), vec![NodeId(2), NodeId(2)]);

    let result = GraphicalViewer::new(
        model,
        RectangleFactory { lifecycle },
        Rectangle::new(0.0, 0.0, 800.0, 600.0),
    );
    assert!(matches!(result, Err(ViewerError::DuplicateModel)));
}

#[test]
fn initial_projection_failure_deactivates_every_activated_part_once() {
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let result = GraphicalViewer::new(
        DiagramModel::initial(),
        FailingChildFactory {
            lifecycle: Arc::clone(&lifecycle),
            fail_on: NodeId(2),
        },
        Rectangle::new(0.0, 0.0, 800.0, 600.0),
    );

    assert!(result.is_err());
    assert_eq!(
        lifecycle.lock().unwrap().as_slice(),
        ["activate:1", "deactivate:1"]
    );
}

#[test]
fn late_initial_projection_failure_deactivates_the_complete_live_prefix_once() {
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let result = GraphicalViewer::new(
        DiagramModel::initial(),
        FailingChildFactory {
            lifecycle: Arc::clone(&lifecycle),
            fail_on: NodeId(3),
        },
        Rectangle::new(0.0, 0.0, 800.0, 600.0),
    );

    assert!(result.is_err());
    assert_eq!(
        lifecycle.lock().unwrap().as_slice(),
        [
            "activate:1",
            "activate:2",
            "activate:4",
            "deactivate:1",
            "deactivate:2",
            "deactivate:4",
        ]
    );
}

#[test]
fn duplicate_model_identity_faults_refresh_before_projection_changes() {
    let (mut viewer, _) = viewer();
    let children_before = viewer.parts().children(viewer.contents()).unwrap().to_vec();
    viewer
        .model_mut()
        .children
        .insert(NodeId(1), vec![NodeId(2), NodeId(2)]);
    viewer.model_mut().publish(NodeId(1));

    assert!(matches!(viewer.refresh(), Err(ViewerError::DuplicateModel)));
    assert!(viewer.is_faulted());
    assert_eq!(
        viewer.parts().children(viewer.contents()).unwrap(),
        children_before
    );
}

#[test]
fn unknown_and_foreign_visuals_do_not_resolve_to_parts() {
    let (local_viewer, _) = viewer();
    assert_eq!(local_viewer.part_for_visual(FigureId::null()), None);

    let (foreign_viewer, _) = viewer();
    let foreign = foreign_viewer
        .parts()
        .get(foreign_viewer.contents())
        .unwrap()
        .primary_figure();
    assert_eq!(local_viewer.part_for_visual_or_ancestor(foreign), None);
}

#[test]
fn unregistered_internal_visual_resolves_through_its_registered_ancestor() {
    let (mut viewer, _) = viewer();
    let node_two = viewer.part_for_model(NodeId(2)).unwrap();
    let pane = viewer.parts().get(node_two).unwrap().content_pane();
    let internal = viewer
        .runtime_mut()
        .try_add_figure(pane, Box::new(RectangleFigure::new(1.0, 1.0, 5.0, 5.0)))
        .unwrap();

    assert_eq!(viewer.part_for_visual(internal), None);
    assert_eq!(viewer.part_for_visual_or_ancestor(internal), Some(node_two));
}

#[test]
fn root_part_is_not_a_model_part() {
    let (viewer, _) = viewer();
    let root: EditPartId = viewer.parts().root();
    assert_eq!(viewer.parts().get(root).unwrap().model_id(), None);
    assert_eq!(viewer.parts().parent(viewer.contents()), Some(root));
}

#[test]
fn dropping_viewer_deactivates_every_live_part_in_preorder() {
    let lifecycle = {
        let (viewer, lifecycle) = viewer();
        assert!(viewer.parts().get(viewer.contents()).unwrap().is_active());
        lifecycle
    };

    assert_eq!(
        lifecycle.lock().unwrap().as_slice(),
        [
            "activate:1",
            "activate:2",
            "activate:4",
            "activate:3",
            "deactivate:1",
            "deactivate:2",
            "deactivate:4",
            "deactivate:3",
        ]
    );
}
