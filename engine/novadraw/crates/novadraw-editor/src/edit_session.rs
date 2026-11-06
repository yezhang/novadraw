//! Open, typed editing participants and Viewer-owned coordination.

use std::{
    any::{Any, TypeId},
    marker::PhantomData,
    ops::Range,
    sync::Arc,
};

use novadraw::{
    figure::{FlowTextPosition, FlowTextRange},
    geometry::{Point, Rectangle, Translatable},
    render::{CaretGeometry, SelectionQuad},
    runtime::{FigureComponentUpdate, Runtime},
    tree::{FigureId, FigureTree},
};

use crate::{
    Command, DirectTextEditError, DirectTextEditSessionId, DirectTextFeature, DirectTextFeedback,
    EditPartId, FocusLossPolicy, ModelAdapter, ModelRevision, PolicyError, TextEditMode,
    TextInputEvent, TextInputSnapshot, ViewerError,
};

/// Typed identity of a controller implementation.
pub struct EditControllerKey<C: EditController>(PhantomData<fn() -> C>);

impl<C: EditController> EditControllerKey<C> {
    /// Returns the stable type identity of this controller.
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
impl<C: EditController> Copy for EditControllerKey<C> {}
impl<C: EditController> Clone for EditControllerKey<C> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<C: EditController> Default for EditControllerKey<C> {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable coordination conditions, without a copy of the domain draft.
#[derive(Clone, Debug)]
pub struct EditSessionDescriptor {
    feature: DirectTextFeature,
    revision: ModelRevision,
    mode: TextEditMode,
    focus_loss: FocusLossPolicy,
}
impl EditSessionDescriptor {
    /// Creates conditions for a policy-contributed controller.
    pub const fn new(
        feature: DirectTextFeature,
        revision: ModelRevision,
        mode: TextEditMode,
        focus_loss: FocusLossPolicy,
    ) -> Self {
        Self {
            feature,
            revision,
            mode,
            focus_loss,
        }
    }
    /// Returns the application feature.
    pub const fn feature(&self) -> &DirectTextFeature {
        &self.feature
    }
    /// Returns the captured model revision.
    pub const fn source_revision(&self) -> ModelRevision {
        self.revision
    }
    /// Returns the keyboard purpose.
    pub const fn mode(&self) -> TextEditMode {
        self.mode
    }
    /// Returns the focus-loss rule.
    pub const fn focus_loss(&self) -> FocusLossPolicy {
        self.focus_loss
    }
}

/// Same-publication geometry produced by a controller.
pub struct EditGeometry {
    /// Logical surface caret.
    pub caret: CaretGeometry,
    /// Logical surface clip of this editing feature.
    pub viewport: Rectangle,
    /// Selection quads; empty while preedit is active.
    pub selection: Vec<SelectionQuad>,
    /// Composition underline quads.
    pub preedit: Vec<SelectionQuad>,
    /// Whether the host requested a visible composition caret.
    pub caret_visible: bool,
}

impl EditGeometry {
    pub(crate) fn validate(&self) -> Result<(), ViewerError> {
        fn finite(bounds: Rectangle) -> bool {
            [bounds.x, bounds.y, bounds.width, bounds.height]
                .into_iter()
                .all(f64::is_finite)
                && bounds.width >= 0.0
                && bounds.height > 0.0
        }
        if !finite(self.viewport)
            || self.viewport.width == 0.0
            || !finite(self.caret.bounds())
            || self
                .selection
                .iter()
                .chain(&self.preedit)
                .any(|quad| !finite(quad.bounds()))
        {
            return Err(DirectTextEditError::InvalidFeedbackTarget.into());
        }
        Ok(())
    }
}

/// Scoped geometry access to the one admitted feedback target.
///
/// No mutable Runtime, model, or unrelated Figure is exposed.
pub struct EditGeometryContext<'a> {
    pub(crate) runtime: &'a mut Runtime,
    pub(crate) target: FigureId,
    pub(crate) writable: bool,
}
impl EditGeometryContext<'_> {
    /// Returns the current interaction Figure.
    pub const fn target(&self) -> FigureId {
        self.target
    }
    /// Returns the immutable scene query surface.
    pub fn tree(&self) -> &FigureTree {
        self.runtime.tree()
    }
    /// Returns the host's logical viewport when its size has been published.
    pub fn host_viewport(&mut self) -> Option<Rectangle> {
        self.runtime.frames().logical_viewport()
    }
    /// Reads an admitted typed component snapshot.
    pub fn prepared<T: Any + Send + Sync>(&self) -> Result<Arc<T>, ViewerError> {
        self.runtime
            .tree()
            .prepared_presentation(self.target)?
            .and_then(|p| p.shared_data::<T>())
            .ok_or(DirectTextEditError::InvalidFeedbackTarget.into())
    }
    /// Publishes a typed change to this feedback Figure only.
    pub fn update<U: FigureComponentUpdate>(&mut self, update: U) -> Result<(), ViewerError> {
        if !self.writable {
            return Err(DirectTextEditError::InvalidFeedbackTarget.into());
        }
        self.runtime
            .figure(self.target)?
            .update_component(update)
            .map_err(|_| DirectTextEditError::InvalidFeedbackTarget)?;
        Ok(())
    }
    /// Admits the feedback's current immutable geometry.
    pub fn stabilize(&mut self) -> Result<(), ViewerError> {
        self.runtime.frames().stabilize_for_query()?;
        Ok(())
    }
    /// Adjusts the admitted feedback target's layout rectangle only.
    pub fn set_bounds(&mut self, bounds: Rectangle) -> Result<(), ViewerError> {
        if !self.writable {
            return Err(DirectTextEditError::InvalidFeedbackTarget.into());
        }
        self.runtime.figure(self.target)?.set_bounds(bounds)?;
        Ok(())
    }
    /// Returns the feedback rectangle in logical surface coordinates.
    pub fn viewport(&self) -> Result<Rectangle, ViewerError> {
        let bounds = self.tree().figure_bounds(self.target)?;
        let transform = self
            .tree()
            .local_to_surface_transform(self.target)
            .map_err(ViewerError::Coordinate)?;
        let mut result = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        result.transform(transform);
        Ok(result)
    }
    /// Queries the built-in Flow provider; external controllers may use typed snapshots instead.
    pub fn flow_hit(&mut self, point: Point) -> Result<FlowTextPosition, ViewerError> {
        self.runtime
            .text()
            .flow_hit_test(self.target, point)
            .map_err(|_| DirectTextEditError::InvalidTextPosition.into())
    }
    /// Queries Flow movement in the current publication.
    pub fn flow_move(
        &mut self,
        position: FlowTextPosition,
        movement: novadraw::text::TextMovement,
    ) -> Result<FlowTextPosition, ViewerError> {
        self.runtime
            .text()
            .flow_move_position(self.target, position, movement)
            .map_err(|_| DirectTextEditError::InvalidTextPosition.into())
    }
    /// Queries a Flow caret.
    pub fn flow_caret(&mut self, position: FlowTextPosition) -> Result<CaretGeometry, ViewerError> {
        self.runtime
            .text()
            .flow_caret_geometry(self.target, position)
            .map_err(|_| DirectTextEditError::InvalidTextPosition.into())
    }
    /// Queries Flow selection quads.
    pub fn flow_selection(
        &mut self,
        range: FlowTextRange,
    ) -> Result<Vec<SelectionQuad>, ViewerError> {
        self.runtime
            .text()
            .flow_selection_geometry(self.target, range)
            .map_err(|_| DirectTextEditError::InvalidTextPosition.into())
    }
}

/// Domain state owned exclusively by one active editing participant.
///
/// Clones must isolate candidate mutations. Immutable documents/layouts may be shared.
/// Recoverable errors leave the published controller unchanged; unknown-state failures
/// must be returned as existing Viewer/Policy failure channels.
pub trait EditController: Clone + 'static {
    /// Applies a domain input (accept/cancel/focus/lease actions belong to the coordinator).
    fn input(
        &mut self,
        event: &TextInputEvent,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<bool, ViewerError>;
    /// Resolves a surface point and changes this controller's selection.
    fn select_at(
        &mut self,
        point: Point,
        extend: bool,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<(), ViewerError>;
    /// Prepares same-publication caret, selection and composition geometry.
    fn geometry(
        &mut self,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<EditGeometry, ViewerError>;
    /// Returns whether accepting this controller would change the source.
    fn is_changed(&self) -> bool;
    /// Returns whether composition blocks accepting the complete session.
    fn has_composition(&self) -> bool;
    /// Projects a host buffer on demand; it is not a persistent draft.
    fn input_snapshot(&self) -> Result<TextInputSnapshot, ViewerError>;
    /// Returns surface bounds for a host UTF-8 range.
    fn range_bounds(
        &self,
        range: Range<usize>,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<Rectangle, ViewerError>;
}

/// Application adapter for feedback and model-only commands.
pub trait EditSessionPlan<A: ModelAdapter, C: EditController>: 'static {
    /// Builds domain feedback through the ordinary Figure publication path.
    fn feedback(&mut self, controller: &C, model: &A) -> Result<DirectTextFeedback, PolicyError>;
    /// Checks acceptance before releasing input and feedback.
    fn validate(&mut self, _controller: &C, _model: &A) -> Result<(), PolicyError> {
        Ok(())
    }
    /// Builds one model command without retaining a live session or Figure.
    fn command(&mut self, controller: &C, model: &A) -> Result<Box<dyn Command<A>>, PolicyError>;
}

trait StoredController {
    fn clone_box(&self) -> Box<dyn StoredController>;
    fn as_any(&self) -> &dyn Any;
    fn input(
        &mut self,
        event: &TextInputEvent,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<bool, ViewerError>;
    fn select_at(
        &mut self,
        point: Point,
        extend: bool,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<(), ViewerError>;
    fn geometry(
        &mut self,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<EditGeometry, ViewerError>;
    fn is_changed(&self) -> bool;
    fn has_composition(&self) -> bool;
    fn input_snapshot(&self) -> Result<TextInputSnapshot, ViewerError>;
    fn range_bounds(
        &self,
        range: Range<usize>,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<Rectangle, ViewerError>;
}
impl<C: EditController> StoredController for C {
    fn clone_box(&self) -> Box<dyn StoredController> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn input(
        &mut self,
        e: &TextInputEvent,
        c: &mut EditGeometryContext<'_>,
    ) -> Result<bool, ViewerError> {
        EditController::input(self, e, c)
    }
    fn select_at(
        &mut self,
        p: Point,
        e: bool,
        c: &mut EditGeometryContext<'_>,
    ) -> Result<(), ViewerError> {
        EditController::select_at(self, p, e, c)
    }
    fn geometry(&mut self, c: &mut EditGeometryContext<'_>) -> Result<EditGeometry, ViewerError> {
        EditController::geometry(self, c)
    }
    fn is_changed(&self) -> bool {
        EditController::is_changed(self)
    }
    fn has_composition(&self) -> bool {
        EditController::has_composition(self)
    }
    fn input_snapshot(&self) -> Result<TextInputSnapshot, ViewerError> {
        EditController::input_snapshot(self)
    }
    fn range_bounds(
        &self,
        r: Range<usize>,
        c: &mut EditGeometryContext<'_>,
    ) -> Result<Rectangle, ViewerError> {
        EditController::range_bounds(self, r, c)
    }
}

pub(crate) trait StoredPlan<A: ModelAdapter> {
    fn feedback(
        &mut self,
        state: &EditSessionState,
        model: &A,
    ) -> Result<DirectTextFeedback, PolicyError>;
    fn validate(&mut self, state: &EditSessionState, model: &A) -> Result<(), PolicyError>;
    fn command(
        &mut self,
        state: &EditSessionState,
        model: &A,
    ) -> Result<Box<dyn Command<A>>, PolicyError>;
}
struct TypedPlan<C: EditController, P> {
    plan: P,
    key: EditControllerKey<C>,
}
impl<A: ModelAdapter, C: EditController, P: EditSessionPlan<A, C>> StoredPlan<A>
    for TypedPlan<C, P>
{
    fn feedback(&mut self, s: &EditSessionState, a: &A) -> Result<DirectTextFeedback, PolicyError> {
        self.plan.feedback(
            s.controller(self.key)
                .ok_or_else(|| PolicyError::operation("controller identity mismatch"))?,
            a,
        )
    }
    fn validate(&mut self, s: &EditSessionState, a: &A) -> Result<(), PolicyError> {
        self.plan.validate(
            s.controller(self.key)
                .ok_or_else(|| PolicyError::operation("controller identity mismatch"))?,
            a,
        )
    }
    fn command(&mut self, s: &EditSessionState, a: &A) -> Result<Box<dyn Command<A>>, PolicyError> {
        self.plan.command(
            s.controller(self.key)
                .ok_or_else(|| PolicyError::operation("controller identity mismatch"))?,
            a,
        )
    }
}

/// Open Request contribution binding one typed controller to its model adapter.
pub struct EditSessionContribution<A: ModelAdapter> {
    pub(crate) descriptor: EditSessionDescriptor,
    controller: Box<dyn FnOnce(DirectTextEditSessionId, EditPartId) -> Box<dyn StoredController>>,
    pub(crate) plan: Box<dyn StoredPlan<A>>,
    key: TypeId,
}
impl<A: ModelAdapter> EditSessionContribution<A> {
    /// Binds a controller, identity, and plan without exposing erased payloads.
    pub fn new<C: EditController, P: EditSessionPlan<A, C>>(
        key: EditControllerKey<C>,
        descriptor: EditSessionDescriptor,
        controller: C,
        plan: P,
    ) -> Self {
        Self::with_factory(key, descriptor, move |_, _| controller, plan)
    }
    /// Defers controller construction until the coordinator assigns its session identity.
    pub fn with_factory<C: EditController, P: EditSessionPlan<A, C>>(
        key: EditControllerKey<C>,
        descriptor: EditSessionDescriptor,
        factory: impl FnOnce(DirectTextEditSessionId, EditPartId) -> C + 'static,
        plan: P,
    ) -> Self {
        Self {
            descriptor,
            controller: Box::new(move |session, source| Box::new(factory(session, source))),
            plan: Box::new(TypedPlan { key, plan }),
            key: TypeId::of::<C>(),
        }
    }
    /// Adapts a plain-text policy plan to the same typed controller protocol.
    pub fn plain(plan: Box<dyn crate::DirectTextEdit<A>>) -> Self
    where
        A: 'static,
    {
        let descriptor = plan.descriptor().clone();
        Self::with_factory(
            EditControllerKey::new(),
            EditSessionDescriptor::new(
                descriptor.feature().clone(),
                descriptor.source_revision(),
                descriptor.mode(),
                descriptor.focus_loss(),
            ),
            move |session, source| crate::DirectTextEditState::new(session, source, &descriptor),
            crate::direct_edit::PlainTextPlan(plan),
        )
    }
    pub(crate) fn activate(
        self,
        session: DirectTextEditSessionId,
        source: EditPartId,
    ) -> (EditSessionState, Box<dyn StoredPlan<A>>) {
        (
            EditSessionState {
                session,
                source,
                descriptor: self.descriptor,
                controller: (self.controller)(session, source),
                key: self.key,
            },
            self.plan,
        )
    }
}

/// Read-only coordination record; domain state remains inside its typed controller.
pub struct EditSessionState {
    session: DirectTextEditSessionId,
    source: EditPartId,
    descriptor: EditSessionDescriptor,
    controller: Box<dyn StoredController>,
    key: TypeId,
}
impl Clone for EditSessionState {
    fn clone(&self) -> Self {
        Self {
            session: self.session,
            source: self.source,
            descriptor: self.descriptor.clone(),
            controller: self.controller.clone_box(),
            key: self.key,
        }
    }
}
impl EditSessionState {
    pub(crate) fn replace_controller<C: EditController>(
        &mut self,
        key: EditControllerKey<C>,
        controller: C,
    ) -> Result<(), ViewerError> {
        if self.controller(key).is_none() {
            return Err(DirectTextEditError::UnsupportedFeature.into());
        }
        self.controller = Box::new(controller);
        Ok(())
    }
    /// Returns the lease/session identity.
    pub const fn session(&self) -> DirectTextEditSessionId {
        self.session
    }
    /// Returns the active Part.
    pub const fn source(&self) -> EditPartId {
        self.source
    }
    /// Returns the source model revision.
    pub const fn source_revision(&self) -> ModelRevision {
        self.descriptor.source_revision()
    }
    /// Returns the application feature.
    pub const fn feature(&self) -> &DirectTextFeature {
        self.descriptor.feature()
    }
    /// Returns input purpose.
    pub const fn mode(&self) -> TextEditMode {
        self.descriptor.mode()
    }
    /// Returns focus-loss policy.
    pub const fn focus_loss(&self) -> FocusLossPolicy {
        self.descriptor.focus_loss()
    }
    /// Looks up domain state only when the registered controller type matches.
    pub fn controller<C: EditController>(&self, _key: EditControllerKey<C>) -> Option<&C> {
        if self.key != TypeId::of::<C>() {
            return None;
        }
        self.controller.as_any().downcast_ref()
    }
    /// Returns a platform buffer projection on demand.
    pub(crate) fn input_snapshot(&self) -> Result<TextInputSnapshot, ViewerError> {
        self.controller.input_snapshot()
    }
    pub(crate) fn is_changed(&self) -> bool {
        self.controller.is_changed()
    }
    pub(crate) fn has_composition(&self) -> bool {
        self.controller.has_composition()
    }
    pub(crate) fn input(
        &mut self,
        event: &TextInputEvent,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<bool, ViewerError> {
        self.controller.input(event, context)
    }
    pub(crate) fn select_at(
        &mut self,
        point: Point,
        extend: bool,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<(), ViewerError> {
        self.controller.select_at(point, extend, context)
    }
    pub(crate) fn geometry(
        &mut self,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<EditGeometry, ViewerError> {
        self.controller.geometry(context)
    }
    pub(crate) fn range_bounds(
        &self,
        range: Range<usize>,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<Rectangle, ViewerError> {
        self.controller.range_bounds(range, context)
    }
}
