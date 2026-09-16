// Audit-only executable: public API probes, no product or test changes.
use std::{cell::RefCell, convert::Infallible, rc::Rc};
use novadraw_editor::*;
use novadraw_geometry::Rectangle;
use novadraw_scene::{Figure, RectangleFigure};

#[derive(Default)]
struct Observed {
    activated: Vec<u32>,
    deactivated: Vec<u32>,
    target: Option<EditPartId>,
    command_hosts: Vec<u32>,
    fail_child: bool,
    panic_refresh: bool,
}
type Shared = Rc<RefCell<Observed>>;

struct Model {
    revision: ModelRevision,
    events: Vec<ModelEvent<u32, ()>>,
}
impl Model {
    fn new() -> Self {
        Self { revision: ModelRevision::initial(), events: vec![] }
    }
    fn change(&mut self) {
        self.revision = self.revision.next().unwrap();
        self.events.push(ModelEvent::new(self.revision, 2, ()));
    }
}
impl ModelAdapter for Model {
    type ModelId = u32;
    type Event = ();
    type Error = Infallible;
    fn root(&self) -> u32 { 1 }
    fn revision(&self) -> ModelRevision { self.revision }
    fn children(&self, id: u32) -> Result<Vec<u32>, Infallible> {
        Ok(if id == 1 { vec![2, 3] } else { vec![] })
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<u32, ()>> {
        std::mem::take(&mut self.events)
    }
}
struct Factory(Shared);
struct Part(Shared);
struct Policy(Shared);
impl EditPartFactory<Model> for Factory {
    fn create(&mut self, context: PartFactoryContext<u32>, _: &Model)
        -> Result<Box<dyn EditPartBehavior<Model>>, EditPartError>
    {
        if self.0.borrow().fail_child && context.model_id() == 2 {
            return Err(EditPartError::operation("audit child failure"));
        }
        Ok(Box::new(Part(self.0.clone())))
    }
}
impl EditPartBehavior<Model> for Part {
    fn create_figure(&mut self, _: &Model, id: u32) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(Box::new(RectangleFigure::new(0.0, 0.0, 100.0 / id as f64, 100.0)))
    }
    fn create_policies(&mut self, _: &Model, _: u32)
        -> Result<Vec<PolicyInstallation<Model>>, EditPartError>
    {
        Ok(vec![(PolicyRole::Component, Box::new(Policy(self.0.clone())))])
    }
    fn refresh_visuals(&mut self, _: &Model, id: u32, ctx: &mut VisualUpdateContext<'_>)
        -> Result<(), EditPartError>
    {
        if id == 2 && self.0.borrow().panic_refresh {
            ctx.set_primary_bounds(Rectangle::new(70.0, 0.0, 20.0, 20.0))?;
            panic!("audit refresh panic after mutation");
        }
        Ok(())
    }
    fn activate(&mut self, _: &Model, id: u32) -> Result<(), EditPartError> {
        self.0.borrow_mut().activated.push(id);
        Ok(())
    }
    fn deactivate(&mut self, _: &Model, id: u32) {
        self.0.borrow_mut().deactivated.push(id);
    }
}
impl EditPolicy<Model> for Policy {
    fn understands(&self, _: &EditorRequest) -> bool { true }
    fn target(&self, _: PolicyHost<u32>, _: &EditorRequest) -> Option<EditPartId> {
        self.0.borrow().target
    }
    fn command(&mut self, host: PolicyHost<u32>, _: &EditorRequest, _: &Model)
        -> Result<Option<Box<dyn Command<Model>>>, PolicyError>
    {
        self.0.borrow_mut().command_hosts.push(host.model());
        Ok(None)
    }
}
fn make(shared: Shared) -> Result<GraphicalViewer<Model, Factory>, ViewerError> {
    GraphicalViewer::new(Model::new(), Factory(shared), Rectangle::new(0.0, 0.0, 400.0, 300.0))
}
fn main() {
    let shared = Shared::default();
    shared.borrow_mut().fail_child = true;
    let result = make(shared.clone());
    assert!(result.is_err());
    println!("initial_failure: activated={:?} deactivated={:?}",
        shared.borrow().activated, shared.borrow().deactivated);
    assert_eq!(shared.borrow().activated, vec![1]);
    assert!(shared.borrow().deactivated.is_empty());

    let shared = Shared::default();
    let mut viewer = make(shared.clone()).unwrap();
    let source = viewer.part_for_model(2).unwrap();
    let target = viewer.part_for_model(3).unwrap();
    shared.borrow_mut().target = Some(target);
    let request = EditorRequest::Delete(DeleteRequest::new(vec![source], InteractionRevision::initial()));
    viewer.command_for_request(&request).unwrap();
    println!("policy_redirect: requested_target_model=3 actual_command_hosts={:?}",
        shared.borrow().command_hosts);
    assert_eq!(shared.borrow().command_hosts, vec![2]);

    shared.borrow_mut().panic_refresh = true;
    viewer.model_mut().change();
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| viewer.refresh())).is_err();
    println!("projection_panic: caught={panicked} viewer_faulted={}", viewer.is_faulted());
    assert!(panicked);
    assert!(!viewer.is_faulted());
}
