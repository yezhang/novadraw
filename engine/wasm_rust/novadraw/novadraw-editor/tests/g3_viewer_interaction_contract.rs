use std::{collections::HashMap, convert::Infallible};

use novadraw::geometry::Rectangle;
use novadraw::{ClickableFigure, Figure, KeyModifiers, MouseButton, RectangleFigure, RootFigure};
use novadraw_editor::{
    EditPartBehavior, EditPartError, EditPartFactory, GraphicalViewer, HandleRole, ModelAdapter,
    ModelEvent, ModelRevision, PartFactoryContext, ViewerTarget, VisualUpdateContext,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct NodeId(u64);

#[derive(Clone, Copy, Debug)]
struct Changed;

struct DiagramModel {
    revision: ModelRevision,
    children: HashMap<NodeId, Vec<NodeId>>,
    bounds: HashMap<NodeId, Rectangle>,
    events: Vec<ModelEvent<NodeId, Changed>>,
}

impl DiagramModel {
    fn new() -> Self {
        Self {
            revision: ModelRevision::initial(),
            children: HashMap::from([(NodeId(1), vec![NodeId(2), NodeId(3), NodeId(4)])]),
            bounds: HashMap::from([
                (NodeId(1), Rectangle::new(0.0, 0.0, 500.0, 320.0)),
                (NodeId(2), Rectangle::new(20.0, 20.0, 80.0, 60.0)),
                (NodeId(3), Rectangle::new(130.0, 20.0, 80.0, 60.0)),
                (NodeId(4), Rectangle::new(240.0, 20.0, 80.0, 60.0)),
            ]),
            events: Vec::new(),
        }
    }

    fn publish(&mut self, subject: NodeId) {
        self.revision = self.revision.next().unwrap();
        self.events
            .push(ModelEvent::new(self.revision, subject, Changed));
    }
}

impl ModelAdapter for DiagramModel {
    type ModelId = NodeId;
    type Event = Changed;
    type Error = Infallible;

    fn root(&self) -> Self::ModelId {
        NodeId(1)
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

struct DiagramPart;

impl EditPartBehavior<DiagramModel> for DiagramPart {
    fn create_figure(
        &mut self,
        model: &DiagramModel,
        model_id: NodeId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        let bounds = model.bounds[&model_id];
        if model_id == NodeId(4) {
            Ok(Box::new(ClickableFigure::new(bounds)))
        } else if model_id == NodeId(1) {
            Ok(Box::new(RootFigure::new(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
            )))
        } else {
            Ok(Box::new(RectangleFigure::from_bounds(bounds)))
        }
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
    viewer_with_bounds(Rectangle::new(0.0, 0.0, 640.0, 480.0))
}

fn viewer_with_bounds(bounds: Rectangle) -> GraphicalViewer<DiagramModel, DiagramFactory> {
    GraphicalViewer::new(DiagramModel::new(), DiagramFactory, bounds).unwrap()
}

#[test]
fn root_layers_have_stable_scaled_and_unscaled_z_order() {
    let viewer = viewer();
    let layers = viewer.root_layers();
    let tree = viewer.runtime().tree();

    assert_eq!(
        tree.child_order(layers.root()).unwrap(),
        vec![layers.viewport_layer(), layers.feedback(), layers.handles()]
    );
    assert_eq!(
        tree.child_order(layers.viewport_layer()).unwrap(),
        vec![layers.viewport()]
    );
    assert_eq!(
        tree.child_order(layers.viewport()).unwrap(),
        vec![layers.scalable()]
    );
    assert_eq!(
        tree.child_order(layers.scalable()).unwrap(),
        vec![layers.grid(), layers.printable(), layers.scaled_feedback()]
    );
    assert_eq!(
        tree.child_order(layers.printable()).unwrap(),
        vec![layers.primary(), layers.connection()]
    );
    assert_eq!(
        viewer.parts().get(viewer.root()).unwrap().content_pane(),
        layers.primary()
    );
}

#[test]
fn viewport_scroll_changes_targeting_through_the_shared_transform_chain() {
    let mut viewer = viewer_with_bounds(Rectangle::new(0.0, 0.0, 120.0, 100.0));
    viewer.prepare_frame().unwrap();
    let first = viewer.part_for_model(NodeId(2)).unwrap();
    let second = viewer.part_for_model(NodeId(3)).unwrap();

    assert_eq!(viewer.target_at(50.0, 40.0), ViewerTarget::Part(first));
    assert!(
        viewer
            .set_viewport_origin(novadraw::geometry::Point::new(100.0, 0.0))
            .unwrap()
    );
    assert_eq!(viewer.target_at(50.0, 40.0), ViewerTarget::Part(second));
}

#[test]
fn pointer_selection_preserves_order_primary_and_focus() {
    let mut viewer = viewer();
    let node_two = viewer.part_for_model(NodeId(2)).unwrap();
    let node_three = viewer.part_for_model(NodeId(3)).unwrap();

    let first = viewer
        .dispatch_mouse_pressed(30.0, 30.0, MouseButton::Left, KeyModifiers::default())
        .unwrap();
    assert!(!first.dispatch().is_handled());
    assert_eq!(first.target(), ViewerTarget::Part(node_two));
    assert_eq!(viewer.selection().items(), &[node_two]);
    assert_eq!(viewer.selection().focus(), Some(node_two));

    viewer
        .dispatch_mouse_pressed(
            140.0,
            30.0,
            MouseButton::Left,
            KeyModifiers {
                shift: true,
                ..KeyModifiers::default()
            },
        )
        .unwrap();
    assert_eq!(viewer.selection().items(), &[node_two, node_three]);
    assert_eq!(viewer.selection().primary(), Some(node_three));

    viewer
        .dispatch_mouse_pressed(
            30.0,
            30.0,
            MouseButton::Left,
            KeyModifiers {
                control: true,
                ..KeyModifiers::default()
            },
        )
        .unwrap();
    assert_eq!(viewer.selection().items(), &[node_three]);
    assert_eq!(viewer.selection().focus(), Some(node_three));
}

#[test]
fn widget_consumption_blocks_editor_selection_fallback() {
    let mut viewer = viewer();
    let node_two = viewer.part_for_model(NodeId(2)).unwrap();
    viewer.replace_selection(node_two).unwrap();

    let outcome = viewer
        .dispatch_mouse_pressed(250.0, 30.0, MouseButton::Left, KeyModifiers::default())
        .unwrap();

    assert!(outcome.dispatch().is_handled());
    assert_eq!(viewer.selection().items(), &[node_two]);
}

#[test]
fn figure_capture_owns_the_complete_pointer_gesture() {
    let mut viewer = viewer();
    let widget = viewer.part_for_model(NodeId(4)).unwrap();
    let widget_figure = viewer.parts().get(widget).unwrap().primary_figure();

    let pressed = viewer
        .dispatch_mouse_pressed(250.0, 30.0, MouseButton::Left, KeyModifiers::default())
        .unwrap();
    assert_eq!(pressed.dispatch().capture(), Some(widget_figure));

    let dragged = viewer.dispatch_mouse_moved(450.0, 250.0);
    assert!(dragged.is_handled());
    assert_eq!(dragged.target(), Some(widget_figure));
    assert_eq!(dragged.capture(), Some(widget_figure));

    let released = viewer.dispatch_mouse_released(450.0, 250.0, MouseButton::Left);
    assert!(released.is_handled());
    assert_eq!(released.target(), Some(widget_figure));
    assert_eq!(released.capture(), None);
    assert!(viewer.selection().is_empty());
}

#[test]
fn handles_win_targeting_and_feedback_is_transparent() {
    let mut viewer = viewer();
    let owner = viewer.part_for_model(NodeId(2)).unwrap();
    let (_, feedback) = viewer
        .add_feedback_visual(
            Some(owner),
            false,
            Box::new(RectangleFigure::new(20.0, 20.0, 80.0, 60.0)),
        )
        .unwrap();

    assert_eq!(viewer.target_at(30.0, 30.0), ViewerTarget::Part(owner));

    let (handle, _) = viewer
        .add_handle_visual(
            owner,
            Box::new(RectangleFigure::new(20.0, 20.0, 20.0, 20.0)),
        )
        .unwrap();
    assert_eq!(
        viewer.target_at(30.0, 30.0),
        ViewerTarget::Handle {
            id: handle,
            owner,
            role: HandleRole::Selection,
        }
    );
    assert!(viewer.remove_overlay_visual(feedback).unwrap());
}

#[test]
fn removing_parts_reconciles_selection_focus_and_owned_overlays() {
    let mut viewer = viewer();
    let removed = viewer.part_for_model(NodeId(2)).unwrap();
    viewer.replace_selection(removed).unwrap();
    viewer.set_focus(Some(removed)).unwrap();
    let (_, handle) = viewer
        .add_handle_visual(
            removed,
            Box::new(RectangleFigure::new(20.0, 20.0, 20.0, 20.0)),
        )
        .unwrap();

    viewer
        .model_mut()
        .unwrap()
        .children
        .insert(NodeId(1), vec![NodeId(3), NodeId(4)]);
    viewer.model_mut().unwrap().publish(NodeId(1));
    viewer.refresh().unwrap();

    assert!(viewer.selection().is_empty());
    assert_eq!(viewer.selection().focus(), None);
    assert!(!viewer.runtime().tree().is_attached(handle));
    assert_eq!(viewer.visual_owner(handle), None);
}

#[test]
fn background_click_clears_visual_selection_to_contents_fallback() {
    let mut viewer = viewer();
    let selected = viewer.part_for_model(NodeId(2)).unwrap();
    viewer.replace_selection(selected).unwrap();

    let outcome = viewer
        .dispatch_mouse_pressed(450.0, 250.0, MouseButton::Left, KeyModifiers::default())
        .unwrap();

    assert_eq!(outcome.target(), ViewerTarget::Contents(viewer.contents()));
    assert!(viewer.selection().is_empty());
}
