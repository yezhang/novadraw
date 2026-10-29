use crate::alignment::{AlignShapes, AlignmentRequest};
use novadraw::connection::{
    AnchorSemanticKey, ChopboxAnchor, ConnectionFigure, ConnectionLocator, EllipseAnchor,
    PolygonDecorationFigure,
};
use novadraw::container::FreeformLayerFigure;
use novadraw::figure::{EllipseFigure, Figure, FigureStyle, RectangleFigure};
use novadraw::geometry::{Dimension, Point, PointList, Rectangle};
use novadraw::graphics::{Color, DashPattern, Opacity, StrokeStyle};
use novadraw_editor::tool::CreationTemplate;
use novadraw_editor::{
    ChangeBounds, ChangeBoundsKind, ChangeBoundsRequest, Command, CommandError, ComponentSlot,
    CreateChild, CreateRequest, DeleteParts, DeleteRequest, EditPartBehavior, EditPartError,
    EditPartFactory, EditPolicy, EditingContribution, FeedbackVisual, LayoutSlot, ModelAdapter,
    ModelEvent, ModelRevision, PartFactoryContext, PolicyError, PolicyHandlerBuilder, PolicyHost,
    PolicyInstallation, RequestMetadata, ResizeDirection, VisualUpdateContext,
};
use novadraw_editor::{
    ConnectionAnchorContext, ConnectionAnchorDescriptor, ConnectionCreation,
    ConnectionCreationSlot, ConnectionEndpoint, ConnectionFeedbackRoute,
    ConnectionPartFactoryContext, ConnectionReconnectSlot, ConnectionReconnection,
    CreateConnection, CreateConnectionRequest, CreationType, EditPartId, InteractionRevision,
    ModelConnection, ReconnectConnection, ReconnectConnectionRequest, RequestModifiers,
    RequestProtocol, VisualBuildContext,
};
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, collections::BTreeMap, convert::Infallible, rc::Rc};

pub const ROOT: u64 = 0;
pub const FIRST: u64 = 1;
pub const MIN_SIZE: f64 = 24.0;
const OUTLINE_WIDTH: f64 = 2.0;
pub const DEFAULT_WIDTH: f64 = 120.0;
pub const DEFAULT_HEIGHT: f64 = 80.0;
pub const DEFAULT_GRID_SIZE: f64 = 16.0;
const ARROW_LENGTH: f64 = 7.0;
const ARROW_HALF_WIDTH: f64 = 3.0;
const HANDLE_SIZE: f64 = 8.0;
const SELECTION_BLUE: &str = "#2D6991";
const SHOWCASE_LAST_ID: u64 = 12;
const SHOWCASE_LEFT_X: f64 = 60.0;
const SHOWCASE_CENTER_X: f64 = 270.0;
const SHOWCASE_RIGHT_X: f64 = 480.0;
const SHOWCASE_TOP_Y: f64 = 70.0;
const SHOWCASE_BOTTOM_Y: f64 = 310.0;
pub const INITIAL_BOUNDS: Rectangle = showcase_bounds(SHOWCASE_LEFT_X, SHOWCASE_TOP_Y);
#[cfg(test)]
const MINIMAL_SECOND_BOUNDS: Rectangle = Rectangle {
    x: 270.0,
    y: 160.0,
    width: 140.0,
    height: 100.0,
};

const fn showcase_bounds(x: f64, y: f64) -> Rectangle {
    Rectangle {
        x,
        y,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
    }
}

const SHOWCASE_SHAPES: [(u64, Shape); 6] = [
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
            bounds: showcase_bounds(SHOWCASE_CENTER_X, SHOWCASE_TOP_Y),
            kind: ShapeKind::Ellipse,
        },
    ),
    (
        3,
        Shape {
            bounds: showcase_bounds(SHOWCASE_RIGHT_X, SHOWCASE_TOP_Y),
            kind: ShapeKind::Rectangle,
        },
    ),
    (
        4,
        Shape {
            bounds: showcase_bounds(SHOWCASE_LEFT_X, SHOWCASE_BOTTOM_Y),
            kind: ShapeKind::Ellipse,
        },
    ),
    (
        5,
        Shape {
            bounds: showcase_bounds(SHOWCASE_CENTER_X, SHOWCASE_BOTTOM_Y),
            kind: ShapeKind::Rectangle,
        },
    ),
    (
        6,
        Shape {
            bounds: showcase_bounds(SHOWCASE_RIGHT_X, SHOWCASE_BOTTOM_Y),
            kind: ShapeKind::Ellipse,
        },
    ),
];

const SHOWCASE_EDGES: [(u64, Edge); 6] = [
    (
        7,
        Edge {
            source: 1,
            target: 2,
            style: EdgeStyle::Solid,
        },
    ),
    (
        8,
        Edge {
            source: 2,
            target: 3,
            style: EdgeStyle::Dashed,
        },
    ),
    (
        9,
        Edge {
            source: 1,
            target: 4,
            style: EdgeStyle::Solid,
        },
    ),
    (
        10,
        Edge {
            source: 2,
            target: 5,
            style: EdgeStyle::Solid,
        },
    ),
    (
        11,
        Edge {
            source: 4,
            target: 5,
            style: EdgeStyle::Dashed,
        },
    ),
    (
        12,
        Edge {
            source: 5,
            target: 6,
            style: EdgeStyle::Solid,
        },
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

impl ShapeKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Rectangle => "rectangle",
            Self::Ellipse => "ellipse",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub bounds: Rectangle,
    pub kind: ShapeKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShapePresentation {
    pub visible: bool,
    pub locked: bool,
    pub fill: [u8; 4],
    pub stroke: [u8; 4],
    pub opacity: u8,
}

impl ShapePresentation {
    pub const fn for_kind(kind: ShapeKind) -> Self {
        Self {
            visible: true,
            locked: false,
            fill: match kind {
                ShapeKind::Rectangle => [75, 145, 200, 255],
                ShapeKind::Ellipse => [229, 173, 82, 255],
            },
            stroke: [37, 52, 72, 255],
            opacity: 100,
        }
    }

    fn figure_style(self) -> Result<FigureStyle, EditPartError> {
        Ok(FigureStyle {
            foreground: Some(color(self.stroke)),
            background: Some(color(self.fill)),
            alpha: Some(
                Opacity::try_new(f64::from(self.opacity) / 100.0)
                    .map_err(|error| EditPartError::operation(error.to_string()))?,
            ),
            ..FigureStyle::default()
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShapeGroup {
    pub name: String,
    pub members: Vec<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapSettings {
    pub show_grid: bool,
    pub snap_to_grid: bool,
    pub grid_size: f64,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            show_grid: true,
            snap_to_grid: true,
            grid_size: DEFAULT_GRID_SIZE,
        }
    }
}

#[derive(Clone, Default)]
pub struct WorkspaceSettings(Rc<RefCell<SnapSettings>>);

impl WorkspaceSettings {
    pub fn get(&self) -> SnapSettings {
        *self.0.borrow()
    }

    pub fn update(&self, update: impl FnOnce(&mut SnapSettings)) {
        update(&mut self.0.borrow_mut());
    }
}

fn default_name(id: u64, kind: ShapeKind) -> String {
    format!(
        "{} #{id}",
        match kind {
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::Ellipse => "Ellipse",
        }
    )
}

fn color([r, g, b, a]: [u8; 4]) -> Color {
    Color::rgba8(r, g, b, a)
}

pub struct Diagram {
    pub shapes: BTreeMap<u64, Shape>,
    pub order: Vec<u64>,
    pub edges: BTreeMap<u64, Edge>,
    pub names: BTreeMap<u64, String>,
    pub presentations: BTreeMap<u64, ShapePresentation>,
    pub groups: BTreeMap<u64, ShapeGroup>,
    revision: ModelRevision,
    events: Vec<ModelEvent<u64, ()>>,
    next_id: u64,
}

impl Default for Diagram {
    fn default() -> Self {
        Self::showcase()
    }
}

impl Diagram {
    /// Opens the runnable editor with enough topology to exercise projection, anchors and routing.
    pub fn showcase() -> Self {
        let shapes = BTreeMap::from(SHOWCASE_SHAPES);
        Self {
            names: shapes
                .iter()
                .map(|(id, shape)| (*id, default_name(*id, shape.kind)))
                .collect(),
            presentations: shapes
                .iter()
                .map(|(id, shape)| (*id, ShapePresentation::for_kind(shape.kind)))
                .collect(),
            shapes,
            order: SHOWCASE_SHAPES.map(|(id, _)| id).to_vec(),
            edges: BTreeMap::from(SHOWCASE_EDGES),
            groups: BTreeMap::new(),
            revision: ModelRevision::initial(),
            events: Vec::new(),
            next_id: SHOWCASE_LAST_ID + 1,
        }
    }

    #[cfg(test)]
    pub(crate) fn minimal() -> Self {
        let shapes = BTreeMap::from([
            SHOWCASE_SHAPES[0],
            (
                2,
                Shape {
                    bounds: MINIMAL_SECOND_BOUNDS,
                    kind: ShapeKind::Ellipse,
                },
            ),
        ]);
        Self {
            names: shapes
                .iter()
                .map(|(id, shape)| (*id, default_name(*id, shape.kind)))
                .collect(),
            presentations: shapes
                .iter()
                .map(|(id, shape)| (*id, ShapePresentation::for_kind(shape.kind)))
                .collect(),
            shapes,
            order: vec![FIRST, 2],
            edges: BTreeMap::new(),
            groups: BTreeMap::new(),
            revision: ModelRevision::initial(),
            events: Vec::new(),
            next_id: 3,
        }
    }

    fn publish(&mut self, subject: u64) {
        self.revision = self.revision.next().expect("demo model revision exhausted");
        self.events
            .push(ModelEvent::new(self.revision, subject, ()));
    }

    pub fn accepts_edge(&self, source: u64, target: u64, excluding: Option<u64>) -> bool {
        source != target
            && self.shapes.contains_key(&source)
            && self.shapes.contains_key(&target)
            && !self
                .edges
                .iter()
                .any(|(id, e)| Some(*id) != excluding && e.source == source && e.target == target)
    }

    pub fn presentation(&self, id: u64) -> ShapePresentation {
        self.presentations
            .get(&id)
            .copied()
            .unwrap_or_else(|| ShapePresentation::for_kind(self.shapes[&id].kind))
    }

    pub fn is_locked(&self, id: u64) -> bool {
        self.presentations
            .get(&id)
            .is_some_and(|presentation| presentation.locked)
    }

    pub fn name(&self, id: u64) -> String {
        self.names.get(&id).cloned().unwrap_or_else(|| {
            self.shapes.get(&id).map_or_else(
                || format!("Item #{id}"),
                |shape| default_name(id, shape.kind),
            )
        })
    }

    pub const fn next_identity(&self) -> u64 {
        self.next_id
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
            self.order.clone()
        } else {
            Vec::new()
        })
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<u64, ()>> {
        std::mem::take(&mut self.events)
    }
    fn connections(&self) -> Result<Vec<ModelConnection<u64>>, Infallible> {
        Ok(self
            .edges
            .iter()
            .map(|(id, edge)| ModelConnection::new(*id, edge.source, edge.target))
            .collect())
    }
}

pub struct MoveShape {
    pub id: u64,
    pub before: Rectangle,
    pub after: Rectangle,
}
impl Command<Diagram> for MoveShape {
    fn label(&self) -> &str {
        "Move / resize shape"
    }
    fn can_execute(&self, model: &Diagram) -> bool {
        model.shapes.contains_key(&self.id) && !model.is_locked(self.id) && valid_bounds(self.after)
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

pub struct RenameShape {
    pub id: u64,
    pub before: String,
    pub after: String,
}

impl Command<Diagram> for RenameShape {
    fn label(&self) -> &str {
        "Rename shape"
    }

    fn can_execute(&self, model: &Diagram) -> bool {
        model.shapes.contains_key(&self.id)
            && !self.after.trim().is_empty()
            && self.before != self.after
    }

    fn execute(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.names.insert(self.id, self.after.trim().to_owned());
        model.publish(self.id);
        Ok(())
    }

    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.names.insert(self.id, self.before.clone());
        model.publish(self.id);
        Ok(())
    }
}

pub struct UpdateShapePresentation {
    pub id: u64,
    pub before: ShapePresentation,
    pub after: ShapePresentation,
}

impl Command<Diagram> for UpdateShapePresentation {
    fn label(&self) -> &str {
        "Change shape presentation"
    }

    fn can_execute(&self, model: &Diagram) -> bool {
        model.shapes.contains_key(&self.id)
            && self.before != self.after
            && self.after.opacity <= 100
    }

    fn execute(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.presentations.insert(self.id, self.after);
        model.publish(self.id);
        Ok(())
    }

    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.presentations.insert(self.id, self.before);
        model.publish(self.id);
        Ok(())
    }
}

pub struct ReorderShapes {
    pub before: Vec<u64>,
    pub after: Vec<u64>,
}

impl Command<Diagram> for ReorderShapes {
    fn label(&self) -> &str {
        "Reorder shapes"
    }

    fn can_execute(&self, model: &Diagram) -> bool {
        self.before == model.order
            && self.before != self.after
            && self.after.len() == model.shapes.len()
            && self.after.iter().all(|id| model.shapes.contains_key(id))
            && self
                .after
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.after.len()
    }

    fn execute(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.order.clone_from(&self.after);
        model.publish(ROOT);
        Ok(())
    }

    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.order.clone_from(&self.before);
        model.publish(ROOT);
        Ok(())
    }
}

pub struct ReplaceGroups {
    pub before: BTreeMap<u64, ShapeGroup>,
    pub after: BTreeMap<u64, ShapeGroup>,
}

impl Command<Diagram> for ReplaceGroups {
    fn label(&self) -> &str {
        "Change groups"
    }

    fn can_execute(&self, model: &Diagram) -> bool {
        self.before == model.groups
            && self.before != self.after
            && valid_groups(&self.after, &model.shapes)
    }

    fn execute(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.groups.clone_from(&self.after);
        if let Some(id) = model.groups.keys().next_back() {
            model.next_id = model.next_id.max(id.saturating_add(1));
        }
        model.publish(ROOT);
        Ok(())
    }

    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.groups.clone_from(&self.before);
        model.publish(ROOT);
        Ok(())
    }
}

fn valid_groups(groups: &BTreeMap<u64, ShapeGroup>, shapes: &BTreeMap<u64, Shape>) -> bool {
    let mut assigned = std::collections::BTreeSet::new();
    groups.iter().all(|(id, group)| {
        *id != ROOT
            && *id != u64::MAX
            && !group.name.trim().is_empty()
            && !group.members.is_empty()
            && group
                .members
                .iter()
                .all(|member| shapes.contains_key(member) && assigned.insert(*member))
    })
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
        model.order.push(self.id);
        model
            .names
            .insert(self.id, default_name(self.id, self.shape.kind));
        model
            .presentations
            .insert(self.id, ShapePresentation::for_kind(self.shape.kind));
        model.next_id = model.next_id.max(self.id + 1);
        model.publish(ROOT);
        Ok(())
    }
    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.remove(&self.id);
        model.order.retain(|id| *id != self.id);
        model.names.remove(&self.id);
        model.presentations.remove(&self.id);
        model.publish(ROOT);
        Ok(())
    }
}

struct RemoveShape {
    id: u64,
    shape: Shape,
    index: usize,
    edges: BTreeMap<u64, Edge>,
    name: Option<String>,
    presentation: Option<ShapePresentation>,
    groups_before: BTreeMap<u64, ShapeGroup>,
}
impl Command<Diagram> for RemoveShape {
    fn label(&self) -> &str {
        "Delete shape"
    }
    fn execute(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        self.index = model
            .order
            .iter()
            .position(|id| *id == self.id)
            .ok_or_else(|| CommandError::operation("shape no longer exists"))?;
        self.edges = model
            .edges
            .iter()
            .filter(|(_, e)| e.source == self.id || e.target == self.id)
            .map(|(id, e)| (*id, *e))
            .collect();
        model.edges.retain(|id, _| !self.edges.contains_key(id));
        model.shapes.remove(&self.id);
        model.order.remove(self.index);
        self.name = model.names.remove(&self.id);
        self.presentation = model.presentations.remove(&self.id);
        self.groups_before = model.groups.clone();
        model.groups.retain(|_, group| {
            group.members.retain(|member| *member != self.id);
            !group.members.is_empty()
        });
        model.publish(ROOT);
        Ok(())
    }
    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.insert(self.id, self.shape);
        model.order.insert(self.index, self.id);
        model.edges.extend(self.edges.clone());
        if let Some(name) = &self.name {
            model.names.insert(self.id, name.clone());
        }
        if let Some(presentation) = self.presentation {
            model.presentations.insert(self.id, presentation);
        }
        model.groups.clone_from(&self.groups_before);
        model.publish(ROOT);
        Ok(())
    }
}

fn shape_figure(shape: Shape, presentation: ShapePresentation) -> Box<dyn Figure> {
    let fill = color(presentation.fill);
    let stroke = color(presentation.stroke);
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

pub(crate) fn selection_handle(point: Point) -> RectangleFigure {
    RectangleFigure::new_with_color(
        point.x() - HANDLE_SIZE / 2.0,
        point.y() - HANDLE_SIZE / 2.0,
        HANDLE_SIZE,
        HANDLE_SIZE,
        Color::WHITE,
    )
    .with_stroke(Color::from_hex(SELECTION_BLUE).unwrap(), 1.0)
}

pub(crate) fn resize_handle_sites(bounds: Rectangle) -> [(Point, ResizeDirection); 8] {
    let center_x = bounds.x + bounds.width / 2.0;
    let center_y = bounds.y + bounds.height / 2.0;
    [
        (Point::new(bounds.x, bounds.y), ResizeDirection::NorthWest),
        (Point::new(center_x, bounds.y), ResizeDirection::North),
        (
            Point::new(bounds.x + bounds.width, bounds.y),
            ResizeDirection::NorthEast,
        ),
        (
            Point::new(bounds.x + bounds.width, center_y),
            ResizeDirection::East,
        ),
        (
            Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
            ResizeDirection::SouthEast,
        ),
        (
            Point::new(center_x, bounds.y + bounds.height),
            ResizeDirection::South,
        ),
        (
            Point::new(bounds.x, bounds.y + bounds.height),
            ResizeDirection::SouthWest,
        ),
        (Point::new(bounds.x, center_y), ResizeDirection::West),
    ]
}

fn shape_change_feedback(shape: Shape) -> Vec<FeedbackVisual> {
    let bounds = shape.bounds;
    let stroke = Color::from_hex(SELECTION_BLUE).unwrap();
    let outline: Box<dyn Figure> = match shape.kind {
        ShapeKind::Rectangle => Box::new(
            RectangleFigure::new_with_color(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                Color::TRANSPARENT,
            )
            .with_stroke(stroke, OUTLINE_WIDTH),
        ),
        ShapeKind::Ellipse => Box::new(
            EllipseFigure::new_with_color(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                Color::TRANSPARENT,
            )
            .with_stroke(stroke, OUTLINE_WIDTH),
        ),
    };
    let mut feedback = vec![FeedbackVisual::scaled(outline)];
    feedback.extend(
        resize_handle_sites(bounds)
            .into_iter()
            .map(|(point, _)| FeedbackVisual::scaled(Box::new(selection_handle(point)))),
    );
    feedback
}

fn snap_value(value: f64, settings: SnapSettings) -> f64 {
    if settings.snap_to_grid && settings.grid_size.is_finite() && settings.grid_size > 0.0 {
        (value / settings.grid_size).round() * settings.grid_size
    } else {
        value
    }
}

fn snap_changed_bounds(
    before: Rectangle,
    transformed: Rectangle,
    request: &ChangeBoundsRequest,
    settings: SnapSettings,
) -> Rectangle {
    if !settings.snap_to_grid {
        return transformed;
    }
    match request.kind() {
        ChangeBoundsKind::Move => Rectangle::new(
            before.x + snap_value(request.move_delta().x(), settings),
            before.y + snap_value(request.move_delta().y(), settings),
            before.width,
            before.height,
        ),
        ChangeBoundsKind::Resize(direction) => {
            let mut left = transformed.x;
            let mut top = transformed.y;
            let mut right = transformed.x + transformed.width;
            let mut bottom = transformed.y + transformed.height;
            if matches!(
                direction,
                ResizeDirection::West | ResizeDirection::NorthWest | ResizeDirection::SouthWest
            ) {
                left = snap_value(left, settings);
            }
            if matches!(
                direction,
                ResizeDirection::East | ResizeDirection::NorthEast | ResizeDirection::SouthEast
            ) {
                right = snap_value(right, settings);
            }
            if matches!(
                direction,
                ResizeDirection::North | ResizeDirection::NorthEast | ResizeDirection::NorthWest
            ) {
                top = snap_value(top, settings);
            }
            if matches!(
                direction,
                ResizeDirection::South | ResizeDirection::SouthEast | ResizeDirection::SouthWest
            ) {
                bottom = snap_value(bottom, settings);
            }
            Rectangle::new(left, top, right - left, bottom - top)
        }
    }
}

struct ShapePolicy {
    workspace: WorkspaceSettings,
}
impl EditPolicy<Diagram> for ShapePolicy {
    fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Diagram, Self>) {
        builder.handle::<ChangeBounds>(Self::change);
        builder.handle::<AlignShapes>(Self::align);
        builder.handle::<DeleteParts>(Self::delete);
    }
}
impl ShapePolicy {
    fn align(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        request: &AlignmentRequest,
        model: &Diagram,
    ) -> Result<Option<EditingContribution<Diagram>>, PolicyError> {
        if model.is_locked(host.model()) {
            return Ok(None);
        }
        let before = model.shapes[&host.model()].bounds;
        Ok(Some(EditingContribution::command(Box::new(MoveShape {
            id: host.model(),
            before,
            after: request.alignment.bounds(before, request.reference),
        }))))
    }

    fn change(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        req: &ChangeBoundsRequest,
        model: &Diagram,
    ) -> Result<Option<EditingContribution<Diagram>>, PolicyError> {
        if model.is_locked(host.model()) {
            return Ok(None);
        }
        let shape = model.shapes[&host.model()];
        let after = snap_changed_bounds(
            shape.bounds,
            req.transformed_bounds(shape.bounds),
            req,
            self.workspace.get(),
        );
        Ok(Some(EditingContribution::new(
            Some(Box::new(MoveShape {
                id: host.model(),
                before: shape.bounds,
                after,
            })),
            shape_change_feedback(Shape {
                bounds: after,
                ..shape
            }),
        )))
    }
    fn delete(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        _: &DeleteRequest,
        model: &Diagram,
    ) -> Result<Option<EditingContribution<Diagram>>, PolicyError> {
        if model.is_locked(host.model()) {
            return Ok(None);
        }
        Ok(Some(EditingContribution::command(Box::new(RemoveShape {
            id: host.model(),
            shape: model.shapes[&host.model()],
            index: 0,
            edges: BTreeMap::new(),
            name: None,
            presentation: None,
            groups_before: BTreeMap::new(),
        }))))
    }
}

struct CanvasPolicy {
    workspace: WorkspaceSettings,
}
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
        if !valid_bounds(req.bounds()) || model.next_id == u64::MAX {
            return Ok(None);
        }
        let settings = self.workspace.get();
        let requested = req.bounds();
        let shape = Shape {
            bounds: Rectangle::new(
                snap_value(requested.x, settings),
                snap_value(requested.y, settings),
                requested.width,
                requested.height,
            ),
            kind,
        };
        Ok(Some(EditingContribution::new(
            Some(Box::new(InsertShape {
                id: model.next_id,
                shape,
            })),
            vec![FeedbackVisual::scaled(shape_figure(
                shape,
                ShapePresentation::for_kind(kind),
            ))],
        )))
    }
}

pub struct ShapePart {
    workspace: WorkspaceSettings,
}
impl EditPartBehavior<Diagram> for ShapePart {
    fn create_figure(
        &mut self,
        model: &Diagram,
        id: u64,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(if id == ROOT {
            Box::new(FreeformLayerFigure::new(0.0, 0.0, 640.0, 480.0))
        } else {
            shape_figure(model.shapes[&id], model.presentation(id))
        })
    }
    fn create_policies(
        &mut self,
        _: &Diagram,
        id: u64,
    ) -> Result<Vec<PolicyInstallation<Diagram>>, EditPartError> {
        let policy = if id == ROOT {
            PolicyInstallation::new(
                LayoutSlot::key(),
                CanvasPolicy {
                    workspace: self.workspace.clone(),
                },
            )
        } else {
            PolicyInstallation::new(
                ComponentSlot::key(),
                ShapePolicy {
                    workspace: self.workspace.clone(),
                },
            )
        };
        let mut policies =
            vec![policy.map_err(|error| EditPartError::operation(error.to_string()))?];
        if id != ROOT {
            policies.push(
                PolicyInstallation::new(ConnectionCreationSlot::key(), NodeConnectionPolicy)
                    .map_err(|e| EditPartError::operation(e.to_string()))?,
            );
        }
        Ok(policies)
    }
    fn refresh_visuals(
        &mut self,
        model: &Diagram,
        id: u64,
        ctx: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        if id != ROOT {
            let presentation = model.presentation(id);
            ctx.set_primary_bounds(model.shapes[&id].bounds)?;
            ctx.set_primary_style(presentation.figure_style()?)?;
            ctx.set_primary_visible(presentation.visible)?;
            ctx.set_primary_enabled(!presentation.locked)?;
        }
        Ok(())
    }
    fn source_connection_anchor(
        &mut self,
        model: &Diagram,
        id: u64,
        ctx: ConnectionAnchorContext<u64>,
    ) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
        anchor(model, id, ctx)
    }
    fn target_connection_anchor(
        &mut self,
        model: &Diagram,
        id: u64,
        ctx: ConnectionAnchorContext<u64>,
    ) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
        anchor(model, id, ctx)
    }
}

pub struct ShapeFactory {
    workspace: WorkspaceSettings,
}

impl ShapeFactory {
    pub fn new(workspace: WorkspaceSettings) -> Self {
        Self { workspace }
    }
}

impl EditPartFactory<Diagram> for ShapeFactory {
    fn create(
        &mut self,
        _: PartFactoryContext<u64>,
        _: &Diagram,
    ) -> Result<Box<dyn EditPartBehavior<Diagram>>, EditPartError> {
        Ok(Box::new(ShapePart {
            workspace: self.workspace.clone(),
        }))
    }
    fn create_connection(
        &mut self,
        _: ConnectionPartFactoryContext<u64>,
        _: &Diagram,
    ) -> Result<Box<dyn EditPartBehavior<Diagram>>, EditPartError> {
        Ok(Box::new(EdgePart))
    }
}

fn valid_bounds(b: Rectangle) -> bool {
    [b.x, b.y, b.width, b.height]
        .into_iter()
        .all(f64::is_finite)
        && b.width >= MIN_SIZE
        && b.height >= MIN_SIZE
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeStyle {
    Solid,
    Dashed,
}
impl EdgeStyle {
    pub fn name(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Dashed => "dashed",
        }
    }
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "solid" => Some(Self::Solid),
            "dashed" => Some(Self::Dashed),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub source: u64,
    pub target: u64,
    pub style: EdgeStyle,
}

#[derive(Clone, Copy)]
pub struct ShapeTemplate(pub ShapeKind);
impl CreationTemplate<Diagram> for ShapeTemplate {
    type Request = CreateChild;
    fn default_size(&self) -> Dimension {
        Dimension::new(DEFAULT_WIDTH, DEFAULT_HEIGHT)
    }
    fn request(
        &self,
        contents: EditPartId,
        bounds: Rectangle,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> (RequestMetadata, CreateRequest) {
        (
            RequestMetadata::new(revision, vec![contents]),
            CreateRequest::new(
                contents,
                CreationType::new(self.0.name()).unwrap(),
                bounds,
                modifiers,
                revision,
            ),
        )
    }
}

fn anchor(
    model: &Diagram,
    id: u64,
    ctx: ConnectionAnchorContext<u64>,
) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
    let Some(shape) = model.shapes.get(&id) else {
        return Ok(None);
    };
    let owner = ctx.endpoint_figure();
    let (name, anchor): (_, Box<dyn novadraw::connection::ConnectionAnchor>) = match shape.kind {
        ShapeKind::Rectangle => ("chopbox", Box::new(ChopboxAnchor::new(owner))),
        ShapeKind::Ellipse => ("ellipse", Box::new(EllipseAnchor::new(owner))),
    };
    let key = AnchorSemanticKey::new(Some(owner), name, vec![])
        .map_err(|e| EditPartError::operation(e.to_string()))?;
    Ok(Some(ConnectionAnchorDescriptor::new(key, anchor)))
}

struct InsertEdge {
    id: u64,
    edge: Edge,
}
impl Command<Diagram> for InsertEdge {
    fn label(&self) -> &str {
        "Create connection"
    }
    fn can_execute(&self, m: &Diagram) -> bool {
        !m.edges.contains_key(&self.id)
            && self.id != u64::MAX
            && m.accepts_edge(self.edge.source, self.edge.target, None)
    }
    fn execute(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        m.edges.insert(self.id, self.edge);
        m.next_id = m.next_id.max(self.id + 1);
        m.publish(ROOT);
        Ok(())
    }
    fn undo(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        m.edges.remove(&self.id);
        m.publish(ROOT);
        Ok(())
    }
}

struct DeleteEdge {
    id: u64,
    removed: Option<Edge>,
}
impl Command<Diagram> for DeleteEdge {
    fn label(&self) -> &str {
        "Delete connection"
    }
    fn execute(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        // An earlier node deletion in the same compound may have removed this edge.
        self.removed = m.edges.remove(&self.id);
        m.publish(ROOT);
        Ok(())
    }
    fn undo(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        if let Some(edge) = self.removed {
            m.edges.insert(self.id, edge);
        }
        m.publish(ROOT);
        Ok(())
    }
}

struct ReconnectEdge {
    id: u64,
    before: Edge,
    after: Edge,
}
impl Command<Diagram> for ReconnectEdge {
    fn label(&self) -> &str {
        "Reconnect connection"
    }
    fn can_execute(&self, m: &Diagram) -> bool {
        m.edges.contains_key(&self.id)
            && m.accepts_edge(self.after.source, self.after.target, Some(self.id))
    }
    fn execute(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        m.edges.insert(self.id, self.after);
        m.publish(self.id);
        Ok(())
    }
    fn undo(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        m.edges.insert(self.id, self.before);
        m.publish(self.id);
        Ok(())
    }
}

struct EdgeCreation {
    source: u64,
    style: EdgeStyle,
}
impl ConnectionCreation<Diagram> for EdgeCreation {
    fn can_complete(
        &self,
        source: PolicyHost<u64>,
        target: PolicyHost<u64>,
        _: &CreateConnectionRequest,
        m: &Diagram,
    ) -> Result<bool, PolicyError> {
        Ok(source.model() == self.source
            && m.next_id != u64::MAX
            && m.accepts_edge(source.model(), target.model(), None))
    }
    fn feedback_with_route(
        &mut self,
        _: PolicyHost<u64>,
        _: Option<PolicyHost<u64>>,
        _: &CreateConnectionRequest,
        route: Option<ConnectionFeedbackRoute>,
        _: &Diagram,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        edge_preview(route)
    }
    fn command(
        &mut self,
        source: PolicyHost<u64>,
        target: PolicyHost<u64>,
        _: &CreateConnectionRequest,
        m: &Diagram,
    ) -> Result<Box<dyn Command<Diagram>>, PolicyError> {
        Ok(Box::new(InsertEdge {
            id: m.next_id,
            edge: Edge {
                source: source.model(),
                target: target.model(),
                style: self.style,
            },
        }))
    }
}
fn edge_preview(
    route: Option<ConnectionFeedbackRoute>,
) -> Result<Vec<FeedbackVisual>, PolicyError> {
    let Some(route) = route else {
        return Ok(Vec::new());
    };
    Ok(vec![FeedbackVisual::scaled(Box::new(
        novadraw::figure::PolylineFigure::new(
            route.source().x(),
            route.source().y(),
            route.target().x(),
            route.target().y(),
        ),
    ))])
}
struct NodeConnectionPolicy;
impl EditPolicy<Diagram> for NodeConnectionPolicy {
    fn register_handlers(b: &mut PolicyHandlerBuilder<'_, Diagram, Self>) {
        b.handle::<CreateConnection>(Self::start);
    }
}
impl NodeConnectionPolicy {
    fn start(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        req: &CreateConnectionRequest,
        _: &Diagram,
    ) -> Result<Option<<CreateConnection as RequestProtocol<Diagram>>::Contribution>, PolicyError>
    {
        Ok(
            EdgeStyle::from_name(req.connection_type().as_str()).map(|style| {
                Box::new(EdgeCreation {
                    source: host.model(),
                    style,
                }) as Box<dyn ConnectionCreation<Diagram>>
            }),
        )
    }
}

struct EdgeReconnection {
    id: u64,
    before: Edge,
}
impl EdgeReconnection {
    fn after(&self, fixed: u64, candidate: u64, endpoint: ConnectionEndpoint) -> Edge {
        match endpoint {
            ConnectionEndpoint::Source => Edge {
                source: candidate,
                target: fixed,
                ..self.before
            },
            ConnectionEndpoint::Target => Edge {
                source: fixed,
                target: candidate,
                ..self.before
            },
        }
    }
}
impl ConnectionReconnection<Diagram> for EdgeReconnection {
    fn can_complete(
        &self,
        _: PolicyHost<u64>,
        fixed: PolicyHost<u64>,
        candidate: PolicyHost<u64>,
        req: &ReconnectConnectionRequest,
        m: &Diagram,
    ) -> Result<bool, PolicyError> {
        let edge = self.after(fixed.model(), candidate.model(), req.endpoint());
        Ok(m.accepts_edge(edge.source, edge.target, Some(self.id)))
    }
    fn feedback_with_route(
        &mut self,
        _: PolicyHost<u64>,
        _: PolicyHost<u64>,
        _: Option<PolicyHost<u64>>,
        _: &ReconnectConnectionRequest,
        route: Option<ConnectionFeedbackRoute>,
        _: &Diagram,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        edge_preview(route)
    }
    fn command(
        &mut self,
        _: PolicyHost<u64>,
        fixed: PolicyHost<u64>,
        candidate: PolicyHost<u64>,
        req: &ReconnectConnectionRequest,
        _: &Diagram,
    ) -> Result<Box<dyn Command<Diagram>>, PolicyError> {
        Ok(Box::new(ReconnectEdge {
            id: self.id,
            before: self.before,
            after: self.after(fixed.model(), candidate.model(), req.endpoint()),
        }))
    }
}
struct EdgePolicy;
impl EditPolicy<Diagram> for EdgePolicy {
    fn register_handlers(b: &mut PolicyHandlerBuilder<'_, Diagram, Self>) {
        b.handle::<DeleteParts>(Self::delete);
        b.handle::<ReconnectConnection>(Self::reconnect);
    }
}
impl EdgePolicy {
    fn delete(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        _: &DeleteRequest,
        _: &Diagram,
    ) -> Result<Option<EditingContribution<Diagram>>, PolicyError> {
        Ok(Some(EditingContribution::command(Box::new(DeleteEdge {
            id: host.model(),
            removed: None,
        }))))
    }
    fn reconnect(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        _: &ReconnectConnectionRequest,
        m: &Diagram,
    ) -> Result<Option<<ReconnectConnection as RequestProtocol<Diagram>>::Contribution>, PolicyError>
    {
        Ok(m.edges.get(&host.model()).map(|edge| {
            Box::new(EdgeReconnection {
                id: host.model(),
                before: *edge,
            }) as Box<dyn ConnectionReconnection<Diagram>>
        }))
    }
}

struct EdgePart;

fn target_arrow() -> Result<PolygonDecorationFigure, novadraw::connection::DecorationError> {
    PolygonDecorationFigure::from_template(PointList::from_points(vec![
        Point::ZERO,
        Point::new(-1.0, 1.0),
        Point::new(-1.0, -1.0),
    ]))?
    .with_scale(ARROW_LENGTH, ARROW_HALF_WIDTH)
    .map(|arrow| arrow.with_fill_color(Color::BLACK))
}

impl EditPartBehavior<Diagram> for EdgePart {
    fn create_figure(&mut self, m: &Diagram, id: u64) -> Result<Box<dyn Figure>, EditPartError> {
        let dash = match m.edges[&id].style {
            EdgeStyle::Solid => DashPattern::Solid,
            EdgeStyle::Dashed => DashPattern::Dash,
        };
        let stroke = StrokeStyle::default()
            .with_width(OUTLINE_WIDTH)
            .map_err(|e| EditPartError::operation(e.to_string()))?
            .with_dash_pattern(dash);
        Ok(Box::new(
            ConnectionFigure::new()
                .with_stroke(Color::BLACK, OUTLINE_WIDTH)
                .with_stroke_style(stroke),
        ))
    }
    fn configure_visual(
        &mut self,
        _: &Diagram,
        _: u64,
        ctx: &mut VisualBuildContext<'_>,
    ) -> Result<(), EditPartError> {
        let arrow = target_arrow().map_err(|e| EditPartError::operation(e.to_string()))?;
        ctx.add_connection_child(Box::new(arrow), Box::new(ConnectionLocator::Target))?;
        Ok(())
    }
    fn create_policies(
        &mut self,
        _: &Diagram,
        _: u64,
    ) -> Result<Vec<PolicyInstallation<Diagram>>, EditPartError> {
        // One open policy handles both families in one slot; duplicate registration would dispatch twice.
        Ok(vec![
            PolicyInstallation::new(ConnectionReconnectSlot::key(), EdgePolicy)
                .map_err(|e| EditPartError::operation(e.to_string()))?,
        ])
    }

    fn refresh_visuals(
        &mut self,
        model: &Diagram,
        id: u64,
        ctx: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        let edge = model.edges[&id];
        ctx.set_primary_visible(
            model.presentation(edge.source).visible && model.presentation(edge.target).visible,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::connection::{ConnectionDecorationBehavior, LocatorPlacement};

    #[test]
    fn target_arrow_matches_the_official_shapes_default_span() {
        let arrow = target_arrow().unwrap();
        let geometry = arrow
            .prepare_decoration_geometry(LocatorPlacement {
                point: Point::new(100.0, 100.0),
                reference: Point::new(90.0, 100.0),
            })
            .unwrap();
        let (min_x, max_x, min_y, max_y) = geometry.local_points().iter().fold(
            (
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
            ),
            |(min_x, max_x, min_y, max_y), point| {
                (
                    min_x.min(point.x()),
                    max_x.max(point.x()),
                    min_y.min(point.y()),
                    max_y.max(point.y()),
                )
            },
        );
        assert!((max_x - min_x - ARROW_LENGTH).abs() < f64::EPSILON);
        assert!((max_y - min_y - ARROW_HALF_WIDTH * 2.0).abs() < f64::EPSILON);
    }
}

/// Persisted document contains business identities only, never runtime handles.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    version: u32,
    #[serde(deserialize_with = "unique_ids")]
    shapes: BTreeMap<u64, Shape>,
    order: Vec<u64>,
    #[serde(deserialize_with = "unique_ids")]
    edges: BTreeMap<u64, Edge>,
    #[serde(default, deserialize_with = "unique_ids")]
    names: BTreeMap<u64, String>,
    #[serde(default, deserialize_with = "unique_ids")]
    presentations: BTreeMap<u64, ShapePresentation>,
    #[serde(default, deserialize_with = "unique_ids")]
    groups: BTreeMap<u64, ShapeGroup>,
}

fn unique_ids<'de, D, T>(deserializer: D) -> Result<BTreeMap<u64, T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Unique<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Unique<T> {
        type Value = BTreeMap<u64, T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an object with unique numeric identities")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut values = BTreeMap::new();
            while let Some((id, value)) = map.next_entry()? {
                if values.insert(id, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate model identity"));
                }
            }
            Ok(values)
        }
    }
    deserializer.deserialize_map(Unique(std::marker::PhantomData))
}
const DOCUMENT_VERSION: u32 = 2;
impl Diagram {
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&Document {
            version: DOCUMENT_VERSION,
            shapes: self.shapes.clone(),
            order: self.order.clone(),
            edges: self.edges.clone(),
            names: self.names.clone(),
            presentations: self.presentations.clone(),
            groups: self.groups.clone(),
        })
        .map_err(|e| e.to_string())
    }
    pub fn from_json(json: &str) -> Result<Self, String> {
        let doc: Document = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if !(1..=DOCUMENT_VERSION).contains(&doc.version) {
            return Err("unsupported Shapes document version".into());
        }
        let ids = doc
            .order
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if ids.len() != doc.order.len()
            || ids.len() != doc.shapes.len()
            || !doc.shapes.keys().all(|id| ids.contains(id) && *id != ROOT)
            || doc.shapes.values().any(|s| !valid_bounds(s.bounds))
            || doc
                .edges
                .keys()
                .any(|id| *id == ROOT || doc.shapes.contains_key(id))
            || doc
                .names
                .iter()
                .any(|(id, name)| !doc.shapes.contains_key(id) || name.trim().is_empty())
            || doc.presentations.iter().any(|(id, presentation)| {
                !doc.shapes.contains_key(id) || presentation.opacity > 100
            })
            || doc
                .groups
                .keys()
                .any(|id| *id == ROOT || doc.shapes.contains_key(id) || doc.edges.contains_key(id))
            || !valid_groups(&doc.groups, &doc.shapes)
        {
            return Err("invalid shape identity, order or geometry".into());
        }
        let names = doc
            .shapes
            .iter()
            .map(|(id, shape)| {
                (
                    *id,
                    doc.names
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| default_name(*id, shape.kind)),
                )
            })
            .collect();
        let presentations = doc
            .shapes
            .iter()
            .map(|(id, shape)| {
                (
                    *id,
                    doc.presentations
                        .get(id)
                        .copied()
                        .unwrap_or_else(|| ShapePresentation::for_kind(shape.kind)),
                )
            })
            .collect();
        let next_id = doc
            .shapes
            .keys()
            .chain(doc.edges.keys())
            .chain(doc.groups.keys())
            .max()
            .copied()
            .unwrap_or(ROOT)
            .checked_add(1)
            .ok_or("document identity exhausted")?;
        let model = Self {
            shapes: doc.shapes,
            order: doc.order,
            edges: doc.edges,
            names,
            presentations,
            groups: doc.groups,
            revision: ModelRevision::initial(),
            events: Vec::new(),
            next_id,
        };
        if model
            .edges
            .iter()
            .any(|(id, e)| !model.accepts_edge(e.source, e.target, Some(*id)))
        {
            return Err("invalid connection endpoints or duplicate edge".into());
        }
        Ok(model)
    }
}
