use novadraw::{
    event::{KeyModifiers, MouseButton},
    figure::{Figure, RectangleFigure, RootFigure},
    geometry::{Dimension, Point, Rectangle},
};
use novadraw_editor::{
    Command, CommandError, EditPartBehavior, EditPartError, EditPartFactory, EditPartId,
    EditPolicy, EditingContribution, EditorDomain, GraphicalViewer, InteractionRevision,
    LayoutSlot, ModelAdapter, ModelEvent, ModelRevision, PartFactoryContext, PolicyError,
    PolicyHandlerBuilder, PolicyHost, PolicyInstallation, RequestKey, RequestMetadata,
    RequestModifiers, RequestProtocol,
    tool::{CreationTemplate, CreationTool, MarqueeTool, Tool, ToolKey},
};
use std::{collections::BTreeMap, convert::Infallible};

struct Model {
    nodes: BTreeMap<u64, Rectangle>,
    revision: ModelRevision,
    events: Vec<ModelEvent<u64, ()>>,
}
impl ModelAdapter for Model {
    type ModelId = u64;
    type Event = ();
    type Error = Infallible;
    fn root(&self) -> u64 {
        0
    }
    fn revision(&self) -> ModelRevision {
        self.revision
    }
    fn children(&self, id: u64) -> Result<Vec<u64>, Infallible> {
        Ok(if id == 0 {
            self.nodes.keys().copied().collect()
        } else {
            vec![]
        })
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<u64, ()>> {
        std::mem::take(&mut self.events)
    }
}
impl Model {
    fn publish(&mut self) {
        self.revision = self.revision.next().unwrap();
        self.events.push(ModelEvent::new(self.revision, 0, ()));
    }
}

// This family is entirely external; there is no CreateChild or string type dispatch.
struct CreateCard;
struct CardPayload {
    bounds: Rectangle,
    enabled: bool,
}
impl RequestProtocol<Model> for CreateCard {
    type Payload = CardPayload;
    type Contribution = EditingContribution<Model>;
    fn key() -> RequestKey<Self> {
        RequestKey::new("external-card")
    }
}
struct CardTemplate(bool);
impl CreationTemplate<Model> for CardTemplate {
    type Request = CreateCard;
    fn default_size(&self) -> Dimension {
        Dimension::new(40.0, 30.0)
    }
    fn request(
        &self,
        contents: EditPartId,
        bounds: Rectangle,
        _: RequestModifiers,
        revision: InteractionRevision,
    ) -> (RequestMetadata, CardPayload) {
        (
            RequestMetadata::new(revision, vec![contents]),
            CardPayload {
                bounds,
                enabled: self.0,
            },
        )
    }
}
struct Insert {
    id: u64,
    bounds: Rectangle,
}
impl Command<Model> for Insert {
    fn label(&self) -> &str {
        "External card"
    }
    fn execute(&mut self, m: &mut Model) -> Result<(), CommandError> {
        m.nodes.insert(self.id, self.bounds);
        m.publish();
        Ok(())
    }
    fn undo(&mut self, m: &mut Model) -> Result<(), CommandError> {
        m.nodes.remove(&self.id);
        m.publish();
        Ok(())
    }
}
struct Policy;
impl EditPolicy<Model> for Policy {
    fn register_handlers(b: &mut PolicyHandlerBuilder<'_, Model, Self>) {
        b.handle::<CreateCard>(Self::create);
    }
}
impl Policy {
    fn create(
        &mut self,
        _: PolicyHost<u64>,
        _: &RequestMetadata,
        p: &CardPayload,
        m: &Model,
    ) -> Result<Option<EditingContribution<Model>>, PolicyError> {
        if !p.enabled {
            return Ok(None);
        }
        let id = m.nodes.keys().max().unwrap() + 1;
        Ok(Some(EditingContribution::command(Box::new(Insert {
            id,
            bounds: p.bounds,
        }))))
    }
}
struct Part;
impl EditPartBehavior<Model> for Part {
    fn create_figure(&mut self, m: &Model, id: u64) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(if id == 0 {
            Box::new(RootFigure::new(0.0, 0.0, 600.0, 400.0).expect("valid detached figure input"))
        } else {
            Box::new(RectangleFigure::from_bounds(m.nodes[&id]).expect("valid figure bounds"))
        })
    }
    fn create_policies(
        &mut self,
        _: &Model,
        id: u64,
    ) -> Result<Vec<PolicyInstallation<Model>>, EditPartError> {
        Ok(if id == 0 {
            vec![
                PolicyInstallation::new(LayoutSlot::key(), Policy)
                    .map_err(|e| EditPartError::operation(e.to_string()))?,
            ]
        } else {
            vec![]
        })
    }
}
struct Factory;
impl EditPartFactory<Model> for Factory {
    fn create(
        &mut self,
        _: PartFactoryContext<u64>,
        _: &Model,
    ) -> Result<Box<dyn EditPartBehavior<Model>>, EditPartError> {
        Ok(Box::new(Part))
    }
}
fn viewer() -> GraphicalViewer<Model, Factory> {
    GraphicalViewer::new(
        Model {
            nodes: BTreeMap::from([
                (1, Rectangle::new(40.0, 50.0, 60.0, 40.0)),
                (2, Rectangle::new(160.0, 50.0, 60.0, 40.0)),
            ]),
            revision: ModelRevision::initial(),
            events: vec![],
        },
        Factory,
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
    )
    .unwrap()
}
fn gesture(
    d: &mut EditorDomain<Model>,
    v: &mut GraphicalViewer<Model, Factory>,
    start: Point,
    end: Point,
    modifiers: KeyModifiers,
) {
    d.pointer_pressed(v, start, MouseButton::Left, modifiers)
        .unwrap();
    d.pointer_moved(v, end).unwrap();
    d.pointer_released(v, end, MouseButton::Left).unwrap();
}

#[test]
fn external_typed_template_tool_and_drop_share_policy_history_and_coordinates() {
    let mut v = viewer();
    let mut d = EditorDomain::new();
    let template = CardTemplate(true);
    d.set_active_tool(&mut v, CreationTool::new(template))
        .unwrap();
    gesture(
        &mut d,
        &mut v,
        Point::new(300.0, 140.0),
        Point::new(270.0, 100.0),
        KeyModifiers::default(),
    );
    assert_eq!(
        v.model().nodes[&3],
        Rectangle::new(270.0, 100.0, 30.0, 40.0)
    );
    d.mark_save_location().unwrap();
    assert!(!d.is_dirty());
    d.drop_template(
        &mut v,
        &CardTemplate(true),
        Point::new(350.0, 200.0),
        RequestModifiers::default(),
    )
    .unwrap();
    assert_eq!(
        v.model().nodes[&4],
        Rectangle::new(350.0, 200.0, 40.0, 30.0)
    );
    assert!(d.is_dirty());
    d.undo(&mut v).unwrap();
    assert!(!d.is_dirty());
    d.redo(&mut v).unwrap();
    assert!(d.is_dirty());
    assert!(
        d.drop_template(
            &mut v,
            &CardTemplate(false),
            Point::ZERO,
            RequestModifiers::default()
        )
        .is_err()
    );
    assert_eq!(v.model().nodes.len(), 4);
}

#[test]
fn creation_cancel_and_click_have_no_preview_model_mutation() {
    let mut v = viewer();
    let mut d = EditorDomain::new();
    d.set_active_tool(&mut v, CreationTool::new(CardTemplate(true)))
        .unwrap();
    d.pointer_pressed(
        &mut v,
        Point::new(300.0, 120.0),
        MouseButton::Left,
        KeyModifiers::default(),
    )
    .unwrap();
    d.pointer_moved(&mut v, Point::new(380.0, 180.0)).unwrap();
    assert_eq!(v.model().nodes.len(), 2);
    d.cancel_tool(&mut v).unwrap();
    d.pointer_released(&mut v, Point::new(380.0, 180.0), MouseButton::Left)
        .unwrap();
    assert!(!d.has_undo());
    d.set_active_tool(&mut v, CreationTool::new(CardTemplate(true)))
        .unwrap();
    gesture(
        &mut d,
        &mut v,
        Point::new(300.0, 120.0),
        Point::new(300.0, 120.0),
        KeyModifiers::default(),
    );
    assert_eq!(
        v.model().nodes[&3],
        Rectangle::new(300.0, 120.0, 40.0, 30.0)
    );
}

#[test]
fn marquee_enclosure_append_toggle_cancel_and_foreign_batch_rejection() {
    let mut v = viewer();
    let mut d = EditorDomain::new();
    let first = v.part_for_model(1).unwrap();
    let second = v.part_for_model(2).unwrap();
    d.set_active_tool(&mut v, MarqueeTool::new()).unwrap();
    gesture(
        &mut d,
        &mut v,
        Point::new(25.0, 35.0),
        Point::new(115.0, 105.0),
        KeyModifiers::default(),
    );
    assert_eq!(v.selection().items(), &[first]);
    d.set_active_tool(&mut v, MarqueeTool::new()).unwrap();
    gesture(
        &mut d,
        &mut v,
        Point::new(150.0, 35.0),
        Point::new(235.0, 105.0),
        KeyModifiers {
            shift: true,
            ..KeyModifiers::default()
        },
    );
    assert_eq!(v.selection().items(), &[first, second]);
    d.set_active_tool(&mut v, MarqueeTool::new()).unwrap();
    gesture(
        &mut d,
        &mut v,
        Point::new(25.0, 35.0),
        Point::new(115.0, 105.0),
        KeyModifiers {
            control: true,
            ..KeyModifiers::default()
        },
    );
    assert_eq!(v.selection().items(), &[second]);
    d.set_active_tool(&mut v, MarqueeTool::new()).unwrap();
    d.pointer_pressed(
        &mut v,
        Point::ZERO,
        MouseButton::Left,
        KeyModifiers::default(),
    )
    .unwrap();
    d.pointer_moved(&mut v, Point::new(240.0, 120.0)).unwrap();
    d.cancel_tool(&mut v).unwrap();
    assert_eq!(v.selection().items(), &[second]);
    let foreign = viewer().part_for_model(1).unwrap();
    assert!(v.set_selection(&[first, foreign]).is_err());
    assert_eq!(v.selection().items(), &[second]);
    assert!(!d.has_undo());
    assert_eq!(
        <MarqueeTool as Tool<Model>>::key(&MarqueeTool::new()),
        ToolKey::<MarqueeTool>::new("marquee").erase()
    );
}
