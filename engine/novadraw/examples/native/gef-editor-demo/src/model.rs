use novadraw::figure::{EllipseFigure, Figure, RectangleFigure, RootFigure};
use novadraw::geometry::Rectangle;
use novadraw::graphics::Color;
use novadraw_editor::{
    ChangeBounds, ChangeBoundsRequest, Command, CommandError, ComponentSlot, CreateChild,
    CreateRequest, DeleteParts, DeleteRequest, EditPartBehavior, EditPartError, EditPartFactory,
    EditPolicy, EditingContribution, FeedbackVisual, LayoutSlot, ModelAdapter, ModelEvent,
    ModelRevision, PartFactoryContext, PolicyError, PolicyHandlerBuilder, PolicyHost,
    PolicyInstallation, RequestMetadata, VisualUpdateContext,
};
use std::{collections::BTreeMap, convert::Infallible};

pub const ROOT: u64 = 0;
pub const FIRST: u64 = 1;
pub const INITIAL_BOUNDS: Rectangle = Rectangle {
    x: 60.0,
    y: 70.0,
    width: 120.0,
    height: 80.0,
};
const MIN_SIZE: f64 = 24.0;
const OUTLINE_WIDTH: f64 = 2.0;

#[derive(Clone, Copy)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

#[derive(Clone, Copy)]
pub struct Shape {
    pub bounds: Rectangle,
    pub kind: ShapeKind,
}

pub struct Diagram {
    pub shapes: BTreeMap<u64, Shape>,
    revision: ModelRevision,
    events: Vec<ModelEvent<u64, ()>>,
    next_id: u64,
}

impl Default for Diagram {
    fn default() -> Self {
        Self {
            shapes: BTreeMap::from([
                (
                    FIRST,
                    Shape {
                        bounds: INITIAL_BOUNDS,
                        kind: ShapeKind::Rectangle,
                    },
                ),
                (
                    2,
                    Shape {
                        bounds: Rectangle::new(270.0, 160.0, 140.0, 100.0),
                        kind: ShapeKind::Ellipse,
                    },
                ),
            ]),
            revision: ModelRevision::initial(),
            events: Vec::new(),
            next_id: 3,
        }
    }
}

impl Diagram {
    fn publish(&mut self, subject: u64) {
        self.revision = self.revision.next().expect("demo model revision exhausted");
        self.events
            .push(ModelEvent::new(self.revision, subject, ()));
    }
}

impl ModelAdapter for Diagram {
    type ModelId = u64;
    type Event = ();
    type Error = Infallible;
    fn root(&self) -> u64 {
        ROOT
    }
    fn revision(&self) -> ModelRevision {
        self.revision
    }
    fn children(&self, id: u64) -> Result<Vec<u64>, Infallible> {
        Ok(if id == ROOT {
            self.shapes.keys().copied().collect()
        } else {
            Vec::new()
        })
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<u64, ()>> {
        std::mem::take(&mut self.events)
    }
}

struct MoveShape {
    id: u64,
    before: Rectangle,
    after: Rectangle,
}
impl Command<Diagram> for MoveShape {
    fn label(&self) -> &str {
        "Move / resize shape"
    }
    fn can_execute(&self, _: &Diagram) -> bool {
        self.after.width >= MIN_SIZE && self.after.height >= MIN_SIZE
    }
    fn execute(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.get_mut(&self.id).unwrap().bounds = self.after;
        model.publish(self.id);
        Ok(())
    }
    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.get_mut(&self.id).unwrap().bounds = self.before;
        model.publish(self.id);
        Ok(())
    }
}

struct InsertShape {
    id: u64,
    shape: Shape,
}
impl Command<Diagram> for InsertShape {
    fn label(&self) -> &str {
        "Create shape"
    }
    fn execute(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.insert(self.id, self.shape);
        model.next_id = model.next_id.max(self.id + 1);
        model.publish(ROOT);
        Ok(())
    }
    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.remove(&self.id);
        model.publish(ROOT);
        Ok(())
    }
}

struct RemoveShape {
    id: u64,
    shape: Shape,
}
impl Command<Diagram> for RemoveShape {
    fn label(&self) -> &str {
        "Delete shape"
    }
    fn execute(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.remove(&self.id);
        model.publish(ROOT);
        Ok(())
    }
    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.insert(self.id, self.shape);
        model.publish(ROOT);
        Ok(())
    }
}

fn shape_figure(shape: Shape) -> Box<dyn Figure> {
    let fill = Color::from_hex(match shape.kind {
        ShapeKind::Rectangle => "#4b91c8",
        ShapeKind::Ellipse => "#e5ad52",
    })
    .unwrap();
    let stroke = Color::from_hex("#253448").unwrap();
    let b = shape.bounds;
    match shape.kind {
        ShapeKind::Rectangle => Box::new(
            RectangleFigure::new_with_color(b.x, b.y, b.width, b.height, fill)
                .with_stroke(stroke, OUTLINE_WIDTH),
        ),
        ShapeKind::Ellipse => Box::new(
            EllipseFigure::new_with_color(b.x, b.y, b.width, b.height, fill)
                .with_stroke(stroke, OUTLINE_WIDTH),
        ),
    }
}

struct ShapePolicy;
impl EditPolicy<Diagram> for ShapePolicy {
    fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Diagram, Self>) {
        builder.handle::<ChangeBounds>(Self::change);
        builder.handle::<DeleteParts>(Self::delete);
    }
}
impl ShapePolicy {
    fn change(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        req: &ChangeBoundsRequest,
        model: &Diagram,
    ) -> Result<Option<EditingContribution<Diagram>>, PolicyError> {
        let shape = model.shapes[&host.model()];
        let after = req.transformed_bounds(shape.bounds);
        Ok(Some(EditingContribution::new(
            Some(Box::new(MoveShape {
                id: host.model(),
                before: shape.bounds,
                after,
            })),
            vec![FeedbackVisual::scaled(shape_figure(Shape {
                bounds: after,
                ..shape
            }))],
        )))
    }
    fn delete(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        _: &DeleteRequest,
        model: &Diagram,
    ) -> Result<Option<EditingContribution<Diagram>>, PolicyError> {
        Ok(Some(EditingContribution::command(Box::new(RemoveShape {
            id: host.model(),
            shape: model.shapes[&host.model()],
        }))))
    }
}

struct CanvasPolicy;
impl EditPolicy<Diagram> for CanvasPolicy {
    fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Diagram, Self>) {
        builder.handle::<CreateChild>(Self::create);
    }
}
impl CanvasPolicy {
    fn create(
        &mut self,
        _: PolicyHost<u64>,
        _: &RequestMetadata,
        req: &CreateRequest,
        model: &Diagram,
    ) -> Result<Option<EditingContribution<Diagram>>, PolicyError> {
        let kind = match req.creation_type().as_str() {
            "rectangle" => ShapeKind::Rectangle,
            "ellipse" => ShapeKind::Ellipse,
            _ => return Err(PolicyError::operation("unsupported shape")),
        };
        Ok(Some(EditingContribution::command(Box::new(InsertShape {
            id: model.next_id,
            shape: Shape {
                bounds: req.bounds(),
                kind,
            },
        }))))
    }
}

pub struct ShapePart;
impl EditPartBehavior<Diagram> for ShapePart {
    fn create_figure(
        &mut self,
        model: &Diagram,
        id: u64,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(if id == ROOT {
            Box::new(RootFigure::new(0.0, 0.0, 640.0, 480.0))
        } else {
            shape_figure(model.shapes[&id])
        })
    }
    fn create_policies(
        &mut self,
        _: &Diagram,
        id: u64,
    ) -> Result<Vec<PolicyInstallation<Diagram>>, EditPartError> {
        let policy = if id == ROOT {
            PolicyInstallation::new(LayoutSlot::key(), CanvasPolicy)
        } else {
            PolicyInstallation::new(ComponentSlot::key(), ShapePolicy)
        };
        Ok(vec![policy.map_err(|error| {
            EditPartError::operation(error.to_string())
        })?])
    }
    fn refresh_visuals(
        &mut self,
        model: &Diagram,
        id: u64,
        ctx: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        if id != ROOT {
            ctx.set_primary_bounds(model.shapes[&id].bounds)?;
        }
        Ok(())
    }
}

pub struct ShapeFactory;
impl EditPartFactory<Diagram> for ShapeFactory {
    fn create(
        &mut self,
        _: PartFactoryContext<u64>,
        _: &Diagram,
    ) -> Result<Box<dyn EditPartBehavior<Diagram>>, EditPartError> {
        Ok(Box::new(ShapePart))
    }
}
