//! Open editing tools and gesture-scoped trackers.

use std::{any::Any, error::Error, fmt, marker::PhantomData};

use novadraw::event::{DispatchOutcome, KeyEvent, KeyModifiers, MonotonicTime, MouseButton};
use novadraw::geometry::{Dimension, Point, Vec2};
use novadraw::tree::FigureId;

use crate::{
    BendpointOperation, BendpointRequest, ChangeBounds, ChangeBoundsRequest, Command,
    CompoundCommand, ConnectionCreation, ConnectionEndpoint, ConnectionPartId,
    ConnectionReconnection, CreateConnectionRequest, CreationType, EditBendpoint, EditPartId,
    EditingContribution, FeedbackVisual, InteractionRevision, ModelAdapter,
    ReconnectConnectionRequest, RequestMetadata, RequestModifiers, RequestProtocol,
    RequestResolution, ResizeAction, ResizeDirection, ViewerError, ViewerInputOutcome,
    ViewerTarget, request::ErasedRequestKey,
};

const DRAG_START_DISTANCE: f64 = 2.0;

/// Erased identity retained by an active Tool.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ErasedToolKey {
    type_id: std::any::TypeId,
    name: &'static str,
}

impl ErasedToolKey {
    /// Returns the diagnostic Tool name.
    pub const fn name(self) -> &'static str {
        self.name
    }
}

/// Typed identity of one Tool implementation.
pub struct ToolKey<T: ?Sized> {
    name: &'static str,
    marker: PhantomData<fn() -> T>,
}

impl<T: ?Sized> ToolKey<T> {
    /// Creates a typed Tool identity.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }
}

impl<T: 'static> ToolKey<T> {
    /// Erases only the Tool type while preserving its identity and diagnostic name.
    pub fn erase(self) -> ErasedToolKey {
        ErasedToolKey {
            type_id: std::any::TypeId::of::<T>(),
            name: self.name,
        }
    }
}

impl<T: ?Sized> Clone for ToolKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for ToolKey<T> {}

/// Normalized pointer input supplied to a Tool callback.
#[derive(Clone, Copy, Debug)]
pub struct ToolMouseEvent {
    location: Point,
    button: MouseButton,
    modifiers: KeyModifiers,
    revision: InteractionRevision,
}

impl ToolMouseEvent {
    /// Creates one pointer event.
    pub const fn new(
        location: Point,
        button: MouseButton,
        modifiers: KeyModifiers,
        revision: InteractionRevision,
    ) -> Self {
        Self {
            location,
            button,
            modifiers,
            revision,
        }
    }

    /// Returns the pointer location in the logical surface domain.
    pub const fn location(self) -> Point {
        self.location
    }

    /// Returns the pointer button.
    pub const fn button(self) -> MouseButton {
        self.button
    }

    /// Returns the modifier snapshot.
    pub const fn modifiers(self) -> KeyModifiers {
        self.modifiers
    }

    /// Returns the interaction revision.
    pub const fn revision(self) -> InteractionRevision {
        self.revision
    }
}

/// Framework control flow requested after one Tool callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolDirective {
    /// Keep the current Tool active.
    Continue,
    /// Release pointer capture while keeping the current Tool active.
    ReleaseCapture,
    /// Replace the current Tool with the default selection Tool.
    Deactivate,
}

pub(crate) type ErasedRequestContributions = Vec<(EditPartId, Box<dyn Any>)>;

pub(crate) trait ToolHost<A: ModelAdapter> {
    fn dispatch_request_erased(
        &mut self,
        key: ErasedRequestKey,
        metadata: &RequestMetadata,
        payload: &dyn Any,
    ) -> Result<ErasedRequestContributions, ViewerError>;
    fn attach_feedback(
        &mut self,
        contributions: Vec<(EditPartId, FeedbackVisual)>,
    ) -> Result<Vec<FigureId>, ViewerError>;
    fn dispatch_pressed(
        &mut self,
        location: Point,
        button: MouseButton,
        modifiers: KeyModifiers,
        update_selection: bool,
    ) -> Result<ViewerInputOutcome, ViewerError>;
    fn dispatch_moved(&mut self, location: Point) -> DispatchOutcome;
    fn dispatch_released(&mut self, location: Point, button: MouseButton) -> DispatchOutcome;
    fn target_at(&self, location: Point) -> ViewerTarget;
    fn selection(&self) -> &[EditPartId];
    fn model_point_from_surface(&self, location: Point) -> Result<Point, ViewerError>;
    fn connection_point_from_surface(
        &self,
        connection: ConnectionPartId,
        location: Point,
    ) -> Result<Point, ViewerError>;
    fn connection_routing_point_from_surface(&self, location: Point) -> Result<Point, ViewerError>;
    fn as_connection_part(&self, part: EditPartId) -> Option<ConnectionPartId>;
    fn replace_selection(&mut self, part: EditPartId) -> Result<(), ViewerError>;
    fn set_focus(&mut self, part: Option<EditPartId>) -> Result<(), ViewerError>;
    fn remove_overlay_visuals(&mut self, figures: &[FigureId]) -> Result<(), ViewerError>;
    fn start_connection_creation(
        &mut self,
        request: &CreateConnectionRequest,
    ) -> Result<Option<Box<dyn ConnectionCreation<A>>>, ViewerError>;
    fn show_connection_feedback(
        &mut self,
        plan: &mut dyn ConnectionCreation<A>,
        request: &CreateConnectionRequest,
    ) -> Result<Vec<FigureId>, ViewerError>;
    fn connection_command(
        &mut self,
        plan: &mut dyn ConnectionCreation<A>,
        request: &CreateConnectionRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError>;
    fn start_connection_reconnection(
        &mut self,
        request: &ReconnectConnectionRequest,
    ) -> Result<Option<Box<dyn ConnectionReconnection<A>>>, ViewerError>;
    fn show_reconnection_feedback(
        &mut self,
        plan: &mut dyn ConnectionReconnection<A>,
        request: &ReconnectConnectionRequest,
    ) -> Result<Vec<FigureId>, ViewerError>;
    fn reconnection_command(
        &mut self,
        plan: &mut dyn ConnectionReconnection<A>,
        request: &ReconnectConnectionRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError>;
    fn add_autoexpose_range_reserve(
        &mut self,
        location: Point,
        feedback: &mut Vec<FigureId>,
    ) -> Result<(), ViewerError>;
}

/// Narrow Viewer and request-dispatch access supplied to an active Tool.
pub struct ToolContext<'a, A: ModelAdapter> {
    host: &'a mut dyn ToolHost<A>,
    pending_command: Option<Box<dyn Command<A>>>,
}

impl<'a, A: ModelAdapter + 'static> ToolContext<'a, A> {
    pub(crate) fn new(host: &'a mut dyn ToolHost<A>) -> Self {
        Self {
            host,
            pending_command: None,
        }
    }

    /// Dispatches one typed request through installed policies.
    pub fn dispatch<R>(
        &mut self,
        metadata: &RequestMetadata,
        payload: &R::Payload,
    ) -> Result<RequestResolution<R::Contribution>, ToolError>
    where
        R: RequestProtocol<A>,
    {
        let contributions = self
            .host
            .dispatch_request_erased(ErasedRequestKey::of(R::key()), metadata, payload)?
            .into_iter()
            .map(|(_, contribution)| {
                contribution
                    .downcast::<R::Contribution>()
                    .map(|contribution| *contribution)
                    .map_err(|_| ToolError::protocol("request contribution descriptor mismatch"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(R::reduce(contributions).map_err(ViewerError::from)?)
    }

    /// Resolves command contributions for one typed request.
    pub fn command_for<R>(
        &mut self,
        metadata: &RequestMetadata,
        payload: &R::Payload,
    ) -> Result<Option<Box<dyn Command<A>>>, ToolError>
    where
        R: RequestProtocol<A, Contribution = EditingContribution<A>>,
    {
        let mut commands = self
            .dispatch::<R>(metadata, payload)?
            .into_contributions()
            .into_iter()
            .filter_map(|contribution| contribution.into_parts().0)
            .collect::<Vec<_>>();
        Ok(match commands.len() {
            0 => None,
            1 => commands.pop(),
            _ => {
                let mut compound = CompoundCommand::new(R::key().name());
                for command in commands {
                    compound.push(command);
                }
                Some(Box::new(compound))
            }
        })
    }

    /// Resolves and attaches feedback contributions for one typed request.
    pub fn show_feedback<R>(
        &mut self,
        metadata: &RequestMetadata,
        payload: &R::Payload,
    ) -> Result<Vec<FigureId>, ToolError>
    where
        R: RequestProtocol<A, Contribution = EditingContribution<A>>,
    {
        let mut feedback = Vec::new();
        for (owner, contribution) in
            self.host
                .dispatch_request_erased(ErasedRequestKey::of(R::key()), metadata, payload)?
        {
            let contribution = contribution
                .downcast::<EditingContribution<A>>()
                .map_err(|_| ToolError::protocol("request contribution descriptor mismatch"))?;
            feedback.extend(
                contribution
                    .into_parts()
                    .1
                    .into_iter()
                    .map(|visual| (owner, visual)),
            );
        }
        Ok(self.host.attach_feedback(feedback)?)
    }

    /// Submits at most one model command for execution after the callback returns.
    pub fn submit_command(&mut self, command: Box<dyn Command<A>>) -> Result<(), ToolError> {
        if self.pending_command.replace(command).is_some() {
            return Err(ToolError::protocol(
                "a Tool callback submitted multiple commands",
            ));
        }
        Ok(())
    }

    pub(crate) fn take_command(&mut self) -> Option<Box<dyn Command<A>>> {
        self.pending_command.take()
    }

    /// Dispatches a pointer press through the Viewer.
    pub fn dispatch_pressed(
        &mut self,
        event: ToolMouseEvent,
        update_selection: bool,
    ) -> Result<ViewerInputOutcome, ToolError> {
        Ok(self.host.dispatch_pressed(
            event.location(),
            event.button(),
            event.modifiers(),
            update_selection,
        )?)
    }

    /// Dispatches pointer movement through the Viewer.
    pub fn dispatch_moved(&mut self, location: Point) -> DispatchOutcome {
        self.host.dispatch_moved(location)
    }

    /// Dispatches pointer release through the Viewer.
    pub fn dispatch_released(&mut self, location: Point, button: MouseButton) -> DispatchOutcome {
        self.host.dispatch_released(location, button)
    }

    /// Resolves the topmost Editor target at a surface point.
    pub fn target_at(&self, location: Point) -> ViewerTarget {
        self.host.target_at(location)
    }

    /// Returns the current ordered Viewer selection.
    pub fn selection(&self) -> &[EditPartId] {
        self.host.selection()
    }

    /// Converts a logical surface point into the model-content domain.
    pub fn model_point_from_surface(&self, location: Point) -> Result<Point, ToolError> {
        Ok(self.host.model_point_from_surface(location)?)
    }

    fn connection_point_from_surface(
        &self,
        connection: ConnectionPartId,
        location: Point,
    ) -> Result<Point, ToolError> {
        Ok(self
            .host
            .connection_point_from_surface(connection, location)?)
    }

    fn connection_routing_point_from_surface(&self, location: Point) -> Result<Point, ToolError> {
        Ok(self.host.connection_routing_point_from_surface(location)?)
    }

    /// Converts a live connection EditPart into its typed identity.
    pub fn as_connection_part(&self, part: EditPartId) -> Option<ConnectionPartId> {
        self.host.as_connection_part(part)
    }

    /// Replaces Viewer selection with one part.
    pub fn replace_selection(&mut self, part: EditPartId) -> Result<(), ToolError> {
        Ok(self.host.replace_selection(part)?)
    }

    /// Replaces Viewer focus.
    pub fn set_focus(&mut self, part: Option<EditPartId>) -> Result<(), ToolError> {
        Ok(self.host.set_focus(part)?)
    }

    /// Removes an atomic batch of Tool-owned overlay visuals.
    pub fn remove_overlay_visuals(&mut self, figures: &[FigureId]) -> Result<(), ToolError> {
        Ok(self.host.remove_overlay_visuals(figures)?)
    }

    fn start_connection_creation(
        &mut self,
        request: &CreateConnectionRequest,
    ) -> Result<Option<Box<dyn ConnectionCreation<A>>>, ToolError> {
        Ok(self.host.start_connection_creation(request)?)
    }

    fn show_connection_feedback(
        &mut self,
        plan: &mut dyn ConnectionCreation<A>,
        request: &CreateConnectionRequest,
    ) -> Result<Vec<FigureId>, ToolError> {
        Ok(self.host.show_connection_feedback(plan, request)?)
    }

    fn connection_command(
        &mut self,
        plan: &mut dyn ConnectionCreation<A>,
        request: &CreateConnectionRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ToolError> {
        Ok(self.host.connection_command(plan, request)?)
    }

    fn start_connection_reconnection(
        &mut self,
        request: &ReconnectConnectionRequest,
    ) -> Result<Option<Box<dyn ConnectionReconnection<A>>>, ToolError> {
        Ok(self.host.start_connection_reconnection(request)?)
    }

    fn show_reconnection_feedback(
        &mut self,
        plan: &mut dyn ConnectionReconnection<A>,
        request: &ReconnectConnectionRequest,
    ) -> Result<Vec<FigureId>, ToolError> {
        Ok(self.host.show_reconnection_feedback(plan, request)?)
    }

    fn reconnection_command(
        &mut self,
        plan: &mut dyn ConnectionReconnection<A>,
        request: &ReconnectConnectionRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ToolError> {
        Ok(self.host.reconnection_command(plan, request)?)
    }

    fn add_autoexpose_range_reserve(
        &mut self,
        location: Point,
        feedback: &mut Vec<FigureId>,
    ) -> Result<(), ToolError> {
        Ok(self.host.add_autoexpose_range_reserve(location, feedback)?)
    }
}

/// Object-safe lifecycle shared by built-in and external Tools.
pub trait Tool<A: ModelAdapter>: 'static {
    /// Returns this Tool implementation's identity.
    fn key(&self) -> ErasedToolKey;

    /// Activates the Tool.
    fn activate(&mut self, _context: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        Ok(())
    }

    /// Deactivates the Tool.
    fn deactivate(&mut self, _context: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        Ok(())
    }

    /// Cancels transient Tool state.
    fn cancel(&mut self, context: &mut ToolContext<'_, A>) -> Result<(), ToolError>;

    /// Handles a pointer press.
    fn mouse_pressed(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError>;

    /// Handles pointer movement.
    fn mouse_moved(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError>;

    /// Handles pointer release.
    fn mouse_released(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError>;

    /// Handles a keyboard event.
    fn key_event(
        &mut self,
        _context: &mut ToolContext<'_, A>,
        _event: KeyEvent,
    ) -> Result<ToolDirective, ToolError> {
        Ok(ToolDirective::Continue)
    }

    /// Handles a monotonic timer tick.
    fn tick(
        &mut self,
        _context: &mut ToolContext<'_, A>,
        _now: MonotonicTime,
    ) -> Result<ToolDirective, ToolError> {
        Ok(ToolDirective::Continue)
    }

    /// Returns whether this Tool owns an active gesture.
    fn has_active_gesture(&self) -> bool {
        false
    }

    /// Returns whether the active gesture supports viewport auto-expose.
    fn supports_autoexpose(&self) -> bool {
        false
    }

    /// Returns dragged parts used to stabilize feedback projection.
    fn dragged_parts(&self) -> Option<&[EditPartId]> {
        None
    }

    /// Recomputes gesture feedback after an external viewport change.
    fn refresh(
        &mut self,
        _context: &mut ToolContext<'_, A>,
        _location: Point,
        _revision: InteractionRevision,
    ) -> Result<(), ToolError> {
        Ok(())
    }

    /// Removes transient feedback without ending the Tool.
    fn clear_transient_feedback(
        &mut self,
        _context: &mut ToolContext<'_, A>,
    ) -> Result<(), ToolError> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DragKind {
    Move,
    Resize(ResizeDirection),
}

struct DragGesture {
    start_surface: Point,
    start_model: Point,
    parts: Vec<EditPartId>,
    kind: DragKind,
    modifiers: RequestModifiers,
    collapse_on_click: Option<EditPartId>,
    drag_started: bool,
    feedback: Vec<FigureId>,
}

/// Failure while updating or cancelling a Tool gesture.
#[derive(Debug)]
pub enum ToolError {
    /// Viewer targeting, policy feedback, or overlay cleanup failed.
    Viewer(ViewerError),
    /// Tool protocol or lifecycle state was inconsistent.
    Protocol(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Viewer(error) => error.fmt(formatter),
            Self::Protocol(message) => message.fmt(formatter),
        }
    }
}

impl Error for ToolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Viewer(error) => Some(error),
            Self::Protocol(_) => None,
        }
    }
}

impl From<ViewerError> for ToolError {
    fn from(value: ViewerError) -> Self {
        Self::Viewer(value)
    }
}

impl ToolError {
    /// Creates a Tool protocol or lifecycle failure.
    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol(message.into())
    }
}

/// Default selection and bounds-manipulation Tool.
#[derive(Default)]
pub struct SelectionTool {
    gesture: Option<DragGesture>,
}

impl SelectionTool {
    /// Creates an idle SelectionTool.
    pub const fn new() -> Self {
        Self { gesture: None }
    }

    /// Returns whether a Tool-owned pointer gesture is active.
    pub const fn is_active(&self) -> bool {
        self.gesture.is_some()
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.gesture
            .as_ref()
            .is_some_and(|gesture| gesture.drag_started)
    }

    pub(crate) fn dragged_parts(&self) -> Option<&[EditPartId]> {
        self.gesture
            .as_ref()
            .filter(|gesture| gesture.drag_started)
            .map(|gesture| gesture.parts.as_slice())
    }
}

struct ConnectionGesture<A: ModelAdapter> {
    source: EditPartId,
    connection_type: CreationType,
    modifiers: RequestModifiers,
    plan: Box<dyn ConnectionCreation<A>>,
    feedback: Vec<FigureId>,
}

/// One-shot, two-stage connection-creation Tool.
pub struct ConnectionCreationTool<A: ModelAdapter> {
    connection_type: CreationType,
    gesture: Option<ConnectionGesture<A>>,
}

struct ReconnectGesture<A: ModelAdapter> {
    start: Point,
    modifiers: RequestModifiers,
    plan: Box<dyn ConnectionReconnection<A>>,
    drag_started: bool,
    feedback: Vec<FigureId>,
}

/// Drag Tool for moving one endpoint of an existing connection.
pub struct ConnectionEndpointTool<A: ModelAdapter> {
    connection: ConnectionPartId,
    endpoint: ConnectionEndpoint,
    gesture: Option<ReconnectGesture<A>>,
}

struct BendpointGesture {
    connection: ConnectionPartId,
    operation: BendpointOperation,
    start_surface: Point,
    modifiers: RequestModifiers,
    drag_started: bool,
    feedback: Vec<FigureId>,
}

/// Drag Tool for creating or moving one connection bendpoint.
#[derive(Default)]
pub struct ConnectionBendpointTool {
    pending: Option<(ConnectionPartId, BendpointOperation)>,
    gesture: Option<BendpointGesture>,
}

impl ConnectionBendpointTool {
    /// Creates an idle bendpoint Tool.
    pub const fn new() -> Self {
        Self {
            pending: None,
            gesture: None,
        }
    }

    pub(crate) const fn for_handle(
        connection: ConnectionPartId,
        operation: BendpointOperation,
    ) -> Self {
        Self {
            pending: Some((connection, operation)),
            gesture: None,
        }
    }

    /// Returns whether one bendpoint drag is active.
    pub const fn is_active(&self) -> bool {
        self.gesture.is_some()
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.gesture
            .as_ref()
            .is_some_and(|gesture| gesture.drag_started)
    }
}

impl<A: ModelAdapter> ConnectionEndpointTool<A> {
    /// Creates a Tool locked to one connection endpoint.
    pub const fn new(connection: ConnectionPartId, endpoint: ConnectionEndpoint) -> Self {
        Self {
            connection,
            endpoint,
            gesture: None,
        }
    }

    /// Returns whether the endpoint drag was accepted by a policy.
    pub const fn is_active(&self) -> bool {
        self.gesture.is_some()
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.gesture
            .as_ref()
            .is_some_and(|gesture| gesture.drag_started)
    }
}

impl<A: ModelAdapter> ConnectionCreationTool<A> {
    /// Arms a Tool for one connection of the supplied application type.
    pub fn new(connection_type: CreationType) -> Self {
        Self {
            connection_type,
            gesture: None,
        }
    }

    /// Returns whether the source stage has been accepted.
    pub const fn is_started(&self) -> bool {
        self.gesture.is_some()
    }
}

impl<A: ModelAdapter + 'static> Tool<A> for SelectionTool {
    fn key(&self) -> ErasedToolKey {
        ToolKey::<Self>::new("selection").erase()
    }

    fn cancel(&mut self, context: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some(gesture) = self.gesture.as_mut() {
            context.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        self.gesture = None;
        Ok(())
    }

    fn mouse_pressed(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError> {
        <Self as Tool<A>>::cancel(self, context)?;
        let outcome = context.dispatch_pressed(event, true)?;
        if outcome.dispatch().is_handled() || event.button() != MouseButton::Left {
            return Ok((ToolDirective::Continue, outcome));
        }
        let (parts, kind, collapse_on_click) = match outcome.target() {
            ViewerTarget::Part(part) if context.selection().contains(&part) => {
                let collapse = (!event.modifiers().shift
                    && !event.modifiers().control
                    && !event.modifiers().meta
                    && context.selection().len() > 1)
                    .then_some(part);
                (context.selection().to_vec(), DragKind::Move, collapse)
            }
            ViewerTarget::Handle { owner, action, .. } => {
                let Some(direction) = action.get::<ResizeAction>().copied() else {
                    return Ok((ToolDirective::Continue, outcome));
                };
                (vec![owner], DragKind::Resize(direction), None)
            }
            ViewerTarget::Contents(_) | ViewerTarget::Part(_) => {
                return Ok((ToolDirective::Continue, outcome));
            }
        };
        self.gesture = Some(DragGesture {
            start_surface: event.location(),
            start_model: context.model_point_from_surface(event.location())?,
            parts,
            kind,
            modifiers: request_modifiers(event.modifiers()),
            collapse_on_click,
            drag_started: false,
            feedback: Vec::new(),
        });
        Ok((ToolDirective::Continue, outcome))
    }

    fn mouse_moved(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = context.dispatch_moved(event.location());
        <Self as Tool<A>>::refresh(self, context, event.location(), event.revision())?;
        Ok((ToolDirective::Continue, dispatch))
    }

    fn mouse_released(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = context.dispatch_released(event.location(), event.button());
        let Some(gesture) = self.gesture.as_mut() else {
            return Ok((ToolDirective::Continue, dispatch));
        };
        context.remove_overlay_visuals(&gesture.feedback)?;
        gesture.feedback.clear();
        let gesture = self
            .gesture
            .take()
            .expect("validated SelectionTool gesture must remain active");
        let surface_delta = event.location() - gesture.start_surface;
        let model_location = context.model_point_from_surface(event.location())?;
        let delta = model_location - gesture.start_model;
        if surface_delta.length() < DRAG_START_DISTANCE
            && let Some(part) = gesture.collapse_on_click
        {
            context.replace_selection(part)?;
            context.set_focus(Some(part))?;
        }
        if event.button() == MouseButton::Left && surface_delta.length() >= DRAG_START_DISTANCE {
            let request = change_bounds_request(&gesture, model_location, delta, event.revision());
            let metadata = RequestMetadata::new(request.revision(), request.parts().to_vec());
            if let Some(command) = context.command_for::<ChangeBounds>(&metadata, &request)? {
                context.submit_command(command)?;
            }
        }
        Ok((ToolDirective::Continue, dispatch))
    }

    fn has_active_gesture(&self) -> bool {
        self.is_active()
    }

    fn supports_autoexpose(&self) -> bool {
        self.is_dragging()
    }

    fn dragged_parts(&self) -> Option<&[EditPartId]> {
        SelectionTool::dragged_parts(self)
    }

    fn refresh(
        &mut self,
        context: &mut ToolContext<'_, A>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<(), ToolError> {
        let Some(gesture) = &mut self.gesture else {
            return Ok(());
        };
        if (location - gesture.start_surface).length() < DRAG_START_DISTANCE {
            return Ok(());
        }
        gesture.drag_started = true;
        let model_location = context.model_point_from_surface(location)?;
        let delta = model_location - gesture.start_model;
        context.remove_overlay_visuals(&gesture.feedback)?;
        gesture.feedback.clear();
        let request = change_bounds_request(gesture, model_location, delta, revision);
        let metadata = RequestMetadata::new(request.revision(), request.parts().to_vec());
        gesture.feedback = context.show_feedback::<ChangeBounds>(&metadata, &request)?;
        Ok(())
    }

    fn clear_transient_feedback(
        &mut self,
        context: &mut ToolContext<'_, A>,
    ) -> Result<(), ToolError> {
        if let Some(gesture) = self.gesture.as_mut() {
            context.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        Ok(())
    }
}

impl<A: ModelAdapter + 'static> Tool<A> for ConnectionBendpointTool {
    fn key(&self) -> ErasedToolKey {
        ToolKey::<Self>::new("connection-bendpoint").erase()
    }

    fn cancel(&mut self, context: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some(gesture) = self.gesture.as_mut() {
            context.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        self.gesture = None;
        Ok(())
    }

    fn mouse_pressed(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError> {
        let outcome = context.dispatch_pressed(event, false)?;
        if !outcome.dispatch().is_handled()
            && event.button() == MouseButton::Left
            && let Some((connection, operation)) = self.pending.take()
        {
            self.gesture = Some(BendpointGesture {
                connection,
                operation,
                start_surface: event.location(),
                modifiers: request_modifiers(event.modifiers()),
                drag_started: false,
                feedback: Vec::new(),
            });
        }
        Ok((ToolDirective::Continue, outcome))
    }

    fn mouse_moved(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = context.dispatch_moved(event.location());
        <Self as Tool<A>>::refresh(self, context, event.location(), event.revision())?;
        Ok((ToolDirective::Continue, dispatch))
    }

    fn mouse_released(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = context.dispatch_released(event.location(), event.button());
        let Some(gesture) = self.gesture.as_mut() else {
            return Ok((ToolDirective::Deactivate, dispatch));
        };
        context.remove_overlay_visuals(&gesture.feedback)?;
        gesture.feedback.clear();
        let gesture = self
            .gesture
            .take()
            .expect("validated bendpoint gesture must remain active");
        if event.button() == MouseButton::Left
            && (event.location() - gesture.start_surface).length() >= DRAG_START_DISTANCE
        {
            let location =
                context.connection_point_from_surface(gesture.connection, event.location())?;
            let request = BendpointRequest::new(
                gesture.connection,
                gesture.operation,
                location,
                gesture.modifiers,
                event.revision(),
            );
            let metadata =
                RequestMetadata::new(request.revision(), vec![request.connection().edit_part()]);
            if let Some(command) = context.command_for::<EditBendpoint>(&metadata, &request)? {
                context.submit_command(command)?;
            }
        }
        Ok((ToolDirective::Deactivate, dispatch))
    }

    fn has_active_gesture(&self) -> bool {
        self.is_active()
    }

    fn supports_autoexpose(&self) -> bool {
        self.is_dragging()
    }

    fn refresh(
        &mut self,
        context: &mut ToolContext<'_, A>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<(), ToolError> {
        let Some(gesture) = &mut self.gesture else {
            return Ok(());
        };
        if (location - gesture.start_surface).length() < DRAG_START_DISTANCE {
            return Ok(());
        }
        gesture.drag_started = true;
        let location = context.connection_point_from_surface(gesture.connection, location)?;
        context.remove_overlay_visuals(&gesture.feedback)?;
        gesture.feedback.clear();
        let request = BendpointRequest::new(
            gesture.connection,
            gesture.operation,
            location,
            gesture.modifiers,
            revision,
        );
        let metadata =
            RequestMetadata::new(request.revision(), vec![request.connection().edit_part()]);
        gesture.feedback = context.show_feedback::<EditBendpoint>(&metadata, &request)?;
        Ok(())
    }

    fn clear_transient_feedback(
        &mut self,
        context: &mut ToolContext<'_, A>,
    ) -> Result<(), ToolError> {
        if let Some(gesture) = self.gesture.as_mut() {
            context.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        Ok(())
    }
}

impl<A: ModelAdapter + 'static> Tool<A> for ConnectionEndpointTool<A> {
    fn key(&self) -> ErasedToolKey {
        ToolKey::<Self>::new("connection-endpoint").erase()
    }

    fn cancel(&mut self, context: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some(gesture) = self.gesture.as_mut() {
            context.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        self.gesture = None;
        Ok(())
    }

    fn mouse_pressed(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError> {
        let outcome = context.dispatch_pressed(event, false)?;
        if outcome.dispatch().is_handled() || event.button() != MouseButton::Left {
            return Ok((ToolDirective::Deactivate, outcome));
        }
        let request_location = context.connection_routing_point_from_surface(event.location())?;
        let request = ReconnectConnectionRequest::new(
            self.connection,
            self.endpoint,
            request_location,
            request_modifiers(event.modifiers()),
            event.revision(),
        );
        if let Some(plan) = context.start_connection_reconnection(&request)? {
            self.gesture = Some(ReconnectGesture {
                start: event.location(),
                modifiers: request_modifiers(event.modifiers()),
                plan,
                drag_started: false,
                feedback: Vec::new(),
            });
            Ok((ToolDirective::Continue, outcome))
        } else {
            Ok((ToolDirective::Deactivate, outcome))
        }
    }

    fn mouse_moved(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = context.dispatch_moved(event.location());
        <Self as Tool<A>>::refresh(self, context, event.location(), event.revision())?;
        Ok((ToolDirective::Continue, dispatch))
    }

    fn mouse_released(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = context.dispatch_released(event.location(), event.button());
        let Some(gesture) = self.gesture.as_mut() else {
            return Ok((ToolDirective::Deactivate, dispatch));
        };
        context.remove_overlay_visuals(&gesture.feedback)?;
        gesture.feedback.clear();
        let mut gesture = self
            .gesture
            .take()
            .expect("validated reconnect gesture must remain active");
        if event.button() == MouseButton::Left
            && (event.location() - gesture.start).length() >= DRAG_START_DISTANCE
        {
            let candidate = match context.target_at(event.location()) {
                ViewerTarget::Part(part) => Some(part),
                ViewerTarget::Handle { .. } | ViewerTarget::Contents(_) => None,
            };
            let request_location =
                context.connection_routing_point_from_surface(event.location())?;
            let request = ReconnectConnectionRequest::new(
                self.connection,
                self.endpoint,
                request_location,
                gesture.modifiers,
                event.revision(),
            )
            .with_target_candidate(candidate);
            if let Some(command) = context.reconnection_command(gesture.plan.as_mut(), &request)? {
                context.submit_command(command)?;
            }
        }
        Ok((ToolDirective::Deactivate, dispatch))
    }

    fn has_active_gesture(&self) -> bool {
        self.is_active()
    }

    fn supports_autoexpose(&self) -> bool {
        self.is_dragging()
    }

    fn refresh(
        &mut self,
        context: &mut ToolContext<'_, A>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<(), ToolError> {
        let Some(gesture) = &mut self.gesture else {
            return Ok(());
        };
        if (location - gesture.start).length() < DRAG_START_DISTANCE {
            return Ok(());
        }
        gesture.drag_started = true;
        let candidate = match context.target_at(location) {
            ViewerTarget::Part(part) => Some(part),
            ViewerTarget::Handle { .. } | ViewerTarget::Contents(_) => None,
        };
        let request_location = context.connection_routing_point_from_surface(location)?;
        let request = ReconnectConnectionRequest::new(
            self.connection,
            self.endpoint,
            request_location,
            gesture.modifiers,
            revision,
        )
        .with_target_candidate(candidate);
        context.remove_overlay_visuals(&gesture.feedback)?;
        gesture.feedback.clear();
        gesture.feedback = context.show_reconnection_feedback(gesture.plan.as_mut(), &request)?;
        context.add_autoexpose_range_reserve(location, &mut gesture.feedback)?;
        Ok(())
    }

    fn clear_transient_feedback(
        &mut self,
        context: &mut ToolContext<'_, A>,
    ) -> Result<(), ToolError> {
        if let Some(gesture) = self.gesture.as_mut() {
            context.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        Ok(())
    }
}

impl<A: ModelAdapter + 'static> Tool<A> for ConnectionCreationTool<A> {
    fn key(&self) -> ErasedToolKey {
        ToolKey::<Self>::new("connection-creation").erase()
    }

    fn cancel(&mut self, context: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some(gesture) = self.gesture.as_mut() {
            context.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        self.gesture = None;
        Ok(())
    }

    fn mouse_pressed(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError> {
        let outcome = context.dispatch_pressed(event, false)?;
        if outcome.dispatch().is_handled() || event.button() != MouseButton::Left {
            return Ok((ToolDirective::Continue, outcome));
        }
        let ViewerTarget::Part(target) = outcome.target() else {
            return Ok((ToolDirective::Continue, outcome));
        };
        let request_location = context.connection_routing_point_from_surface(event.location())?;
        if let Some(gesture) = &mut self.gesture {
            let request = CreateConnectionRequest::new(
                gesture.connection_type.clone(),
                gesture.source,
                request_location,
                gesture.modifiers,
                event.revision(),
            )
            .with_target_candidate(Some(target));
            context.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
            if let Some(command) = context.connection_command(gesture.plan.as_mut(), &request)? {
                context.submit_command(command)?;
                self.gesture = None;
                return Ok((ToolDirective::Deactivate, outcome));
            }
            gesture.feedback = context.show_connection_feedback(gesture.plan.as_mut(), &request)?;
            return Ok((ToolDirective::Continue, outcome));
        }
        let request = CreateConnectionRequest::new(
            self.connection_type.clone(),
            target,
            request_location,
            request_modifiers(event.modifiers()),
            event.revision(),
        );
        let Some(mut plan) = context.start_connection_creation(&request)? else {
            return Ok((ToolDirective::Continue, outcome));
        };
        let feedback = context.show_connection_feedback(plan.as_mut(), &request)?;
        self.gesture = Some(ConnectionGesture {
            source: target,
            connection_type: self.connection_type.clone(),
            modifiers: request_modifiers(event.modifiers()),
            plan,
            feedback,
        });
        Ok((ToolDirective::Continue, outcome))
    }

    fn mouse_moved(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = context.dispatch_moved(event.location());
        <Self as Tool<A>>::refresh(self, context, event.location(), event.revision())?;
        Ok((ToolDirective::Continue, dispatch))
    }

    fn mouse_released(
        &mut self,
        context: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        Ok((
            ToolDirective::Continue,
            context.dispatch_released(event.location(), event.button()),
        ))
    }

    fn has_active_gesture(&self) -> bool {
        self.is_started()
    }

    fn supports_autoexpose(&self) -> bool {
        self.is_started()
    }

    fn refresh(
        &mut self,
        context: &mut ToolContext<'_, A>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<(), ToolError> {
        let Some(gesture) = &mut self.gesture else {
            return Ok(());
        };
        let target = match context.target_at(location) {
            ViewerTarget::Part(part) => Some(part),
            ViewerTarget::Handle { .. } | ViewerTarget::Contents(_) => None,
        };
        let request_location = context.connection_routing_point_from_surface(location)?;
        let request = CreateConnectionRequest::new(
            gesture.connection_type.clone(),
            gesture.source,
            request_location,
            gesture.modifiers,
            revision,
        )
        .with_target_candidate(target);
        context.remove_overlay_visuals(&gesture.feedback)?;
        gesture.feedback.clear();
        gesture.feedback = context.show_connection_feedback(gesture.plan.as_mut(), &request)?;
        context.add_autoexpose_range_reserve(location, &mut gesture.feedback)?;
        Ok(())
    }

    fn clear_transient_feedback(
        &mut self,
        context: &mut ToolContext<'_, A>,
    ) -> Result<(), ToolError> {
        if let Some(gesture) = self.gesture.as_mut() {
            context.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        Ok(())
    }
}

fn request_modifiers(modifiers: KeyModifiers) -> RequestModifiers {
    RequestModifiers {
        shift: modifiers.shift,
        control: modifiers.control,
        alt: modifiers.alt,
        meta: modifiers.meta,
    }
}

fn change_bounds_request(
    gesture: &DragGesture,
    location: Point,
    delta: Vec2,
    revision: InteractionRevision,
) -> ChangeBoundsRequest {
    match gesture.kind {
        DragKind::Move => ChangeBoundsRequest::moving(
            gesture.parts.clone(),
            location,
            delta,
            gesture.modifiers,
            revision,
        ),
        DragKind::Resize(direction) => {
            let (move_delta, size_delta) = resize_deltas(direction, delta);
            ChangeBoundsRequest::resizing(
                gesture.parts.clone(),
                location,
                move_delta,
                size_delta,
                direction,
                gesture.modifiers,
                revision,
            )
        }
    }
}

fn resize_deltas(direction: ResizeDirection, delta: Vec2) -> (Vec2, Dimension) {
    let dx = delta.x();
    let dy = delta.y();
    match direction {
        ResizeDirection::North => (Vec2::new(0.0, dy), Dimension::new(0.0, -dy)),
        ResizeDirection::NorthEast => (Vec2::new(0.0, dy), Dimension::new(dx, -dy)),
        ResizeDirection::East => (Vec2::ZERO, Dimension::new(dx, 0.0)),
        ResizeDirection::SouthEast => (Vec2::ZERO, Dimension::new(dx, dy)),
        ResizeDirection::South => (Vec2::ZERO, Dimension::new(0.0, dy)),
        ResizeDirection::SouthWest => (Vec2::new(dx, 0.0), Dimension::new(-dx, dy)),
        ResizeDirection::West => (Vec2::new(dx, 0.0), Dimension::new(-dx, 0.0)),
        ResizeDirection::NorthWest => (delta, Dimension::new(-dx, -dy)),
    }
}
