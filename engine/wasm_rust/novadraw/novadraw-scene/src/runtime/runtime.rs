use std::{collections::HashMap, sync::Arc};

use novadraw_render::{
    BackendCapabilities, BuiltinFont, DamageMode, FontData, FontDescriptor, FrameId, ImageData,
    ImageDecodeError, NdCanvas, RenderOutcome, RenderSubmission, ResourceDelta, ResourceId,
    SurfaceInfo, TextConstraints, TextError, TextLayout, TextLayoutEngine,
};

use crate::PropertyValue;
use crate::connection::{ConnectionRuntime, FigureTreeSceneRead};
use crate::container::layer::LayeredPaneState;
use crate::figure::border::BorderSnapshot;
use crate::mutation::{PendingMutation, PendingMutationKind};
use crate::{
    ActionListener, Alignment, AnchorGeometry, AnchorGeometryKey, AnchorId, Border,
    ClickableSnapshot, ClickableVisualState, ConnectionAnchor, ConnectionId, ConnectionRouter,
    ConnectionRuntimeError, ConnectionStateSnapshot, CoordinateSpace, CursorIcon,
    DependencySubject, Direction, EventDispatcher, Figure, FigureId, FigureStyle, FigureTree,
    FocusChange, FocusError, FocusTraversalDirection, FocusTraversalOutcome, FocusTraversalPolicy,
    FontId, FreeformError, ImageDisplayState, ImageFigure, ImageId, InteractionState, Key,
    KeyModifiers, LabelFigure, LayerError, LayerKey, LayerPlacement, LayeredPane,
    LayeredPaneHandle, ListenerId, MouseButton, PendingMutations, PropertyChangeListener,
    Rectangle, ResourceError, ResourceRegistry, ResourceStatus, RouteOutput, RouterBinding,
    RouterId, RoutingConstraint, SceneDispatchContext, ShapeMutationError, StackLayout,
    TextPlacement, TreeOrderFocusTraversal, UpdateEvent, UpdateListener, UpdateManager,
    ValidationError, WheelEvent, WidgetError, ZoomEvent,
};
use novadraw_geometry::{Dimension, Vec2};

/// Owns one scene and enforces its input, mutation, and update transaction boundaries.
pub struct Runtime {
    tree: FigureTree,
    interaction: InteractionState,
    interaction_dispatcher: EventDispatcher,
    focus_traversal_policy: Box<dyn FocusTraversalPolicy>,
    updates: UpdateManager,
    mutations: PendingMutations,
    full_redraw_pending: bool,
    next_frame_id: FrameId,
    in_flight: Option<InFlightFrame>,
    last_surface: Option<SurfaceInfo>,
    resources: ResourceRegistry,
    text: Box<dyn TextLayoutEngine>,
    builtin_fonts: HashMap<BuiltinFont, FontId>,
    layered_panes: HashMap<FigureId, LayeredPaneState>,
    connections: ConnectionRuntime,
    anchor_geometries: HashMap<(FigureId, AnchorGeometryKey), AnchorGeometry>,
    connection_error: Option<ConnectionRuntimeError>,
}

struct InFlightFrame {
    id: FrameId,
    resources: ResourceDelta,
}

impl Runtime {
    pub fn new(tree: FigureTree) -> Self {
        Self::with_text_layout_engine(tree, Box::new(novadraw_render::ParleyTextEngine::new()))
    }

    pub fn with_text_layout_engine(tree: FigureTree, text: Box<dyn TextLayoutEngine>) -> Self {
        let mut runtime = Self {
            tree,
            interaction: InteractionState::default(),
            interaction_dispatcher: EventDispatcher,
            focus_traversal_policy: Box::new(TreeOrderFocusTraversal),
            updates: UpdateManager::new(),
            mutations: PendingMutations::new(),
            full_redraw_pending: true,
            next_frame_id: FrameId::INITIAL,
            in_flight: None,
            last_surface: None,
            resources: ResourceRegistry::new(),
            text,
            builtin_fonts: HashMap::new(),
            layered_panes: HashMap::new(),
            connections: ConnectionRuntime::new(),
            anchor_geometries: HashMap::new(),
            connection_error: None,
        };
        runtime.initialize_layered_panes();
        runtime
    }

    pub fn empty() -> Self {
        Self::new(FigureTree::new())
    }

    pub fn tree(&self) -> &FigureTree {
        &self.tree
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
        self.text.layout(text, font, constraints)
    }

    /// Refreshes immutable Label text snapshots from the current Runtime-owned text engine.
    pub fn refresh_label_layouts(&mut self) -> Result<(), TextError> {
        let changed = self
            .tree
            .refresh_label_layouts(self.text.as_mut(), &self.resources)?;
        for figure in changed {
            self.refresh_label_icon_geometry(figure);
            self.tree.mark_invalid(&mut self.updates, figure);
            self.tree.repaint(&mut self.updates, figure, None);
        }
        Ok(())
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

    fn refresh_image_figures(&mut self) {
        for figure in self.tree.refresh_image_figures(&self.resources) {
            self.tree.mark_invalid(&mut self.updates, figure);
            self.tree.repaint(&mut self.updates, figure, None);
        }
    }

    fn refresh_title_bar_borders(&mut self) -> Result<(), TextError> {
        let Some(contents) = self.tree.get_contents() else {
            return Ok(());
        };
        let mut figures = vec![contents];
        figures.extend(self.tree.descendant_ids(contents).unwrap_or_default());
        for figure in figures {
            let Some(title) = self
                .tree
                .get_block(figure)
                .and_then(|block| block.figure.get_border())
                .and_then(Border::title_bar)
                .map(|border| border.title().to_string())
            else {
                continue;
            };
            let style = self
                .tree
                .resolved_style(figure)
                .expect("attached Figure style");
            let font = FontDescriptor::parse(&style.font)?;
            let cached = self
                .tree
                .border_snapshot(figure)
                .map(BorderSnapshot::text_layout)
                .is_some_and(|layout| {
                    layout.key().text() == title
                        && layout.key().font() == &font
                        && layout.key().constraints() == TextConstraints::UNBOUNDED
                        && layout.key().engine_revision() == self.text.revision()
                });
            if cached {
                continue;
            }
            let layout = self
                .text
                .layout(&title, &font, TextConstraints::UNBOUNDED)?;
            let snapshot = self
                .tree
                .get_block(figure)
                .and_then(|block| block.figure.get_border())
                .and_then(Border::title_bar)
                .map(|border| BorderSnapshot::title_bar(border.measure(layout)))
                .expect("title bar border was checked before measurement");
            if self.tree.set_border_snapshot(figure, snapshot) {
                self.tree.mark_invalid(&mut self.updates, figure);
                self.tree.repaint(&mut self.updates, figure, None);
            }
        }
        Ok(())
    }

    pub fn direct_connection_router(&self) -> RouterId {
        self.connections.direct_router()
    }

    pub fn register_connection_anchor(&mut self, anchor: Box<dyn ConnectionAnchor>) -> AnchorId {
        self.connections.register_anchor(anchor)
    }

    pub fn remove_connection_anchor(
        &mut self,
        anchor: AnchorId,
    ) -> Result<Box<dyn ConnectionAnchor>, ConnectionRuntimeError> {
        self.connections.remove_anchor(anchor)
    }

    pub fn set_anchor_geometry(
        &mut self,
        figure: FigureId,
        key: AnchorGeometryKey,
        geometry: AnchorGeometry,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        if !self.tree.is_attached(figure) {
            return Err(ConnectionRuntimeError::UnknownFigure(figure));
        }
        let bounds = geometry.bounds();
        if key.is_border_box()
            || !bounds.x.is_finite()
            || !bounds.y.is_finite()
            || !bounds.width.is_finite()
            || !bounds.height.is_finite()
            || bounds.width <= 0.0
            || bounds.height <= 0.0
        {
            return Err(ConnectionRuntimeError::InvalidAnchorGeometry);
        }
        self.anchor_geometries
            .insert((figure, key.clone()), geometry);
        self.connections
            .invalidate_dependency(&DependencySubject::NamedAnchorRegion(figure, key))
    }

    pub fn anchor_geometry(
        &self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Option<&AnchorGeometry> {
        self.anchor_geometries.get(&(figure, key.clone()))
    }

    pub fn register_connection_router(&mut self, router: Box<dyn ConnectionRouter>) -> RouterId {
        self.connections.register_router(router)
    }

    pub fn remove_connection_router(
        &mut self,
        router: RouterId,
    ) -> Result<Box<dyn ConnectionRouter>, ConnectionRuntimeError> {
        self.connections.remove_router(router)
    }

    pub fn set_connection_layer_router(
        &mut self,
        layer: FigureId,
        router: RouterId,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        if !self.tree.is_attached(layer) {
            return Err(ConnectionRuntimeError::UnknownFigure(layer));
        }
        self.connections.set_layer_router(layer, router)
    }

    pub fn register_connection_state(
        &mut self,
        figure: FigureId,
        source: Option<AnchorId>,
        target: Option<AnchorId>,
        router: RouterBinding,
        constraint: Option<Box<dyn RoutingConstraint>>,
    ) -> Result<ConnectionId, ConnectionRuntimeError> {
        if !self.tree.is_attached(figure) {
            return Err(ConnectionRuntimeError::UnknownFigure(figure));
        }
        if !self.tree.is_connection_figure(figure) {
            return Err(ConnectionRuntimeError::NotConnectionFigure(figure));
        }
        let connection = ConnectionId::from_figure(figure);
        self.connections
            .register_connection(connection, source, target, router, constraint)?;
        Ok(connection)
    }

    pub fn remove_connection_state(
        &mut self,
        connection: ConnectionId,
    ) -> Result<(), ConnectionRuntimeError> {
        self.connections.remove_connection(connection)
    }

    pub fn connection_state(
        &self,
        connection: ConnectionId,
    ) -> Result<ConnectionStateSnapshot, ConnectionRuntimeError> {
        self.connections.state(connection)
    }

    pub fn set_connection_source(
        &mut self,
        connection: ConnectionId,
        source: Option<AnchorId>,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.connections.set_source(connection, source)
    }

    pub fn set_connection_target(
        &mut self,
        connection: ConnectionId,
        target: Option<AnchorId>,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.connections.set_target(connection, target)
    }

    pub fn set_connection_router_binding(
        &mut self,
        connection: ConnectionId,
        router: RouterBinding,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.connections.set_router_binding(connection, router)
    }

    pub fn set_connection_constraint(
        &mut self,
        connection: ConnectionId,
        constraint: Option<Box<dyn RoutingConstraint>>,
    ) -> Result<(), ConnectionRuntimeError> {
        self.connections.set_constraint(connection, constraint)
    }

    pub fn resolve_connection_route(
        &mut self,
        connection: ConnectionId,
        routing_space: CoordinateSpace,
    ) -> Result<RouteOutput, ConnectionRuntimeError> {
        let routing_order: Vec<_> = match routing_space {
            CoordinateSpace::ChildContent(parent) => self
                .tree
                .child_order(parent)
                .unwrap_or_default()
                .into_iter()
                .map(ConnectionId::from_figure)
                .collect(),
            _ => vec![connection],
        };
        let scene = FigureTreeSceneRead::new(&self.tree, &self.anchor_geometries);
        let result = self
            .connections
            .route(connection, routing_space, &scene, &routing_order);
        match result {
            Ok(output) => {
                if !self.tree.commit_connection_route(
                    &mut self.updates,
                    connection.figure(),
                    output.points(),
                ) {
                    return Err(ConnectionRuntimeError::NotConnectionFigure(
                        connection.figure(),
                    ));
                }
                Ok(output)
            }
            Err(error) => {
                self.tree
                    .clear_connection_route(&mut self.updates, connection.figure());
                Err(error)
            }
        }
    }

    pub fn invalidate_connection_dependency(
        &mut self,
        subject: &DependencySubject,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        self.connections.invalidate_dependency(subject)
    }

    pub fn dirty_connections(&self) -> Vec<ConnectionId> {
        self.connections.dirty_connections()
    }

    pub fn last_connection_error(&self) -> Option<&ConnectionRuntimeError> {
        self.connection_error.as_ref()
    }

    pub fn take_connection_error(&mut self) -> Option<ConnectionRuntimeError> {
        self.connection_error.take()
    }

    pub fn set_contents(&mut self, figure: Box<dyn Figure>) -> FigureId {
        let id = self.tree.set_contents(figure);
        self.register_label_icon_dependency(id);
        self.register_image_figure_dependency(id);
        if let Err(error) = self.connections.invalidate_all() {
            self.connection_error = Some(error);
        }
        self.register_layered_pane(id);
        self.retain_interactive_figures();
        self.full_redraw_pending = true;
        self.tree.mark_invalid(&mut self.updates, id);
        self.tree.repaint(&mut self.updates, id, None);
        id
    }

    pub fn add_figure(&mut self, parent: FigureId, figure: Box<dyn Figure>) -> FigureId {
        let id = self.tree.add_child(&mut self.updates, parent, figure);
        self.register_layered_pane(id);
        self.register_label_icon_dependency(id);
        self.register_image_figure_dependency(id);
        id
    }

    pub fn add_layered_pane(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<LayeredPaneHandle<'_>, LayerError> {
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
        Ok(LayeredPaneHandle::new(pane_id, self))
    }

    pub fn layered_pane(&mut self, pane_id: FigureId) -> Result<LayeredPaneHandle<'_>, LayerError> {
        if !self.tree.is_layered_pane(pane_id) || !self.tree.is_attached(pane_id) {
            return Err(LayerError::UnknownPane);
        }
        self.layered_panes.entry(pane_id).or_default();
        Ok(LayeredPaneHandle::new(pane_id, self))
    }

    pub(crate) fn add_layer(
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
        Ok(child)
    }

    pub(crate) fn remove_layer(
        &mut self,
        pane_id: FigureId,
        key: &LayerKey,
    ) -> Result<FigureId, LayerError> {
        self.ensure_layered_pane(pane_id)?;
        let child = self.layer(pane_id, key)?;
        if self.tree.parent_id(child) != Some(pane_id) {
            return Err(LayerError::InconsistentState);
        }
        if !self
            .tree
            .remove_layer_child(&mut self.updates, pane_id, child)
        {
            return Err(LayerError::InconsistentState);
        }
        self.layered_panes
            .get_mut(&pane_id)
            .expect("validated layered pane state must exist")
            .remove_key(key);
        self.invalidate_connection_figure_change(child, true);
        self.retain_runtime_state();
        Ok(child)
    }

    pub(crate) fn move_layer(
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

    pub fn remove_figure(&mut self, parent: FigureId, child: FigureId) -> bool {
        let changed = self.tree.remove_child(&mut self.updates, parent, child);
        if changed {
            self.invalidate_connection_figure_change(child, true);
        }
        self.retain_interactive_figures();
        changed
    }

    pub fn reparent(&mut self, child: FigureId, new_parent: FigureId) -> bool {
        let changed = self.tree.reparent(&mut self.updates, child, new_parent);
        if changed {
            self.invalidate_connection_figure_change(child, true);
            self.retain_interactive_figures();
        }
        changed
    }

    pub fn set_bounds(&mut self, id: FigureId, bounds: novadraw_geometry::Rectangle) -> bool {
        let is_contents = self.tree.get_contents() == Some(id);
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

    pub fn set_visible(&mut self, id: FigureId, visible: bool) -> bool {
        let changed = self
            .tree
            .set_visible_with_update(&mut self.updates, id, visible);
        self.retain_interactive_figures();
        changed
    }

    pub fn set_enabled(&mut self, id: FigureId, enabled: bool) -> bool {
        let changed = self
            .tree
            .set_enabled_with_update(&mut self.updates, id, enabled);
        self.retain_interactive_figures();
        self.sync_clickable_visuals();
        changed
    }

    pub fn set_focusable(&mut self, id: FigureId, focusable: bool) -> bool {
        let changed = self.tree.set_focusable(id, focusable);
        if changed {
            self.retain_interactive_figures();
        }
        changed
    }

    pub fn set_focus_traversable(&mut self, id: FigureId, traversable: bool) -> bool {
        let changed = self.tree.set_focus_traversable(id, traversable);
        if changed {
            self.retain_interactive_figures();
        }
        changed
    }

    pub fn set_figure_style(&mut self, id: FigureId, style: FigureStyle) -> bool {
        self.tree
            .set_figure_style_with_update(&mut self.updates, id, style)
    }

    pub fn set_label_text(
        &mut self,
        id: FigureId,
        text: impl Into<String>,
    ) -> Result<bool, ShapeMutationError> {
        let text = text.into();
        let old = self.label(id)?.text().to_string();
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
        self.label(id)?
            .text_layout()
            .ok_or(ShapeMutationError::WrongCapability(id))
    }

    pub fn label_text(&self, id: FigureId) -> Result<&str, ShapeMutationError> {
        Ok(self.label(id)?.text())
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

    pub fn set_label_icon(
        &mut self,
        id: FigureId,
        icon: Option<ImageId>,
    ) -> Result<bool, ShapeMutationError> {
        let old = self.label(id)?.icon();
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

    pub fn set_label_text_placement(
        &mut self,
        id: FigureId,
        placement: TextPlacement,
    ) -> Result<bool, ShapeMutationError> {
        let old = self.label(id)?.text_placement();
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

    pub fn set_label_alignment(
        &mut self,
        id: FigureId,
        alignment: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        let old = self.label(id)?.label_alignment();
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

    pub fn set_label_text_alignment(
        &mut self,
        id: FigureId,
        alignment: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        let old = self.label(id)?.text_alignment();
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

    pub fn set_label_icon_alignment(
        &mut self,
        id: FigureId,
        alignment: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        let old = self.label(id)?.icon_alignment();
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

    pub fn set_label_icon_text_gap(
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
        let old = self.label(id)?.icon_text_gap();
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

    pub fn set_image_figure(
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

    pub fn set_image_alignment(
        &mut self,
        id: FigureId,
        alignment: Alignment,
    ) -> Result<bool, ShapeMutationError> {
        self.tree
            .set_image_alignment(&mut self.updates, id, alignment)
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

    fn label(&self, id: FigureId) -> Result<&LabelFigure, ShapeMutationError> {
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

    pub fn point_list_points(&self, id: FigureId) -> Result<Vec<Vec2>, ShapeMutationError> {
        if self.tree.figure_bounds(id).is_none() {
            return Err(ShapeMutationError::UnknownFigure(id));
        }
        self.tree
            .point_list_points(id)
            .ok_or(ShapeMutationError::WrongCapability(id))
    }

    pub fn replace_points(
        &mut self,
        id: FigureId,
        points: Vec<Vec2>,
    ) -> Result<bool, ShapeMutationError> {
        self.tree.commit_point_list(&mut self.updates, id, points)
    }

    pub fn insert_point(
        &mut self,
        id: FigureId,
        index: usize,
        point: Vec2,
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

    pub fn set_point(
        &mut self,
        id: FigureId,
        index: usize,
        point: Vec2,
    ) -> Result<bool, ShapeMutationError> {
        let mut points = self.point_list_points(id)?;
        let len = points.len();
        let Some(existing) = points.get_mut(index) else {
            return Err(ShapeMutationError::PointIndexOutOfRange { index, len });
        };
        *existing = point;
        self.replace_points(id, points)
    }

    pub fn remove_point(&mut self, id: FigureId, index: usize) -> Result<bool, ShapeMutationError> {
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

    pub fn clear_points(&mut self, id: FigureId) -> Result<bool, ShapeMutationError> {
        self.replace_points(id, Vec::new())
    }

    pub fn set_border(
        &mut self,
        id: FigureId,
        border: impl Border + 'static,
    ) -> Result<bool, ShapeMutationError> {
        self.replace_border(id, Some(Arc::new(border)))
    }

    pub fn replace_border(
        &mut self,
        id: FigureId,
        border: Option<Arc<dyn Border>>,
    ) -> Result<bool, ShapeMutationError> {
        self.tree.replace_border(&mut self.updates, id, border)
    }

    pub fn set_corner_dimensions(
        &mut self,
        id: FigureId,
        dimensions: Dimension,
    ) -> Result<bool, ShapeMutationError> {
        self.tree
            .set_corner_dimensions_with_update(&mut self.updates, id, dimensions)
    }

    pub fn set_triangle_direction(
        &mut self,
        id: FigureId,
        direction: Direction,
    ) -> Result<bool, ShapeMutationError> {
        self.tree
            .set_triangle_direction_with_update(&mut self.updates, id, direction)
    }

    pub fn set_opaque(&mut self, id: FigureId, opaque: bool) -> bool {
        if !self.tree.set_opaque(id, opaque) {
            return false;
        }
        self.tree.repaint(&mut self.updates, id, None);
        true
    }

    pub fn translate(&mut self, id: FigureId, dx: f64, dy: f64) -> bool {
        let Some(bounds) = self.tree.figure_bounds(id) else {
            return false;
        };
        self.set_bounds(
            id,
            novadraw_geometry::Rectangle::new(
                bounds.x + dx,
                bounds.y + dy,
                bounds.width,
                bounds.height,
            ),
        )
    }

    pub fn into_tree(self) -> FigureTree {
        self.tree
    }

    pub fn add_update_listener(&mut self, listener: Box<dyn UpdateListener>) {
        self.updates.add_listener(listener);
    }

    pub fn add_property_listener(
        &mut self,
        listener: Box<dyn PropertyChangeListener>,
    ) -> ListenerId {
        self.updates.add_property_listener(listener)
    }

    pub fn add_action_listener(&mut self, listener: Box<dyn ActionListener>) -> ListenerId {
        self.updates.add_action_listener(listener)
    }

    pub fn remove_listener(&mut self, id: ListenerId) -> bool {
        self.updates.remove_listener(id)
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

    pub fn set_clickable_selected(
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

    pub fn set_rollover_enabled(
        &mut self,
        id: FigureId,
        enabled: bool,
    ) -> Result<bool, WidgetError> {
        self.tree
            .set_clickable_rollover_enabled(&mut self.updates, id, enabled)
    }

    pub fn has_pending_update(&self) -> bool {
        self.full_redraw_pending
            || self.updates.is_update_queued()
            || self.resources.has_pending_delta()
    }

    pub fn last_validation_error(&self) -> Option<&ValidationError> {
        self.updates.last_validation_error()
    }

    pub fn take_validation_error(&mut self) -> Option<ValidationError> {
        self.updates.take_validation_error()
    }

    pub fn request_full_redraw(&mut self) {
        self.full_redraw_pending = true;
    }

    pub fn register_image(&mut self) -> ImageId {
        self.resources.register_image()
    }

    pub fn register_font(&mut self) -> FontId {
        self.resources.register_font()
    }

    pub fn register_builtin_font(&mut self, builtin: BuiltinFont) -> Result<FontId, ResourceError> {
        if let Some(id) = self.builtin_fonts.get(&builtin) {
            return Ok(*id);
        }
        let id = self.register_font();
        self.complete_font(id, FontData::new(builtin.bytes().to_vec()))?;
        self.builtin_fonts.insert(builtin, id);
        Ok(id)
    }

    pub fn resource_status(
        &self,
        resource_id: ResourceId,
    ) -> Result<&ResourceStatus, ResourceError> {
        self.resources.status(resource_id)
    }

    pub fn add_resource_dependency(
        &mut self,
        resource_id: ResourceId,
        figure: FigureId,
    ) -> Result<(), ResourceError> {
        if !self.tree.is_attached(figure) {
            return Err(ResourceError::UnknownFigure);
        }
        self.resources.add_dependency(resource_id, figure)
    }

    pub fn remove_resource_dependency(
        &mut self,
        resource_id: ResourceId,
        figure: FigureId,
    ) -> Result<bool, ResourceError> {
        self.resources.remove_dependency(resource_id, figure)
    }

    pub fn complete_image(&mut self, id: ImageId, image: ImageData) -> Result<(), ResourceError> {
        let dependents = self.resources.complete_image(id, image)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
    }

    pub fn complete_image_bytes(
        &mut self,
        id: ImageId,
        bytes: &[u8],
        scale: f64,
    ) -> Result<(), ImageDecodeError> {
        match ImageData::decode(bytes, scale) {
            Ok(image) => self
                .complete_image(id, image)
                .map_err(|_| ImageDecodeError::ResourceUpdateFailed),
            Err(error) => {
                let _ = self.fail_resource(id.resource_id(), error.to_string());
                Err(error)
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn complete_image_file(
        &mut self,
        id: ImageId,
        path: impl AsRef<std::path::Path>,
        scale: f64,
    ) -> Result<(), ImageDecodeError> {
        match ImageData::decode_file(path, scale) {
            Ok(image) => self
                .complete_image(id, image)
                .map_err(|_| ImageDecodeError::ResourceUpdateFailed),
            Err(error) => {
                let _ = self.fail_resource(id.resource_id(), error.to_string());
                Err(error)
            }
        }
    }

    pub fn complete_font(&mut self, id: FontId, font: FontData) -> Result<(), ResourceError> {
        let revision = self
            .resources
            .next_revision(id.resource_id(), crate::ResourceKind::Font)?;
        self.text
            .register_font(id.resource_id(), revision, font.bytes())
            .map_err(|_| ResourceError::InvalidFontData)?;
        let dependents = self.resources.complete_font(id, font)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
    }

    pub fn fail_resource(
        &mut self,
        id: ResourceId,
        reason: impl Into<String>,
    ) -> Result<(), ResourceError> {
        if self.resources.kind(id)? == crate::ResourceKind::Font {
            self.text.remove_font(id);
        }
        let dependents = self.resources.fail(id, reason)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
    }

    pub fn remove_resource(&mut self, id: ResourceId) -> Result<(), ResourceError> {
        if self.resources.kind(id)? == crate::ResourceKind::Font {
            self.text.remove_font(id);
            self.builtin_fonts
                .retain(|_, font_id| font_id.resource_id() != id);
        }
        let dependents = self.resources.remove(id)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
    }

    pub fn dispatch_mouse_moved(&mut self, x: f64, y: f64) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_moved(ctx, x, y));
    }

    pub fn dispatch_mouse_pressed(&mut self, x: f64, y: f64, button: MouseButton) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_pressed(ctx, x, y, button));
    }

    pub fn dispatch_mouse_released(&mut self, x: f64, y: f64, button: MouseButton) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_released(ctx, x, y, button));
    }

    pub fn dispatch_mouse_double_clicked(&mut self, x: f64, y: f64, button: MouseButton) {
        self.dispatch(|dispatcher, ctx| {
            dispatcher.dispatch_mouse_double_clicked(ctx, x, y, button)
        });
    }

    pub fn dispatch_mouse_hover(&mut self, x: f64, y: f64) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_hover(ctx, x, y));
    }

    pub fn dispatch_scroll(&mut self, event: WheelEvent) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_scroll(ctx, event));
    }

    pub fn dispatch_zoom(&mut self, event: ZoomEvent) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_zoom(ctx, event));
    }

    pub fn dispatch_key_pressed(&mut self, key: Key, modifiers: KeyModifiers) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_key_pressed(ctx, key, modifiers));
    }

    pub fn dispatch_key_released(&mut self, key: Key, modifiers: KeyModifiers) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_key_released(ctx, key, modifiers));
    }

    pub fn request_focus(&mut self, target: FigureId) -> Result<FocusChange, FocusError> {
        self.validate_direct_focus(target)?;
        Ok(self.dispatch(|dispatcher, ctx| dispatcher.set_focus(ctx, Some(target))))
    }

    pub fn clear_focus(&mut self) -> FocusChange {
        self.dispatch(|dispatcher, ctx| dispatcher.release_focus(ctx))
    }

    pub fn release_focus(&mut self) -> FocusChange {
        self.clear_focus()
    }

    pub fn traverse_focus(&mut self, direction: FocusTraversalDirection) -> FocusTraversalOutcome {
        let Some(scope) = self.tree.get_contents() else {
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
            FocusChange::Changed { .. } => FocusTraversalOutcome::Moved(candidate),
            FocusChange::Unchanged => FocusTraversalOutcome::Boundary,
        }
    }

    pub fn set_focus_traversal_policy(&mut self, policy: Box<dyn FocusTraversalPolicy>) {
        self.focus_traversal_policy = policy;
    }

    pub fn cancel_gestures(&mut self) {
        self.dispatch(|dispatcher, ctx| dispatcher.cancel_gestures(ctx));
    }

    /// Applies all callback effects and structural mutations before returning.
    fn dispatch<R>(
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
            changed |= match mutation.into_kind() {
                PendingMutationKind::AddLayerFigure {
                    pane,
                    figure,
                    key,
                    placement,
                } => self.add_layer(pane, figure, key, placement).is_ok(),
                PendingMutationKind::RemoveLayer { pane, key } => {
                    self.remove_layer(pane, &key).is_ok()
                }
                PendingMutationKind::MoveLayer {
                    pane,
                    key,
                    placement,
                } => self.move_layer(pane, &key, placement).unwrap_or(false),
                PendingMutationKind::ReparentLayer {
                    child,
                    new_pane,
                    key,
                    placement,
                } => self
                    .reparent_layer(child, new_pane, key, placement)
                    .unwrap_or(false),
                PendingMutationKind::RemoveChild { parent, child } => {
                    let removed = self.tree.apply_pending_mutations(
                        &mut self.updates,
                        vec![PendingMutation::from_kind(
                            PendingMutationKind::RemoveChild { parent, child },
                        )],
                    );
                    if removed {
                        self.invalidate_connection_figure_change(child, true);
                    }
                    removed
                }
                PendingMutationKind::Reparent { child, new_parent } => {
                    let reparented = self.tree.apply_pending_mutations(
                        &mut self.updates,
                        vec![PendingMutation::from_kind(PendingMutationKind::Reparent {
                            child,
                            new_parent,
                        })],
                    );
                    if reparented {
                        self.invalidate_connection_figure_change(child, true);
                    }
                    reparented
                }
                kind => self.tree.apply_pending_mutations(
                    &mut self.updates,
                    vec![PendingMutation::from_kind(kind)],
                ),
            };
        }
        changed
    }

    fn initialize_layered_panes(&mut self) {
        let Some(contents) = self.tree.get_contents() else {
            return;
        };
        let mut ids = vec![contents];
        ids.extend(self.tree.descendant_ids(contents).unwrap_or_default());
        for id in ids {
            self.register_layered_pane(id);
        }
    }

    fn register_layered_pane(&mut self, pane_id: FigureId) {
        if !self.tree.is_layered_pane(pane_id) {
            return;
        }
        if self.tree.get_block_layout_manager(pane_id).is_none() {
            self.tree
                .set_block_layout_manager(pane_id, Box::new(StackLayout));
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
        if !invalid_focus {
            return;
        }

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
        if self.tree.get_block(target).is_none() {
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

    /// Produces one complete renderer submission at a stable transaction boundary.
    pub fn prepare_submission(
        &mut self,
        surface: SurfaceInfo,
        capabilities: BackendCapabilities,
    ) -> Option<RenderSubmission> {
        if self.in_flight.is_some() {
            return None;
        }

        let mutations = self.mutations.drain();
        if !mutations.is_empty() {
            self.apply_runtime_mutations(mutations);
            self.retain_interactive_figures();
        }

        let surface_changed = self.last_surface != Some(surface);
        self.last_surface = Some(surface);
        if surface_changed {
            self.full_redraw_pending = true;
        }
        if !surface.is_renderable() {
            self.full_redraw_pending = true;
            return None;
        }
        self.refresh_image_figures();
        if self.refresh_title_bar_borders().is_err() || self.refresh_label_layouts().is_err() {
            return None;
        }

        let has_resource_delta = self.resources.has_pending_delta();
        let mut canvas = if self.updates.is_update_queued() {
            self.tree.perform_update(&mut self.updates)
        } else if self.full_redraw_pending {
            self.tree.render()
        } else if has_resource_delta {
            NdCanvas::new()
        } else {
            return None;
        };
        if self.updates.last_validation_error().is_some() {
            self.full_redraw_pending = true;
            return None;
        }

        if self.full_redraw_pending
            || (canvas.damage().mode() == DamageMode::Partial
                && !capabilities.supports_partial_damage())
        {
            if canvas.commands().is_empty() {
                canvas = self.tree.render();
            } else {
                canvas.damage_mut().set_full();
            }
        }

        let frame_id = self.next_frame_id;
        self.next_frame_id = self.next_frame_id.next();
        let resources = self.resources.take_delta();
        let submission = canvas.to_submission_for_frame(surface, resources.clone(), frame_id);
        self.full_redraw_pending = false;
        self.in_flight = Some(InFlightFrame {
            id: frame_id,
            resources,
        });
        self.updates.emit_update_event(UpdateEvent::Prepared {
            frame_id,
            damage: submission.damage.mode(),
        });
        self.updates.flush_notifications(&mut self.tree);
        Some(submission)
    }

    /// Completes the in-flight frame and restores work when presentation failed.
    pub fn complete_submission(&mut self, frame_id: FrameId, outcome: RenderOutcome) -> bool {
        let Some(in_flight) = self.in_flight.take() else {
            return false;
        };
        if in_flight.id != frame_id {
            self.in_flight = Some(in_flight);
            return false;
        }

        if outcome != RenderOutcome::Presented {
            self.full_redraw_pending = true;
            self.resources.restore_delta(in_flight.resources);
        }
        self.updates
            .emit_update_event(UpdateEvent::Submitted { frame_id, outcome });
        self.updates.flush_notifications(&mut self.tree);
        true
    }

    /// Prepares an incremental frame when the runtime has pending work.
    pub fn prepare_frame(&mut self) -> Option<NdCanvas> {
        self.refresh_image_figures();
        if self.refresh_title_bar_borders().is_err() || self.refresh_label_layouts().is_err() {
            return None;
        }
        if self.updates.is_update_queued() {
            self.full_redraw_pending = false;
            return Some(self.tree.perform_update(&mut self.updates));
        }
        if std::mem::take(&mut self.full_redraw_pending) {
            return Some(self.tree.render());
        }
        None
    }

    /// Records the complete visible tree, independent of pending update state.
    pub fn record_full_frame(&mut self) -> NdCanvas {
        let _ = self.refresh_label_layouts();
        self.tree.render()
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
    use crate::{
        Bounded, EventContext, FigureEvent, FigureEventHandler, FocusEvent, FocusEventKind,
        Rectangle, RectangleFigure,
    };
    use novadraw_core::Color;
    use slotmap::Key;
    use std::sync::{Arc, Mutex};

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
        let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));

        runtime.interaction.set_mouse_target(Some(root));

        assert_eq!(runtime.interaction().mouse_target(), Some(root));
        assert_eq!(runtime.tree().get_contents(), Some(root));
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

        assert_eq!(submission.resources.added.len(), 1);
        assert_eq!(submission.resources.added[0].id, id.resource_id());
        assert_eq!(submission.resources.added[0].revision, 1);
        assert!(matches!(
            &submission.resources.added[0].payload,
            novadraw_render::ResourcePayload::Font(_)
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
            runtime.resources.take_delta().removed,
            vec![id.resource_id()]
        );
    }

    #[test]
    fn runtime_accepts_a_non_parley_text_layout_engine() {
        struct StubTextEngine;

        impl TextLayoutEngine for StubTextEngine {
            fn revision(&self) -> u64 {
                42
            }

            fn register_font(
                &mut self,
                _resource_id: ResourceId,
                _revision: u64,
                _bytes: &[u8],
            ) -> Result<(), TextError> {
                Ok(())
            }

            fn remove_font(&mut self, _resource_id: ResourceId) {}

            fn layout(
                &mut self,
                _text: &str,
                _font: &FontDescriptor,
                _constraints: TextConstraints,
            ) -> Result<TextLayout, TextError> {
                Ok(TextLayout::default())
            }
        }

        let mut runtime =
            Runtime::with_text_layout_engine(FigureTree::new(), Box::new(StubTextEngine));

        assert_eq!(runtime.text_revision(), 42);
        assert!(
            runtime
                .layout_text(
                    "custom",
                    &FontDescriptor::default(),
                    TextConstraints::UNBOUNDED,
                )
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn direct_and_traversal_focus_eligibility_are_independent() {
        let mut runtime = Runtime::empty();
        let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let traversal_only =
            runtime.add_figure(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
        let direct_only =
            runtime.add_figure(root, Box::new(RectangleFigure::new(30.0, 0.0, 20.0, 20.0)));
        runtime.set_focus_traversable(traversal_only, true);
        runtime.set_focusable(direct_only, true);

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
        let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let target = runtime.add_figure(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
        runtime.set_focusable(root, true);
        runtime.request_focus(root).unwrap();

        assert_eq!(
            runtime.request_focus(FigureId::null()),
            Err(FocusError::UnknownFigure(FigureId::null()))
        );
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::NotFocusable(target))
        );
        runtime.set_focusable(target, true);
        runtime.set_visible(target, false);
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::Hidden(target))
        );
        runtime.set_visible(target, true);
        runtime.set_enabled(target, false);
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::Disabled(target))
        );
        assert_eq!(runtime.interaction().focus_owner(), Some(root));

        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::Detached(target))
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
                Transition::Hide => assert!(runtime.set_visible(probe, false)),
                Transition::Disable => assert!(runtime.set_enabled(probe, false)),
                Transition::Remove => assert!(runtime.remove_figure(root, probe)),
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
        let hidden_parent =
            runtime.add_figure(root, Box::new(RectangleFigure::new(40.0, 40.0, 40.0, 40.0)));
        runtime.set_visible(hidden_parent, false);

        assert!(runtime.reparent(probe, hidden_parent));

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
        let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        runtime.set_figure_style(
            root,
            FigureStyle {
                cursor: Some(CursorIcon::Crosshair),
                tooltip: Some(Some("root tip".to_string())),
                ..FigureStyle::default()
            },
        );

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
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![255, 0, 0, 255], 1.0))
            .unwrap();

        let first = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(first.frame_id, FrameId::INITIAL);
        assert_eq!(first.surface, surface(100, 100));
        assert_eq!(first.resources.added.len(), 1);
        assert_eq!(first.resources.added[0].id, image.resource_id());
        assert_eq!(first.damage.mode(), DamageMode::Full);
        assert!(runtime.complete_submission(first.frame_id, RenderOutcome::Presented));

        runtime.request_full_redraw();
        let second = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(second.frame_id, first.frame_id.next());
        assert!(runtime.complete_submission(second.frame_id, RenderOutcome::Presented));
        assert!(
            runtime
                .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
                .is_none()
        );
    }

    #[test]
    fn resource_only_completion_produces_a_submission_without_scene_damage() {
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![0, 0, 0, 0], 1.0))
            .unwrap();
        let resource_only = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(resource_only.damage.mode(), DamageMode::None);
        assert!(resource_only.commands.is_empty());
        assert_eq!(resource_only.resources.added.len(), 1);
        assert_eq!(resource_only.resources.added[0].id, image.resource_id());
    }

    #[test]
    fn resource_completion_invalidates_and_repaints_dependent_figure() {
        let mut runtime = Runtime::empty();
        let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child =
            runtime.add_figure(root, Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

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
        assert_eq!(submission.resources.added[0].id, image.resource_id());
        assert_ne!(submission.damage.mode(), DamageMode::None);
    }

    #[test]
    fn replacing_contents_removes_resource_dependencies_from_detached_figures() {
        let mut runtime = Runtime::empty();
        let old_contents =
            runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let image = runtime.register_image();
        runtime
            .add_resource_dependency(image.resource_id(), old_contents)
            .unwrap();
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let replacement = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(replacement.frame_id, RenderOutcome::Presented);
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
        let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child =
            runtime.add_figure(root, Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

        runtime.set_bounds(
            child,
            novadraw_geometry::Rectangle::new(15.0, 15.0, 20.0, 20.0),
        );
        let partial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(partial.damage.mode(), DamageMode::Partial);
        runtime.complete_submission(partial.frame_id, RenderOutcome::Presented);

        runtime.set_bounds(
            child,
            novadraw_geometry::Rectangle::new(20.0, 20.0, 20.0, 20.0),
        );
        let promoted = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::FULL_FRAME_ONLY)
            .unwrap();
        assert_eq!(promoted.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn moving_contents_forces_full_damage_to_clear_exposed_pixels() {
        let mut runtime = Runtime::empty();
        let root = runtime.set_contents(Box::new(RectangleFigure::new(20.0, 20.0, 60.0, 60.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

        assert!(runtime.translate(root, 10.0, 10.0));
        let moved = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(moved.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn surface_change_and_retry_force_full_damage() {
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
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
        assert!(runtime.complete_submission(initial.frame_id, RenderOutcome::Retry));

        let retry = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(retry.damage.mode(), DamageMode::Full);
        assert_eq!(retry.resources.added.len(), 1);
        assert_eq!(retry.resources.added[0].id, image.resource_id());
        runtime.complete_submission(retry.frame_id, RenderOutcome::Presented);

        let resized = runtime
            .prepare_submission(surface(120, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(resized.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn retry_restores_in_flight_resource_updates_before_newer_updates() {
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

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
        runtime.complete_submission(first_update.frame_id, RenderOutcome::Retry);

        let retry = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        let revisions: Vec<_> = retry
            .resources
            .added
            .iter()
            .map(|update| update.revision)
            .collect();

        assert_eq!(revisions, vec![1, 2]);
    }

    #[test]
    fn dpi_change_updates_surface_metadata_and_forces_full_damage() {
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

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
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));

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
    fn submission_notifications_are_flushed_in_causal_order() {
        struct CaptureUpdates(Arc<Mutex<Vec<UpdateEvent>>>);

        impl UpdateListener for CaptureUpdates {
            fn on_update_event(&self, event: UpdateEvent) {
                self.0.lock().unwrap().push(event);
            }

            fn on_figure_event(&self, _event: FigureEvent) {}

            fn on_notify(&self, _block_id: FigureId) {}
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let mut runtime = Runtime::empty();
        runtime.add_update_listener(Box::new(CaptureUpdates(events.clone())));
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));

        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(submission.frame_id, RenderOutcome::Presented);

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
