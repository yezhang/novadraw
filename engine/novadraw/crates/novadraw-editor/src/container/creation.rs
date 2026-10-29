use super::{
    CandidateId, ConstraintGeometry, ContainerEditError, GeometryTolerance, MAX_CONTAINMENT_DEPTH,
    PolicyGeneration,
    geometry::{finite_point, inverse, map_point},
};
use crate::{
    Command, ContributionLimit, EditPartId, EditorNamespace, FeedbackVisual, InteractionRevision,
    ModelAdapter, ModelRevision, PartKind, RequestMetadata, RequestModifiers, RequestPayload,
    RequestProtocol, Tool, ToolContext, ToolDirective, ToolError, ToolKey, ToolMouseEvent,
    ViewerInputOutcome,
};
use novadraw::{
    event::{DispatchOutcome, MouseButton},
    geometry::{Affine2D, Dimension, Point, Rectangle},
    tree::FigureId,
};
use uuid::Uuid;

/// Framework snapshot of a press in the root contents domain.
///
/// ```compile_fail
/// use novadraw_editor::container::CreationStart;
/// use novadraw::geometry::Point;
/// fn forge(start: &mut CreationStart, point: Point) {
///     start.point = point;
/// }
/// ```
#[derive(Clone, Debug)]
pub struct CreationStart {
    namespace: EditorNamespace,
    revision: ModelRevision,
    contents: EditPartId,
    point: Point,
    surface: Point,
    modifiers: RequestModifiers,
}

struct CreationGeometry {
    origin: Point,
    end: Option<Point>,
    size: Dimension,
}

impl CreationStart {
    /// Returns the fixed root-contents creation origin.
    pub const fn point(&self) -> Point {
        self.point
    }
    /// Returns the baseline model revision.
    pub const fn revision(&self) -> ModelRevision {
        self.revision
    }
}

/// Parent-domain creation input, with no existing source or removal permission.
///
/// ```compile_fail
/// use novadraw_editor::{EditPartId, container::CreationCandidate};
/// fn forge(candidate: &mut CreationCandidate<u32>, target: EditPartId) {
///     candidate.target = target;
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct CreationCandidate<I> {
    id: CandidateId,
    namespace: EditorNamespace,
    target: EditPartId,
    target_model: I,
    revision: ModelRevision,
    interaction: InteractionRevision,
    generation: PolicyGeneration,
    bounds: Rectangle,
    pointer: Point,
    target_to_surface: Affine2D,
    modifiers: RequestModifiers,
}

impl<I: Copy> CreationCandidate<I> {
    /// Returns the unique computation identity.
    pub const fn id(&self) -> CandidateId {
        self.id
    }
    /// Returns the owning Viewer.
    pub const fn namespace(&self) -> EditorNamespace {
        self.namespace
    }
    /// Returns the actual aggregation host.
    pub const fn target(&self) -> EditPartId {
        self.target
    }
    /// Returns the stable target model identity.
    pub const fn target_model(&self) -> I {
        self.target_model
    }
    /// Returns the checked model revision.
    pub const fn revision(&self) -> ModelRevision {
        self.revision
    }
    /// Returns the current input revision.
    pub const fn interaction_revision(&self) -> InteractionRevision {
        self.interaction
    }
    /// Returns the target's policy version.
    pub const fn generation(&self) -> PolicyGeneration {
        self.generation
    }
    /// Returns finite normalized bounds in the target child-content domain.
    pub const fn bounds(&self) -> Rectangle {
        self.bounds
    }
    /// Returns the current pointer in the same parent domain.
    pub const fn pointer(&self) -> Point {
        self.pointer
    }
    /// Returns the current feedback projection map.
    pub const fn target_to_surface(&self) -> Affine2D {
        self.target_to_surface
    }
    /// Returns the template's input modifiers.
    pub const fn modifiers(&self) -> RequestModifiers {
        self.modifiers
    }
}

/// Owned declaration binding creation to its complete checked candidate.
///
/// ```compile_fail
/// use novadraw_editor::container::{CreationPlanHeader, CreationCandidate};
/// fn forge(header: &mut CreationPlanHeader<u32>, candidate: CreationCandidate<u32>) {
///     header.candidate = candidate;
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct CreationPlanHeader<I> {
    candidate: CreationCandidate<I>,
}

impl<I: Copy> CreationPlanHeader<I> {
    /// Copies the complete parent-domain declaration.
    pub fn from_candidate(candidate: &CreationCandidate<I>) -> Self {
        Self {
            candidate: candidate.clone(),
        }
    }
    /// Returns all declared candidate fields.
    pub const fn candidate(&self) -> &CreationCandidate<I> {
        &self.candidate
    }
}

/// Immutable application creation plan with one model transaction.
pub trait CreationPlan<A: ModelAdapter>: 'static {
    /// Returns a declaration read exactly once by the shared checker.
    fn header(&self) -> &CreationPlanHeader<A::ModelId>;
    /// Returns visuals already projected to the existing feedback domain.
    fn feedback(&self) -> Result<Vec<FeedbackVisual>, ContainerEditError>;
    /// Builds an atomic model command after validation and successful cleanup.
    fn build_command(self, model: &A) -> Result<Option<Box<dyn Command<A>>>, ContainerEditError>;
}

/// Open parent-domain template consumed by both creation gestures and host drops.
pub trait ContainerCreationTemplate<A: ModelAdapter>: 'static {
    /// Application-specific immutable input.
    type Payload: RequestPayload;
    /// Application-specific creation plan.
    type Plan: CreationPlan<A>;
    /// Ordinary registry request family.
    type Request: RequestProtocol<A, Payload = Self::Payload, Contribution = Self::Plan>;
    /// Returns the click/drop size in target-parent logical units.
    fn default_size(&self) -> Dimension;
    /// Builds input for the current complete candidate.
    fn payload(&self, candidate: &CreationCandidate<A::ModelId>) -> Self::Payload;
}

/// Creation validation result with a private constructor.
///
/// ```compile_fail
/// use novadraw_editor::{ModelAdapter, container::{
///     CheckedCreationPlan, ContainerCreationTemplate, CreationPlanHeader,
/// }};
/// fn forge<A: ModelAdapter, T: ContainerCreationTemplate<A>>(
///     plan: T::Plan, header: CreationPlanHeader<A::ModelId>,
/// ) {
///     let checked: CheckedCreationPlan<A, T> = CheckedCreationPlan { plan, header };
/// }
/// ```
pub struct CheckedCreationPlan<A: ModelAdapter, T: ContainerCreationTemplate<A>> {
    pub(crate) plan: T::Plan,
    header: CreationPlanHeader<A::ModelId>,
}

impl<A: ModelAdapter, T: ContainerCreationTemplate<A>> CheckedCreationPlan<A, T> {
    /// Returns the owned declaration used for validation.
    pub const fn header(&self) -> &CreationPlanHeader<A::ModelId> {
        &self.header
    }
}

impl<A: ModelAdapter + 'static> ToolContext<'_, A> {
    /// Synchronizes and records a finite press in root contents coordinates.
    pub fn begin_container_creation(
        &mut self,
        surface: Point,
        modifiers: RequestModifiers,
    ) -> Result<CreationStart, ToolError> {
        self.synchronize_model()?;
        finite_point(surface)?;
        let contents = self.contents();
        let map = inverse(self.child_content_to_surface(contents)?)?
            .ok_or(ContainerEditError::Singular)?;
        Ok(CreationStart {
            namespace: self.part_tree().namespace(),
            revision: self.model().revision(),
            contents,
            point: map_point(map, surface)?,
            surface,
            modifiers,
        })
    }

    /// Cancels a creation snapshot when model identity or revision has changed.
    pub fn validate_container_creation(&mut self, start: &CreationStart) -> Result<(), ToolError> {
        self.synchronize_model()?;
        if self.part_tree().namespace() != start.namespace
            || self.contents() != start.contents
            || self.model().revision() != start.revision
        {
            return Err(ContainerEditError::Invalidated.into());
        }
        Ok(())
    }

    fn creation_candidate(
        &self,
        target: EditPartId,
        pointer: Point,
        geometry: CreationGeometry,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> Result<CreationCandidate<A::ModelId>, ToolError> {
        let node = self
            .part_tree()
            .get(target)
            .filter(|node| node.is_active() && node.kind() == PartKind::Containment)
            .ok_or(ContainerEditError::Identity(target))?;
        let target_model = node
            .model_id()
            .ok_or(ContainerEditError::Identity(target))?;
        let target_to_surface = self.child_content_to_surface(target)?;
        let parent_inverse = inverse(target_to_surface)?.ok_or(ContainerEditError::Singular)?;
        let pointer = map_point(parent_inverse, pointer)?;
        let origin = finite_point(geometry.origin)?;
        let bounds = if let Some(end) = geometry.end {
            let end = finite_point(end)?;
            let delta = finite_point(Point::new(end.x() - origin.x(), end.y() - origin.y()))?;
            Rectangle::new(
                origin.x().min(end.x()),
                origin.y().min(end.y()),
                delta.x().abs(),
                delta.y().abs(),
            )
        } else {
            Rectangle::new(
                origin.x(),
                origin.y(),
                geometry.size.width,
                geometry.size.height,
            )
        };
        // Includes dimension, corner, and derived-overflow checks, also for degenerate bounds.
        let bounds = ConstraintGeometry::xy(bounds)?.xy_bounds(GeometryTolerance::EXACT)?;
        Ok(CreationCandidate {
            id: CandidateId(Uuid::new_v4()),
            namespace: self.part_tree().namespace(),
            target,
            target_model,
            revision: self.model().revision(),
            interaction: revision,
            generation: self.policy_generation(target),
            bounds,
            pointer,
            target_to_surface,
            modifiers,
        })
    }

    /// Reprojects a fixed root-contents press and maps both current endpoints to the target.
    pub fn container_creation_candidate(
        &mut self,
        start: &CreationStart,
        target: EditPartId,
        pointer: Point,
        default_size: Dimension,
        dragging: bool,
        revision: InteractionRevision,
    ) -> Result<CreationCandidate<A::ModelId>, ToolError> {
        self.validate_container_creation(start)?;
        let origin = map_point(self.child_content_to_surface(start.contents)?, start.point)?;
        let parent_inverse =
            inverse(self.child_content_to_surface(target)?)?.ok_or(ContainerEditError::Singular)?;
        self.creation_candidate(
            target,
            pointer,
            CreationGeometry {
                origin: map_point(parent_inverse, origin)?,
                size: default_size,
                end: if dragging {
                    Some(map_point(parent_inverse, pointer)?)
                } else {
                    None
                },
            },
            start.modifiers,
            revision,
        )
    }

    /// Maps a drop using an explicit grab offset measured in target-parent units.
    pub fn container_drop_candidate(
        &mut self,
        target: EditPartId,
        pointer: Point,
        default_size: Dimension,
        grab_offset: Point,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> Result<CreationCandidate<A::ModelId>, ToolError> {
        self.synchronize_model()?;
        finite_point(grab_offset)?;
        let map = self.child_content_to_surface(target)?;
        let local = map_point(inverse(map)?.ok_or(ContainerEditError::Singular)?, pointer)?;
        let origin = finite_point(Point::new(
            local.x() - grab_offset.x(),
            local.y() - grab_offset.y(),
        ))?;
        self.creation_candidate(
            target,
            pointer,
            CreationGeometry {
                origin,
                size: default_size,
                end: None,
            },
            modifiers,
            revision,
        )
    }

    /// Resolves and validates one creation plan at its actual host without source permissions.
    pub fn resolve_container_creation<T: ContainerCreationTemplate<A>>(
        &mut self,
        template: &T,
        candidate: &CreationCandidate<A::ModelId>,
    ) -> Result<Option<CheckedCreationPlan<A, T>>, ToolError> {
        self.validate_creation_candidate(candidate)?;
        let metadata = RequestMetadata::new(candidate.interaction, vec![]);
        let mut plans = self
            .aggregate_at::<T::Request>(
                candidate.target,
                &metadata,
                &template.payload(candidate),
                ContributionLimit::AtMostOne,
            )?
            .into_contributions();
        let Some(plan) = plans.pop() else {
            return Ok(None);
        };
        let header = plan.header().clone();
        if header != CreationPlanHeader::from_candidate(candidate) {
            return Err(ContainerEditError::Mismatch(
                "creation header differs from current candidate",
            )
            .into());
        }
        if self.model().revision() != candidate.revision
            || self.policy_generation(candidate.target) != candidate.generation
        {
            return Err(ContainerEditError::Invalidated.into());
        }
        Ok(Some(CheckedCreationPlan { plan, header }))
    }

    fn validate_creation_candidate(
        &mut self,
        candidate: &CreationCandidate<A::ModelId>,
    ) -> Result<(), ToolError> {
        self.synchronize_model()?;
        if candidate.namespace != self.part_tree().namespace()
            || candidate.revision != self.model().revision()
            || candidate.generation != self.policy_generation(candidate.target)
            || candidate.target_to_surface != self.child_content_to_surface(candidate.target)?
        {
            return Err(ContainerEditError::Invalidated.into());
        }
        Ok(())
    }

    /// Publishes checked parent-domain creation feedback through the same public tool services.
    pub fn show_container_creation_feedback<T: ContainerCreationTemplate<A>>(
        &mut self,
        checked: &CheckedCreationPlan<A, T>,
    ) -> Result<Vec<FigureId>, ToolError> {
        self.validate_creation_candidate(&checked.header.candidate)?;
        let visuals = checked.plan.feedback()?;
        self.attach_feedback(checked.header.candidate.target, visuals)
    }

    /// Revalidates after cleanup, then builds and stages one guarded creation transaction.
    pub fn submit_container_creation<T: ContainerCreationTemplate<A>>(
        &mut self,
        checked: CheckedCreationPlan<A, T>,
    ) -> Result<(), ToolError> {
        self.validate_creation_candidate(&checked.header.candidate)?;
        if let Some(command) = checked.plan.build_command(self.model())? {
            self.submit_command(super::guard_command(
                command,
                checked.header.candidate.revision,
            ))?;
        }
        Ok(())
    }

    /// Probes containment ancestors for a drop, using the same checked creation protocol.
    pub fn resolve_container_drop<T: ContainerCreationTemplate<A>>(
        &mut self,
        template: &T,
        pointer: Point,
        grab_offset: Point,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> Result<Option<CheckedCreationPlan<A, T>>, ToolError> {
        self.synchronize_model()?;
        let mut target = self.container_target_at(pointer, &[])?;
        for _ in 0..MAX_CONTAINMENT_DEPTH {
            let candidate = self.container_drop_candidate(
                target,
                pointer,
                template.default_size(),
                grab_offset,
                modifiers,
                revision,
            )?;
            if let Some(plan) = self.resolve_container_creation(template, &candidate)? {
                return Ok(Some(plan));
            }
            let Some(parent) = self.part_tree().parent(target) else {
                return Ok(None);
            };
            if parent == self.part_tree().root() {
                return Ok(None);
            }
            target = parent;
        }
        Err(ContainerEditError::Batch("creation candidate depth exceeds 10000").into())
    }
}

struct Gesture {
    start: CreationStart,
    dragging: bool,
    visited: Vec<(EditPartId, PolicyGeneration)>,
    feedback: Vec<FigureId>,
}

/// One-shot creation using the same open template as programmatic parent-domain drops.
pub struct ContainerCreationTool<T> {
    template: T,
    gesture: Option<Gesture>,
}

impl<T> ContainerCreationTool<T> {
    /// Creates an idle template tool.
    pub const fn new(template: T) -> Self {
        Self {
            template,
            gesture: None,
        }
    }

    fn plan<A: ModelAdapter + 'static>(
        template: &T,
        ctx: &mut ToolContext<'_, A>,
        gesture: &mut Gesture,
        pointer: Point,
        revision: InteractionRevision,
    ) -> Result<Option<CheckedCreationPlan<A, T>>, ToolError>
    where
        T: ContainerCreationTemplate<A>,
    {
        ctx.validate_container_creation(&gesture.start)?;
        if gesture
            .visited
            .iter()
            .any(|(host, generation)| ctx.policy_generation(*host) != *generation)
        {
            return Err(ContainerEditError::Invalidated.into());
        }
        let delta = finite_point(Point::new(
            pointer.x() - gesture.start.surface.x(),
            pointer.y() - gesture.start.surface.y(),
        ))?;
        gesture.dragging |= delta.x().abs() >= crate::tool::DRAG_START_DISTANCE
            || delta.y().abs() >= crate::tool::DRAG_START_DISTANCE;
        let mut target = ctx.container_target_at(pointer, &[])?;
        for _ in 0..MAX_CONTAINMENT_DEPTH {
            if !gesture.visited.iter().any(|(host, _)| *host == target) {
                gesture
                    .visited
                    .push((target, ctx.policy_generation(target)));
            }
            let candidate = ctx.container_creation_candidate(
                &gesture.start,
                target,
                pointer,
                template.default_size(),
                gesture.dragging,
                revision,
            )?;
            if let Some(plan) = ctx.resolve_container_creation(template, &candidate)? {
                return Ok(Some(plan));
            }
            let Some(parent) = ctx.part_tree().parent(target) else {
                return Ok(None);
            };
            if parent == ctx.part_tree().root() {
                return Ok(None);
            }
            target = parent;
        }
        Err(ContainerEditError::Batch("creation candidate depth exceeds 10000").into())
    }
}

impl<A: ModelAdapter + 'static, T: ContainerCreationTemplate<A>> Tool<A>
    for ContainerCreationTool<T>
{
    fn key(&self) -> crate::ErasedToolKey {
        ToolKey::<Self>::new("container-creation").erase()
    }
    fn cancel(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        self.clear_transient_feedback(ctx)?;
        self.gesture = None;
        Ok(())
    }
    fn deactivate(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        self.cancel(ctx)
    }
    fn mouse_pressed(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError> {
        self.cancel(ctx)?;
        ctx.synchronize_model()?;
        let outcome = ctx.dispatch_pressed(event, false)?;
        if !outcome.dispatch().is_handled() && event.button() == MouseButton::Left {
            let modifiers = event.modifiers();
            self.gesture = Some(Gesture {
                start: ctx.begin_container_creation(
                    event.location(),
                    RequestModifiers {
                        shift: modifiers.shift,
                        control: modifiers.control,
                        alt: modifiers.alt,
                        meta: modifiers.meta,
                    },
                )?,
                dragging: false,
                visited: vec![],
                feedback: vec![],
            });
        }
        Ok((ToolDirective::Continue, outcome))
    }
    fn mouse_moved(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = ctx.dispatch_moved(event.location());
        self.refresh(ctx, event.location(), event.revision())?;
        Ok((
            if self.gesture.is_some() {
                ToolDirective::Continue
            } else {
                ToolDirective::ReleaseCapture
            },
            dispatch,
        ))
    }
    fn mouse_released(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = ctx.dispatch_released(event.location(), event.button());
        if event.button() != MouseButton::Left {
            return Ok((ToolDirective::Continue, dispatch));
        }
        let Some(gesture) = &mut self.gesture else {
            return Ok((ToolDirective::ReleaseCapture, dispatch));
        };
        let result = Self::plan(
            &self.template,
            ctx,
            gesture,
            event.location(),
            event.revision(),
        );
        self.cancel(ctx)?;
        match result {
            Ok(Some(checked)) => {
                ctx.submit_container_creation(checked)?;
            }
            Ok(None) => {}
            Err(ToolError::Container(ContainerEditError::Invalidated)) => {
                return Ok((ToolDirective::ReleaseCapture, dispatch));
            }
            Err(error) => return Err(error),
        }
        Ok((ToolDirective::Deactivate, dispatch))
    }
    fn has_active_gesture(&self) -> bool {
        self.gesture.is_some()
    }
    fn supports_autoexpose(&self) -> bool {
        self.gesture.is_some()
    }
    fn refresh(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<(), ToolError> {
        self.clear_transient_feedback(ctx)?;
        let Some(gesture) = &mut self.gesture else {
            return Ok(());
        };
        let result = Self::plan(&self.template, ctx, gesture, location, revision);
        let result = match result {
            Ok(Some(checked)) => (|| {
                gesture.feedback = ctx.show_container_creation_feedback(&checked)?;
                ctx.add_autoexpose_range_reserve(location, &mut gesture.feedback)?;
                Ok(())
            })(),
            Ok(None) => Ok(()),
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => Ok(()),
            Err(ToolError::Container(ContainerEditError::Invalidated)) => self.cancel(ctx),
            Err(error) => {
                self.cancel(ctx)?;
                Err(error)
            }
        }
    }
    fn clear_transient_feedback(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some(gesture) = &mut self.gesture {
            ctx.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        Ok(())
    }
}
