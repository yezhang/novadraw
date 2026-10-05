use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    sync::Arc,
};

use crate::render::{
    BackendCapabilities, BackendSessionId, BuiltinFont, CaretGeometry, DamageMode, FontDescriptor,
    FrameId, NdCanvas, RenderOutcome, RenderSubmission, ResourceSync, SelectionQuad, SurfaceInfo,
    TextConstraints, TextError, TextInteractionError, TextLayout, TextLayoutEngine, TextMovement,
    UnsupportedRenderCapability,
};

use crate::PropertyValue;
use crate::connection::{ConnectionRuntime, FigureTreeSceneRead};
use crate::container::layer::LayeredPaneState;
use crate::figure::border::BorderSnapshot;
use crate::geometry::{Dimension, Point, PointList, Translatable};
use crate::mutation::{
    ComponentUpdateError, ComponentUpdateReceipt, FigureComponentUpdate, PendingMutation,
    PendingMutationKind, RuntimeMutationError, SizeOverrideKind,
};
use crate::runtime::accessibility::AccessibilityManager;
use crate::runtime::tooltip::TooltipController;
use crate::{
    AccessibilityAction, AccessibilityError, AccessibilityNodeId, AccessibilitySnapshot,
    AccessibilityUpdate, ActionListener, Alignment, AncestorListener, AnchorGeometry,
    AnchorGeometryKey, AnchorId, Border, ChildClippingStrategy, ClickableSnapshot,
    ClickableVisualState, ConnectionAnchor, ConnectionId, ConnectionLocatorStrategy,
    ConnectionRouter, ConnectionRoutingStats, ConnectionRuntimeError, ConnectionStateSnapshot,
    CoordinateListener, CoordinateSpace, CursorIcon, DependencySubject, DirectRouter, Direction,
    EventDispatcher, Figure, FigureId, FigureListener, FigureStyle, FigureTree, FlowPage,
    FlowTextPosition, FlowTextRange, FlowWrapping, FocusChange, FocusError,
    FocusTraversalDirection, FocusTraversalOutcome, FocusTraversalPolicy, FontId, FreeformError,
    ImageDisplayState, ImageFigure, ImageId, InteractionState, Key, KeyModifiers, LabelFigure,
    LayerError, LayerKey, LayerPlacement, LayeredPane, LayeredPaneHandle, LayoutConstraint,
    LayoutListener, LayoutManager, ListenerId, ListenerScope, MonotonicTime, MouseButton,
    ObservationListener, PendingMutations, PolygonScaleMode, PropertyChangeListener, Rectangle,
    ResourceError, ResourceRegistry, RouteError, RouteMetadata, RouteOutput, RouteRequest,
    RouterBinding, RouterId, RoutingConstraint, RoutingGroupScope, ScaleHandle,
    SceneDispatchContext, ScrollBarVisibility, ScrollPaneHandle, ShapeMutationError,
    StableQueryError, StableSceneQuery, StackLayout, TextPlacement, TimeError, TooltipSnapshot,
    TooltipTiming, TooltipUpdate, TrackedSceneQuery, TreeOrderFocusTraversal, UnresolvedConnection,
    UpdateEvent, UpdateListener, UpdateManager, ValidationError, ViewportHandle, WheelEvent,
    WidgetError, ZoomEvent, ZoomManager,
};

mod component_update;
mod connection_service;
mod frame_resource;
mod frame_submission;

const DERIVED_STATE_FEEDBACK_LIMIT: usize = 16;
const DERIVED_WORK_KIND_COUNT: usize = 6;

#[derive(Clone, Copy, Debug)]
#[repr(usize)]
enum DerivedWorkKind {
    IntrinsicMetrics,
    Layout,
    DependencyInvalidation,
    Routing,
    PostRouteGeometry,
    Presentation,
}

#[derive(Default)]
struct DerivedWorkSet {
    queued: [bool; DERIVED_WORK_KIND_COUNT],
}

impl DerivedWorkSet {
    fn insert(&mut self, kind: DerivedWorkKind) {
        self.queued[kind as usize] = true;
    }

    fn pop_next(&mut self) -> Option<DerivedWorkKind> {
        const ORDER: [DerivedWorkKind; DERIVED_WORK_KIND_COUNT] = [
            DerivedWorkKind::IntrinsicMetrics,
            DerivedWorkKind::Layout,
            DerivedWorkKind::DependencyInvalidation,
            DerivedWorkKind::Routing,
            DerivedWorkKind::PostRouteGeometry,
            DerivedWorkKind::Presentation,
        ];
        ORDER.into_iter().find(|kind| {
            let queued = &mut self.queued[*kind as usize];
            std::mem::take(queued)
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FramePreparationError {
    Faulted,
    Text(TextError),
    Graphics(crate::graphics::GraphicsError),
    Validation(ValidationError),
    Accessibility(AccessibilityError),
    UnsupportedRenderCapability(UnsupportedRenderCapability),
    InvalidGraphicsInput(crate::render::InvalidGraphicsInput),
    DidNotConverge,
}

impl fmt::Display for FramePreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Faulted => formatter.write_str("Runtime is faulted"),
            Self::Text(error) => error.fmt(formatter),
            Self::Graphics(error) => error.fmt(formatter),
            Self::Validation(error) => error.fmt(formatter),
            Self::Accessibility(error) => error.fmt(formatter),
            Self::UnsupportedRenderCapability(error) => error.fmt(formatter),
            Self::InvalidGraphicsInput(error) => error.fmt(formatter),
            Self::DidNotConverge => formatter.write_str("derived state did not converge"),
        }
    }
}

impl std::error::Error for FramePreparationError {}

#[derive(Clone, Debug)]
pub enum FramePreparation {
    Ready(RenderSubmission),
    Idle,
    Suspended,
    AwaitingCompletion,
    Error(FramePreparationError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendSessionError {
    Faulted,
    Exhausted,
}

/// Cumulative work counters for Runtime-owned text layout refreshes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextLayoutStats {
    /// Style-propagation nodes inspected while refreshing Label intrinsic layouts.
    pub label_style_nodes_visited: u64,
    /// Label figures considered for intrinsic layout refresh.
    pub label_figures_refreshed: u64,
    /// Style-propagation nodes inspected while refreshing TextFlow layouts.
    pub text_flow_style_nodes_visited: u64,
    /// TextFlow figures considered for layout refresh.
    pub text_flow_figures_refreshed: u64,
}

impl fmt::Display for BackendSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Faulted => formatter.write_str("Runtime is faulted"),
            Self::Exhausted => formatter.write_str("backend session id space is exhausted"),
        }
    }
}

impl std::error::Error for BackendSessionError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LogicalViewportResizeError {
    Faulted,
    InvalidSize { width: f64, height: f64 },
}

impl fmt::Display for LogicalViewportResizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Faulted => formatter.write_str("Runtime is faulted"),
            Self::InvalidSize { width, height } => {
                write!(
                    formatter,
                    "invalid logical viewport size: {width} x {height}"
                )
            }
        }
    }
}

impl std::error::Error for LogicalViewportResizeError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextFlowQueryError {
    UnknownFigure(FigureId),
    WrongCapability(FigureId),
    NonInvertibleTransform(FigureId),
    Interaction(TextInteractionError),
}

impl fmt::Display for TextFlowQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFigure(id) => write!(formatter, "unknown Figure: {id:?}"),
            Self::WrongCapability(id) => {
                write!(
                    formatter,
                    "Figure does not support TextFlow queries: {id:?}"
                )
            }
            Self::NonInvertibleTransform(id) => {
                write!(
                    formatter,
                    "Figure has no invertible surface transform: {id:?}"
                )
            }
            Self::Interaction(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TextFlowQueryError {}

impl From<TextInteractionError> for TextFlowQueryError {
    fn from(error: TextInteractionError) -> Self {
        Self::Interaction(error)
    }
}

/// Owns one scene and enforces its input, mutation, and update transaction boundaries.
pub struct Runtime {
    tree: FigureTree,
    interaction: InteractionState,
    interaction_dispatcher: EventDispatcher,
    tooltip_controller: TooltipController,
    accessibility: AccessibilityManager,
    focus_traversal_policy: Box<dyn FocusTraversalPolicy>,
    updates: UpdateManager,
    mutations: PendingMutations,
    full_redraw_pending: bool,
    backend_session_id: BackendSessionId,
    session_sync_pending: bool,
    next_frame_id: FrameId,
    in_flight: Option<InFlightFrame>,
    last_surface: Option<SurfaceInfo>,
    logical_viewport: Option<Rectangle>,
    resources: ResourceRegistry,
    text: Box<dyn TextLayoutEngine>,
    text_layout_stats: TextLayoutStats,
    builtin_fonts: HashMap<BuiltinFont, FontId>,
    layered_panes: HashMap<FigureId, LayeredPaneState>,
    connections: ConnectionRuntime,
    anchor_geometries: HashMap<(FigureId, AnchorGeometryKey), AnchorGeometry>,
    connection_error: Option<ConnectionRuntimeError>,
    deferred_mutation_errors: VecDeque<RuntimeMutationError>,
    derivation_epoch: u64,
    stable_epoch: u64,
    last_stabilization_error: Option<FramePreparationError>,
    faulted: bool,
}

/// Short-lived, Runtime-backed mutation facade for one attached Figure.
pub struct FigureEditor<'a> {
    figure: FigureId,
    runtime: &'a mut Runtime,
}

/// Short-lived, Runtime-backed mutation facade for one attached Label.
pub struct LabelEditor<'a> {
    label: FigureId,
    runtime: &'a mut Runtime,
}

/// Short-lived, Runtime-backed mutation facade for one attached clickable Figure.
pub struct ClickableEditor<'a> {
    clickable: FigureId,
    runtime: &'a mut Runtime,
}

/// Short-lived, Runtime-backed mutation facade for one attached ImageFigure.
pub struct ImageEditor<'a> {
    image: FigureId,
    runtime: &'a mut Runtime,
}

/// Short-lived, Runtime-backed mutation facade for one attached point-list Figure.
pub struct PointListEditor<'a> {
    point_list: FigureId,
    runtime: &'a mut Runtime,
}

/// Short-lived, Runtime-backed mutation facade for one attached scalable polygon.
pub struct ScalablePolygonEditor<'a> {
    polygon: FigureId,
    runtime: &'a mut Runtime,
}

/// Short-lived, Runtime-backed mutation facade for one attached TextFlow.
pub struct TextFlowEditor<'a> {
    text_flow: FigureId,
    runtime: &'a mut Runtime,
}

/// Short-lived, Runtime-backed mutation facade for one attached bordered Figure.
pub struct BorderEditor<'a> {
    bordered: FigureId,
    runtime: &'a mut Runtime,
}

/// Short-lived, Runtime-backed mutation facade for one attached RoundedRectangleFigure.
pub struct RoundedRectangleEditor<'a> {
    rounded_rectangle: FigureId,
    runtime: &'a mut Runtime,
}

/// Short-lived, Runtime-backed mutation facade for one attached TriangleFigure.
pub struct TriangleEditor<'a> {
    triangle: FigureId,
    runtime: &'a mut Runtime,
}

/// Short-lived, Runtime-backed mutation facade for one attached container Figure.
pub struct ContainerEditor<'a> {
    container: FigureId,
    runtime: &'a mut Runtime,
}

/// Short-lived mutation facade for one attached Viewport.
pub struct ViewportEditor<'a> {
    viewport: ViewportHandle,
    runtime: &'a mut Runtime,
}

/// Short-lived mutation facade for one attached scalable Figure.
pub struct ScaleEditor<'a> {
    scalable: ScaleHandle,
    runtime: &'a mut Runtime,
}

/// Short-lived mutation facade for one attached ScrollPane.
pub struct ScrollPaneEditor<'a> {
    pane: ScrollPaneHandle,
    runtime: &'a mut Runtime,
}

/// Short-lived mutation facade for one attached Viewport/Scale pair.
pub struct ZoomEditor<'a> {
    zoom: &'a ZoomManager,
    runtime: &'a mut Runtime,
}

struct InFlightFrame {
    session_id: BackendSessionId,
    id: FrameId,
    resources: ResourceSync,
}

impl FigureEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.figure
    }

    pub fn reparent(&mut self, new_parent: FigureId) -> Result<bool, RuntimeMutationError> {
        self.runtime.reparent(self.figure, new_parent)
    }

    pub fn set_layout_constraint<C>(&mut self, constraint: C) -> Result<bool, RuntimeMutationError>
    where
        C: LayoutConstraint,
    {
        self.runtime.set_layout_constraint(self.figure, constraint)
    }

    pub fn remove_layout_constraint(&mut self) -> Result<bool, RuntimeMutationError> {
        self.runtime.remove_layout_constraint(self.figure)
    }

    pub fn set_preferred_size(&mut self, size: (f64, f64)) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_preferred_size(self.figure, size)
    }

    pub fn clear_preferred_size(&mut self) -> Result<bool, RuntimeMutationError> {
        self.runtime.clear_preferred_size(self.figure)
    }

    pub fn set_minimum_size(&mut self, size: (f64, f64)) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_minimum_size(self.figure, size)
    }

    pub fn clear_minimum_size(&mut self) -> Result<bool, RuntimeMutationError> {
        self.runtime.clear_minimum_size(self.figure)
    }

    pub fn set_maximum_size(&mut self, size: (f64, f64)) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_maximum_size(self.figure, size)
    }

    pub fn clear_maximum_size(&mut self) -> Result<bool, RuntimeMutationError> {
        self.runtime.clear_maximum_size(self.figure)
    }

    pub fn set_bounds(&mut self, bounds: Rectangle) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_bounds(self.figure, bounds)
    }

    pub fn translate(&mut self, dx: f64, dy: f64) -> Result<bool, RuntimeMutationError> {
        self.runtime.translate(self.figure, dx, dy)
    }

    pub fn set_visible(&mut self, visible: bool) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_visible(self.figure, visible)
    }

    pub fn set_enabled(&mut self, enabled: bool) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_enabled(self.figure, enabled)
    }

    pub fn set_focusable(&mut self, focusable: bool) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_focusable(self.figure, focusable)
    }

    pub fn set_focus_traversable(
        &mut self,
        traversable: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_focus_traversable(self.figure, traversable)
    }

    pub fn set_style(&mut self, style: FigureStyle) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_figure_style(self.figure, style)
    }

    pub fn set_opaque(&mut self, opaque: bool) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_opaque(self.figure, opaque)
    }

    pub fn revalidate(&mut self) -> Result<(), RuntimeMutationError> {
        self.runtime.revalidate(self.figure)
    }

    pub fn repaint(&mut self, rect: Option<Rectangle>) -> Result<(), RuntimeMutationError> {
        self.runtime.repaint(self.figure, rect)
    }

    pub fn update_component<U>(
        &mut self,
        update: U,
    ) -> Result<ComponentUpdateReceipt, ComponentUpdateError<U::Error>>
    where
        U: FigureComponentUpdate,
    {
        self.runtime.update_component(self.figure, update)
    }
}

impl LabelEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.label
    }

    pub fn set_text(&mut self, text: impl Into<String>) -> Result<bool, ShapeMutationError> {
        self.runtime.set_label_text(self.label, text)
    }

    pub fn set_icon(&mut self, icon: Option<ImageId>) -> Result<bool, ShapeMutationError> {
        self.runtime.set_label_icon(self.label, icon)
    }

    pub fn set_text_placement(
        &mut self,
        placement: TextPlacement,
    ) -> Result<bool, ShapeMutationError> {
        self.runtime.set_label_text_placement(self.label, placement)
    }

    pub fn set_alignment(&mut self, alignment: Alignment) -> Result<bool, ShapeMutationError> {
        self.runtime.set_label_alignment(self.label, alignment)
    }

    pub fn set_text_alignment(&mut self, alignment: Alignment) -> Result<bool, ShapeMutationError> {
        self.runtime.set_label_text_alignment(self.label, alignment)
    }

    pub fn set_icon_alignment(&mut self, alignment: Alignment) -> Result<bool, ShapeMutationError> {
        self.runtime.set_label_icon_alignment(self.label, alignment)
    }

    pub fn set_icon_text_gap(&mut self, gap: f64) -> Result<bool, ShapeMutationError> {
        self.runtime.set_label_icon_text_gap(self.label, gap)
    }
}

impl ClickableEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.clickable
    }

    pub fn set_selected(&mut self, selected: bool) -> Result<bool, WidgetError> {
        self.runtime
            .set_clickable_selected(self.clickable, selected)
    }

    pub fn set_rollover_enabled(&mut self, enabled: bool) -> Result<bool, WidgetError> {
        self.runtime.set_rollover_enabled(self.clickable, enabled)
    }
}

impl ImageEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.image
    }

    pub fn set_image(&mut self, image: ImageId) -> Result<bool, ShapeMutationError> {
        self.runtime.set_image_figure(self.image, image)
    }

    pub fn set_alignment(&mut self, alignment: Alignment) -> Result<bool, ShapeMutationError> {
        self.runtime.set_image_alignment(self.image, alignment)
    }
}

impl PointListEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.point_list
    }

    pub fn replace_points(&mut self, points: Vec<Point>) -> Result<bool, ShapeMutationError> {
        self.runtime.replace_points(self.point_list, points)
    }

    pub fn insert_point(&mut self, index: usize, point: Point) -> Result<bool, ShapeMutationError> {
        self.runtime.insert_point(self.point_list, index, point)
    }

    pub fn set_point(&mut self, index: usize, point: Point) -> Result<bool, ShapeMutationError> {
        self.runtime.set_point(self.point_list, index, point)
    }

    pub fn remove_point(&mut self, index: usize) -> Result<bool, ShapeMutationError> {
        self.runtime.remove_point(self.point_list, index)
    }

    pub fn clear_points(&mut self) -> Result<bool, ShapeMutationError> {
        self.runtime.clear_points(self.point_list)
    }

    pub fn set_stroke_width(&mut self, stroke_width: f64) -> Result<bool, ShapeMutationError> {
        self.runtime
            .set_point_list_stroke_width(self.point_list, stroke_width)
    }

    /// Replaces all stroke attributes in one bounds/damage transaction.
    pub fn set_stroke_style(
        &mut self,
        stroke: crate::render::StrokeStyle,
    ) -> Result<bool, ShapeMutationError> {
        self.runtime
            .set_point_list_stroke_style(self.point_list, stroke)
    }

    pub fn stroke_style(&self) -> Result<crate::render::StrokeStyle, ShapeMutationError> {
        self.runtime.point_list_stroke_style(self.point_list)
    }

    pub fn set_miter_limit(&mut self, limit: f64) -> Result<bool, ShapeMutationError> {
        let stroke = self
            .stroke_style()?
            .with_miter_limit(limit)
            .map_err(ShapeMutationError::InvalidStroke)?;
        self.set_stroke_style(stroke)
    }

    pub fn set_line_cap(
        &mut self,
        cap: crate::render::LineCap,
    ) -> Result<bool, ShapeMutationError> {
        let stroke = self.stroke_style()?.with_cap(cap);
        self.set_stroke_style(stroke)
    }

    pub fn set_dash_pattern(
        &mut self,
        dash: crate::render::DashPattern,
    ) -> Result<bool, ShapeMutationError> {
        let stroke = self.stroke_style()?.with_dash_pattern(dash);
        self.set_stroke_style(stroke)
    }

    pub fn set_dash_offset(&mut self, offset: f64) -> Result<bool, ShapeMutationError> {
        let stroke = self
            .stroke_style()?
            .with_dash_offset(offset)
            .map_err(ShapeMutationError::InvalidStroke)?;
        self.set_stroke_style(stroke)
    }

    pub fn set_line_join(
        &mut self,
        line_join: crate::render::LineJoin,
    ) -> Result<bool, ShapeMutationError> {
        self.runtime
            .set_point_list_line_join(self.point_list, line_join)
    }
}

impl ScalablePolygonEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.polygon
    }

    pub fn replace_template(&mut self, template: PointList) -> Result<bool, ShapeMutationError> {
        self.runtime
            .replace_scalable_polygon_template(self.polygon, template)
    }

    pub fn set_scale_mode(&mut self, mode: PolygonScaleMode) -> Result<bool, ShapeMutationError> {
        self.runtime
            .set_scalable_polygon_scale_mode(self.polygon, mode)
    }

    pub fn set_alignment(
        &mut self,
        horizontal: Alignment,
        vertical: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        self.runtime
            .set_scalable_polygon_alignment(self.polygon, horizontal, vertical)
    }
}

impl TextFlowEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.text_flow
    }

    pub fn replace_page(&mut self, page: FlowPage) -> Result<bool, ShapeMutationError> {
        self.runtime.replace_text_flow_page(self.text_flow, page)
    }

    pub fn set_wrapping(&mut self, wrapping: FlowWrapping) -> Result<bool, ShapeMutationError> {
        self.runtime
            .set_text_flow_wrapping(self.text_flow, wrapping)
    }
}

impl BorderEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.bordered
    }

    pub fn set(&mut self, border: impl Border + 'static) -> Result<bool, ShapeMutationError> {
        self.runtime.set_border(self.bordered, border)
    }

    pub fn replace(&mut self, border: Option<Arc<dyn Border>>) -> Result<bool, ShapeMutationError> {
        self.runtime.replace_border(self.bordered, border)
    }
}

impl RoundedRectangleEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.rounded_rectangle
    }

    pub fn set_corner_dimensions(
        &mut self,
        dimensions: Dimension,
    ) -> Result<bool, ShapeMutationError> {
        self.runtime
            .set_corner_dimensions(self.rounded_rectangle, dimensions)
    }
}

impl TriangleEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.triangle
    }

    pub fn set_direction(&mut self, direction: Direction) -> Result<bool, ShapeMutationError> {
        self.runtime
            .set_triangle_direction(self.triangle, direction)
    }
}

impl ContainerEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.container
    }

    pub fn add(&mut self, figure: Box<dyn Figure>) -> Result<FigureId, RuntimeMutationError> {
        self.runtime.add_figure(self.container, figure)
    }

    /// Inserts a new child at `0..=children.len()` as one Runtime operation.
    pub fn insert(
        &mut self,
        index: usize,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.runtime
            .insert_figure(self.container, index, figure, None)
    }

    /// Atomically inserts a child and its parent-owned layout constraint.
    ///
    /// A rejected index, admission or constraint publishes no partial state.
    pub fn insert_with_constraint<C: LayoutConstraint>(
        &mut self,
        index: usize,
        figure: Box<dyn Figure>,
        constraint: C,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.runtime
            .insert_figure(self.container, index, figure, Some(Box::new(constraint)))
    }

    pub fn remove(&mut self, child: FigureId) -> Result<bool, RuntimeMutationError> {
        self.runtime.remove_figure(self.container, child)
    }

    pub fn set_layout_manager(
        &mut self,
        manager: Box<dyn LayoutManager>,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_layout_manager(self.container, manager)
    }

    pub fn clear_layout_manager(&mut self) -> Result<bool, RuntimeMutationError> {
        self.runtime.clear_layout_manager(self.container)
    }

    pub fn move_child_to_index(
        &mut self,
        child: FigureId,
        index: usize,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime
            .move_child_to_index(self.container, child, index)
    }

    pub fn set_child_order(&mut self, order: &[FigureId]) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_child_order(self.container, order)
    }

    pub fn bring_child_to_front(&mut self, child: FigureId) -> Result<bool, RuntimeMutationError> {
        self.runtime.bring_child_to_front(self.container, child)
    }

    pub fn send_child_to_back(&mut self, child: FigureId) -> Result<bool, RuntimeMutationError> {
        self.runtime.send_child_to_back(self.container, child)
    }

    pub fn set_child_clipping_strategy(
        &mut self,
        strategy: ChildClippingStrategy,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime
            .set_child_clipping_strategy(self.container, strategy)
    }
}

impl ViewportEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.viewport.figure_id()
    }

    pub fn set_view_location(&mut self, x: f64, y: f64) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_view_location(&self.viewport, x, y)
    }

    pub fn set_horizontal_location(&mut self, x: f64) -> Result<bool, RuntimeMutationError> {
        let current = self.viewport.view_location();
        self.set_view_location(x, current.y())
    }

    pub fn set_vertical_location(&mut self, y: f64) -> Result<bool, RuntimeMutationError> {
        let current = self.viewport.view_location();
        self.set_view_location(current.x(), y)
    }

    pub fn scroll_by(&mut self, dx: f64, dy: f64) -> Result<bool, RuntimeMutationError> {
        let current = self.viewport.view_location();
        self.set_view_location(current.x() + dx, current.y() + dy)
    }

    pub fn set_contents(
        &mut self,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        let viewport = self.viewport.clone();
        self.runtime.guarded_runtime_mutation(move |runtime| {
            runtime.validate_attached_figure(viewport.figure_id())?;
            if let Some(previous) = viewport.contents(&runtime.tree) {
                runtime.dispose_subtree(previous)?;
            }
            runtime.add_figure_inner(viewport.figure_id(), figure)
        })
    }

    pub fn set_tracks_width(&mut self, tracks: bool) -> Result<bool, RuntimeMutationError> {
        let viewport = self.viewport.clone();
        self.runtime.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(viewport.figure_id())?;
            viewport
                .set_tracks_width(&mut runtime.tree, &mut runtime.updates, tracks)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub fn set_tracks_height(&mut self, tracks: bool) -> Result<bool, RuntimeMutationError> {
        let viewport = self.viewport.clone();
        self.runtime.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(viewport.figure_id())?;
            viewport
                .set_tracks_height(&mut runtime.tree, &mut runtime.updates, tracks)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }
}

impl ScaleEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.scalable.figure_id()
    }

    pub fn set_scale(&mut self, scale: f64) -> Result<bool, RuntimeMutationError> {
        let scalable = self.scalable.clone();
        self.runtime.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(scalable.figure_id())?;
            scalable
                .set_scale(&mut runtime.tree, &mut runtime.updates, scale)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }
}

impl ScrollPaneEditor<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.pane.pane_id()
    }

    pub fn set_scroll_bar_visibility(
        &mut self,
        horizontal: ScrollBarVisibility,
        vertical: ScrollBarVisibility,
    ) -> Result<bool, RuntimeMutationError> {
        let pane = self.pane.clone();
        self.runtime.guarded_runtime_mutation(|runtime| {
            pane.set_scroll_bar_visibility(
                &mut runtime.tree,
                &mut runtime.updates,
                horizontal,
                vertical,
            )
            .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub fn scroll_to(&mut self, x: f64, y: f64) -> Result<bool, RuntimeMutationError> {
        let viewport = self.pane.viewport().clone();
        self.runtime.set_view_location(&viewport, x, y)
    }

    pub fn set_contents(
        &mut self,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        let viewport = self.pane.viewport().clone();
        ViewportEditor {
            viewport,
            runtime: self.runtime,
        }
        .set_contents(figure)
    }
}

impl ZoomEditor<'_> {
    pub fn set_zoom(&mut self, scale: f64) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_zoom_at(self.zoom, scale, None)
    }

    pub fn set_zoom_at(
        &mut self,
        scale: f64,
        anchor: Option<Point>,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_zoom_at(self.zoom, scale, anchor)
    }

    pub fn zoom_in(&mut self) -> Result<bool, RuntimeMutationError> {
        self.mutate(|zoom, tree, updates| zoom.zoom_in(tree, updates))
    }

    pub fn zoom_out(&mut self) -> Result<bool, RuntimeMutationError> {
        self.mutate(|zoom, tree, updates| zoom.zoom_out(tree, updates))
    }

    pub fn fit_all(&mut self) -> Result<bool, RuntimeMutationError> {
        self.runtime.fit_zoom_to_contents(self.zoom)
    }

    pub fn fit_width(&mut self) -> Result<bool, RuntimeMutationError> {
        self.mutate(|zoom, tree, updates| zoom.fit_width(tree, updates))
    }

    pub fn fit_height(&mut self) -> Result<bool, RuntimeMutationError> {
        self.mutate(|zoom, tree, updates| zoom.fit_height(tree, updates))
    }

    fn mutate(
        &mut self,
        operation: impl FnOnce(
            &ZoomManager,
            &mut FigureTree,
            &mut UpdateManager,
        ) -> Result<bool, crate::ZoomError>,
    ) -> Result<bool, RuntimeMutationError> {
        let zoom = self.zoom;
        self.runtime.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(zoom.viewport().figure_id())?;
            runtime.validate_attached_figure(zoom.scalable().figure_id())?;
            operation(zoom, &mut runtime.tree, &mut runtime.updates)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }
}

impl Runtime {
    pub fn new(tree: FigureTree) -> Self {
        Self::with_text_layout_engine(tree, Box::new(crate::render::ParleyTextEngine::new()))
    }

    pub fn with_text_layout_engine(tree: FigureTree, text: Box<dyn TextLayoutEngine>) -> Self {
        let namespace = tree.namespace();
        let resources = ResourceRegistry::with_namespace(namespace.as_uuid());
        let backend_session_id = BackendSessionId::initial(resources.namespace());
        let mut updates = UpdateManager::with_namespace(namespace);
        for root in tree.invalid_validation_roots() {
            updates.add_invalid_figure(root);
        }
        let mut runtime = Self {
            tree,
            interaction: InteractionState::default(),
            interaction_dispatcher: EventDispatcher,
            tooltip_controller: TooltipController::default(),
            accessibility: AccessibilityManager::default(),
            focus_traversal_policy: Box::new(TreeOrderFocusTraversal),
            updates,
            mutations: PendingMutations::new(),
            full_redraw_pending: true,
            backend_session_id,
            session_sync_pending: true,
            next_frame_id: FrameId::INITIAL,
            in_flight: None,
            last_surface: None,
            logical_viewport: None,
            resources,
            text,
            text_layout_stats: TextLayoutStats::default(),
            builtin_fonts: HashMap::new(),
            layered_panes: HashMap::new(),
            connections: ConnectionRuntime::with_namespace(namespace),
            anchor_geometries: HashMap::new(),
            connection_error: None,
            deferred_mutation_errors: VecDeque::new(),
            derivation_epoch: 0,
            stable_epoch: 0,
            last_stabilization_error: None,
            faulted: false,
        };
        runtime.initialize_layered_panes();
        runtime.activate_attached_figures();
        runtime
    }

    pub fn empty() -> Self {
        Self::new(FigureTree::new())
    }

    pub fn tree(&self) -> &FigureTree {
        &self.tree
    }

    pub fn figure(&mut self, figure: FigureId) -> Result<FigureEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(figure)?;
        Ok(FigureEditor {
            figure,
            runtime: self,
        })
    }

    pub fn label(&mut self, label: FigureId) -> Result<LabelEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(label)?;
        if self.tree.label(label).is_none() {
            return Err(RuntimeMutationError::WrongCapability {
                figure: label,
                capability: "label mutation",
            });
        }
        Ok(LabelEditor {
            label,
            runtime: self,
        })
    }

    pub fn clickable(
        &mut self,
        clickable: FigureId,
    ) -> Result<ClickableEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(clickable)?;
        if self.tree.clickable_snapshot(clickable).is_none() {
            return Err(RuntimeMutationError::WrongCapability {
                figure: clickable,
                capability: "clickable mutation",
            });
        }
        Ok(ClickableEditor {
            clickable,
            runtime: self,
        })
    }

    pub fn image(&mut self, image: FigureId) -> Result<ImageEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(image)?;
        if self.tree.image_figure(image).is_none() {
            return Err(RuntimeMutationError::WrongCapability {
                figure: image,
                capability: "image mutation",
            });
        }
        Ok(ImageEditor {
            image,
            runtime: self,
        })
    }

    pub fn point_list(
        &mut self,
        point_list: FigureId,
    ) -> Result<PointListEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(point_list)?;
        if self.tree.point_list_style(point_list).is_none() {
            return Err(RuntimeMutationError::WrongCapability {
                figure: point_list,
                capability: "point-list mutation",
            });
        }
        Ok(PointListEditor {
            point_list,
            runtime: self,
        })
    }

    pub fn scalable_polygon(
        &mut self,
        polygon: FigureId,
    ) -> Result<ScalablePolygonEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(polygon)?;
        if !self.tree.has_scalable_polygon_capability(polygon) {
            return Err(RuntimeMutationError::WrongCapability {
                figure: polygon,
                capability: "scalable-polygon mutation",
            });
        }
        Ok(ScalablePolygonEditor {
            polygon,
            runtime: self,
        })
    }

    pub fn text_flow(
        &mut self,
        text_flow: FigureId,
    ) -> Result<TextFlowEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(text_flow)?;
        if self.tree.text_flow(text_flow).is_none() {
            return Err(RuntimeMutationError::WrongCapability {
                figure: text_flow,
                capability: "text-flow mutation",
            });
        }
        Ok(TextFlowEditor {
            text_flow,
            runtime: self,
        })
    }

    pub fn border(&mut self, bordered: FigureId) -> Result<BorderEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(bordered)?;
        if !self.tree.has_border_capability(bordered) {
            return Err(RuntimeMutationError::WrongCapability {
                figure: bordered,
                capability: "border mutation",
            });
        }
        Ok(BorderEditor {
            bordered,
            runtime: self,
        })
    }

    pub fn rounded_rectangle(
        &mut self,
        rounded_rectangle: FigureId,
    ) -> Result<RoundedRectangleEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(rounded_rectangle)?;
        if !self
            .tree
            .has_rounded_rectangle_capability(rounded_rectangle)
        {
            return Err(RuntimeMutationError::WrongCapability {
                figure: rounded_rectangle,
                capability: "rounded-rectangle mutation",
            });
        }
        Ok(RoundedRectangleEditor {
            rounded_rectangle,
            runtime: self,
        })
    }

    pub fn triangle(
        &mut self,
        triangle: FigureId,
    ) -> Result<TriangleEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(triangle)?;
        if !self.tree.has_triangle_capability(triangle) {
            return Err(RuntimeMutationError::WrongCapability {
                figure: triangle,
                capability: "triangle mutation",
            });
        }
        Ok(TriangleEditor {
            triangle,
            runtime: self,
        })
    }

    pub fn container(
        &mut self,
        container: FigureId,
    ) -> Result<ContainerEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(container)?;
        Ok(ContainerEditor {
            container,
            runtime: self,
        })
    }

    pub fn viewport(
        &mut self,
        viewport: FigureId,
    ) -> Result<ViewportEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(viewport)?;
        let viewport =
            self.tree
                .viewport_handle(viewport)
                .ok_or(RuntimeMutationError::WrongCapability {
                    figure: viewport,
                    capability: "viewport mutation",
                })?;
        Ok(ViewportEditor {
            viewport,
            runtime: self,
        })
    }

    pub fn scalable(
        &mut self,
        scalable: FigureId,
    ) -> Result<ScaleEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(scalable)?;
        let scalable =
            self.tree
                .scale_handle(scalable)
                .ok_or(RuntimeMutationError::WrongCapability {
                    figure: scalable,
                    capability: "scale mutation",
                })?;
        Ok(ScaleEditor {
            scalable,
            runtime: self,
        })
    }

    pub fn scroll_pane(
        &mut self,
        pane: &ScrollPaneHandle,
    ) -> Result<ScrollPaneEditor<'_>, RuntimeMutationError> {
        self.validate_attached_figure(pane.pane_id())?;
        self.validate_attached_figure(pane.viewport().figure_id())?;
        Ok(ScrollPaneEditor {
            pane: pane.clone(),
            runtime: self,
        })
    }

    pub fn zoom<'a>(
        &'a mut self,
        zoom: &'a ZoomManager,
    ) -> Result<ZoomEditor<'a>, RuntimeMutationError> {
        self.validate_attached_figure(zoom.viewport().figure_id())?;
        self.validate_attached_figure(zoom.scalable().figure_id())?;
        Ok(ZoomEditor {
            zoom,
            runtime: self,
        })
    }

    pub fn freeform_extent(&self, figure: FigureId) -> Result<Rectangle, FreeformError> {
        self.tree.freeform_extent(figure)
    }

    pub fn interaction(&self) -> &InteractionState {
        &self.interaction
    }

    pub fn cursor_icon(&self) -> CursorIcon {
        self.interaction
            .cursor_target()
            .and_then(|id| self.tree.resolved_style(id))
            .map_or(CursorIcon::Default, |style| style.cursor)
    }

    pub fn tooltip(&self) -> Option<String> {
        self.interaction
            .hover_source()
            .and_then(|id| self.tree.resolved_style(id))
            .and_then(|style| style.tooltip)
    }

    pub fn set_tooltip_timing(&mut self, timing: TooltipTiming) -> Result<bool, TimeError> {
        self.tooltip_controller.set_timing(timing)
    }

    pub fn advance_time(&mut self, now: MonotonicTime) -> Result<bool, TimeError> {
        if self.faulted {
            return Ok(false);
        }
        self.tooltip_controller.advance_time(now)
    }

    pub fn next_wake_deadline(&self) -> Option<MonotonicTime> {
        self.tooltip_controller.next_wake_deadline()
    }

    pub fn visible_tooltip(&self) -> Option<&TooltipSnapshot> {
        self.tooltip_controller.visible_snapshot()
    }

    pub fn take_tooltip_updates(&mut self) -> Vec<TooltipUpdate> {
        self.tooltip_controller.take_updates()
    }

    pub fn accessibility_snapshot(&self) -> Option<&AccessibilitySnapshot> {
        self.accessibility.snapshot()
    }

    pub fn take_accessibility_updates(&mut self) -> Vec<AccessibilityUpdate> {
        self.accessibility.take_updates()
    }

    pub fn request_accessibility_snapshot(&mut self) {
        self.accessibility.reset();
    }

    pub fn perform_accessibility_action(
        &mut self,
        node: AccessibilityNodeId,
        action: AccessibilityAction,
    ) -> Result<bool, AccessibilityError> {
        if self.faulted {
            return Err(AccessibilityError::RuntimeFaulted);
        }
        let figure = match node {
            AccessibilityNodeId::Root(namespace) => {
                if namespace != self.tree.namespace() {
                    return Err(AccessibilityError::ForeignRuntime(node));
                }
                return Err(AccessibilityError::UnsupportedAction { node, action });
            }
            AccessibilityNodeId::Figure(figure) => {
                if figure.namespace() != self.tree.namespace() {
                    return Err(AccessibilityError::ForeignRuntime(node));
                }
                figure
            }
        };
        let snapshot = self
            .accessibility
            .snapshot()
            .ok_or(AccessibilityError::Unavailable(node))?;
        let semantic_node = snapshot
            .nodes
            .iter()
            .find(|candidate| candidate.id == node)
            .ok_or(AccessibilityError::UnknownNode(node))?;
        let focusable = semantic_node.state.focusable;
        let default_action = semantic_node.default_action;
        match action {
            AccessibilityAction::Focus if focusable => self
                .request_focus(figure)
                .map(|change| matches!(change, FocusChange::Changed { .. }))
                .map_err(|_| AccessibilityError::Unavailable(node)),
            AccessibilityAction::Default
                if default_action == Some(AccessibilityAction::Default) =>
            {
                self.do_click(figure)
                    .map_err(|_| AccessibilityError::Unavailable(node))
            }
            _ => Err(AccessibilityError::UnsupportedAction { node, action }),
        }
    }

    pub fn resources(&self) -> &ResourceRegistry {
        &self.resources
    }

    pub fn builtin_font(&self, font: BuiltinFont) -> Option<FontId> {
        self.builtin_fonts.get(&font).copied()
    }

    pub fn text_revision(&self) -> u64 {
        self.text.revision()
    }

    pub fn layout_text(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        if self.faulted {
            return Err(TextError::RuntimeFaulted);
        }
        self.guarded(|runtime| runtime.text.layout(text, font, constraints))
    }

    /// Refreshes immutable Label text snapshots from the current Runtime-owned text engine.
    pub fn refresh_label_layouts(&mut self) -> Result<(), TextError> {
        if self.faulted {
            return Err(TextError::RuntimeFaulted);
        }
        self.guarded(|runtime| {
            runtime.refresh_label_intrinsic_metrics()?;
            runtime.refresh_label_presentations()?;
            Ok(())
        })
    }

    fn refresh_label_intrinsic_metrics(&mut self) -> Result<bool, TextError> {
        let refresh = self
            .tree
            .refresh_label_intrinsic_layouts(self.text.as_mut(), &self.resources)?;
        self.text_layout_stats.label_style_nodes_visited = self
            .text_layout_stats
            .label_style_nodes_visited
            .saturating_add(refresh.style_nodes_visited);
        self.text_layout_stats.label_figures_refreshed = self
            .text_layout_stats
            .label_figures_refreshed
            .saturating_add(refresh.figures_refreshed);
        let changed = refresh.changed;
        let any_changed = !changed.is_empty();
        for figure in changed {
            self.tree.mark_invalid(&mut self.updates, figure);
            self.tree.repaint(&mut self.updates, figure, None);
        }
        Ok(any_changed)
    }

    fn refresh_label_presentations(&mut self) -> Result<bool, TextError> {
        let changed = self
            .tree
            .refresh_label_presentations(self.text.as_mut(), &mut self.updates)?;
        let any_changed = !changed.is_empty();
        for figure in changed {
            self.refresh_label_icon_geometry(figure);
            self.tree.repaint(&mut self.updates, figure, None);
        }
        Ok(any_changed)
    }

    fn refresh_text_flow_layouts(&mut self) -> Result<bool, TextError> {
        let refresh = self
            .tree
            .refresh_text_flow_layouts(self.text.as_mut(), &mut self.updates)?;
        self.text_layout_stats.text_flow_style_nodes_visited = self
            .text_layout_stats
            .text_flow_style_nodes_visited
            .saturating_add(refresh.style_nodes_visited);
        self.text_layout_stats.text_flow_figures_refreshed = self
            .text_layout_stats
            .text_flow_figures_refreshed
            .saturating_add(refresh.figures_refreshed);
        let changed = refresh.changed;
        let any_changed = !changed.is_empty();
        for figure in changed {
            self.tree.mark_invalid(&mut self.updates, figure);
            self.tree.repaint(&mut self.updates, figure, None);
        }
        Ok(any_changed)
    }

    fn refresh_label_icon_geometry(&mut self, figure: FigureId) {
        let key = AnchorGeometryKey::icon();
        let next = self
            .tree
            .label(figure)
            .and_then(LabelFigure::icon_bounds)
            .filter(|bounds| bounds.width > 0.0 && bounds.height > 0.0)
            .map(AnchorGeometry::Rectangle);
        let map_key = (figure, key.clone());
        let changed = match next {
            Some(geometry) if self.anchor_geometries.get(&map_key) != Some(&geometry) => {
                self.anchor_geometries.insert(map_key, geometry);
                true
            }
            Some(_) => false,
            None => self.anchor_geometries.remove(&map_key).is_some(),
        };
        if changed
            && let Err(error) = self
                .connections
                .invalidate_dependency(&DependencySubject::NamedAnchorRegion(figure, key))
        {
            self.connection_error = Some(error);
        }
    }

    fn refresh_image_figures(&mut self) -> bool {
        let changed = self.tree.refresh_image_figures(&self.resources);
        let any_changed = !changed.is_empty();
        for figure in changed {
            self.tree.mark_invalid(&mut self.updates, figure);
            self.tree.repaint(&mut self.updates, figure, None);
        }
        any_changed
    }

    fn refresh_owner_scoped_borders(&mut self) -> Result<bool, TextError> {
        let Some(contents) = self.tree.contents() else {
            return Ok(false);
        };
        let mut figures = vec![contents];
        figures.extend(self.tree.descendant_ids(contents).unwrap_or_default());
        let mut changed = false;
        for figure in figures {
            let has_owner_snapshot = self
                .tree
                .node(figure)
                .and_then(|block| block.figure.get_border())
                .is_some_and(Border::has_owner_snapshot);
            if !has_owner_snapshot {
                continue;
            }
            let style = self
                .tree
                .resolved_style(figure)
                .expect("attached Figure style");
            let font = FontDescriptor::parse(&style.font)?;
            let previous = self.tree.border_snapshot(figure).cloned();
            let Some(snapshot) = self
                .tree
                .node(figure)
                .and_then(|block| block.figure.get_border())
                .map(|border| {
                    border.resolve_owner_snapshot(previous.as_ref(), &font, self.text.as_mut())
                })
                .transpose()?
                .flatten()
            else {
                continue;
            };
            if self.tree.set_border_snapshot(figure, snapshot) {
                changed = true;
                self.tree.mark_invalid(&mut self.updates, figure);
                self.tree.repaint(&mut self.updates, figure, None);
            }
        }
        Ok(changed)
    }

    /// Returns cumulative text layout refresh work counters.
    pub fn text_layout_stats(&self) -> TextLayoutStats {
        self.text_layout_stats
    }

    pub fn set_contents(
        &mut self,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| runtime.set_contents_inner(figure))
    }

    fn set_contents_inner(
        &mut self,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.connections
            .validate_disposal()
            .map_err(|_| RuntimeMutationError::Rejected)?;
        let previous = self.tree.contents();
        let id = self
            .tree
            .try_add_child_to(self.tree.synthetic_root(), figure)
            .map_err(RuntimeMutationError::Graph)?;
        if let Some(previous) = previous {
            self.dispose_subtree(previous)?;
        }
        self.tree.designate_contents(id);
        self.accessibility.reset();
        self.register_label_icon_dependency(id);
        self.register_image_figure_dependency(id);
        if let Err(error) = self.connections.invalidate_all() {
            self.connection_error = Some(error);
        }
        self.register_layered_pane(id);
        self.retain_interactive_figures();
        self.full_redraw_pending = true;
        let validation_root = if self.logical_viewport.is_some() {
            self.tree.synthetic_root()
        } else {
            id
        };
        self.tree.mark_invalid(&mut self.updates, validation_root);
        self.tree.repaint(&mut self.updates, id, None);
        self.tree
            .complete_attachment(id, self.tree.synthetic_root());
        Ok(id)
    }

    pub(crate) fn add_figure(
        &mut self,
        parent: FigureId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| runtime.add_figure_checked(parent, figure))
    }

    fn insert_figure(
        &mut self,
        parent: FigureId,
        index: usize,
        figure: Box<dyn Figure>,
        constraint: Option<Box<dyn LayoutConstraint>>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            runtime.validate_attached_figure(parent)?;
            if runtime.tree.is_layered_pane(parent) {
                return Err(RuntimeMutationError::LayeredParent(parent));
            }
            let id = match constraint {
                Some(constraint) => runtime
                    .tree
                    .insert_child_with_constraint_at(parent, index, figure, constraint)
                    .map_err(|error| match error {
                        crate::tree::ChildInsertionError::Graph(error) => {
                            RuntimeMutationError::Graph(error)
                        }
                        crate::tree::ChildInsertionError::Layout(error) => {
                            RuntimeMutationError::Layout(error)
                        }
                    })?,
                None => runtime.tree.insert_child_at(parent, index, figure)?,
            };
            runtime
                .tree
                .invalidate_child_insertion(&mut runtime.updates, parent, id);
            runtime.complete_figure_insertion(parent, id);
            Ok(id)
        })
    }

    /// Adds a Viewport Figure and returns its transactional handle.
    pub fn add_viewport(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ViewportHandle, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            runtime.validate_attached_figure(parent)?;
            if runtime.tree.is_layered_pane(parent) {
                return Err(RuntimeMutationError::LayeredParent(parent));
            }
            let viewport = runtime.tree.add_viewport_to(parent, bounds)?;
            runtime.tree.mark_invalid(&mut runtime.updates, parent);
            runtime
                .tree
                .mark_invalid(&mut runtime.updates, viewport.figure_id());
            runtime
                .tree
                .repaint(&mut runtime.updates, viewport.figure_id(), None);
            runtime
                .tree
                .complete_attachment(viewport.figure_id(), parent);
            Ok(viewport)
        })
    }

    fn add_figure_checked(
        &mut self,
        parent: FigureId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.validate_attached_figure(parent)?;
        if self.tree.is_layered_pane(parent) {
            return Err(RuntimeMutationError::LayeredParent(parent));
        }
        self.add_figure_inner(parent, figure)
    }

    fn add_figure_inner(
        &mut self,
        parent: FigureId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        let id = self.tree.try_add_child(&mut self.updates, parent, figure)?;
        self.complete_figure_insertion(parent, id);
        Ok(id)
    }

    fn complete_figure_insertion(&mut self, parent: FigureId, id: FigureId) {
        self.register_layered_pane(id);
        self.register_label_icon_dependency(id);
        self.register_image_figure_dependency(id);
        self.tree.complete_attachment(id, parent);
    }

    pub fn add_layered_pane(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<LayeredPaneHandle<'_>, LayerError> {
        if self.faulted {
            return Err(LayerError::Faulted);
        }
        let pane_id = self.guarded(|runtime| runtime.add_layered_pane_inner(parent, bounds))?;
        Ok(LayeredPaneHandle::new(pane_id, self))
    }

    fn add_layered_pane_inner(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<FigureId, LayerError> {
        let pane_id = self.tree.try_add_child_to(
            parent,
            Box::new(LayeredPane::new(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
            )),
        )?;
        self.register_layered_pane(pane_id);
        self.tree.mark_invalid(&mut self.updates, parent);
        self.tree.mark_invalid(&mut self.updates, pane_id);
        self.tree.repaint(&mut self.updates, pane_id, None);
        self.tree.complete_attachment(pane_id, parent);
        Ok(pane_id)
    }

    pub fn layered_pane(&mut self, pane_id: FigureId) -> Result<LayeredPaneHandle<'_>, LayerError> {
        if self.faulted {
            return Err(LayerError::Faulted);
        }
        self.guarded(|runtime| runtime.validate_layered_pane_handle(pane_id))?;
        Ok(LayeredPaneHandle::new(pane_id, self))
    }

    fn validate_layered_pane_handle(&mut self, pane_id: FigureId) -> Result<(), LayerError> {
        if !self.tree.is_layered_pane(pane_id) || !self.tree.is_attached(pane_id) {
            return Err(LayerError::UnknownPane);
        }
        self.layered_panes.entry(pane_id).or_default();
        Ok(())
    }

    pub(crate) fn add_layer(
        &mut self,
        pane_id: FigureId,
        figure: Box<dyn Figure>,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<FigureId, LayerError> {
        if self.faulted {
            return Err(LayerError::Faulted);
        }
        self.guarded(|runtime| runtime.add_layer_inner(pane_id, figure, key, placement))
    }

    fn add_layer_inner(
        &mut self,
        pane_id: FigureId,
        figure: Box<dyn Figure>,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<FigureId, LayerError> {
        self.ensure_layered_pane(pane_id)?;
        if figure.layer().is_none() {
            return Err(LayerError::NotLayer);
        }
        let child_is_layered_pane = figure
            .container()
            .is_some_and(|container| container.child_policy() == crate::ChildPolicy::Layered);
        let state = self
            .layered_panes
            .get(&pane_id)
            .ok_or(LayerError::InconsistentState)?;
        if state.contains_key(&key) {
            return Err(LayerError::DuplicateKey);
        }
        let target_index = self.resolve_layer_index(pane_id, state, &placement, None)?;
        let child = self
            .tree
            .add_layer_child(&mut self.updates, pane_id, figure)?;
        let last_index = self
            .tree
            .child_order(pane_id)
            .map_or(0, |order| order.len().saturating_sub(1));
        if target_index != last_index {
            let _ = self.tree.move_child_to_index(pane_id, child, target_index);
        }
        self.layered_panes
            .get_mut(&pane_id)
            .expect("validated layered pane state must exist")
            .insert(key, child);
        if child_is_layered_pane {
            self.register_layered_pane(child);
        }
        self.tree.complete_attachment(child, pane_id);
        Ok(child)
    }

    pub(crate) fn remove_layer(
        &mut self,
        pane_id: FigureId,
        key: &LayerKey,
    ) -> Result<FigureId, LayerError> {
        if self.faulted {
            return Err(LayerError::Faulted);
        }
        self.guarded(|runtime| runtime.remove_layer_inner(pane_id, key))
    }

    fn remove_layer_inner(
        &mut self,
        pane_id: FigureId,
        key: &LayerKey,
    ) -> Result<FigureId, LayerError> {
        self.ensure_layered_pane(pane_id)?;
        let child = self.layer(pane_id, key)?;
        if self.tree.parent_id(child) != Some(pane_id) {
            return Err(LayerError::InconsistentState);
        }
        self.dispose_subtree(child)
            .map_err(|_| LayerError::InconsistentState)?;
        Ok(child)
    }

    pub(crate) fn move_layer(
        &mut self,
        pane_id: FigureId,
        key: &LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        if self.faulted {
            return Err(LayerError::Faulted);
        }
        self.guarded(|runtime| runtime.move_layer_inner(pane_id, key, placement))
    }

    fn move_layer_inner(
        &mut self,
        pane_id: FigureId,
        key: &LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        self.ensure_layered_pane(pane_id)?;
        let state = self
            .layered_panes
            .get(&pane_id)
            .ok_or(LayerError::InconsistentState)?;
        let child = state.layer(key).ok_or(LayerError::UnknownKey)?;
        let target_index = self.resolve_layer_index(pane_id, state, &placement, Some(child))?;
        let Some(old_index) = self.tree.child_z_index(pane_id, child) else {
            return Err(LayerError::InconsistentState);
        };
        if old_index == target_index {
            return Ok(false);
        }
        if !self.tree.move_child_to_index(pane_id, child, target_index) {
            return Err(LayerError::InconsistentState);
        }
        self.tree.repaint(&mut self.updates, pane_id, None);
        Ok(true)
    }

    pub(crate) fn reparent_layer(
        &mut self,
        child: FigureId,
        new_pane: FigureId,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        if self.faulted {
            return Err(LayerError::Faulted);
        }
        self.guarded(|runtime| runtime.reparent_layer_inner(child, new_pane, key, placement))
    }

    fn reparent_layer_inner(
        &mut self,
        child: FigureId,
        new_pane: FigureId,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        self.ensure_layered_pane(new_pane)?;
        if !self.tree.is_layer_figure(child) {
            return Err(LayerError::NotLayer);
        }
        let target_state = self
            .layered_panes
            .get(&new_pane)
            .ok_or(LayerError::InconsistentState)?;
        if target_state.contains_key(&key) {
            return Err(LayerError::DuplicateKey);
        }
        if target_state.contains_child(child) {
            return Err(LayerError::AlreadyLayerMember);
        }
        let target_index = self.resolve_layer_index(new_pane, target_state, &placement, None)?;
        let old_parent = self
            .tree
            .parent_id(child)
            .ok_or(LayerError::InconsistentState)?;
        if self.tree.is_layered_pane(old_parent)
            && self
                .layered_panes
                .get(&old_parent)
                .is_none_or(|state| !state.contains_child(child))
        {
            return Err(LayerError::InconsistentState);
        }
        if !self
            .tree
            .reparent_layer_child(&mut self.updates, child, new_pane)
        {
            return Ok(false);
        }
        if let Some(state) = self.layered_panes.get_mut(&old_parent) {
            state.remove_child(child);
        }
        self.layered_panes
            .get_mut(&new_pane)
            .expect("validated layered pane state must exist")
            .insert(key, child);
        let last_index = self
            .tree
            .child_order(new_pane)
            .map_or(0, |order| order.len().saturating_sub(1));
        if target_index != last_index {
            let _ = self.tree.move_child_to_index(new_pane, child, target_index);
        }
        self.invalidate_connection_figure_change(child, true);
        self.tree.complete_detachment(child, old_parent);
        self.tree.complete_attachment(child, new_pane);
        Ok(true)
    }

    pub(crate) fn layer(&self, pane_id: FigureId, key: &LayerKey) -> Result<FigureId, LayerError> {
        self.layered_panes
            .get(&pane_id)
            .ok_or(LayerError::InconsistentState)?
            .layer(key)
            .ok_or(LayerError::UnknownKey)
    }

    pub(crate) fn layer_key(
        &self,
        pane_id: FigureId,
        child: FigureId,
    ) -> Result<LayerKey, LayerError> {
        self.layered_panes
            .get(&pane_id)
            .ok_or(LayerError::InconsistentState)?
            .key(child)
            .cloned()
            .ok_or(LayerError::UnknownKey)
    }

    pub(crate) fn layer_ids(&self, pane_id: FigureId) -> Result<Vec<FigureId>, LayerError> {
        let state = self
            .layered_panes
            .get(&pane_id)
            .ok_or(LayerError::InconsistentState)?;
        let order = self
            .tree
            .child_order(pane_id)
            .ok_or(LayerError::UnknownPane)?;
        if order.len() != state.len() || order.iter().any(|child| !state.contains_child(*child)) {
            return Err(LayerError::InconsistentState);
        }
        Ok(order)
    }

    pub(crate) fn remove_figure(
        &mut self,
        parent: FigureId,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| runtime.remove_figure_checked(parent, child))
    }

    fn remove_figure_checked(
        &mut self,
        parent: FigureId,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(parent)?;
        self.validate_attached_figure(child)?;
        if self.tree.is_layered_pane(parent) {
            return Err(RuntimeMutationError::LayeredParent(parent));
        }
        if self.tree.parent_id(child) != Some(parent) {
            return Err(RuntimeMutationError::InvalidParentRelation { parent, child });
        }
        self.dispose_subtree(child)?;
        Ok(true)
    }

    pub fn is_faulted(&self) -> bool {
        self.faulted
    }

    pub fn dispose_subtree(&mut self, root: FigureId) -> Result<(), RuntimeMutationError> {
        self.validate_attached_figure(root)?;
        let ids = self
            .tree
            .disposal_ids(root)
            .map_err(RuntimeMutationError::Graph)?;
        let parent = self
            .tree
            .parent_id(root)
            .ok_or(RuntimeMutationError::Graph(
                crate::GraphMutationError::InvalidParentRelation,
            ))?;
        self.connections
            .validate_disposal()
            .map_err(|_| RuntimeMutationError::Rejected)?;
        self.guarded(|runtime| {
            let damage = runtime.updates.freeze_removed_damage(&runtime.tree, &ids);
            let removed: std::collections::HashSet<_> = ids.iter().copied().collect();
            let focused = runtime
                .interaction
                .focus_owner()
                .filter(|id| removed.contains(id));
            let retired = runtime.tree.extract_subtree(&ids, &mut runtime.updates);
            runtime.interaction.forget_figures(&removed);
            runtime.updates.forget_figures(&removed);
            let listeners = runtime.updates.retire_listeners(&removed);
            let connections = runtime.connections.retire_figures(&removed);
            runtime
                .resources
                .retain_dependencies(|id| !removed.contains(&id));
            runtime.layered_panes.retain(|id, state| {
                state.retain_children(|child| !removed.contains(&child));
                !removed.contains(id)
            });
            runtime
                .anchor_geometries
                .retain(|(id, _), _| !removed.contains(id));
            for rect in damage {
                runtime.updates.add_frozen_surface_region(rect);
            }
            runtime.tree.mark_invalid(&mut runtime.updates, parent);
            if let Some(node) = retired.nodes.iter().find(|node| Some(node.id) == focused) {
                crate::EventContext::retired_focus_lost(
                    node,
                    &mut runtime.mutations,
                    &mut runtime.updates,
                    &mut runtime.tree,
                );
            }
            retired.complete();
            drop(connections);
            drop(listeners);
        });
        Ok(())
    }

    fn guarded<T>(&mut self, operation: impl FnOnce(&mut Self) -> T) -> T {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(self))) {
            Ok(value) => value,
            Err(payload) => {
                self.faulted = true;
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn guarded_runtime_mutation<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, RuntimeMutationError>,
    ) -> Result<T, RuntimeMutationError> {
        if self.faulted {
            return Err(RuntimeMutationError::Faulted);
        }
        self.guarded(operation)
    }

    fn guarded_shape_mutation<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, ShapeMutationError>,
    ) -> Result<T, ShapeMutationError> {
        if self.faulted {
            return Err(ShapeMutationError::Faulted);
        }
        self.guarded(operation)
    }

    fn guarded_widget_mutation<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, WidgetError>,
    ) -> Result<T, WidgetError> {
        if self.faulted {
            return Err(WidgetError::Faulted);
        }
        self.guarded(operation)
    }

    fn guarded_connection_mutation<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, ConnectionRuntimeError>,
    ) -> Result<T, ConnectionRuntimeError> {
        if self.faulted {
            return Err(ConnectionRuntimeError::Faulted);
        }
        self.guarded(operation)
    }

    fn guarded_resource_mutation<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, ResourceError>,
    ) -> Result<T, ResourceError> {
        if self.faulted {
            return Err(ResourceError::Faulted);
        }
        self.guarded(operation)
    }

    pub(crate) fn reparent(
        &mut self,
        child: FigureId,
        new_parent: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| runtime.reparent_checked(child, new_parent))
    }

    fn reparent_checked(
        &mut self,
        child: FigureId,
        new_parent: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(child)?;
        self.validate_attached_figure(new_parent)?;
        let old_parent = self.tree.parent_id(child);
        if old_parent.is_some_and(|parent| self.tree.is_layered_pane(parent)) {
            return Err(RuntimeMutationError::LayeredParent(
                old_parent.expect("checked parent"),
            ));
        }
        if self.tree.is_layered_pane(new_parent) {
            return Err(RuntimeMutationError::LayeredParent(new_parent));
        }
        self.reparent_inner(child, new_parent)
    }

    fn reparent_inner(
        &mut self,
        child: FigureId,
        new_parent: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        let old_parent = self
            .tree
            .parent_id(child)
            .ok_or(RuntimeMutationError::DetachedFigure(child))?;
        let prepared_constraint = if old_parent == new_parent {
            None
        } else {
            let scene = FigureTreeSceneRead::new(&self.tree, &self.anchor_geometries);
            self.connections.prepare_reparent_constraint(
                ConnectionId::from_figure(child),
                CoordinateSpace::ChildContent(old_parent),
                CoordinateSpace::ChildContent(new_parent),
                &scene,
            )?
        };
        let changed = self.tree.reparent(&mut self.updates, child, new_parent)?;
        if changed {
            if let Some(constraint) = prepared_constraint {
                self.connections
                    .commit_reparent_constraint(ConnectionId::from_figure(child), constraint);
            }
            self.invalidate_connection_figure_change(child, true);
            self.retain_interactive_figures();
            self.tree.complete_detachment(child, old_parent);
            self.tree.complete_attachment(child, new_parent);
        }
        Ok(changed)
    }

    pub(crate) fn set_layout_manager(
        &mut self,
        container: FigureId,
        manager: Box<dyn LayoutManager>,
    ) -> Result<bool, RuntimeMutationError> {
        self.replace_layout_manager(container, Some(manager))
    }

    pub(crate) fn clear_layout_manager(
        &mut self,
        container: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.replace_layout_manager(container, None)
    }

    fn replace_layout_manager(
        &mut self,
        container: FigureId,
        manager: Option<Box<dyn LayoutManager>>,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            runtime.replace_layout_manager_inner(container, manager)
        })
    }

    fn replace_layout_manager_inner(
        &mut self,
        container: FigureId,
        manager: Option<Box<dyn LayoutManager>>,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(container)?;
        if let Some(manager) = manager.as_deref() {
            self.tree
                .validate_layout_manager_constraints(container, manager)?;
        }
        if !self.tree.replace_layout_manager(container, manager) {
            return Ok(false);
        }
        self.tree.mark_invalid(&mut self.updates, container);
        Ok(true)
    }

    pub(crate) fn set_layout_constraint<C>(
        &mut self,
        child: FigureId,
        constraint: C,
    ) -> Result<bool, RuntimeMutationError>
    where
        C: LayoutConstraint,
    {
        self.set_boxed_layout_constraint(child, Box::new(constraint))
    }

    fn set_boxed_layout_constraint(
        &mut self,
        child: FigureId,
        constraint: Box<dyn LayoutConstraint>,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            runtime.set_boxed_layout_constraint_inner(child, constraint)
        })
    }

    fn set_boxed_layout_constraint_inner(
        &mut self,
        child: FigureId,
        constraint: Box<dyn LayoutConstraint>,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(child)?;
        let parent = self
            .tree
            .parent_id(child)
            .ok_or(RuntimeMutationError::DetachedFigure(child))?;
        if let Some(manager) = self.tree.layout_manager(parent) {
            manager.validate_constraint(parent, child, constraint.as_ref())?;
        }
        if !self.tree.set_boxed_constraint(child, constraint) {
            return Err(RuntimeMutationError::InvalidParentRelation { parent, child });
        }
        self.tree.mark_invalid(&mut self.updates, parent);
        Ok(true)
    }

    pub(crate) fn remove_layout_constraint(
        &mut self,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| runtime.remove_layout_constraint_inner(child))
    }

    fn remove_layout_constraint_inner(
        &mut self,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(child)?;
        let parent = self
            .tree
            .parent_id(child)
            .ok_or(RuntimeMutationError::DetachedFigure(child))?;
        if !self.tree.remove_constraint(child) {
            return Ok(false);
        }
        self.tree.mark_invalid(&mut self.updates, parent);
        Ok(true)
    }

    pub(crate) fn set_preferred_size(
        &mut self,
        figure: FigureId,
        size: (f64, f64),
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Preferred, Some(size))
    }

    pub(crate) fn clear_preferred_size(
        &mut self,
        figure: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Preferred, None)
    }

    pub(crate) fn set_minimum_size(
        &mut self,
        figure: FigureId,
        size: (f64, f64),
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Minimum, Some(size))
    }

    pub(crate) fn clear_minimum_size(
        &mut self,
        figure: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Minimum, None)
    }

    pub(crate) fn set_maximum_size(
        &mut self,
        figure: FigureId,
        size: (f64, f64),
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Maximum, Some(size))
    }

    pub(crate) fn clear_maximum_size(
        &mut self,
        figure: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Maximum, None)
    }

    fn set_size_override(
        &mut self,
        figure: FigureId,
        kind: SizeOverrideKind,
        size: Option<(f64, f64)>,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| runtime.set_size_override_inner(figure, kind, size))
    }

    fn set_size_override_inner(
        &mut self,
        figure: FigureId,
        kind: SizeOverrideKind,
        size: Option<(f64, f64)>,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(figure)?;
        if let Some(size) = size
            && (!size.0.is_finite() || !size.1.is_finite() || size.0 < 0.0 || size.1 < 0.0)
        {
            return Err(RuntimeMutationError::InvalidSize { figure, size });
        }
        let changed = match kind {
            SizeOverrideKind::Preferred => self.tree.set_preferred_size(figure, size),
            SizeOverrideKind::Minimum => self.tree.set_minimum_size(figure, size),
            SizeOverrideKind::Maximum => self.tree.set_maximum_size(figure, size),
        };
        if changed {
            self.tree.mark_invalid(&mut self.updates, figure);
        }
        Ok(changed)
    }

    pub(crate) fn move_child_to_index(
        &mut self,
        parent: FigureId,
        child: FigureId,
        index: usize,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.move_child_to_index_inner(parent, child, index)
        })
    }

    fn move_child_to_index_inner(
        &mut self,
        parent: FigureId,
        child: FigureId,
        index: usize,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_child_order_mutation(parent, child, index)?;
        if self.tree.child_z_index(parent, child) == Some(index) {
            return Ok(false);
        }
        if !self.tree.move_child_to_index(parent, child, index) {
            return Err(RuntimeMutationError::InvalidParentRelation { parent, child });
        }
        self.tree.repaint(&mut self.updates, parent, None);
        if let Err(error) = self.connections.invalidate_all() {
            self.connection_error = Some(error);
        }
        Ok(true)
    }

    /// Atomically replaces the complete direct-child order of a non-layered parent.
    pub(crate) fn set_child_order(
        &mut self,
        parent: FigureId,
        order: &[FigureId],
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| runtime.set_child_order_inner(parent, order))
    }

    fn set_child_order_inner(
        &mut self,
        parent: FigureId,
        order: &[FigureId],
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(parent)?;
        if self.tree.is_layered_pane(parent) {
            return Err(RuntimeMutationError::LayeredParent(parent));
        }
        let current = self
            .tree
            .child_order(parent)
            .expect("attached Figure must have a child list");
        if current.as_slice() == order {
            return Ok(false);
        }
        if current.len() != order.len() {
            return Err(RuntimeMutationError::InvalidChildOrder { parent });
        }
        let mut seen = HashSet::with_capacity(order.len());
        for child in order {
            self.validate_attached_figure(*child)?;
            if self.tree.parent_id(*child) != Some(parent) || !seen.insert(*child) {
                return Err(RuntimeMutationError::InvalidChildOrder { parent });
            }
        }
        if !self.tree.set_child_order(parent, order) {
            return Err(RuntimeMutationError::InvalidChildOrder { parent });
        }
        self.tree.repaint(&mut self.updates, parent, None);
        if let Err(error) = self.connections.invalidate_all() {
            self.connection_error = Some(error);
        }
        Ok(true)
    }

    pub(crate) fn bring_child_to_front(
        &mut self,
        parent: FigureId,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(parent)?;
        self.validate_attached_figure(child)?;
        let child_count = self
            .tree
            .child_order(parent)
            .ok_or(RuntimeMutationError::UnknownOrDisposedFigure(parent))?
            .len();
        let index = child_count
            .checked_sub(1)
            .ok_or(RuntimeMutationError::InvalidParentRelation { parent, child })?;
        self.move_child_to_index(parent, child, index)
    }

    pub(crate) fn send_child_to_back(
        &mut self,
        parent: FigureId,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.move_child_to_index(parent, child, 0)
    }

    pub(crate) fn set_child_clipping_strategy(
        &mut self,
        figure: FigureId,
        strategy: ChildClippingStrategy,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(figure)?;
        if !self.tree.set_child_clipping_strategy(figure, strategy) {
            return Ok(false);
        }
        self.full_redraw_pending = true;
        Ok(true)
    }

    pub fn take_deferred_mutation_errors(&mut self) -> Vec<RuntimeMutationError> {
        self.deferred_mutation_errors.drain(..).collect()
    }

    fn validate_attached_figure(&self, figure: FigureId) -> Result<(), RuntimeMutationError> {
        if self.faulted {
            return Err(RuntimeMutationError::Faulted);
        }
        if figure.namespace() != self.tree.namespace() {
            return Err(RuntimeMutationError::ForeignRuntime(figure));
        }
        if figure == self.tree.synthetic_root() {
            return Err(RuntimeMutationError::SyntheticRootOperation(figure));
        }
        if self.tree.node(figure).is_none() {
            return Err(RuntimeMutationError::UnknownOrDisposedFigure(figure));
        }
        if !self.tree.is_attached(figure) {
            return Err(RuntimeMutationError::DetachedFigure(figure));
        }
        Ok(())
    }

    fn validate_child_order_mutation(
        &self,
        parent: FigureId,
        child: FigureId,
        index: usize,
    ) -> Result<(), RuntimeMutationError> {
        self.validate_attached_figure(parent)?;
        self.validate_attached_figure(child)?;
        if self.tree.is_layered_pane(parent) {
            return Err(RuntimeMutationError::LayeredParent(parent));
        }
        let children = self
            .tree
            .child_order(parent)
            .expect("attached Figure must have a child list");
        if !children.contains(&child) {
            return Err(RuntimeMutationError::InvalidParentRelation { parent, child });
        }
        if index >= children.len() {
            return Err(RuntimeMutationError::InvalidChildIndex {
                parent,
                index,
                child_count: children.len(),
            });
        }
        Ok(())
    }

    pub(crate) fn set_bounds(
        &mut self,
        id: FigureId,
        bounds: crate::geometry::Rectangle,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            if !bounds.x.is_finite()
                || !bounds.y.is_finite()
                || !bounds.width.is_finite()
                || !bounds.height.is_finite()
                || bounds.width < 0.0
                || bounds.height < 0.0
            {
                return Err(RuntimeMutationError::InvalidBounds { figure: id, bounds });
            }
            Ok(runtime.set_bounds_inner(id, bounds))
        })
    }

    fn set_bounds_inner(&mut self, id: FigureId, bounds: crate::geometry::Rectangle) -> bool {
        let is_contents = self.tree.contents() == Some(id);
        let changed = self.tree.set_bounds_with_update(
            &mut self.updates,
            id,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        );
        if changed && is_contents {
            // The synthetic tree root has no drawable background to repair
            // pixels exposed by a moved or resized contents node.
            self.full_redraw_pending = true;
        }
        if changed {
            self.invalidate_connection_figure_change(id, false);
        }
        changed
    }

    pub(crate) fn set_visible(
        &mut self,
        id: FigureId,
        visible: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_visible_inner(id, visible))
        })
    }

    fn set_visible_inner(&mut self, id: FigureId, visible: bool) -> bool {
        let changed = self
            .tree
            .set_visible_with_update(&mut self.updates, id, visible);
        self.retain_interactive_figures();
        changed
    }

    pub(crate) fn set_enabled(
        &mut self,
        id: FigureId,
        enabled: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_enabled_inner(id, enabled))
        })
    }

    fn set_enabled_inner(&mut self, id: FigureId, enabled: bool) -> bool {
        let changed = self
            .tree
            .set_enabled_with_update(&mut self.updates, id, enabled);
        self.retain_interactive_figures();
        self.sync_clickable_visuals();
        changed
    }

    pub(crate) fn set_focusable(
        &mut self,
        id: FigureId,
        focusable: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_focusable_inner(id, focusable))
        })
    }

    fn set_focusable_inner(&mut self, id: FigureId, focusable: bool) -> bool {
        let changed = self.tree.set_focusable(id, focusable);
        if changed {
            self.retain_interactive_figures();
        }
        changed
    }

    pub(crate) fn set_focus_traversable(
        &mut self,
        id: FigureId,
        traversable: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_focus_traversable_inner(id, traversable))
        })
    }

    fn set_focus_traversable_inner(&mut self, id: FigureId, traversable: bool) -> bool {
        let changed = self.tree.set_focus_traversable(id, traversable);
        if changed {
            self.retain_interactive_figures();
        }
        changed
    }

    pub(crate) fn set_figure_style(
        &mut self,
        id: FigureId,
        style: FigureStyle,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_figure_style_inner(id, style))
        })
    }

    fn set_figure_style_inner(&mut self, id: FigureId, style: FigureStyle) -> bool {
        let changed = self
            .tree
            .set_figure_style_with_update(&mut self.updates, id, style);
        if changed {
            self.sync_tooltip();
        }
        changed
    }

    pub(crate) fn set_label_text(
        &mut self,
        id: FigureId,
        text: impl Into<String>,
    ) -> Result<bool, ShapeMutationError> {
        let text = text.into();
        self.guarded_shape_mutation(move |runtime| runtime.set_label_text_inner(id, text))
    }

    fn set_label_text_inner(
        &mut self,
        id: FigureId,
        text: String,
    ) -> Result<bool, ShapeMutationError> {
        let old = self.label_figure(id)?.text().to_string();
        self.tree.mutate_label(
            &mut self.updates,
            id,
            "text",
            PropertyValue::Text(old),
            PropertyValue::Text(text.clone()),
            |label| label.set_text(text),
            true,
        )
    }

    pub fn label_text_layout(&self, id: FigureId) -> Result<&TextLayout, ShapeMutationError> {
        self.label_figure(id)?
            .text_layout()
            .ok_or(ShapeMutationError::WrongCapability(id))
    }

    pub fn text_flow_layout(&self, id: FigureId) -> Result<&TextLayout, ShapeMutationError> {
        if self.tree.figure_bounds(id).is_none() {
            return Err(ShapeMutationError::UnknownFigure(id));
        }
        self.tree
            .text_flow(id)
            .and_then(crate::TextFlowFigure::text_layout)
            .ok_or(ShapeMutationError::WrongCapability(id))
    }

    pub fn text_flow_hit_test(
        &self,
        id: FigureId,
        surface_point: Point,
    ) -> Result<FlowTextPosition, TextFlowQueryError> {
        let flow = self.text_flow_for_query(id)?;
        let transform = self
            .tree
            .surface_to_local_transform(id)
            .ok_or(TextFlowQueryError::NonInvertibleTransform(id))?;
        flow.hit_test_text(transform.transform_point(surface_point))
            .map_err(Into::into)
    }

    pub fn text_flow_caret_geometry(
        &self,
        id: FigureId,
        position: FlowTextPosition,
    ) -> Result<CaretGeometry, TextFlowQueryError> {
        let mut geometry = self.text_flow_local_caret_geometry(id, position)?;
        let transform = self
            .tree
            .local_to_surface_transform(id)
            .ok_or(TextFlowQueryError::NonInvertibleTransform(id))?;
        let mut bounds = geometry.bounds();
        bounds.transform(transform);
        geometry = CaretGeometry::new(bounds, geometry.line_index());
        Ok(geometry)
    }

    /// Returns caret geometry in the TextFlow Figure's node-local coordinates.
    pub fn text_flow_local_caret_geometry(
        &self,
        id: FigureId,
        position: FlowTextPosition,
    ) -> Result<CaretGeometry, TextFlowQueryError> {
        self.text_flow_for_query(id)?
            .caret_geometry(position)
            .map_err(Into::into)
    }

    pub fn text_flow_selection_geometry(
        &self,
        id: FigureId,
        range: FlowTextRange,
    ) -> Result<Vec<SelectionQuad>, TextFlowQueryError> {
        let flow = self.text_flow_for_query(id)?;
        let transform = self
            .tree
            .local_to_surface_transform(id)
            .ok_or(TextFlowQueryError::NonInvertibleTransform(id))?;
        flow.selection_geometry(range)?
            .into_iter()
            .map(|quad| {
                let mut bounds = quad.bounds();
                bounds.transform(transform);
                Ok(SelectionQuad::new(bounds, quad.line_index()))
            })
            .collect()
    }

    pub fn text_flow_move_position(
        &self,
        id: FigureId,
        position: FlowTextPosition,
        movement: TextMovement,
    ) -> Result<FlowTextPosition, TextFlowQueryError> {
        self.text_flow_for_query(id)?
            .move_text_position(position, movement)
            .map_err(Into::into)
    }

    fn text_flow_for_query(
        &self,
        id: FigureId,
    ) -> Result<&crate::TextFlowFigure, TextFlowQueryError> {
        if self.tree.figure_bounds(id).is_none() {
            return Err(TextFlowQueryError::UnknownFigure(id));
        }
        self.tree
            .text_flow(id)
            .ok_or(TextFlowQueryError::WrongCapability(id))
    }

    pub fn label_text(&self, id: FigureId) -> Result<&str, ShapeMutationError> {
        Ok(self.label_figure(id)?.text())
    }

    pub fn title_bar_text_layout(&self, id: FigureId) -> Result<&TextLayout, ShapeMutationError> {
        if self.tree.figure_bounds(id).is_none() {
            return Err(ShapeMutationError::UnknownFigure(id));
        }
        self.tree
            .border_snapshot(id)
            .map(BorderSnapshot::text_layout)
            .ok_or(ShapeMutationError::WrongCapability(id))
    }

    pub(crate) fn set_label_icon(
        &mut self,
        id: FigureId,
        icon: Option<ImageId>,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(|runtime| runtime.set_label_icon_inner(id, icon))
    }

    fn set_label_icon_inner(
        &mut self,
        id: FigureId,
        icon: Option<ImageId>,
    ) -> Result<bool, ShapeMutationError> {
        let old = self.label_figure(id)?.icon();
        let changed = self.tree.mutate_label(
            &mut self.updates,
            id,
            "icon",
            old.map_or(PropertyValue::None, |value| {
                PropertyValue::Text(format!("{value:?}"))
            }),
            icon.map_or(PropertyValue::None, |value| {
                PropertyValue::Text(format!("{value:?}"))
            }),
            |label| label.set_icon(icon),
            true,
        )?;
        if changed {
            if let Some(old) = old {
                let _ = self.resources.remove_dependency(old.resource_id(), id);
            }
            if let Some(icon) = icon {
                self.add_resource_dependency(icon.resource_id(), id)
                    .map_err(|_| ShapeMutationError::WrongCapability(id))?;
            }
        }
        Ok(changed)
    }

    pub(crate) fn set_label_text_placement(
        &mut self,
        id: FigureId,
        placement: TextPlacement,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(|runtime| runtime.set_label_text_placement_inner(id, placement))
    }

    fn set_label_text_placement_inner(
        &mut self,
        id: FigureId,
        placement: TextPlacement,
    ) -> Result<bool, ShapeMutationError> {
        let old = self.label_figure(id)?.text_placement();
        self.tree.mutate_label(
            &mut self.updates,
            id,
            "text_placement",
            PropertyValue::Text(format!("{old:?}")),
            PropertyValue::Text(format!("{placement:?}")),
            |label| label.set_text_placement(placement),
            true,
        )
    }

    pub(crate) fn set_label_alignment(
        &mut self,
        id: FigureId,
        alignment: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(|runtime| runtime.set_label_alignment_inner(id, alignment))
    }

    fn set_label_alignment_inner(
        &mut self,
        id: FigureId,
        alignment: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        let old = self.label_figure(id)?.label_alignment();
        self.tree.mutate_label(
            &mut self.updates,
            id,
            "label_alignment",
            PropertyValue::Text(format!("{old:?}")),
            PropertyValue::Text(format!("{alignment:?}")),
            |label| label.set_label_alignment(alignment),
            false,
        )
    }

    pub(crate) fn set_label_text_alignment(
        &mut self,
        id: FigureId,
        alignment: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(|runtime| runtime.set_label_text_alignment_inner(id, alignment))
    }

    fn set_label_text_alignment_inner(
        &mut self,
        id: FigureId,
        alignment: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        let old = self.label_figure(id)?.text_alignment();
        self.tree.mutate_label(
            &mut self.updates,
            id,
            "text_alignment",
            PropertyValue::Text(format!("{old:?}")),
            PropertyValue::Text(format!("{alignment:?}")),
            |label| label.set_text_alignment(alignment),
            false,
        )
    }

    pub(crate) fn set_label_icon_alignment(
        &mut self,
        id: FigureId,
        alignment: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(|runtime| runtime.set_label_icon_alignment_inner(id, alignment))
    }

    fn set_label_icon_alignment_inner(
        &mut self,
        id: FigureId,
        alignment: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        let old = self.label_figure(id)?.icon_alignment();
        self.tree.mutate_label(
            &mut self.updates,
            id,
            "icon_alignment",
            PropertyValue::Text(format!("{old:?}")),
            PropertyValue::Text(format!("{alignment:?}")),
            |label| label.set_icon_alignment(alignment),
            false,
        )
    }

    pub(crate) fn set_label_icon_text_gap(
        &mut self,
        id: FigureId,
        gap: f64,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(|runtime| runtime.set_label_icon_text_gap_inner(id, gap))
    }

    fn set_label_icon_text_gap_inner(
        &mut self,
        id: FigureId,
        gap: f64,
    ) -> Result<bool, ShapeMutationError> {
        if !gap.is_finite() {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        if gap < 0.0 {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let old = self.label_figure(id)?.icon_text_gap();
        self.tree.mutate_label(
            &mut self.updates,
            id,
            "icon_text_gap",
            PropertyValue::Number(old),
            PropertyValue::Number(gap),
            |label| label.set_icon_text_gap(gap),
            true,
        )
    }

    pub(crate) fn set_image_figure(
        &mut self,
        id: FigureId,
        image: ImageId,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(|runtime| runtime.set_image_figure_inner(id, image))
    }

    fn set_image_figure_inner(
        &mut self,
        id: FigureId,
        image: ImageId,
    ) -> Result<bool, ShapeMutationError> {
        let previous = self
            .tree
            .image_figure(id)
            .map(ImageFigure::image)
            .ok_or_else(|| {
                if self.tree.figure_bounds(id).is_some() {
                    ShapeMutationError::WrongCapability(id)
                } else {
                    ShapeMutationError::UnknownFigure(id)
                }
            })?;
        let changed = self.tree.set_image_figure(&mut self.updates, id, image)?;
        if changed {
            let _ = self.resources.remove_dependency(previous.resource_id(), id);
            self.add_resource_dependency(image.resource_id(), id)
                .map_err(|_| ShapeMutationError::WrongCapability(id))?;
        }
        Ok(changed)
    }

    pub(crate) fn set_image_alignment(
        &mut self,
        id: FigureId,
        alignment: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(|runtime| {
            runtime
                .tree
                .set_image_alignment(&mut runtime.updates, id, alignment)
        })
    }

    pub fn image_display_state(
        &self,
        id: FigureId,
    ) -> Result<ImageDisplayState, ShapeMutationError> {
        self.tree
            .image_figure(id)
            .map(ImageFigure::display_state)
            .ok_or_else(|| {
                if self.tree.figure_bounds(id).is_some() {
                    ShapeMutationError::WrongCapability(id)
                } else {
                    ShapeMutationError::UnknownFigure(id)
                }
            })
    }

    fn label_figure(&self, id: FigureId) -> Result<&LabelFigure, ShapeMutationError> {
        if self.tree.figure_bounds(id).is_none() {
            return Err(ShapeMutationError::UnknownFigure(id));
        }
        self.tree
            .label(id)
            .ok_or(ShapeMutationError::WrongCapability(id))
    }

    fn register_label_icon_dependency(&mut self, id: FigureId) {
        if let Some(icon) = self.tree.label(id).and_then(LabelFigure::icon) {
            let _ = self.resources.add_dependency(icon.resource_id(), id);
        }
    }

    fn register_image_figure_dependency(&mut self, id: FigureId) {
        if let Some(image) = self.tree.image_figure(id).map(ImageFigure::image) {
            let _ = self.resources.add_dependency(image.resource_id(), id);
        }
    }

    pub fn point_list_points(&self, id: FigureId) -> Result<Vec<Point>, ShapeMutationError> {
        if self.tree.figure_bounds(id).is_none() {
            return Err(ShapeMutationError::UnknownFigure(id));
        }
        self.tree
            .point_list_points(id)
            .ok_or(ShapeMutationError::WrongCapability(id))
    }

    pub(crate) fn replace_points(
        &mut self,
        id: FigureId,
        points: Vec<Point>,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(move |runtime| {
            runtime
                .tree
                .commit_point_list(&mut runtime.updates, id, points)
        })
    }

    pub(crate) fn insert_point(
        &mut self,
        id: FigureId,
        index: usize,
        point: Point,
    ) -> Result<bool, ShapeMutationError> {
        let mut points = self.point_list_points(id)?;
        if index > points.len() {
            return Err(ShapeMutationError::PointIndexOutOfRange {
                index,
                len: points.len(),
            });
        }
        points.insert(index, point);
        self.replace_points(id, points)
    }

    pub(crate) fn set_point(
        &mut self,
        id: FigureId,
        index: usize,
        point: Point,
    ) -> Result<bool, ShapeMutationError> {
        let mut points = self.point_list_points(id)?;
        let len = points.len();
        let Some(existing) = points.get_mut(index) else {
            return Err(ShapeMutationError::PointIndexOutOfRange { index, len });
        };
        *existing = point;
        self.replace_points(id, points)
    }

    pub(crate) fn remove_point(
        &mut self,
        id: FigureId,
        index: usize,
    ) -> Result<bool, ShapeMutationError> {
        let mut points = self.point_list_points(id)?;
        if index >= points.len() {
            return Err(ShapeMutationError::PointIndexOutOfRange {
                index,
                len: points.len(),
            });
        }
        points.remove(index);
        self.replace_points(id, points)
    }

    pub(crate) fn clear_points(&mut self, id: FigureId) -> Result<bool, ShapeMutationError> {
        self.replace_points(id, Vec::new())
    }

    pub(crate) fn set_point_list_stroke_width(
        &mut self,
        id: FigureId,
        stroke_width: f64,
    ) -> Result<bool, ShapeMutationError> {
        if !stroke_width.is_finite() {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        if stroke_width < 0.0 {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let stroke = self
            .point_list_stroke_style(id)?
            .with_width(stroke_width)
            .map_err(ShapeMutationError::InvalidStroke)?;
        self.set_point_list_stroke_style(id, stroke)
    }

    fn point_list_stroke_style(
        &self,
        id: FigureId,
    ) -> Result<crate::render::StrokeStyle, ShapeMutationError> {
        if self.is_faulted() {
            return Err(ShapeMutationError::Faulted);
        }
        self.tree.point_list_style(id).ok_or_else(|| {
            if self.tree.figure_bounds(id).is_some() {
                ShapeMutationError::WrongCapability(id)
            } else {
                ShapeMutationError::UnknownFigure(id)
            }
        })
    }

    fn set_point_list_stroke_style(
        &mut self,
        id: FigureId,
        stroke: crate::render::StrokeStyle,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(move |runtime| {
            runtime
                .tree
                .set_point_list_stroke_style(&mut runtime.updates, id, stroke)
        })
    }

    pub(crate) fn set_point_list_line_join(
        &mut self,
        id: FigureId,
        line_join: crate::render::LineJoin,
    ) -> Result<bool, ShapeMutationError> {
        let stroke = self.point_list_stroke_style(id)?.with_join(line_join);
        self.set_point_list_stroke_style(id, stroke)
    }

    pub(crate) fn replace_scalable_polygon_template(
        &mut self,
        id: FigureId,
        template: PointList,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(move |runtime| {
            runtime
                .tree
                .replace_scalable_polygon_template(&mut runtime.updates, id, template)
        })
    }

    pub(crate) fn set_scalable_polygon_scale_mode(
        &mut self,
        id: FigureId,
        mode: PolygonScaleMode,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(move |runtime| {
            runtime
                .tree
                .set_scalable_polygon_scale_mode(&mut runtime.updates, id, mode)
        })
    }

    pub(crate) fn set_scalable_polygon_alignment(
        &mut self,
        id: FigureId,
        horizontal: Alignment,
        vertical: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(move |runtime| {
            runtime.tree.set_scalable_polygon_alignment(
                &mut runtime.updates,
                id,
                horizontal,
                vertical,
            )
        })
    }

    pub(crate) fn replace_text_flow_page(
        &mut self,
        id: FigureId,
        page: FlowPage,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(move |runtime| {
            runtime
                .tree
                .replace_text_flow_page(&mut runtime.updates, id, page)
        })
    }

    pub(crate) fn set_text_flow_wrapping(
        &mut self,
        id: FigureId,
        wrapping: FlowWrapping,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(move |runtime| {
            runtime
                .tree
                .set_text_flow_wrapping(&mut runtime.updates, id, wrapping)
        })
    }

    pub(crate) fn set_border(
        &mut self,
        id: FigureId,
        border: impl Border + 'static,
    ) -> Result<bool, ShapeMutationError> {
        self.replace_border(id, Some(Arc::new(border)))
    }

    pub(crate) fn replace_border(
        &mut self,
        id: FigureId,
        border: Option<Arc<dyn Border>>,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(move |runtime| {
            runtime
                .tree
                .replace_border(&mut runtime.updates, id, border)
        })
    }

    pub(crate) fn set_corner_dimensions(
        &mut self,
        id: FigureId,
        dimensions: Dimension,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(|runtime| {
            runtime
                .tree
                .set_corner_dimensions_with_update(&mut runtime.updates, id, dimensions)
        })
    }

    pub(crate) fn set_triangle_direction(
        &mut self,
        id: FigureId,
        direction: Direction,
    ) -> Result<bool, ShapeMutationError> {
        self.guarded_shape_mutation(|runtime| {
            runtime
                .tree
                .set_triangle_direction_with_update(&mut runtime.updates, id, direction)
        })
    }

    pub(crate) fn set_opaque(
        &mut self,
        id: FigureId,
        opaque: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_opaque_inner(id, opaque))
        })
    }

    fn set_opaque_inner(&mut self, id: FigureId, opaque: bool) -> bool {
        if !self.tree.set_opaque(id, opaque) {
            return false;
        }
        self.tree.repaint(&mut self.updates, id, None);
        true
    }

    pub(crate) fn translate(
        &mut self,
        id: FigureId,
        dx: f64,
        dy: f64,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(id)?;
        let bounds = self
            .tree
            .figure_bounds(id)
            .ok_or(RuntimeMutationError::UnknownOrDisposedFigure(id))?;
        self.set_bounds(
            id,
            crate::geometry::Rectangle::new(
                bounds.x + dx,
                bounds.y + dy,
                bounds.width,
                bounds.height,
            ),
        )
    }

    pub fn add_update_listener(&mut self, listener: Box<dyn UpdateListener>) -> ListenerId {
        self.add_update_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub fn add_update_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn UpdateListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub fn add_figure_listener(&mut self, listener: Box<dyn FigureListener>) -> ListenerId {
        self.add_figure_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub fn add_figure_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn FigureListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_figure_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub fn add_coordinate_listener(&mut self, listener: Box<dyn CoordinateListener>) -> ListenerId {
        self.add_coordinate_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub fn add_coordinate_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn CoordinateListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_coordinate_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub fn add_ancestor_listener(&mut self, listener: Box<dyn AncestorListener>) -> ListenerId {
        self.add_ancestor_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub fn add_ancestor_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn AncestorListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_ancestor_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub fn add_property_listener(
        &mut self,
        listener: Box<dyn PropertyChangeListener>,
    ) -> ListenerId {
        self.add_property_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub fn add_property_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn PropertyChangeListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_property_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub fn add_action_listener(&mut self, listener: Box<dyn ActionListener>) -> ListenerId {
        self.add_action_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub fn add_action_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn ActionListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_action_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub fn add_layout_listener(&mut self, listener: Box<dyn LayoutListener>) -> ListenerId {
        self.add_layout_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub fn add_layout_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn LayoutListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_layout_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub fn add_observation_listener(
        &mut self,
        listener: Box<dyn ObservationListener>,
    ) -> ListenerId {
        self.add_observation_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub fn add_observation_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn ObservationListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_observation_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub fn remove_listener(&mut self, id: ListenerId) -> bool {
        if self.faulted {
            return false;
        }
        self.updates.remove_listener(id)
    }

    fn validate_listener_scope(&self, scope: ListenerScope) -> Result<(), RuntimeMutationError> {
        match scope {
            ListenerScope::Runtime => {
                if self.faulted {
                    Err(RuntimeMutationError::Faulted)
                } else {
                    Ok(())
                }
            }
            ListenerScope::Figure(owner) => self.validate_attached_figure(owner),
        }
    }

    pub fn clickable_snapshot(&self, id: FigureId) -> Result<ClickableSnapshot, WidgetError> {
        if self.tree.figure_bounds(id).is_none() {
            return Err(WidgetError::UnknownFigure(id));
        }
        self.tree
            .clickable_snapshot(id)
            .ok_or(WidgetError::WrongCapability(id))
    }

    pub fn do_click(&mut self, id: FigureId) -> Result<bool, WidgetError> {
        self.guarded_widget_mutation(|runtime| runtime.do_click_inner(id))
    }

    fn do_click_inner(&mut self, id: FigureId) -> Result<bool, WidgetError> {
        if self.tree.figure_bounds(id).is_none() {
            return Err(WidgetError::UnknownFigure(id));
        }
        if self.tree.clickable_snapshot(id).is_none() {
            return Err(WidgetError::WrongCapability(id));
        }
        let changed = self.tree.activate_clickable(&mut self.updates, id);
        self.sync_clickable_visuals();
        Ok(changed)
    }

    pub(crate) fn set_clickable_selected(
        &mut self,
        id: FigureId,
        selected: bool,
    ) -> Result<bool, WidgetError> {
        self.guarded_widget_mutation(|runtime| runtime.set_clickable_selected_inner(id, selected))
    }

    fn set_clickable_selected_inner(
        &mut self,
        id: FigureId,
        selected: bool,
    ) -> Result<bool, WidgetError> {
        let changed = self
            .tree
            .set_clickable_selected(&mut self.updates, id, selected)?;
        self.sync_clickable_visuals();
        Ok(changed)
    }

    pub(crate) fn set_rollover_enabled(
        &mut self,
        id: FigureId,
        enabled: bool,
    ) -> Result<bool, WidgetError> {
        self.guarded_widget_mutation(|runtime| {
            runtime
                .tree
                .set_clickable_rollover_enabled(&mut runtime.updates, id, enabled)
        })
    }

    pub fn has_pending_update(&self) -> bool {
        self.full_redraw_pending
            || (self.session_sync_pending && self.in_flight.is_none())
            || self.updates.is_update_queued()
            || self.resources.has_pending_delta()
            || !self.mutations.is_empty()
            || !self.connections.dirty_connections().is_empty()
            || !self.tree.notification_effects().is_empty()
            || !self.updates.notification_effects().is_empty()
            || self.derivation_epoch != self.stable_epoch
    }

    pub fn stable_query(&self) -> Result<StableSceneQuery<'_>, StableQueryError> {
        if self.faulted {
            return Err(StableQueryError::Faulted);
        }
        if self.has_pending_update() {
            return Err(StableQueryError::NotStable {
                latest_stable_epoch: self.stable_epoch,
            });
        }
        Ok(StableSceneQuery::new(self.stable_epoch, &self.tree))
    }

    pub fn last_validation_error(&self) -> Option<&ValidationError> {
        self.updates.last_validation_error()
    }

    pub fn take_validation_error(&mut self) -> Option<ValidationError> {
        self.updates.take_validation_error()
    }

    pub fn request_full_redraw(&mut self) {
        if !self.faulted {
            self.full_redraw_pending = true;
        }
    }

    /// Queues validation for an attached Figure.
    pub(crate) fn revalidate(&mut self, figure: FigureId) -> Result<(), RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(figure)?;
            runtime.tree.mark_invalid(&mut runtime.updates, figure);
            Ok(())
        })
    }

    /// Queues repaint damage expressed in the Figure's node-local domain.
    pub(crate) fn repaint(
        &mut self,
        figure: FigureId,
        rect: Option<Rectangle>,
    ) -> Result<(), RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(figure)?;
            if let Some(bounds) = rect
                && (!bounds.x.is_finite()
                    || !bounds.y.is_finite()
                    || !bounds.width.is_finite()
                    || !bounds.height.is_finite()
                    || bounds.width < 0.0
                    || bounds.height < 0.0)
            {
                return Err(RuntimeMutationError::InvalidBounds { figure, bounds });
            }
            runtime.tree.repaint(&mut runtime.updates, figure, rect);
            Ok(())
        })
    }

    pub fn dispatch_mouse_moved(&mut self, x: f64, y: f64) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.tooltip_controller
            .set_pointer_position(Point::new(x, y));
        let outcome = self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_moved(ctx, x, y));
        self.sync_tooltip();
        outcome
    }

    pub fn dispatch_mouse_pressed(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.tooltip_controller
            .set_pointer_position(Point::new(x, y));
        let outcome =
            self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_pressed(ctx, x, y, button));
        self.tooltip_controller.dismiss();
        outcome
    }

    pub fn dispatch_mouse_released(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.tooltip_controller
            .set_pointer_position(Point::new(x, y));
        let outcome =
            self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_released(ctx, x, y, button));
        self.tooltip_controller.dismiss();
        outcome
    }

    pub fn dispatch_mouse_double_clicked(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.tooltip_controller
            .set_pointer_position(Point::new(x, y));
        let outcome = self.dispatch(|dispatcher, ctx| {
            dispatcher.dispatch_mouse_double_clicked(ctx, x, y, button)
        });
        self.tooltip_controller.dismiss();
        outcome
    }

    pub fn dispatch_mouse_hover(&mut self, x: f64, y: f64) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.tooltip_controller
            .set_pointer_position(Point::new(x, y));
        let outcome = self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_hover(ctx, x, y));
        self.sync_tooltip();
        outcome
    }

    pub fn dispatch_scroll(&mut self, event: WheelEvent) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        let outcome = self.dispatch(|dispatcher, ctx| dispatcher.dispatch_scroll(ctx, event));
        self.tooltip_controller.dismiss();
        outcome
    }

    pub(crate) fn set_view_location(
        &mut self,
        viewport: &ViewportHandle,
        x: f64,
        y: f64,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(viewport.figure_id())?;
            viewport
                .set_view_location(&mut runtime.tree, &mut runtime.updates, x, y)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub(crate) fn set_zoom_at(
        &mut self,
        zoom: &ZoomManager,
        scale: f64,
        anchor: Option<crate::geometry::Point>,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(zoom.viewport().figure_id())?;
            runtime.validate_attached_figure(zoom.scalable().figure_id())?;
            zoom.set_zoom_at(&mut runtime.tree, &mut runtime.updates, scale, anchor)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub(crate) fn fit_zoom_to_contents(
        &mut self,
        zoom: &ZoomManager,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(zoom.viewport().figure_id())?;
            runtime.validate_attached_figure(zoom.scalable().figure_id())?;
            zoom.fit_all(&mut runtime.tree, &mut runtime.updates)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub fn dispatch_zoom(&mut self, event: ZoomEvent) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_zoom(ctx, event))
    }

    pub fn dispatch_key_pressed(
        &mut self,
        key: Key,
        modifiers: KeyModifiers,
    ) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        let outcome =
            self.dispatch(|dispatcher, ctx| dispatcher.dispatch_key_pressed(ctx, key, modifiers));
        self.tooltip_controller.dismiss();
        outcome
    }

    pub fn dispatch_key_released(
        &mut self,
        key: Key,
        modifiers: KeyModifiers,
    ) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        let outcome =
            self.dispatch(|dispatcher, ctx| dispatcher.dispatch_key_released(ctx, key, modifiers));
        self.tooltip_controller.dismiss();
        outcome
    }

    pub fn request_focus(&mut self, target: FigureId) -> Result<FocusChange, FocusError> {
        if self.faulted {
            return Err(FocusError::Faulted);
        }
        self.validate_direct_focus(target)?;
        let change = self.dispatch(|dispatcher, ctx| dispatcher.set_focus(ctx, Some(target)));
        if matches!(change, FocusChange::Changed { .. }) {
            self.tooltip_controller.dismiss();
        }
        Ok(change)
    }

    pub fn clear_focus(&mut self) -> FocusChange {
        if self.faulted {
            return FocusChange::Unchanged;
        }
        let change = self.dispatch(|dispatcher, ctx| dispatcher.release_focus(ctx));
        if matches!(change, FocusChange::Changed { .. }) {
            self.tooltip_controller.dismiss();
        }
        change
    }

    pub fn release_focus(&mut self) -> FocusChange {
        self.clear_focus()
    }

    pub fn traverse_focus(&mut self, direction: FocusTraversalDirection) -> FocusTraversalOutcome {
        if self.faulted {
            return FocusTraversalOutcome::Boundary;
        }
        self.guarded(|runtime| runtime.traverse_focus_inner(direction))
    }

    fn traverse_focus_inner(
        &mut self,
        direction: FocusTraversalDirection,
    ) -> FocusTraversalOutcome {
        let Some(scope) = self.tree.contents() else {
            return FocusTraversalOutcome::Boundary;
        };
        let candidate = self.focus_traversal_policy.traverse(
            &self.tree,
            scope,
            self.interaction.focus_owner(),
            direction,
        );
        let Some(candidate) = candidate else {
            return FocusTraversalOutcome::Boundary;
        };
        let in_scope = candidate == scope || self.tree.is_ancestor_of(scope, candidate);
        if !in_scope || !self.tree.can_traverse_focus(candidate) {
            return FocusTraversalOutcome::Boundary;
        }

        match self.dispatch(|dispatcher, ctx| dispatcher.set_focus(ctx, Some(candidate))) {
            FocusChange::Changed { .. } => {
                self.tooltip_controller.dismiss();
                FocusTraversalOutcome::Moved(candidate)
            }
            FocusChange::Unchanged => FocusTraversalOutcome::Boundary,
        }
    }

    pub fn set_focus_traversal_policy(&mut self, policy: Box<dyn FocusTraversalPolicy>) {
        if !self.faulted {
            self.focus_traversal_policy = policy;
        }
    }

    pub fn cancel_gestures(&mut self) {
        if self.faulted {
            return;
        }
        self.dispatch(|dispatcher, ctx| dispatcher.cancel_gestures(ctx));
    }

    pub fn pointer_exited(&mut self) {
        if self.faulted {
            return;
        }
        if let Some(point) = self.tooltip_controller.pointer_position() {
            self.dispatch(|dispatcher, ctx| {
                dispatcher.dispatch_pointer_exited(ctx, point.x(), point.y())
            });
        }
        self.tooltip_controller.clear_pointer_position();
    }

    /// Applies all callback effects and structural mutations before returning.
    fn dispatch<R>(
        &mut self,
        action: impl FnOnce(&mut EventDispatcher, &mut SceneDispatchContext<'_>) -> R,
    ) -> R {
        assert!(!self.faulted, "cannot dispatch into a faulted Runtime");
        self.guarded(|runtime| runtime.dispatch_inner(action))
    }

    fn dispatch_inner<R>(
        &mut self,
        action: impl FnOnce(&mut EventDispatcher, &mut SceneDispatchContext<'_>) -> R,
    ) -> R {
        let result = {
            let mut context = SceneDispatchContext::new(
                &mut self.tree,
                &mut self.interaction,
                &mut self.updates,
                &mut self.mutations,
            );
            action(&mut self.interaction_dispatcher, &mut context)
        };
        let mutations = self.mutations.drain();
        self.apply_runtime_mutations(mutations);
        self.retain_interactive_figures();
        self.sync_clickable_visuals();
        result
    }

    fn ensure_layered_pane(&mut self, pane_id: FigureId) -> Result<(), LayerError> {
        if !self.tree.is_layered_pane(pane_id) || !self.tree.is_attached(pane_id) {
            return Err(LayerError::UnknownPane);
        }
        self.layered_panes.entry(pane_id).or_default();
        Ok(())
    }

    fn apply_runtime_mutations(&mut self, mutations: Vec<PendingMutation>) -> bool {
        let mut changed = false;
        for mutation in mutations {
            match self.apply_runtime_mutation(mutation.into_kind()) {
                Ok(mutation_changed) => changed |= mutation_changed,
                Err(error) => self.deferred_mutation_errors.push_back(error),
            }
        }
        changed
    }

    fn apply_runtime_mutation(
        &mut self,
        mutation: PendingMutationKind,
    ) -> Result<bool, RuntimeMutationError> {
        match mutation {
            PendingMutationKind::SetLayoutManager { container, manager } => {
                self.replace_layout_manager(container, manager)
            }
            PendingMutationKind::SetLayoutConstraint { child, constraint } => {
                self.set_boxed_layout_constraint(child, constraint)
            }
            PendingMutationKind::RemoveLayoutConstraint { child } => {
                self.remove_layout_constraint(child)
            }
            PendingMutationKind::SetSizeOverride { figure, kind, size } => {
                self.set_size_override(figure, kind, size)
            }
            PendingMutationKind::MoveChildToIndex {
                parent,
                child,
                index,
            } => self.move_child_to_index(parent, child, index),
            PendingMutationKind::BringChildToFront { parent, child } => {
                self.bring_child_to_front(parent, child)
            }
            PendingMutationKind::SendChildToBack { parent, child } => {
                self.send_child_to_back(parent, child)
            }
            PendingMutationKind::SetChildClippingStrategy { figure, strategy } => {
                self.set_child_clipping_strategy(figure, strategy)
            }
            PendingMutationKind::AddLayerFigure {
                pane,
                figure,
                key,
                placement,
            } => self
                .add_layer(pane, figure, key, placement)
                .map(|_| true)
                .map_err(|_| RuntimeMutationError::Rejected),
            PendingMutationKind::RemoveLayer { pane, key } => self
                .remove_layer(pane, &key)
                .map(|_| true)
                .map_err(|_| RuntimeMutationError::Rejected),
            PendingMutationKind::MoveLayer {
                pane,
                key,
                placement,
            } => self
                .move_layer(pane, &key, placement)
                .map_err(|_| RuntimeMutationError::Rejected),
            PendingMutationKind::ReparentLayer {
                child,
                new_pane,
                key,
                placement,
            } => self
                .reparent_layer(child, new_pane, key, placement)
                .map_err(|_| RuntimeMutationError::Rejected),
            PendingMutationKind::RemoveChild { parent, child } => self.remove_figure(parent, child),
            PendingMutationKind::Reparent { child, new_parent } => self.reparent(child, new_parent),
            PendingMutationKind::AddChildFigure { parent, figure } => {
                self.add_figure(parent, figure).map(|_| true)
            }
            PendingMutationKind::UpdateComponent(update) => update.apply(self),
        }
    }

    fn initialize_layered_panes(&mut self) {
        let Some(contents) = self.tree.contents() else {
            return;
        };
        let mut ids = vec![contents];
        ids.extend(self.tree.descendant_ids(contents).unwrap_or_default());
        for id in ids {
            self.register_layered_pane(id);
        }
    }

    fn activate_attached_figures(&mut self) {
        for (figure, parent) in self.tree.attached_ids_parent_first() {
            self.tree.complete_attachment(figure, parent);
        }
    }

    fn register_layered_pane(&mut self, pane_id: FigureId) {
        if !self.tree.is_layered_pane(pane_id) {
            return;
        }
        if self.tree.layout_manager(pane_id).is_none() {
            self.tree
                .replace_layout_manager(pane_id, Some(Box::new(StackLayout)));
        }
        self.layered_panes.entry(pane_id).or_default();
    }

    fn resolve_layer_index(
        &self,
        pane_id: FigureId,
        state: &LayeredPaneState,
        placement: &LayerPlacement,
        moving: Option<FigureId>,
    ) -> Result<usize, LayerError> {
        let order = self
            .tree
            .child_order(pane_id)
            .ok_or(LayerError::UnknownPane)?;
        let filtered: Vec<_> = order
            .into_iter()
            .filter(|child| Some(*child) != moving)
            .collect();
        match placement {
            LayerPlacement::Last => Ok(filtered.len()),
            LayerPlacement::Before(reference) | LayerPlacement::After(reference) => {
                let reference_child = state.layer(reference).ok_or(LayerError::UnknownKey)?;
                if Some(reference_child) == moving {
                    return self
                        .tree
                        .child_z_index(pane_id, reference_child)
                        .ok_or(LayerError::InconsistentState);
                }
                let index = filtered
                    .iter()
                    .position(|child| *child == reference_child)
                    .ok_or(LayerError::InconsistentState)?;
                if matches!(placement, LayerPlacement::After(_)) {
                    Ok(index + 1)
                } else {
                    Ok(index)
                }
            }
        }
    }

    fn retain_runtime_state(&mut self) {
        let tree = &self.tree;
        self.layered_panes.retain(|pane, state| {
            if !tree.is_attached(*pane) || !tree.is_layered_pane(*pane) {
                return false;
            }
            state.retain_children(|child| tree.parent_id(child) == Some(*pane));
            true
        });
        self.connections
            .retain_connections(|connection| tree.is_attached(connection));
        self.anchor_geometries
            .retain(|(figure, _), _| tree.is_attached(*figure));
    }

    fn invalidate_connection_figure_change(&mut self, figure: FigureId, topology_changed: bool) {
        if let Err(error) = self
            .connections
            .invalidate_figure_change(figure, topology_changed)
        {
            self.connection_error = Some(error);
        }
    }

    fn retain_interactive_figures(&mut self) {
        self.retain_runtime_state();
        self.interaction.reconcile_non_focus(&self.tree);
        self.resources
            .retain_dependencies(|id| self.tree.is_attached(id));

        let invalid_focus = self
            .interaction
            .focus_owner()
            .is_some_and(|id| !self.tree.can_retain_focus(id));
        if invalid_focus {
            {
                let mut context = SceneDispatchContext::new(
                    &mut self.tree,
                    &mut self.interaction,
                    &mut self.updates,
                    &mut self.mutations,
                );
                self.interaction_dispatcher.set_focus(&mut context, None);
            }
            let mutations = self.mutations.drain();
            self.apply_runtime_mutations(mutations);
            self.interaction.reconcile_non_focus(&self.tree);
            self.resources
                .retain_dependencies(|id| self.tree.is_attached(id));
            self.tooltip_controller.dismiss();
        }
        self.sync_tooltip();
    }

    fn sync_tooltip(&mut self) {
        let source = if self.interaction.captured().is_none() {
            let source = self
                .interaction
                .cursor_target()
                .and_then(|id| self.tree.tooltip_source(id));
            self.interaction
                .set_hover_source(source.as_ref().map(|(id, _)| *id));
            source
        } else {
            None
        };
        self.tooltip_controller.reconcile(source);
    }

    fn sync_clickable_visuals(&mut self) {
        let cursor_target = self.interaction.cursor_target();
        let focus_owner = self.interaction.focus_owner();
        for id in self.tree.clickable_ids() {
            let hovered = cursor_target
                .is_some_and(|target| target == id || self.tree.is_ancestor_of(id, target));
            let visual = ClickableVisualState {
                hovered,
                pressed: self.interaction.is_keyboard_pressed(id)
                    || (hovered && self.interaction.is_pointer_pressed(id)),
                focused: focus_owner == Some(id),
                enabled: self.tree.is_effectively_enabled(id),
            };
            self.tree
                .sync_clickable_visual(&mut self.updates, id, visual);
        }
    }

    fn validate_direct_focus(&self, target: FigureId) -> Result<(), FocusError> {
        if self.tree.node(target).is_none() {
            return Err(FocusError::UnknownFigure(target));
        }
        if !self.tree.is_attached(target) {
            return Err(FocusError::Detached(target));
        }
        if !self.tree.is_effectively_visible(target) {
            return Err(FocusError::Hidden(target));
        }
        if !self.tree.is_effectively_enabled(target) {
            return Err(FocusError::Disabled(target));
        }
        if !self.tree.is_focusable(target) {
            return Err(FocusError::NotFocusable(target));
        }
        Ok(())
    }

    fn invalidate_resource_dependents(&mut self, dependents: Vec<FigureId>) {
        for figure in dependents {
            if self.tree.is_attached(figure) {
                self.tree.mark_invalid(&mut self.updates, figure);
                self.tree.repaint(&mut self.updates, figure, None);
            }
        }
    }
}

fn route_metadata_in_local(metadata: &RouteMetadata, path_bounds: Rectangle) -> RouteMetadata {
    let translate_endpoint = |endpoint: crate::RouteEndpoint| crate::RouteEndpoint {
        reference: Point::new(
            endpoint.reference.x() - path_bounds.x,
            endpoint.reference.y() - path_bounds.y,
        ),
        site: crate::AnchorSite {
            point: Point::new(
                endpoint.site.point.x() - path_bounds.x,
                endpoint.site.point.y() - path_bounds.y,
            ),
            outward_normal: endpoint.site.outward_normal,
        },
    };
    RouteMetadata {
        source: translate_endpoint(metadata.source),
        target: translate_endpoint(metadata.target),
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;
    use crate::render::{FontData, ImageData, ResourceId};
    use crate::{
        Bounded, EventContext, FigureEvent, FigureEventHandler, FocusEvent, FocusEventKind,
        Rectangle, RectangleFigure, ResourceStatus,
    };
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    struct PaintCounterFigure {
        paints: Arc<AtomicUsize>,
    }

    impl Figure for PaintCounterFigure {
        fn name(&self) -> &'static str {
            "PaintCounterFigure"
        }

        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 100.0)
        }

        fn paint_figure(&self, _canvas: &mut NdCanvas) {
            self.paints.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct FocusProbeFigure {
        bounds: Rectangle,
        events: Arc<Mutex<Vec<FocusEvent>>>,
    }

    impl Bounded for FocusProbeFigure {
        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.bounds = Rectangle::new(x, y, width, height);
        }

        fn name(&self) -> &'static str {
            "FocusProbeFigure"
        }
    }

    impl Figure for FocusProbeFigure {
        fn initial_bounds(&self) -> Rectangle {
            self.bounds
        }

        fn name(&self) -> &'static str {
            Bounded::name(self)
        }

        fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
            Some(self)
        }
    }

    impl FigureEventHandler for FocusProbeFigure {
        fn on_focus_gained(&self, event: &FocusEvent, _ctx: &mut EventContext<'_>) -> bool {
            self.events.lock().unwrap().push(*event);
            true
        }

        fn on_focus_lost(&self, event: &FocusEvent, _ctx: &mut EventContext<'_>) -> bool {
            self.events.lock().unwrap().push(*event);
            true
        }
    }

    fn focused_probe_runtime() -> (Runtime, FigureId, FigureId, Arc<Mutex<Vec<FocusEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let probe = tree.add_child_to(
            root,
            Box::new(FocusProbeFigure {
                bounds: Rectangle::new(10.0, 10.0, 20.0, 20.0),
                events: Arc::clone(&events),
            }),
        );
        tree.set_focusable(probe, true);
        let mut runtime = Runtime::new(tree);
        assert!(matches!(
            runtime.request_focus(probe),
            Ok(FocusChange::Changed { .. })
        ));
        (runtime, root, probe, events)
    }

    fn surface(width: u32, height: u32) -> SurfaceInfo {
        SurfaceInfo {
            logical_width: f64::from(width),
            logical_height: f64::from(height),
            pixel_width: width,
            pixel_height: height,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn runtime_imports_builder_validation_state_before_first_frame() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child = tree
            .builder()
            .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
            .unwrap();
        tree.builder()
            .set_layout_manager(root, Box::new(StackLayout::new()))
            .unwrap();

        let mut runtime = Runtime::new(tree);
        assert!(!runtime.tree().is_valid(root));

        assert!(runtime.prepare_frame().is_some());
        assert!(runtime.tree().is_valid(root));
        assert_eq!(
            runtime.tree().figure_bounds(child),
            Some(Rectangle::new(0.0, 0.0, 100.0, 100.0))
        );
    }

    #[test]
    fn first_full_submission_paints_each_figure_once() {
        let paints = Arc::new(AtomicUsize::new(0));
        let mut tree = FigureTree::new();
        let root = tree.builder().set_contents(Box::new(PaintCounterFigure {
            paints: Arc::clone(&paints),
        }));
        tree.builder().validate_subtree(root).unwrap();
        let mut runtime = Runtime::new(tree);

        assert!(matches!(
            runtime
                .prepare_submission_state(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::Ready(_)
        ));
        assert_eq!(paints.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn dispatch_flushes_structural_mutations_before_returning() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            100.0,
            100.0,
            Color::WHITE,
        )));
        let mut runtime = Runtime::new(tree);

        runtime
            .mutations
            .enqueue(crate::runtime::mutation::PendingMutation::add_child_figure(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)),
            ));
        runtime.dispatch(|_, _| {});

        assert_eq!(runtime.tree.child_order(root).unwrap().len(), 1);
        assert!(runtime.has_pending_update());
    }

    #[test]
    fn runtime_owns_interaction_state_separately_from_tree() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");

        runtime.interaction.set_mouse_target(Some(root));

        assert_eq!(runtime.interaction().mouse_target(), Some(root));
        assert_eq!(runtime.tree().contents(), Some(root));
    }

    #[test]
    fn runtime_owns_an_isolated_text_layout_context() {
        let mut first = Runtime::empty();
        let mut second = Runtime::empty();
        first.register_builtin_font(BuiltinFont::Inter).unwrap();
        second.register_builtin_font(BuiltinFont::Inter).unwrap();
        let font = FontDescriptor::default();

        let first_layout = first
            .layout_text("runtime text", &font, TextConstraints::UNBOUNDED)
            .unwrap();
        let second_layout = second
            .layout_text("runtime text", &font, TextConstraints::UNBOUNDED)
            .unwrap();

        assert!(first_layout.width() > 0.0);
        assert_eq!(first_layout.width(), second_layout.width());
        assert_eq!(first.text_revision(), 1);
        assert_eq!(second.text_revision(), 1);
    }

    #[test]
    fn builtin_fonts_require_explicit_runtime_registration() {
        let mut runtime = Runtime::empty();
        let font = FontDescriptor::default();

        assert_eq!(runtime.builtin_font(BuiltinFont::Inter), None);
        assert_eq!(
            runtime.layout_text("text", &font, TextConstraints::UNBOUNDED),
            Err(TextError::NoUsableFont)
        );

        let id = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
        assert_eq!(runtime.builtin_font(BuiltinFont::Inter), Some(id));
        assert_eq!(
            runtime.register_builtin_font(BuiltinFont::Inter).unwrap(),
            id
        );
        assert!(
            !runtime
                .layout_text("text", &font, TextConstraints::UNBOUNDED)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn explicit_font_registration_is_included_in_the_next_submission() {
        let mut runtime = Runtime::empty();
        let id = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();

        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert!(matches!(
            &submission.resources,
            ResourceSync::Snapshot(snapshot)
                if matches!(
                    snapshot.ready.as_slice(),
                    [crate::render::ResourceUpdate {
                        id: resource_id,
                        revision: 1,
                        payload: crate::render::ResourcePayload::Font(_),
                    }] if *resource_id == id.resource_id()
                )
        ));
    }

    #[test]
    fn invalid_font_replacement_preserves_the_ready_revision() {
        let mut runtime = Runtime::empty();
        let id = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
        runtime.resources.take_delta();

        assert_eq!(
            runtime.complete_font(id, FontData::new(vec![1, 2, 3])),
            Err(ResourceError::InvalidFontData)
        );
        assert_eq!(
            runtime.resource_status(id.resource_id()),
            Ok(&ResourceStatus::Ready { revision: 1 })
        );
        assert!(runtime.resources.take_delta().is_empty());

        let layout = runtime
            .layout_text(
                "text",
                &FontDescriptor::default(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        assert!(
            layout
                .glyph_runs()
                .iter()
                .all(|run| run.font.resource_id() == id.resource_id() && run.font.revision() == 1)
        );
    }

    #[test]
    fn removing_a_builtin_font_removes_its_layout_and_backend_resources() {
        let mut runtime = Runtime::empty();
        let id = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
        runtime.resources.take_delta();

        runtime.remove_resource(id.resource_id()).unwrap();

        assert_eq!(runtime.builtin_font(BuiltinFont::Inter), None);
        assert_eq!(
            runtime.layout_text(
                "text",
                &FontDescriptor::default(),
                TextConstraints::UNBOUNDED,
            ),
            Err(TextError::NoUsableFont)
        );
        assert_eq!(
            runtime.resources.take_delta().ops,
            vec![crate::render::ResourceOp::Remove(id.resource_id())]
        );
    }

    #[test]
    fn runtime_accepts_a_non_parley_text_layout_engine() {
        use crate::render::{
            FontFaceRef, GlyphRun, PositionedGlyph, TextLayoutParts, TextLineMetrics,
        };

        struct StubTextEngine {
            face: Option<FontFaceRef>,
        }

        impl TextLayoutEngine for StubTextEngine {
            fn revision(&self) -> u64 {
                42
            }

            fn register_font(
                &mut self,
                resource_id: ResourceId,
                revision: u64,
                _bytes: &[u8],
            ) -> Result<(), TextError> {
                self.face = Some(FontFaceRef::new(resource_id, revision, 0));
                Ok(())
            }

            fn remove_font(&mut self, resource_id: ResourceId) {
                if self
                    .face
                    .as_ref()
                    .is_some_and(|face| face.resource_id() == resource_id)
                {
                    self.face = None;
                }
            }

            fn layout(
                &mut self,
                text: &str,
                font: &FontDescriptor,
                constraints: TextConstraints,
            ) -> Result<TextLayout, TextError> {
                let face = self.face.clone().ok_or(TextError::NoUsableFont)?;
                TextLayout::from_parts(TextLayoutParts {
                    text: text.to_owned(),
                    font: font.clone(),
                    constraints,
                    engine_revision: self.revision(),
                    width: 20.0,
                    full_width: 20.0,
                    height: 12.0,
                    lines: vec![TextLineMetrics {
                        ascent: 8.0,
                        descent: 2.0,
                        leading: 2.0,
                        baseline: 8.0,
                        advance: 20.0,
                    }],
                    glyph_runs: vec![GlyphRun {
                        font: face,
                        font_size: font.size,
                        normalized_coords: Vec::new(),
                        skew_degrees: None,
                        glyphs: vec![PositionedGlyph {
                            id: 1,
                            x: 0.0,
                            y: 8.0,
                        }],
                    }],
                    visible_range: 0..text.len(),
                    truncated: false,
                })
            }
        }

        let mut runtime = Runtime::with_text_layout_engine(
            FigureTree::new(),
            Box::new(StubTextEngine { face: None }),
        );
        let font_id = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();

        assert_eq!(runtime.text_revision(), 42);
        let layout = runtime
            .layout_text(
                "custom",
                &FontDescriptor::default(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        assert!(!layout.is_empty());
        assert_eq!(
            layout.glyph_runs()[0].font.resource_id(),
            font_id.resource_id()
        );
    }

    #[test]
    fn direct_and_traversal_focus_eligibility_are_independent() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let traversal_only = runtime
            .add_figure(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)))
            .expect("valid Runtime mutation");
        let direct_only = runtime
            .add_figure(root, Box::new(RectangleFigure::new(30.0, 0.0, 20.0, 20.0)))
            .expect("valid Runtime mutation");
        runtime
            .set_focus_traversable(traversal_only, true)
            .expect("valid Runtime mutation");
        runtime
            .set_focusable(direct_only, true)
            .expect("valid Runtime mutation");

        assert_eq!(
            runtime.request_focus(traversal_only),
            Err(FocusError::NotFocusable(traversal_only))
        );
        assert_eq!(
            runtime.traverse_focus(FocusTraversalDirection::Forward),
            FocusTraversalOutcome::Moved(traversal_only)
        );
        assert_eq!(runtime.interaction().focus_owner(), Some(traversal_only));
        assert_eq!(
            runtime.traverse_focus(FocusTraversalDirection::Forward),
            FocusTraversalOutcome::Boundary
        );

        assert!(matches!(
            runtime.request_focus(direct_only),
            Ok(FocusChange::Changed { .. })
        ));
        assert_eq!(runtime.interaction().focus_owner(), Some(direct_only));
    }

    #[test]
    fn direct_focus_reports_structured_eligibility_errors_without_changing_owner() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let target = runtime
            .add_figure(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)))
            .expect("valid Runtime mutation");
        runtime
            .set_focusable(root, true)
            .expect("valid Runtime mutation");
        runtime.request_focus(root).unwrap();

        assert_eq!(
            runtime.request_focus(FigureId::null()),
            Err(FocusError::UnknownFigure(FigureId::null()))
        );
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::NotFocusable(target))
        );
        runtime
            .set_focusable(target, true)
            .expect("valid Runtime mutation");
        runtime
            .set_visible(target, false)
            .expect("valid Runtime mutation");
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::Hidden(target))
        );
        runtime
            .set_visible(target, true)
            .expect("valid Runtime mutation");
        runtime
            .set_enabled(target, false)
            .expect("valid Runtime mutation");
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::Disabled(target))
        );
        assert_eq!(runtime.interaction().focus_owner(), Some(root));

        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::UnknownFigure(target))
        );
    }

    #[test]
    fn invalid_focus_owner_emits_one_lost_event_for_hide_disable_and_remove() {
        enum Transition {
            Hide,
            Disable,
            Remove,
        }

        for transition in [Transition::Hide, Transition::Disable, Transition::Remove] {
            let (mut runtime, root, probe, events) = focused_probe_runtime();
            match transition {
                Transition::Hide => assert!(
                    runtime
                        .set_visible(probe, false)
                        .expect("valid Runtime mutation")
                ),
                Transition::Disable => assert!(
                    runtime
                        .set_enabled(probe, false)
                        .expect("valid Runtime mutation")
                ),
                Transition::Remove => assert!(
                    runtime
                        .remove_figure(root, probe)
                        .expect("valid Runtime mutation")
                ),
            }

            assert_eq!(runtime.interaction().focus_owner(), None);
            assert_eq!(
                *events.lock().unwrap(),
                vec![
                    FocusEvent {
                        kind: FocusEventKind::Gained,
                        related_target: None,
                    },
                    FocusEvent {
                        kind: FocusEventKind::Lost,
                        related_target: None,
                    },
                ]
            );
        }
    }

    #[test]
    fn reparent_under_hidden_ancestor_releases_focus() {
        let (mut runtime, root, probe, events) = focused_probe_runtime();
        let hidden_parent = runtime
            .add_figure(root, Box::new(RectangleFigure::new(40.0, 40.0, 40.0, 40.0)))
            .expect("valid Runtime mutation");
        runtime
            .set_visible(hidden_parent, false)
            .expect("valid Runtime mutation");

        assert!(
            runtime
                .reparent(probe, hidden_parent)
                .expect("valid Runtime mutation")
        );

        assert_eq!(runtime.interaction().focus_owner(), None);
        assert_eq!(
            events.lock().unwrap().last(),
            Some(&FocusEvent {
                kind: FocusEventKind::Lost,
                related_target: None,
            })
        );
    }

    #[test]
    fn runtime_rejects_invalid_custom_policy_candidate() {
        struct FixedPolicy(FigureId);

        impl FocusTraversalPolicy for FixedPolicy {
            fn traverse(
                &mut self,
                _tree: &FigureTree,
                _scope: FigureId,
                _current: Option<FigureId>,
                _direction: FocusTraversalDirection,
            ) -> Option<FigureId> {
                Some(self.0)
            }
        }

        let mut tree = FigureTree::new();
        let detached = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        tree.set_focus_traversable(detached, true);
        let scope = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let eligible =
            tree.add_child_to(scope, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
        tree.set_focus_traversable(eligible, true);
        let mut runtime = Runtime::new(tree);
        runtime.set_focus_traversal_policy(Box::new(FixedPolicy(detached)));

        assert_eq!(
            runtime.traverse_focus(FocusTraversalDirection::Forward),
            FocusTraversalOutcome::Boundary
        );
        assert_eq!(runtime.interaction().focus_owner(), None);
    }

    #[test]
    fn cursor_and_tooltip_resolve_from_the_current_pointer_targets() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        runtime
            .set_figure_style(
                root,
                FigureStyle {
                    cursor: Some(CursorIcon::Crosshair),
                    tooltip: Some(Some("root tip".to_string())),
                    ..FigureStyle::default()
                },
            )
            .expect("valid Runtime mutation");

        runtime.dispatch_mouse_moved(50.0, 50.0);

        assert_eq!(runtime.cursor_icon(), CursorIcon::Crosshair);
        assert_eq!(runtime.tooltip().as_deref(), Some("root tip"));

        runtime.dispatch_mouse_moved(150.0, 150.0);
        assert_eq!(runtime.cursor_icon(), CursorIcon::Default);
        assert_eq!(runtime.tooltip(), None);
    }

    #[test]
    fn submission_contains_surface_resources_and_monotonic_frame_id() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![255, 0, 0, 255], 1.0))
            .unwrap();

        let first = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(first.frame_id, FrameId::INITIAL);
        assert_eq!(first.surface, surface(100, 100));
        assert!(matches!(
            &first.resources,
            ResourceSync::Snapshot(snapshot)
                if snapshot.ready.len() == 1
                    && snapshot.ready[0].id == image.resource_id()
        ));
        assert_eq!(first.damage.mode(), DamageMode::Full);
        assert!(runtime.complete_submission(
            first.session_id,
            first.frame_id,
            RenderOutcome::Presented
        ));

        runtime.request_full_redraw();
        let second = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(second.frame_id, first.frame_id.next());
        assert!(runtime.complete_submission(
            second.session_id,
            second.frame_id,
            RenderOutcome::Presented
        ));
        assert!(
            runtime
                .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
                .is_none()
        );
    }

    #[test]
    fn resource_only_completion_produces_a_submission_without_scene_damage() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(
            initial.session_id,
            initial.frame_id,
            RenderOutcome::Presented,
        );

        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![0, 0, 0, 0], 1.0))
            .unwrap();
        let resource_only = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(resource_only.damage.mode(), DamageMode::None);
        assert!(resource_only.commands.is_empty());
        assert!(matches!(
            &resource_only.resources,
            ResourceSync::Delta(delta)
                if matches!(
                    delta.ops.as_slice(),
                    [crate::render::ResourceOp::Upsert(update)]
                        if update.id == image.resource_id()
                )
        ));
    }

    #[test]
    fn resource_completion_invalidates_and_repaints_dependent_figure() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let child = runtime
            .add_figure(root, Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(
            initial.session_id,
            initial.frame_id,
            RenderOutcome::Presented,
        );

        let image = runtime.register_image();
        runtime
            .add_resource_dependency(image.resource_id(), child)
            .unwrap();
        runtime
            .complete_image(image, ImageData::from_rgba(2, 2, vec![255; 2 * 2 * 4], 1.0))
            .unwrap();

        assert_eq!(
            runtime.resource_status(image.resource_id()),
            Ok(&ResourceStatus::Ready { revision: 1 })
        );
        assert!(runtime.has_pending_update());
        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert!(matches!(
            &submission.resources,
            ResourceSync::Delta(delta)
                if matches!(
                    delta.ops.as_slice(),
                    [crate::render::ResourceOp::Upsert(update)]
                        if update.id == image.resource_id()
                )
        ));
        assert_ne!(submission.damage.mode(), DamageMode::None);
    }

    #[test]
    fn replacing_contents_removes_resource_dependencies_from_detached_figures() {
        let mut runtime = Runtime::empty();
        let old_contents = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let image = runtime.register_image();
        runtime
            .add_resource_dependency(image.resource_id(), old_contents)
            .unwrap();
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(
            initial.session_id,
            initial.frame_id,
            RenderOutcome::Presented,
        );

        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let replacement = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(
            replacement.session_id,
            replacement.frame_id,
            RenderOutcome::Presented,
        );
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![255; 4], 1.0))
            .unwrap();
        let resource_only = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(resource_only.damage.mode(), DamageMode::None);
    }

    #[test]
    fn backend_capability_promotes_partial_damage_to_full() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let child = runtime
            .add_figure(root, Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(
            initial.session_id,
            initial.frame_id,
            RenderOutcome::Presented,
        );

        runtime
            .set_bounds(
                child,
                crate::geometry::Rectangle::new(15.0, 15.0, 20.0, 20.0),
            )
            .expect("valid Runtime mutation");
        let partial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(partial.damage.mode(), DamageMode::Partial);
        runtime.complete_submission(
            partial.session_id,
            partial.frame_id,
            RenderOutcome::Presented,
        );

        runtime
            .set_bounds(
                child,
                crate::geometry::Rectangle::new(20.0, 20.0, 20.0, 20.0),
            )
            .expect("valid Runtime mutation");
        let promoted = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::FULL_FRAME_ONLY)
            .unwrap();
        assert_eq!(promoted.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn moving_contents_forces_full_damage_to_clear_exposed_pixels() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(20.0, 20.0, 60.0, 60.0)))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(
            initial.session_id,
            initial.frame_id,
            RenderOutcome::Presented,
        );

        assert!(
            runtime
                .translate(root, 10.0, 10.0)
                .expect("valid Runtime mutation")
        );
        let moved = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(moved.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn surface_change_and_retry_force_full_damage() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let image = runtime.register_image();
        runtime
            .complete_image(
                image,
                ImageData::from_rgba(1, 1, vec![255, 255, 255, 255], 1.0),
            )
            .unwrap();
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert!(runtime.complete_submission(
            initial.session_id,
            initial.frame_id,
            RenderOutcome::Retry
        ));

        let retry = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(retry.damage.mode(), DamageMode::Full);
        assert!(matches!(
            &retry.resources,
            ResourceSync::Snapshot(snapshot)
                if snapshot.ready.len() == 1
                    && snapshot.ready[0].id == image.resource_id()
        ));
        runtime.complete_submission(retry.session_id, retry.frame_id, RenderOutcome::Presented);

        let resized = runtime
            .prepare_submission(surface(120, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(resized.damage.mode(), DamageMode::Full);
        assert!(matches!(
            resized.resources,
            ResourceSync::Delta(ref delta) if delta.is_empty()
        ));
    }

    #[test]
    fn retry_restores_in_flight_resource_updates_before_newer_updates() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(
            initial.session_id,
            initial.frame_id,
            RenderOutcome::Presented,
        );

        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();
        let first_update = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![2; 4], 1.0))
            .unwrap();
        runtime.complete_submission(
            first_update.session_id,
            first_update.frame_id,
            RenderOutcome::Retry,
        );

        let retry = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        let ResourceSync::Delta(delta) = &retry.resources else {
            panic!("incremental retry must carry a delta");
        };
        let revisions: Vec<_> = delta
            .ops
            .iter()
            .filter_map(|operation| match operation {
                crate::render::ResourceOp::Upsert(update) => Some(update.revision),
                crate::render::ResourceOp::Remove(_) => None,
            })
            .collect();

        assert_eq!(revisions, vec![1, 2]);
    }

    #[test]
    fn resource_state_transitions_preserve_order_within_one_delta() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(
            initial.session_id,
            initial.frame_id,
            RenderOutcome::Presented,
        );

        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();
        runtime
            .fail_resource(image.resource_id(), "transient")
            .unwrap();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![2; 4], 1.0))
            .unwrap();

        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        let ResourceSync::Delta(delta) = &submission.resources else {
            panic!("established session must use an incremental delta");
        };
        assert!(matches!(
            delta.ops.as_slice(),
            [
                crate::render::ResourceOp::Upsert(crate::render::ResourceUpdate {
                    revision: 1,
                    ..
                }),
                crate::render::ResourceOp::Remove(id),
                crate::render::ResourceOp::Upsert(crate::render::ResourceUpdate {
                    revision: 2,
                    ..
                }),
            ] if *id == image.resource_id()
        ));
    }

    #[test]
    fn backend_session_reset_rejects_old_completion_and_resends_ready_snapshot() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();
        let old = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        let new_session = runtime.reset_backend_session().unwrap();
        assert_ne!(new_session, old.session_id);
        assert!(!runtime.complete_submission(
            old.session_id,
            old.frame_id,
            RenderOutcome::Presented
        ));

        let replacement = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(replacement.session_id, new_session);
        assert_eq!(replacement.damage.mode(), DamageMode::Full);
        assert!(matches!(
            &replacement.resources,
            ResourceSync::Snapshot(snapshot)
                if snapshot.ready.len() == 1
                    && snapshot.ready[0].id == image.resource_id()
        ));
        assert!(runtime.complete_submission(
            replacement.session_id,
            replacement.frame_id,
            RenderOutcome::Presented
        ));

        runtime.request_full_redraw();
        let redraw = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(redraw.session_id, new_session);
        assert!(matches!(
            redraw.resources,
            ResourceSync::Delta(ref delta) if delta.is_empty()
        ));
    }

    #[test]
    fn backend_sessions_are_namespaced_per_runtime() {
        let first = Runtime::empty().backend_session_id();
        let second = Runtime::empty().backend_session_id();

        assert_ne!(first.runtime_namespace(), second.runtime_namespace());
        assert_eq!(first.generation(), 1);
        assert_eq!(second.generation(), 1);
    }

    #[test]
    fn snapshot_retry_refreezes_current_registry_state() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();
        let snapshot = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        runtime
            .fail_resource(image.resource_id(), "transient")
            .unwrap();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![2; 4], 1.0))
            .unwrap();
        assert!(runtime.complete_submission(
            snapshot.session_id,
            snapshot.frame_id,
            RenderOutcome::Retry
        ));

        let retry = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert!(matches!(
            &retry.resources,
            ResourceSync::Snapshot(snapshot)
                if matches!(
                    snapshot.ready.as_slice(),
                    [crate::render::ResourceUpdate {
                        revision: 2,
                        ..
                    }]
                )
        ));
    }

    #[test]
    fn mutations_after_snapshot_freeze_remain_as_incremental_delta() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();
        let snapshot = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![2; 4], 1.0))
            .unwrap();
        assert!(runtime.complete_submission(
            snapshot.session_id,
            snapshot.frame_id,
            RenderOutcome::Presented
        ));

        let delta = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert!(matches!(
            &delta.resources,
            ResourceSync::Delta(delta)
                if matches!(
                    delta.ops.as_slice(),
                    [crate::render::ResourceOp::Upsert(
                        crate::render::ResourceUpdate { revision: 2, .. }
                    )]
                )
        ));
    }

    #[test]
    fn dpi_change_updates_surface_metadata_and_forces_full_damage() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(
            initial.session_id,
            initial.frame_id,
            RenderOutcome::Presented,
        );

        let hidpi_surface = SurfaceInfo {
            logical_width: 100.0,
            logical_height: 100.0,
            pixel_width: 200,
            pixel_height: 200,
            scale_factor: 2.0,
        };
        let submission = runtime
            .prepare_submission(hidpi_surface, BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(submission.surface, hidpi_surface);
        assert_eq!(submission.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn suspended_surface_preserves_full_redraw_request() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");

        assert!(
            runtime
                .prepare_submission(surface(0, 100), BackendCapabilities::RETAINED_PARTIAL)
                .is_none()
        );
        assert!(runtime.has_pending_update());

        let resumed = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(resumed.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn frame_preparation_reports_distinct_non_ready_states() {
        let mut runtime = Runtime::empty();
        let surface = surface(100, 100);
        let FramePreparation::Ready(first) =
            runtime.prepare_submission_state(surface, BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("initial frame must be ready");
        };
        assert!(matches!(
            runtime.prepare_submission_state(surface, BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::AwaitingCompletion
        ));
        assert!(runtime.complete_submission(
            first.session_id,
            first.frame_id,
            RenderOutcome::Presented
        ));
        assert!(matches!(
            runtime.prepare_submission_state(surface, BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::Idle
        ));
        assert!(matches!(
            runtime.prepare_submission_state(
                SurfaceInfo {
                    pixel_width: 0,
                    ..surface
                },
                BackendCapabilities::RETAINED_PARTIAL
            ),
            FramePreparation::Suspended
        ));
        runtime.faulted = true;
        assert!(matches!(
            runtime.prepare_submission_state(surface, BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::Error(FramePreparationError::Faulted)
        ));
    }

    #[test]
    fn frame_preparation_reports_permanent_backend_capability_mismatch() {
        let mut runtime = Runtime::empty();
        runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
        runtime
            .set_contents(Box::new(crate::LabelFigure::new("unsupported")))
            .expect("valid Runtime mutation");

        let preparation = runtime
            .prepare_submission_state(surface(100, 100), BackendCapabilities::FULL_FRAME_ONLY);
        assert!(
            matches!(
                preparation,
                FramePreparation::Error(FramePreparationError::UnsupportedRenderCapability(
                    UnsupportedRenderCapability {
                        capability: crate::render::RenderCapability::GlyphRuns,
                    }
                ))
            ),
            "unexpected preparation state: {preparation:?}"
        );
    }

    #[test]
    fn submission_notifications_are_flushed_in_causal_order() {
        struct CaptureUpdates(Arc<Mutex<Vec<UpdateEvent>>>);

        impl UpdateListener for CaptureUpdates {
            fn on_update_event(&self, event: UpdateEvent) -> crate::ListenerDirective {
                self.0.lock().unwrap().push(event);
                crate::ListenerDirective::Keep
            }

            fn on_figure_event(&self, _event: FigureEvent) -> crate::ListenerDirective {
                crate::ListenerDirective::Keep
            }

            fn on_notify(&self, _figure_id: FigureId) -> crate::ListenerDirective {
                crate::ListenerDirective::Keep
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let mut runtime = Runtime::empty();
        runtime.add_update_listener(Box::new(CaptureUpdates(events.clone())));
        runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");

        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(
            submission.session_id,
            submission.frame_id,
            RenderOutcome::Presented,
        );

        let events = events.lock().unwrap();
        let validated = events
            .iter()
            .position(|event| matches!(event, UpdateEvent::Validated))
            .unwrap();
        let painted = events
            .iter()
            .position(|event| matches!(event, UpdateEvent::Painted { .. }))
            .unwrap();
        let prepared = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    UpdateEvent::Prepared { frame_id, .. } if *frame_id == submission.frame_id
                )
            })
            .unwrap();
        let submitted = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    UpdateEvent::Submitted { frame_id, .. } if *frame_id == submission.frame_id
                )
            })
            .unwrap();
        assert!(validated < painted);
        assert!(painted < prepared);
        assert!(prepared < submitted);
    }

    #[test]
    fn pending_mutations_are_applied_before_validation_and_recording() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let mut runtime = Runtime::new(tree);
        runtime
            .mutations
            .enqueue(crate::runtime::mutation::PendingMutation::add_child_figure(
                root,
                Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)),
            ));

        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(runtime.tree.child_order(root).unwrap().len(), 1);
        assert!(runtime.tree.is_valid(root));
        assert!(!submission.commands.is_empty());
    }
}

#[cfg(test)]
mod adr014_tests {
    use super::*;
    use crate::{ChopboxAnchor, DirectRouter, RectangleFigure, ResourceStatus};
    use std::{cell::Cell, rc::Rc};

    fn scene() -> (Runtime, FigureId) {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .expect("valid Runtime mutation");
        (runtime, root)
    }

    #[test]
    fn scoped_editor_rejects_an_internally_detached_figure() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child = tree
            .builder()
            .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)))
            .expect("valid FigureTree construction");
        let mut updates = UpdateManager::with_namespace(tree.namespace());
        assert!(tree.remove_child(&mut updates, root, child));
        let mut runtime = Runtime::new(tree);

        assert!(matches!(
            runtime.figure(child),
            Err(RuntimeMutationError::DetachedFigure(id)) if id == child
        ));
    }

    #[test]
    fn foreign_figure_does_not_read_or_modify_matching_local_slot() {
        let (_first, foreign) = scene();
        let (mut second, local) = scene();
        let before = second.tree().figure_bounds(local);
        assert!(second.tree().figure_bounds(foreign).is_none());
        assert_eq!(
            second.set_bounds(foreign, Rectangle::new(1.0, 2.0, 3.0, 4.0)),
            Err(RuntimeMutationError::ForeignRuntime(foreign))
        );
        assert_eq!(second.tree().figure_bounds(local), before);
    }

    #[test]
    fn foreign_registry_handles_do_not_remove_local_entries() {
        let (mut first, first_root) = scene();
        let (mut second, second_root) = scene();
        let foreign_anchor =
            first.register_connection_anchor(Box::new(ChopboxAnchor::new(first_root)));
        let local_anchor =
            second.register_connection_anchor(Box::new(ChopboxAnchor::new(second_root)));
        let foreign_router = first.register_connection_router(Box::new(DirectRouter));
        let local_router = second.register_connection_router(Box::new(DirectRouter));
        assert!(second.remove_connection_anchor(foreign_anchor).is_err());
        assert!(second.remove_connection_router(foreign_router).is_err());
        assert!(second.remove_connection_anchor(local_anchor).is_ok());
        assert!(second.remove_connection_router(local_router).is_ok());
    }

    #[test]
    fn foreign_listener_does_not_unregister_local_subscription() {
        let (mut first, foreign_owner) = scene();
        let (mut second, _) = scene();
        let foreign = first.add_update_listener(Box::new(()));
        let local = second.add_update_listener(Box::new(()));
        assert!(matches!(
            second.add_update_listener_scoped(
                ListenerScope::Figure(foreign_owner),
                Box::new(())
            ),
            Err(RuntimeMutationError::ForeignRuntime(id)) if id == foreign_owner
        ));
        assert!(!second.remove_listener(foreign));
        assert!(second.remove_listener(local));
        assert!(!second.remove_listener(local));
    }

    struct DropProbe(Rc<Cell<usize>>);

    impl Figure for DropProbe {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 10.0, 10.0)
        }

        fn name(&self) -> &'static str {
            "DropProbe"
        }
    }

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn remove_releases_subtree_and_invalidates_old_ids() {
        let (mut runtime, root) = scene();
        let drops = Rc::new(Cell::new(0));
        let child = runtime
            .add_figure(root, Box::new(DropProbe(Rc::clone(&drops))))
            .expect("valid Runtime mutation");
        let grandchild = runtime
            .add_figure(child, Box::new(DropProbe(Rc::clone(&drops))))
            .expect("valid Runtime mutation");
        assert!(
            runtime
                .remove_figure(root, child)
                .expect("valid Runtime mutation")
        );
        assert_eq!(drops.get(), 2);
        assert!(runtime.tree().node(child).is_none());
        assert!(runtime.tree().node(grandchild).is_none());
        assert_eq!(
            runtime.remove_figure(root, child),
            Err(RuntimeMutationError::UnknownOrDisposedFigure(child))
        );
    }

    #[test]
    fn dispose_supports_the_maximum_tree_depth() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)));
        let mut deepest = root;
        for _ in 1..crate::MAX_TREE_DEPTH {
            deepest =
                tree.add_child_to(deepest, Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)));
        }
        assert_eq!(tree.depth(deepest), Some(crate::MAX_TREE_DEPTH));

        let mut runtime = Runtime::new(tree);
        runtime.set_preferred_size(deepest, (2.0, 2.0)).unwrap();
        let initial = runtime
            .prepare_frame()
            .expect("deep tree validation and render must complete");
        assert!(!initial.commands().is_empty());
        assert_eq!(runtime.tree().hit_test_simple((0.5, 0.5)), Some(deepest));
        assert!(
            runtime
                .set_bounds(deepest, Rectangle::new(0.0, 0.0, 0.75, 0.75))
                .expect("valid Runtime mutation")
        );
        assert!(
            runtime.prepare_frame().is_some(),
            "deep mutation must complete validation and render"
        );
        runtime.dispose_subtree(root).unwrap();

        assert!(runtime.tree().contents().is_none());
        assert!(runtime.tree().node(root).is_none());
        assert!(runtime.tree().node(deepest).is_none());
    }

    #[test]
    fn contents_replacement_releases_old_tree_and_preserves_namespace() {
        let (mut runtime, root) = scene();
        let drops = Rc::new(Cell::new(0));
        runtime
            .add_figure(root, Box::new(DropProbe(Rc::clone(&drops))))
            .expect("valid Runtime mutation");
        let replacement = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 40.0, 40.0)))
            .expect("valid Runtime mutation");
        assert_eq!(drops.get(), 1);
        assert!(runtime.tree().node(root).is_none());
        assert_ne!(root, replacement);
        assert_eq!(replacement.namespace(), root.namespace());
        assert_eq!(runtime.resources().namespace(), root.namespace().as_uuid());
        assert_eq!(
            runtime.backend_session_id().runtime_namespace(),
            root.namespace().as_uuid()
        );
    }

    #[test]
    fn disposal_removes_owned_listener_but_preserves_shared_registrations() {
        let (mut runtime, root) = scene();
        let owner = runtime
            .add_figure(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
            .expect("valid Runtime mutation");
        let owned = runtime
            .add_update_listener_scoped(ListenerScope::Figure(owner), Box::new(()))
            .unwrap();
        let global = runtime.add_update_listener(Box::new(()));
        let router = runtime.register_connection_router(Box::new(DirectRouter));
        let image = runtime.register_image();
        runtime
            .add_resource_dependency(image.resource_id(), owner)
            .unwrap();
        runtime.dispose_subtree(owner).unwrap();
        assert!(!runtime.remove_listener(owned));
        assert!(runtime.remove_listener(global));
        assert!(runtime.remove_connection_router(router).is_ok());
        assert!(matches!(
            runtime.resource_status(image.resource_id()),
            Ok(ResourceStatus::Pending)
        ));
    }

    struct PanicOnDetach;
    struct PanicOnAttachLayer;

    impl Figure for PanicOnDetach {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 10.0, 10.0)
        }
        fn name(&self) -> &'static str {
            "PanicOnDetach"
        }
        fn lifecycle(&mut self) -> Option<&mut dyn crate::FigureLifecycle> {
            Some(self)
        }
    }

    impl crate::FigureLifecycle for PanicOnDetach {
        fn on_detached(&mut self, _context: crate::FigureLifecycleContext) {
            panic!("lifecycle fault probe");
        }
    }

    impl Figure for PanicOnAttachLayer {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 10.0, 10.0)
        }

        fn name(&self) -> &'static str {
            "PanicOnAttachLayer"
        }

        fn lifecycle(&mut self) -> Option<&mut dyn crate::FigureLifecycle> {
            Some(self)
        }

        fn layer(&self) -> Option<&dyn crate::Layer> {
            Some(self)
        }
    }

    impl crate::FigureLifecycle for PanicOnAttachLayer {
        fn on_attached(&mut self, _context: crate::FigureLifecycleContext) {
            panic!("layer lifecycle fault probe");
        }
    }

    impl crate::Layer for PanicOnAttachLayer {}

    #[test]
    fn lifecycle_panic_happens_after_extraction_and_blocks_recording() {
        let (mut runtime, root) = scene();
        let child = runtime
            .add_figure(root, Box::new(PanicOnDetach))
            .expect("valid Runtime mutation");
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.dispose_subtree(child).unwrap();
        }));
        assert!(failure.is_err());
        assert!(runtime.is_faulted());
        assert!(runtime.tree().node(child).is_none());
        assert_eq!(runtime.tree().child_order(root), Some(vec![]));
        assert!(runtime.prepare_frame().is_none());
        assert!(matches!(
            runtime.add_figure(root, Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0))),
            Err(RuntimeMutationError::Faulted)
        ));
        assert!(matches!(
            runtime.reparent(root, root),
            Err(RuntimeMutationError::Faulted)
        ));
        assert!(matches!(
            runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0))),
            Err(RuntimeMutationError::Faulted)
        ));
    }

    #[test]
    fn layered_lifecycle_panic_faults_runtime_after_structural_commit() {
        let (mut runtime, root) = scene();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut pane = runtime
                .add_layered_pane(root, Rectangle::new(0.0, 0.0, 100.0, 100.0))
                .unwrap();
            pane.add_layer(
                Box::new(PanicOnAttachLayer),
                LayerKey::new("fault-probe").unwrap(),
                LayerPlacement::Last,
            )
            .unwrap();
        }));
        assert!(failure.is_err());
        assert!(runtime.is_faulted());
        assert!(runtime.prepare_frame().is_none());
    }

    #[test]
    fn disposal_freezes_old_damage_before_the_node_is_removed() {
        let (mut runtime, root) = scene();
        let child = runtime
            .add_figure(root, Box::new(RectangleFigure::new(20.0, 20.0, 10.0, 10.0)))
            .expect("valid Runtime mutation");
        runtime.record_full_frame();
        runtime.updates.dirty_regions.clear();
        runtime.dispose_subtree(child).unwrap();
        let frozen = runtime.updates.frozen_surface_regions.first().unwrap();
        assert!(frozen.x <= 20.0 && frozen.y <= 20.0);
        assert!(frozen.x + frozen.width >= 30.0 && frozen.y + frozen.height >= 30.0);
        assert!(runtime.tree().figure_bounds(child).is_none());
    }

    #[test]
    fn reparent_preserves_identity_and_foreign_checked_mutation_is_rejected() {
        let (mut runtime, root) = scene();
        let left = runtime
            .add_figure(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)))
            .expect("valid Runtime mutation");
        let right = runtime
            .add_figure(root, Box::new(RectangleFigure::new(30.0, 0.0, 20.0, 20.0)))
            .expect("valid Runtime mutation");
        let child = runtime
            .add_figure(left, Box::new(RectangleFigure::new(0.0, 0.0, 5.0, 5.0)))
            .expect("valid Runtime mutation");
        assert!(
            runtime
                .reparent(child, right)
                .expect("valid Runtime mutation")
        );
        assert_eq!(runtime.tree().parent_id(child), Some(right));
        let (_, foreign) = scene();
        assert!(matches!(runtime.clear_preferred_size(foreign),
            Err(RuntimeMutationError::ForeignRuntime(id)) if id == foreign));
    }

    #[test]
    fn synthetic_root_is_rejected_by_public_lifecycle_mutations() {
        let (mut runtime, root) = scene();
        let synthetic_root = runtime.tree.synthetic_root();

        assert_eq!(
            runtime.dispose_subtree(synthetic_root),
            Err(RuntimeMutationError::SyntheticRootOperation(synthetic_root))
        );
        assert_eq!(
            runtime.add_figure(
                synthetic_root,
                Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0))
            ),
            Err(RuntimeMutationError::SyntheticRootOperation(synthetic_root))
        );
        assert_eq!(
            runtime.reparent(root, synthetic_root),
            Err(RuntimeMutationError::SyntheticRootOperation(synthetic_root))
        );
    }

    #[test]
    fn moving_runtime_preserves_registry_identity_and_state() {
        let (mut runtime, _) = scene();
        let image = runtime.register_image();
        let listener = runtime.add_update_listener(Box::new(()));
        let router = runtime.register_connection_router(Box::new(DirectRouter));
        let session = runtime.backend_session_id();

        let mut moved = runtime;

        assert_eq!(moved.backend_session_id(), session);
        assert_eq!(
            moved.resource_status(image.resource_id()),
            Ok(&ResourceStatus::Pending)
        );
        assert!(moved.remove_listener(listener));
        assert!(moved.remove_connection_router(router).is_ok());
    }
}
