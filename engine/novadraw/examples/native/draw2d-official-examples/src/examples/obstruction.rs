use super::runtime_with_fonts;
use novadraw::{
    connection::{
        AnchorSite, ConnectionAnchor, ConnectionFigure, ConnectionRouter, CoordinateSpace,
        RouteError, RouteGroupInput, RouteGroupMember, RouteGroupOutput, RouteOutput, RouteRequest,
        RouterBinding, RouterId, RoutingGroupScope, RoutingObstacle,
        VisibilityGraphConnectionRouter,
    },
    event::{EventContext, FigureEventHandler, MouseButton, MouseEvent},
    figure::{
        CONTAINER, ContainerCapability, Figure, FigureCapabilityBuilder,
        FigureCapabilityRegistrationError, FigureContainer, FigureDrawing, FigureLifecycle,
        FigureLifecycleContext, FigureMeasurement, FigurePreparation, FigurePresentation, INPUT,
        InputCapability, LIFECYCLE, LifecycleCapability, PREPARATION, PreparationCapability,
    },
    geometry::{Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    runtime::{
        FigureComponentContext, FigureComponentUpdate, FigureUpdateEffect,
        FigureUpdateEffectContext, PreparedFigureUpdate, Runtime, RuntimeMutationError,
    },
    text::{MeasureContext, TextConstraints, TextError, TextLayout},
    tree::{FigureId, FigureSubtree, SubtreeBindingNodes, SubtreeConnection},
};
use std::sync::{Arc, Mutex};

const WIDTH: f64 = 900.0;
const HEIGHT: f64 = 700.0;
const ENDPOINT_SIZE: f64 = 20.0;
const ADD_THRESHOLD: f64 = 10.0;
const LINE_WIDTH: f64 = 1.0;
const GREEN: Color = Color::rgba(0.0, 1.0, 0.0, 1.0);
const BLUE: Color = Color::rgba(0.0, 0.0, 1.0, 1.0);
const DARK_BLUE: Color = Color::rgba(0.0, 0.0, 0.5, 1.0);
const TEXT_OFFSET_X: f64 = 22.0;
const TEXT_OFFSET_Y: f64 = 15.0;
const TEXT_LINE_STEP: f64 = 12.0;

#[derive(Default)]
struct State {
    obstacles: Vec<FigureId>,
    pairs: Vec<[FigureId; 3]>,
    pending: Option<FigureId>,
    show_segments: bool,
}
type Shared = Arc<Mutex<State>>;

// An external consumer of the same group interface as the built-in solver.
// The mutable list contains scene identities only, never unpublished route outputs.
struct SceneRouter {
    state: Shared,
}
impl ConnectionRouter for SceneRouter {
    fn route(&self, _: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        Err(RouteError::UnsupportedRoutingGroup)
    }
    fn prepares_group_endpoints(&self) -> bool {
        true
    }
    fn routing_group_scope(&self) -> RoutingGroupScope {
        RoutingGroupScope::RoutingDomain
    }
    fn solve_group(
        &self,
        mut input: RouteGroupInput<'_>,
    ) -> Result<Option<RouteGroupOutput>, RouteError> {
        for member in input.members() {
            if let Some(constraint) = member.constraint() {
                return Err(RouteError::ConstraintTypeMismatch {
                    expected: None,
                    actual: constraint.type_name(),
                });
            }
        }
        let ids = self
            .state
            .lock()
            .expect("obstruction state")
            .obstacles
            .clone();
        let space = input.routing_space();
        let members = input
            .members()
            .iter()
            .map(|member| RouteGroupMember::new(member.connection(), *member.metadata(), None))
            .collect::<Vec<_>>();
        let mut obstacles = Vec::new();
        for figure in ids {
            if !input
                .scene()
                .is_attached(figure)
                .map_err(RouteError::Scene)?
                || !input
                    .scene()
                    .is_visible(figure)
                    .map_err(RouteError::Scene)?
            {
                continue;
            }
            let bounds = input
                .scene()
                .border_box(figure)
                .map_err(RouteError::Scene)?;
            let bounds = input
                .scene()
                .map_rect(bounds, CoordinateSpace::FigureLocal(figure), space)
                .map_err(RouteError::Scene)?;
            obstacles.push(RoutingObstacle { figure, bounds });
        }
        VisibilityGraphConnectionRouter::new([]).solve_group(RouteGroupInput::new(
            &members,
            &obstacles,
            space,
            input.scene(),
        ))
    }
}

struct CenterAnchor {
    owner: FigureId,
}
impl ConnectionAnchor for CenterAnchor {
    fn owner(&self) -> Option<FigureId> {
        Some(self.owner)
    }
    fn reference_point(
        &self,
        scene: &mut dyn novadraw::connection::RouteSceneQuery,
        space: CoordinateSpace,
    ) -> Result<Point, novadraw::connection::AnchorError> {
        let bounds = scene.border_box(self.owner)?;
        Ok(scene.map_point(
            bounds.center(),
            CoordinateSpace::FigureLocal(self.owner),
            space,
        )?)
    }
    fn location(
        &self,
        scene: &mut dyn novadraw::connection::RouteSceneQuery,
        _: Point,
        _: CoordinateSpace,
        space: CoordinateSpace,
    ) -> Result<AnchorSite, novadraw::connection::AnchorError> {
        Ok(AnchorSite {
            point: self.reference_point(scene, space)?,
            outward_normal: None,
        })
    }
}

#[derive(Clone, Copy)]
struct Drag {
    start: Point,
    bounds: Rectangle,
}
struct Item {
    bounds: Rectangle,
    endpoint: bool,
    state: Shared,
    drag: Mutex<Option<Drag>>,
}
impl Item {
    fn new(bounds: Rectangle, endpoint: bool, state: Shared) -> Self {
        Self {
            bounds,
            endpoint,
            state,
            drag: Mutex::new(None),
        }
    }
}
impl Figure for Item {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn name(&self) -> &'static str {
        if self.endpoint {
            "ObstructionEndpoint"
        } else {
            "ObstructionObstacle"
        }
    }
    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        if !self.endpoint {
            return x >= 0.0 && y >= 0.0 && x < bounds.width && y < bounds.height;
        }
        let dx = (x - bounds.width / 2.0) / (bounds.width / 2.0);
        let dy = (y - bounds.height / 2.0) / (bounds.height / 2.0);
        dx * dx + dy * dy <= 1.0
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())?;
        if !self.endpoint {
            out.register(PREPARATION, PreparationCapability::of::<Self>())?;
        }
        out.register(LIFECYCLE, LifecycleCapability::of::<Self>())
    }
    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        let local = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        context.set_fill_paint(if self.endpoint { DARK_BLUE } else { GREEN });
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(LINE_WIDTH)?;
        if self.endpoint {
            context.fill_ellipse(local)?;
            context.stroke_ellipse(local)
        } else {
            context.fill_rect(local)?;
            context.stroke_rect(local)
        }
    }
}
struct ItemDrawing {
    bounds: Rectangle,
    labels: [TextLayout; 2],
}
impl FigureDrawing for ItemDrawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        context.set_fill_paint(GREEN);
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(LINE_WIDTH)?;
        context.fill_rect(self.bounds)?;
        context.stroke_rect(self.bounds)?;
        context.set_fill_paint(Color::BLACK);
        let center = self.bounds.center();
        for (index, label) in self.labels.iter().enumerate() {
            context.fill_text(
                label,
                Point::new(
                    center.x() - TEXT_OFFSET_X,
                    center.y() - TEXT_OFFSET_Y + index as f64 * TEXT_LINE_STEP,
                ),
            )?;
        }
        Ok(())
    }
}
impl FigurePreparation for Item {
    fn prepare(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        let labels = [
            context.layout_text(
                &format!("y:{}h:{}", self.bounds.y, self.bounds.height),
                TextConstraints::UNBOUNDED,
            )?,
            context.layout_text(
                &format!("x:{}w:{}", self.bounds.x, self.bounds.width),
                TextConstraints::UNBOUNDED,
            )?,
        ];
        FigurePresentation::new(
            FigureMeasurement::new(bounds.width, bounds.height, None),
            novadraw::geometry::Dimension::new(bounds.width, bounds.height),
            bounds,
            Arc::new(ItemDrawing { bounds, labels }),
        )
    }
}
struct MoveItem(Rectangle);
impl FigureUpdateEffect for MoveItem {
    fn prepare(
        self: Box<Self>,
        context: &mut FigureUpdateEffectContext<'_>,
    ) -> Result<(), RuntimeMutationError> {
        context.set_bounds(self.0)
    }
}
impl FigureComponentUpdate for MoveItem {
    type Figure = Item;
    type Prepared = Rectangle;
    type Error = std::convert::Infallible;
    fn prepare(
        self,
        _: &Item,
        _: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Rectangle>, Self::Error> {
        Ok(PreparedFigureUpdate::geometry_and_paint(self.0).with_effect(MoveItem(self.0)))
    }
    fn commit(bounds: Rectangle, target: &mut Item) {
        target.bounds = bounds;
    }
}
impl FigureLifecycle for Item {
    fn on_attached(&mut self, context: FigureLifecycleContext) {
        if !self.endpoint {
            self.state
                .lock()
                .expect("obstruction state")
                .obstacles
                .push(context.figure_id);
        }
    }
    fn on_detached(&mut self, context: FigureLifecycleContext) {
        let mut state = self.state.lock().expect("obstruction state");
        state
            .obstacles
            .retain(|figure| *figure != context.figure_id);
        state
            .pairs
            .retain(|pair| !pair.contains(&context.figure_id));
        if state.pending == Some(context.figure_id) {
            state.pending = None;
        }
    }
}
impl FigureEventHandler for Item {
    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if let Ok(bounds) = context.scene().figure_bounds(context.target_id()) {
            *self.drag.lock().expect("drag") = Some(Drag {
                start: event.entry_point(),
                bounds,
            });
        }
        true
    }
    fn on_mouse_dragged(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if let Some(drag) = *self.drag.lock().expect("drag") {
            let point = event.entry_point();
            context
                .effects()
                .update_component_later(MoveItem(Rectangle::new(
                    drag.bounds.x + point.x() - drag.start.x(),
                    drag.bounds.y + point.y() - drag.start.y(),
                    drag.bounds.width,
                    drag.bounds.height,
                )));
        }
        true
    }
    fn on_mouse_released(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        self.drag.lock().expect("drag").take();
        if (self.endpoint && event.button == MouseButton::Middle)
            || (!self.endpoint && event.button == MouseButton::Right)
        {
            let figure = context.target_id();
            if let Ok(Some(parent)) = context.scene().parent_id(figure) {
                let pair = self
                    .state
                    .lock()
                    .expect("obstruction state")
                    .pairs
                    .iter()
                    .find(|pair| pair.contains(&figure))
                    .copied();
                if let Some(pair) = pair {
                    for member in pair {
                        context.effects().remove_child_later(parent, member);
                    }
                } else {
                    context.effects().remove_child_later(parent, figure);
                }
            }
        }
        true
    }
}

struct Canvas {
    state: Shared,
    router: RouterId,
    press: Mutex<Option<Point>>,
}
impl Figure for Canvas {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, WIDTH, HEIGHT)
    }
    fn name(&self) -> &'static str {
        "ObstructionCanvas"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())?;
        out.register(INPUT, InputCapability::of::<Self>())
    }
}
impl FigureContainer for Canvas {}
impl FigureEventHandler for Canvas {
    fn on_mouse_pressed(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
        *self.press.lock().expect("press") = Some(Point::new(event.x, event.y));
        true
    }
    fn on_mouse_released(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let start = self.press.lock().expect("press").take();
        let point = Point::new(event.x, event.y);
        let root = context.target_id();
        if event.button == MouseButton::Left {
            if let Some(start) = start.filter(|start| (start.x() - point.x()).abs() > ADD_THRESHOLD)
            {
                let bounds = Rectangle::new(
                    start.x().min(point.x()),
                    start.y().min(point.y()),
                    (point.x() - start.x()).abs(),
                    (point.y() - start.y()).abs(),
                );
                let subtree =
                    FigureSubtree::new(Box::new(Item::new(bounds, false, self.state.clone())));
                context.effects().add_subtree_later(root, subtree, |_| {});
            }
        } else if event.button == MouseButton::Right {
            let pending = self.state.lock().expect("obstruction state").pending;
            if let Some(pending) = pending {
                let Ok(source_bounds) = context.scene().figure_bounds(pending) else {
                    return true;
                };
                let mut subtree = FigureSubtree::new(Box::new(
                    ConnectionFigure::new()
                        .with_stroke(BLUE, LINE_WIDTH)
                        .expect("valid detached figure input"),
                ));
                let connection = subtree.root();
                let source = subtree
                    .add_sibling(
                        connection,
                        Box::new(Item::new(source_bounds, true, self.state.clone())),
                    )
                    .expect("source");
                let target = subtree
                    .add_sibling(
                        connection,
                        Box::new(Item::new(endpoint_bounds(point), true, self.state.clone())),
                    )
                    .expect("target");
                let router = self.router;
                subtree
                    .set_connection_binding(connection, move |nodes: &SubtreeBindingNodes| {
                        Ok(SubtreeConnection {
                            source: Some(Box::new(CenterAnchor {
                                owner: nodes.figure(source).expect("source"),
                            })),
                            target: Some(Box::new(CenterAnchor {
                                owner: nodes.figure(target).expect("target"),
                            })),
                            router: RouterBinding::Explicit { router },
                            constraint: None,
                        })
                    })
                    .expect("binding");
                let state = self.state.clone();
                // Publish pair identity only after the complete subtree has been admitted.
                context.effects().replace_child_subtree_later(
                    root,
                    pending,
                    subtree,
                    move |result| {
                        if let Ok(receipt) = result {
                            let mut state = state.lock().expect("obstruction state");
                            state.pairs.push([
                                receipt.root(),
                                receipt.figure(source).expect("source"),
                                receipt.figure(target).expect("target"),
                            ]);
                            state.pending = None;
                        }
                    },
                );
            } else {
                let subtree = FigureSubtree::new(Box::new(Item::new(
                    endpoint_bounds(point),
                    true,
                    self.state.clone(),
                )));
                let state = self.state.clone();
                context
                    .effects()
                    .add_subtree_later(root, subtree, move |result| {
                        if let Ok(receipt) = result {
                            state.lock().expect("obstruction state").pending = Some(receipt.root());
                        }
                    });
            }
        } else {
            let mut state = self.state.lock().expect("obstruction state");
            state.show_segments = !state.show_segments;
        }
        true
    }
}
fn endpoint_bounds(point: Point) -> Rectangle {
    Rectangle::new(
        point.x() - ENDPOINT_SIZE / 2.0,
        point.y() - ENDPOINT_SIZE / 2.0,
        ENDPOINT_SIZE,
        ENDPOINT_SIZE,
    )
}

pub fn obstruction() -> Runtime {
    fixture().0
}
fn fixture() -> (Runtime, Shared) {
    let mut runtime = runtime_with_fonts();
    let state = Arc::new(Mutex::new(State::default()));
    let router = runtime.connections().register_router(Box::new(SceneRouter {
        state: state.clone(),
    }));
    let root = runtime
        .set_contents(Box::new(Canvas {
            state: state.clone(),
            router,
            press: Mutex::new(None),
        }))
        .expect("Canvas");
    // Exact first five seed=0 heights, derived from the specified 48-bit LCG in Rust.
    let heights = [87.0, 82.0, 86.0, 85.0, 85.0];
    for (i, height) in heights.into_iter().enumerate() {
        let bounds = Rectangle::new(
            (i / 2 * 101 + i % 3 * 10 + 100) as f64,
            (i % 2 * 101 + i % 7 * 6) as f64,
            50.0,
            height,
        );
        runtime
            .figure(root)
            .expect("Canvas")
            .add(Box::new(Item::new(bounds, false, state.clone())))
            .expect("obstacle");
    }
    (runtime, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::connection::ConnectionId;
    fn click(runtime: &mut Runtime, point: Point, button: MouseButton) {
        runtime
            .input()
            .dispatch_mouse_pressed(point.x(), point.y(), button);
        runtime
            .input()
            .dispatch_mouse_released(point.x(), point.y(), button);
    }
    #[test]
    fn obstacles_pending_pair_move_and_delete_use_real_dispatch() {
        let (mut runtime, state) = fixture();
        assert_eq!(state.lock().unwrap().obstacles.len(), 5);
        click(&mut runtime, Point::new(20.0, 50.0), MouseButton::Right);
        assert!(state.lock().unwrap().pending.is_some());
        click(&mut runtime, Point::new(500.0, 50.0), MouseButton::Right);
        let children = state.lock().unwrap().pairs[0];
        assert!(state.lock().unwrap().pending.is_none());
        let root = runtime.tree().parent_id(children[0]).unwrap().unwrap();
        let connection = ConnectionId::from_figure(children[0]);
        runtime
            .connections()
            .resolve_route(connection, CoordinateSpace::ChildContent(root))
            .unwrap();
        assert!(
            runtime
                .tree()
                .connection_route_points(children[0])
                .unwrap()
                .unwrap()
                .len()
                > 2
        );
        runtime
            .input()
            .dispatch_mouse_pressed(20.0, 50.0, MouseButton::Left);
        runtime.input().dispatch_mouse_moved(30.0, 60.0);
        runtime
            .input()
            .dispatch_mouse_released(30.0, 60.0, MouseButton::Left);
        assert_eq!(
            runtime.tree().figure_bounds(children[1]).unwrap().center(),
            Point::new(30.0, 60.0)
        );
        click(&mut runtime, Point::new(30.0, 60.0), MouseButton::Middle);
        assert!(state.lock().unwrap().pairs.is_empty());
        assert!(runtime.connections().state(connection).is_err());
        click(&mut runtime, Point::new(700.0, 500.0), MouseButton::Right);
        click(&mut runtime, Point::new(700.0, 500.0), MouseButton::Middle);
        assert!(state.lock().unwrap().pending.is_none());
    }

    #[test]
    fn multiple_pairs_share_domain_and_obstacle_topology_recomputes_the_whole_group() {
        let (mut runtime, state) = fixture();
        for y in [50.0, 250.0, 350.0] {
            click(&mut runtime, Point::new(20.0, y), MouseButton::Right);
            click(&mut runtime, Point::new(500.0, y), MouseButton::Right);
        }
        assert_eq!(state.lock().unwrap().pairs.len(), 3);
        runtime.frames().stabilize_for_query().unwrap();
        let pairs = state.lock().unwrap().pairs.clone();
        let root = runtime.tree().parent_id(pairs[0][0]).unwrap().unwrap();
        for pair in &pairs {
            assert_eq!(runtime.tree().parent_id(pair[0]).unwrap(), Some(root));
        }
        let before = runtime.connections().routing_stats().route_calculations;
        runtime
            .input()
            .dispatch_mouse_pressed(600.0, 200.0, MouseButton::Left);
        runtime.input().dispatch_mouse_moved(650.0, 260.0);
        runtime
            .input()
            .dispatch_mouse_released(650.0, 260.0, MouseButton::Left);
        assert_eq!(state.lock().unwrap().obstacles.len(), 6);
        runtime.frames().stabilize_for_query().unwrap();
        assert_eq!(
            runtime.connections().routing_stats().route_calculations - before,
            3
        );
        click(&mut runtime, Point::new(625.0, 230.0), MouseButton::Right);
        runtime.frames().stabilize_for_query().unwrap();
        assert_eq!(state.lock().unwrap().obstacles.len(), 5);
        click(&mut runtime, Point::new(20.0, 250.0), MouseButton::Middle);
        runtime.frames().stabilize_for_query().unwrap();
        assert_eq!(state.lock().unwrap().pairs.len(), 2);
        assert_eq!(runtime.tree().child_order(root).unwrap().len(), 11);
        let before = runtime.connections().routing_stats().route_calculations;
        runtime
            .input()
            .dispatch_mouse_pressed(125.0, 40.0, MouseButton::Left);
        runtime.input().dispatch_mouse_moved(155.0, 180.0);
        runtime
            .input()
            .dispatch_mouse_released(155.0, 180.0, MouseButton::Left);
        runtime.frames().stabilize_for_query().unwrap();
        assert_eq!(
            runtime.connections().routing_stats().route_calculations - before,
            2
        );
    }

    #[test]
    fn pending_drag_is_used_when_completing_pair_and_rejected_admission_preserves_it() {
        let (mut runtime, state) = fixture();
        click(&mut runtime, Point::new(20.0, 300.0), MouseButton::Right);
        runtime
            .input()
            .dispatch_mouse_pressed(20.0, 300.0, MouseButton::Left);
        runtime.input().dispatch_mouse_moved(40.0, 320.0);
        runtime
            .input()
            .dispatch_mouse_released(40.0, 320.0, MouseButton::Left);
        click(&mut runtime, Point::new(500.0, 320.0), MouseButton::Right);
        let pair = state.lock().unwrap().pairs[0];
        assert_eq!(
            runtime.tree().figure_bounds(pair[1]).unwrap().center(),
            Point::new(40.0, 320.0)
        );
        click(&mut runtime, Point::new(700.0, 400.0), MouseButton::Right);
        let pending = state.lock().unwrap().pending.unwrap();
        // Binding rejection uses an ordinary unknown Router identity from another Runtime.
        let mut foreign = Runtime::empty();
        let router = foreign
            .connections()
            .register_router(Box::new(novadraw::connection::DirectRouter));
        let root = runtime.tree().parent_id(pending).unwrap().unwrap();
        let mut subtree = FigureSubtree::new(Box::new(ConnectionFigure::new()));
        subtree
            .set_connection_binding(subtree.root(), move |_: &SubtreeBindingNodes| {
                Ok(SubtreeConnection {
                    source: None,
                    target: None,
                    router: RouterBinding::Explicit { router },
                    constraint: None,
                })
            })
            .unwrap();
        assert!(
            runtime
                .figure(root)
                .unwrap()
                .replace_child_subtree(pending, subtree)
                .is_err()
        );
        assert_eq!(state.lock().unwrap().pending, Some(pending));
        assert!(runtime.tree().figure_bounds(pending).is_ok());
    }

    #[test]
    fn pointer_exit_and_release_end_drag_without_leaking_a_gesture() {
        let (mut runtime, state) = fixture();
        let obstacle = state.lock().unwrap().obstacles[0];
        runtime
            .input()
            .dispatch_mouse_pressed(125.0, 40.0, MouseButton::Left);
        runtime.input().pointer_exited();
        runtime.input().dispatch_mouse_moved(950.0, 740.0);
        runtime
            .input()
            .dispatch_mouse_released(950.0, 740.0, MouseButton::Left);
        let bounds = runtime.tree().figure_bounds(obstacle).unwrap();
        runtime.input().dispatch_mouse_moved(900.0, 700.0);
        assert_eq!(runtime.tree().figure_bounds(obstacle).unwrap(), bounds);
        assert!(runtime.take_deferred_mutation_errors().is_empty());
    }
}
