use super::*;
use crate::{
    Command, ContributionLimit, EditPartFactory, EditPartId, EditorNamespace, FeedbackVisual,
    GraphicalViewer, InteractionRevision, ModelAdapter, ModelRevision, PartKind, RequestKey,
    RequestMetadata, RequestPayload, RequestProtocol, ResizeDirection, ToolContext, ToolError,
    container::{CandidateId, ContainerEditError, MoveSource, PolicyGeneration},
};
use novadraw::{
    geometry::{Affine2D, Dimension, Point, Rectangle, Vec2},
    tree::FigureId,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use uuid::Uuid;

/// A variable horizontal edge in the source's geometry domain.
pub struct ResizeHorizontal;
/// A variable vertical edge in the source's geometry domain.
pub struct ResizeVertical;
/// One coupled width correction; height follows the declared aspect ratio.
pub struct ResizeProportional;

/// Application-declared source geometry and finite resize limits.
#[derive(Clone, Debug, PartialEq)]
pub struct ResizeShape {
    bounds: Rectangle,
    local_to_parent: Affine2D,
    minimum: Dimension,
    ratio: Option<f64>,
}
impl ResizeShape {
    /// Declares bounds in an explicit geometry domain, its parent map and minimum dimensions.
    /// A ratio must match the nonzero baseline width/height and requires a corner gesture:
    /// a side gesture cannot change its orthogonal fixed edges while preserving that ratio.
    pub fn new(
        bounds: Rectangle,
        local_to_parent: Affine2D,
        minimum: Dimension,
        ratio: Option<f64>,
    ) -> Result<Self, SnapError> {
        checked_bounds(bounds)?;
        geometry::finite_map(local_to_parent)?;
        if local_to_parent.inverse().is_none()
            || !minimum.width.is_finite()
            || !minimum.height.is_finite()
            || minimum.width < 0.0
            || minimum.height < 0.0
            || ratio.is_some_and(|r| !r.is_finite() || r <= 0.0 || !(1.0 / r).is_finite())
            || ratio.is_some_and(|r| bounds.height <= 0.0 || bounds.width / bounds.height != r)
        {
            return Err(SnapError::Invalid("invalid resize shape limits or mapping"));
        }
        Ok(Self {
            bounds,
            local_to_parent,
            minimum,
            ratio,
        })
    }
    /// Returns baseline bounds in the explicit geometry domain.
    pub const fn bounds(&self) -> Rectangle {
        self.bounds
    }
    /// Returns the immutable geometry-to-parent map.
    pub const fn local_to_parent(&self) -> Affine2D {
        self.local_to_parent
    }
    /// Returns minimum dimensions in geometry units.
    pub const fn minimum(&self) -> Dimension {
        self.minimum
    }
    /// Returns the optional coupled width/height ratio.
    pub const fn ratio(&self) -> Option<f64> {
        self.ratio
    }
}

/// Framework-captured identity, containment and model baseline for one resize gesture.
///
/// ```compile_fail
/// use novadraw_editor::snap::ResizeStart;
/// fn forge(start: &mut ResizeStart<u32>) { start.grab = novadraw::geometry::Point::ZERO; }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct ResizeStart<I> {
    operation: Uuid,
    namespace: EditorNamespace,
    revision: ModelRevision,
    source: MoveSource<I>,
    generations: Vec<(EditPartId, PolicyGeneration)>,
    shape: ResizeShape,
    direction: ResizeDirection,
    grab: Point,
}
impl<I: Copy> ResizeStart<I> {
    /// Returns the source's original containment relation.
    pub const fn source(&self) -> &MoveSource<I> {
        &self.source
    }
    /// Returns the frozen application geometry.
    pub const fn shape(&self) -> &ResizeShape {
        &self.shape
    }
    /// Returns the handle's variable edges.
    pub const fn direction(&self) -> ResizeDirection {
        self.direction
    }
}

/// Complete current resize input; only the framework can construct or correct it.
#[derive(Clone, Debug, PartialEq)]
pub struct ResizeCandidate<I> {
    id: CandidateId,
    start: ResizeStart<I>,
    bounds: Rectangle,
    parent_to_surface: Affine2D,
    pointer: Point,
    interaction: InteractionRevision,
}
impl<I: Copy> ResizeCandidate<I> {
    /// Returns the unique computation identity.
    pub const fn id(&self) -> CandidateId {
        self.id
    }
    /// Returns the immutable gesture baseline.
    pub const fn start(&self) -> &ResizeStart<I> {
        &self.start
    }
    /// Returns current bounds in the declared geometry domain.
    pub const fn bounds(&self) -> Rectangle {
        self.bounds
    }
    /// Returns the current parent-to-surface map.
    pub const fn parent_to_surface(&self) -> Affine2D {
        self.parent_to_surface
    }
    /// Returns the unsnapped latest surface pointer.
    pub const fn pointer(&self) -> Point {
        self.pointer
    }
    /// Returns the current event revision.
    pub const fn interaction_revision(&self) -> InteractionRevision {
        self.interaction
    }
    fn map(&self) -> Result<Affine2D, SnapError> {
        geometry::finite_map(self.parent_to_surface * self.start.shape.local_to_parent)
    }
}

/// Owned complete declaration of one application resize plan.
#[derive(Clone, Debug, PartialEq)]
pub struct ResizePlanHeader<I> {
    candidate: ResizeCandidate<I>,
}
impl<I: Copy> ResizePlanHeader<I> {
    /// Copies the checked request declaration.
    pub fn from_candidate(candidate: &ResizeCandidate<I>) -> Self {
        Self {
            candidate: candidate.clone(),
        }
    }
    /// Returns every field declared by the plan.
    pub const fn candidate(&self) -> &ResizeCandidate<I> {
        &self.candidate
    }
}

/// Open immutable resize plan, with actual geometry and one atomic model transaction.
pub trait ResizePlan<A: ModelAdapter>: 'static {
    /// Returns the complete current request declaration.
    fn header(&self) -> &ResizePlanHeader<A::ModelId>;
    /// Returns actual command bounds in the source's declared geometry domain.
    fn actual_bounds(&self) -> Result<Rectangle, ContainerEditError>;
    /// Returns ordinary feedback visuals.
    fn feedback(&self) -> Result<Vec<FeedbackVisual>, ContainerEditError>;
    /// Builds at most one atomic command after cleanup and final validation.
    fn build_command(self, model: &A) -> Result<Option<Box<dyn Command<A>>>, ContainerEditError>;
}

/// Typed application request shared by built-in and external resize tools.
pub trait ResizeProtocol<A: ModelAdapter>: 'static {
    /// Owned request payload.
    type Payload: RequestPayload;
    /// Immutable actual model plan.
    type Plan: ResizePlan<A>;
    /// Ordinary target policy request family.
    type Request: RequestProtocol<A, Payload = Self::Payload, Contribution = Self::Plan>;
    /// Reads geometry once from the frozen source baseline.
    fn source_shape(
        model: &A,
        source: &MoveSource<A::ModelId>,
    ) -> Result<ResizeShape, ContainerEditError>;
    /// Builds payload from the complete current candidate.
    fn payload(candidate: &ResizeCandidate<A::ModelId>) -> Self::Payload;
}

/// Ordinary parent-policy request for additional allowed resize geometry.
pub struct ResizeLayoutConstraints;
impl<A: ModelAdapter + 'static> RequestProtocol<A> for ResizeLayoutConstraints {
    type Payload = ResizeCandidate<A::ModelId>;
    type Contribution = Vec<RuleCall>;
    fn key() -> RequestKey<Self> {
        RequestKey::new("snap-resize-layout-constraints")
    }
}

/// Public minimum-size evaluator, installed through the ordinary Viewer registry.
/// Fixed edges and aspect ratio are represented by the input's linear freedoms.
pub struct ResizeLimits;
/// Owned parameters for the ordinary minimum-size rule.
pub struct ResizeLimitsPayload {
    inverse: Affine2D,
    minimum: Dimension,
}
impl ResizeLimitsPayload {
    /// Returns the surface-to-geometry map for an external limits evaluator.
    pub const fn surface_to_geometry(&self) -> Affine2D {
        self.inverse
    }
    /// Returns the source's declared minimum dimensions.
    pub const fn minimum(&self) -> Dimension {
        self.minimum
    }
}
impl EvaluationRule for ResizeLimits {
    type Payload = ResizeLimitsPayload;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("resize-limits")
    }
    fn version(&self, _: &Self::Payload) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn evaluate(
        &self,
        payload: &Self::Payload,
        ctx: &EvaluationContext<'_>,
    ) -> Result<bool, SnapError> {
        if ctx.geometry.points().len() != 4 {
            return Err(SnapError::Invalid("resize limits require four controls"));
        }
        let points = ctx
            .geometry
            .points()
            .iter()
            .map(|&point| geometry::finite_point(payload.inverse.transform_point(point)))
            .collect::<Result<Vec<_>, _>>()?;
        let width = points[1].x() - points[0].x();
        let height = points[3].y() - points[0].y();
        if !width.is_finite() || !height.is_finite() {
            return Err(SnapError::Invalid("resize dimension overflow"));
        }
        Ok(width >= payload.minimum.width && height >= payload.minimum.height)
    }
}

/// Actual resize plan bound to retained facts and a single framework computation.
///
/// ```compile_fail
/// use novadraw_editor::{ModelAdapter, snap::{ResizeProtocol, SnappedResizePlan}};
/// fn forge<A: ModelAdapter, P: ResizeProtocol<A>>(plan: &mut SnappedResizePlan<A, P>) {
///     plan.actual = novadraw::geometry::Rectangle::new(0.0, 0.0, 0.0, 0.0);
/// }
/// ```
pub struct SnappedResizePlan<A: ModelAdapter, P: ResizeProtocol<A>> {
    plan: P::Plan,
    header: ResizePlanHeader<A::ModelId>,
    actual: Rectangle,
    solution: SnapSolution,
    geometry: SnapGeometry,
}
impl<A: ModelAdapter, P: ResizeProtocol<A>> SnappedResizePlan<A, P> {
    /// Returns the complete immutable declaration.
    pub const fn header(&self) -> &ResizePlanHeader<A::ModelId> {
        &self.header
    }
    /// Returns the actual model geometry accepted by the final gate.
    pub const fn actual_bounds(&self) -> Rectangle {
        self.actual
    }
    /// Returns the actual evaluated surface controls.
    pub const fn geometry(&self) -> &SnapGeometry {
        &self.geometry
    }
    /// Returns feedback for retained facts.
    pub fn feedback(&self) -> impl Iterator<Item = &FeedbackDeclaration> {
        self.solution.feedback()
    }
}

fn checked_bounds(bounds: Rectangle) -> Result<Rectangle, SnapError> {
    if ![
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        bounds.x + bounds.width,
        bounds.y + bounds.height,
    ]
    .into_iter()
    .all(f64::is_finite)
        || bounds.width < 0.0
        || bounds.height < 0.0
    {
        Err(SnapError::Invalid("invalid resize bounds"))
    } else {
        Ok(bounds)
    }
}
fn corners(bounds: Rectangle) -> [Point; 4] {
    [
        Point::new(bounds.x, bounds.y),
        Point::new(bounds.x + bounds.width, bounds.y),
        Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
        Point::new(bounds.x, bounds.y + bounds.height),
    ]
}
fn edges(direction: ResizeDirection) -> (Option<bool>, Option<bool>) {
    use ResizeDirection::*;
    match direction {
        North => (None, Some(true)),
        NorthEast => (Some(false), Some(true)),
        East => (Some(false), None),
        SouthEast => (Some(false), Some(false)),
        South => (None, Some(false)),
        SouthWest => (Some(true), Some(false)),
        West => (Some(true), None),
        NorthWest => (Some(true), Some(true)),
    }
}
fn freedoms<I: Copy>(candidate: &ResizeCandidate<I>) -> (Vec<DofKey>, Vec<Vec<Vec2>>) {
    let (horizontal, vertical) = edges(candidate.start.direction);
    let ratio = candidate.start.shape.ratio;
    let dofs = if ratio.is_some() {
        vec![DofKey::of::<ResizeProportional>()]
    } else {
        horizontal
            .map(|_| DofKey::of::<ResizeHorizontal>())
            .into_iter()
            .chain(vertical.map(|_| DofKey::of::<ResizeVertical>()))
            .collect()
    };
    let influences = (0..4)
        .map(|index| {
            let left = index == 0 || index == 3;
            let top = index < 2;
            if let Some(ratio) = ratio {
                vec![Vec2::new(
                    if horizontal == Some(left) {
                        if left { -1.0 } else { 1.0 }
                    } else {
                        0.0
                    },
                    if vertical == Some(top) {
                        if top { -1.0 / ratio } else { 1.0 / ratio }
                    } else {
                        0.0
                    },
                )]
            } else {
                horizontal
                    .map(|low| Vec2::new(if low == left { 1.0 } else { 0.0 }, 0.0))
                    .into_iter()
                    .chain(vertical.map(|low| Vec2::new(0.0, if low == top { 1.0 } else { 0.0 })))
                    .collect()
            }
        })
        .collect();
    (dofs, influences)
}
fn corrected_bounds<I: Copy>(
    candidate: &ResizeCandidate<I>,
    values: &[f64],
) -> Result<Rectangle, SnapError> {
    let (dofs, influences) = freedoms(candidate);
    if values.len() != dofs.len() || !values.iter().all(|v| v.is_finite()) {
        return Err(SnapError::Invalid("invalid resize corrections"));
    }
    let mut points = corners(candidate.bounds);
    for (point, influence) in points.iter_mut().zip(influences) {
        for (basis, value) in influence.iter().zip(values) {
            *point = geometry::finite_point(*point + *basis * *value)?;
        }
    }
    checked_bounds(Rectangle::new(
        points[0].x(),
        points[0].y(),
        points[1].x() - points[0].x(),
        points[3].y() - points[0].y(),
    ))
}

impl<A: ModelAdapter + 'static> ToolContext<'_, A> {
    /// Captures source identity and geometry without invoking move or reparent permissions.
    pub fn begin_resize<P: ResizeProtocol<A>>(
        &mut self,
        part: EditPartId,
        direction: ResizeDirection,
        pointer: Point,
    ) -> Result<ResizeStart<A::ModelId>, ToolError> {
        self.synchronize_model()?;
        let node = self
            .part_tree()
            .get(part)
            .filter(|node| node.is_active() && node.kind() == PartKind::Containment)
            .ok_or(SnapError::Identity)?;
        let parent = self.part_tree().parent(part).ok_or(SnapError::Identity)?;
        let parent_node = self
            .part_tree()
            .get(parent)
            .filter(|node| node.is_active() && node.kind() == PartKind::Containment)
            .ok_or(SnapError::Identity)?;
        let source = MoveSource {
            part,
            model: node.model_id().ok_or(SnapError::Identity)?,
            parent,
            parent_model: parent_node.model_id().ok_or(SnapError::Identity)?,
            index: self
                .part_tree()
                .children(parent)
                .and_then(|children| children.iter().position(|child| *child == part))
                .ok_or(SnapError::Identity)?,
        };
        let revision = self.model().revision();
        let generations = vec![
            (part, self.policy_generation(part)),
            (parent, self.policy_generation(parent)),
        ];
        let shape = P::source_shape(self.model(), &source)?;
        let (x, y) = edges(direction);
        if shape.ratio.is_some() && (x.is_none() || y.is_none()) {
            return Err(
                SnapError::Rejected("proportional resize requires two variable edges").into(),
            );
        }
        let map =
            geometry::finite_map(self.child_content_to_surface(parent)? * shape.local_to_parent)?;
        let inverse = map
            .inverse()
            .ok_or(SnapError::Invalid("singular resize map"))?;
        let grab =
            geometry::finite_point(inverse.transform_point(geometry::finite_point(pointer)?))?;
        let start = ResizeStart {
            operation: Uuid::new_v4(),
            namespace: self.part_tree().namespace(),
            revision,
            source,
            generations,
            shape,
            direction,
            grab,
        };
        self.validate_resize(&start)?;
        Ok(start)
    }
    /// Rejects changed model, containment, identity or installed policy generations.
    pub fn validate_resize(&mut self, start: &ResizeStart<A::ModelId>) -> Result<(), ToolError> {
        self.synchronize_model()?;
        let source = &start.source;
        if self.part_tree().namespace() != start.namespace
            || self.model().revision() != start.revision
            || self.part_tree().parent(source.part) != Some(source.parent)
            || self
                .part_tree()
                .children(source.parent)
                .and_then(|c| c.get(source.index))
                != Some(&source.part)
            || [
                (source.part, source.model),
                (source.parent, source.parent_model),
            ]
            .into_iter()
            .any(|(part, model)| {
                self.part_tree().get(part).is_none_or(|node| {
                    !node.is_active()
                        || node.kind() != PartKind::Containment
                        || node.model_id() != Some(model)
                })
            })
            || start
                .generations
                .iter()
                .any(|&(part, generation)| self.policy_generation(part) != generation)
        {
            return Err(SnapError::Invalidated.into());
        }
        Ok(())
    }
    /// Builds from the latest pointer and current map; never accumulates prior snap corrections.
    pub fn resize_candidate(
        &mut self,
        start: &ResizeStart<A::ModelId>,
        pointer: Point,
        interaction: InteractionRevision,
    ) -> Result<ResizeCandidate<A::ModelId>, ToolError> {
        self.validate_resize(start)?;
        let parent_to_surface =
            geometry::finite_map(self.child_content_to_surface(start.source.parent)?)?;
        let map = geometry::finite_map(parent_to_surface * start.shape.local_to_parent)?;
        let inverse = map
            .inverse()
            .ok_or(SnapError::Invalid("singular resize map"))?;
        let pointer = geometry::finite_point(pointer)?;
        let delta = geometry::finite_point(inverse.transform_point(pointer))? - start.grab;
        let mut candidate = ResizeCandidate {
            id: CandidateId(Uuid::new_v4()),
            start: start.clone(),
            bounds: start.shape.bounds,
            parent_to_surface,
            pointer,
            interaction,
        };
        let (x, y) = edges(start.direction);
        let values = if let Some(ratio) = start.shape.ratio {
            // Project the raw displacement onto the one surface-space coupled freedom.
            let basis = Vec2::new(
                if x == Some(true) { -1.0 } else { 1.0 },
                if y == Some(true) {
                    -1.0 / ratio
                } else {
                    1.0 / ratio
                },
            );
            let c = map.coeffs();
            let basis_surface = Vec2::new(
                c[0] * basis.x() + c[2] * basis.y(),
                c[1] * basis.x() + c[3] * basis.y(),
            );
            let delta_surface = Vec2::new(
                c[0] * delta.x() + c[2] * delta.y(),
                c[1] * delta.x() + c[3] * delta.y(),
            );
            let denominator = basis_surface.length_squared();
            if !denominator.is_finite() || denominator <= 0.0 {
                return Err(SnapError::Invalid("invalid proportional resize basis").into());
            }
            let value = delta_surface.dot(basis_surface) / denominator;
            vec![value]
        } else {
            x.map(|_| delta.x())
                .into_iter()
                .chain(y.map(|_| delta.y()))
                .collect()
        };
        candidate.bounds = corrected_bounds(&candidate, &values)?;
        Ok(candidate)
    }
    /// Solves once using an ordinary registered limits rule and fresh parent policy plan.
    pub fn resolve_snapped_resize<P: ResizeProtocol<A>>(
        &mut self,
        session: &mut SnapSession,
        candidate: &ResizeCandidate<A::ModelId>,
        settings: SnapSettings,
        limits: &RuleBinding<impl EvaluationRule<Payload = ResizeLimitsPayload>>,
    ) -> Result<Option<SnappedResizePlan<A, P>>, ToolError> {
        let result = (|| {
            self.validate_resize_candidate(candidate)?;
            let metadata =
                RequestMetadata::new(candidate.interaction, vec![candidate.start.source.part]);
            let mut contributions = self
                .aggregate_at::<ResizeLayoutConstraints>(
                    candidate.start.source.parent,
                    &metadata,
                    candidate,
                    ContributionLimit::AtMostOne,
                )?
                .into_contributions();
            let mut layouts = contributions.pop().unwrap_or_default();
            layouts.push(
                limits.bind(ResizeLimitsPayload {
                    inverse: candidate
                        .map()?
                        .inverse()
                        .ok_or(SnapError::Invalid("singular resize map"))?,
                    minimum: candidate.start.shape.minimum,
                }),
            );
            if layouts.len() > settings.budget().sources.get() {
                return Err(SnapError::Budget("resize layout budget exceeded").into());
            }
            let input = self.snap_resize_input(candidate, settings, layouts)?;
            let solution = self.solve_snapping(session, input)?;
            let mut corrected = candidate.clone();
            corrected.bounds = corrected_bounds(candidate, solution.geometry().values())?;
            self.validate_resize_candidate(&corrected)?;
            let payload = P::payload(&corrected);
            let mut plans = self
                .aggregate_at::<P::Request>(
                    candidate.start.source.parent,
                    &metadata,
                    &payload,
                    ContributionLimit::AtMostOne,
                )?
                .into_contributions();
            let Some(plan) = plans.pop() else {
                return Ok(None);
            };
            self.validate_resize_candidate(&corrected)?;
            let header = plan.header().clone();
            if header != ResizePlanHeader::from_candidate(&corrected) {
                return Err(SnapError::Invalid("resize plan header does not match request").into());
            }
            let actual = checked_bounds(plan.actual_bounds()?)?;
            let geometry = resize_actual_geometry(&solution, &corrected, actual)?;
            let geometry = self.validate_snapping_plan_geometry(session, &solution, geometry)?;
            let plan = SnappedResizePlan {
                plan,
                header,
                actual,
                solution,
                geometry,
            };
            self.recheck_snapped_resize(session, &plan)?;
            Ok(Some(plan))
        })();
        if !matches!(result, Ok(Some(_))) {
            session.latest_solution = None;
        }
        result
    }
    fn validate_resize_candidate(
        &mut self,
        candidate: &ResizeCandidate<A::ModelId>,
    ) -> Result<(), ToolError> {
        self.validate_resize(&candidate.start)?;
        if self.child_content_to_surface(candidate.start.source.parent)?
            != candidate.parent_to_surface
        {
            return Err(SnapError::Invalidated.into());
        }
        Ok(())
    }
    fn recheck_snapped_resize<P: ResizeProtocol<A>>(
        &mut self,
        session: &mut SnapSession,
        plan: &SnappedResizePlan<A, P>,
    ) -> Result<(), ToolError> {
        let result = (|| {
            self.validate_resize_candidate(plan.header.candidate())?;
            if plan.plan.header() != &plan.header || plan.plan.actual_bounds()? != plan.actual {
                return Err(SnapError::Invalidated.into());
            }
            let actual =
                resize_actual_geometry(&plan.solution, plan.header.candidate(), plan.actual)?;
            self.validate_snapping_plan_geometry(session, &plan.solution, actual)?;
            Ok(())
        })();
        if result.is_err() {
            session.latest_solution = None;
        }
        result
    }
    /// Renders only facts that still hold for the plan's actual geometry.
    pub fn show_snapped_resize_feedback<P: ResizeProtocol<A>>(
        &mut self,
        session: &mut SnapSession,
        plan: &SnappedResizePlan<A, P>,
    ) -> Result<Vec<FigureId>, ToolError> {
        let result = (|| {
            self.recheck_snapped_resize(session, plan)?;
            let limit = plan.solution.input.settings.budget().feedback.get();
            let declarations = plan.feedback().cloned().collect::<Vec<_>>();
            let mut visuals = self.render_snap_feedback(&declarations, &plan.geometry, limit)?;
            visuals.append(&mut plan.plan.feedback()?);
            if visuals.len() > limit {
                return Err(SnapError::Budget("combined resize feedback budget exceeded").into());
            }
            self.recheck_snapped_resize(session, plan)?;
            self.attach_feedback(plan.header.candidate.start.source.parent, visuals)
        })();
        if result.is_err() {
            session.latest_solution = None;
        }
        result
    }
    /// After successful cleanup, rechecks and constructs at most one guarded model command.
    pub fn submit_snapped_resize<P: ResizeProtocol<A>>(
        &mut self,
        session: &mut SnapSession,
        plan: SnappedResizePlan<A, P>,
    ) -> Result<(), ToolError> {
        let result = (|| {
            self.recheck_snapped_resize(session, &plan)?;
            if let Some(command) = plan.plan.build_command(self.model())? {
                self.submit_command(crate::container::guard_command(
                    command,
                    plan.header.candidate.start.revision,
                ))?;
            }
            Ok(())
        })();
        session.cancel();
        result
    }
}

impl<A: ModelAdapter, F: EditPartFactory<A>> GraphicalViewer<A, F> {
    /// Builds checked variable-edge controls through the same registry input protocol as moves.
    pub fn snap_resize_input(
        &mut self,
        candidate: &ResizeCandidate<A::ModelId>,
        settings: SnapSettings,
        layouts: Vec<RuleCall>,
    ) -> Result<SnapInput, SnapError> {
        self.snap_ready()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let start = &candidate.start;
            let source = &start.source;
            if start.namespace != self.namespace()
                || start.revision != self.model().revision()
                || start.revision != self.applied_revision()
                || self.parts().parent(source.part) != Some(source.parent)
                || self
                    .parts()
                    .children(source.parent)
                    .and_then(|c| c.get(source.index))
                    != Some(&source.part)
                || [
                    (source.part, source.model),
                    (source.parent, source.parent_model),
                ]
                .into_iter()
                .any(|(part, model)| {
                    self.parts().get(part).is_none_or(|n| {
                        !n.is_active()
                            || n.kind() != PartKind::Containment
                            || n.model_id() != Some(model)
                    })
                })
            {
                return Err(SnapError::Invalidated);
            }
            let map = candidate.map()?;
            let coefficients = map.coeffs();
            let (dofs, influences) = freedoms(candidate);
            let controls = corners(candidate.bounds)
                .into_iter()
                .zip(influences)
                .map(|(point, influence)| {
                    Ok(ControlPoint {
                        origin: geometry::finite_point(map.transform_point(point))?,
                        influence: influence
                            .into_iter()
                            .map(|v| {
                                Vec2::new(
                                    coefficients[0] * v.x() + coefficients[2] * v.y(),
                                    coefficients[1] * v.x() + coefficients[3] * v.y(),
                                )
                            })
                            .collect(),
                    })
                })
                .collect::<Result<Vec<_>, SnapError>>()?;
            let (siblings, scene) = self.snap_scene(source.parent, &[source.part], settings)?;
            let input = SnapInput {
                namespace: start.namespace,
                candidate: candidate.id,
                revision: start.revision,
                gesture: geometry::GestureIdentity::of::<ResizeHorizontal>(
                    start.operation.as_u128(),
                ),
                target: source.parent,
                sources: vec![source.part],
                anchor: source.part,
                generations: start.generations.clone(),
                source_maps: vec![],
                target_map: candidate.parent_to_surface,
                pointer: candidate.pointer,
                dofs,
                controls,
                anchor_controls: moving_controls(start.direction),
                scene,
                siblings,
                viewport: self.snap_viewport()?,
                settings,
                layouts,
            };
            self.validate_snap_input(&input)?;
            Ok(input)
        }));
        self.finish_snap_boundary(result, "snap.resize.input")
    }
}
fn moving_controls(direction: ResizeDirection) -> std::ops::Range<usize> {
    use ResizeDirection::*;
    match direction {
        North => 0..2,
        NorthEast => 1..2,
        East => 1..3,
        SouthEast => 2..3,
        South => 2..4,
        SouthWest => 3..4,
        West => 0..4,
        NorthWest => 0..1,
    }
}
fn resize_actual_geometry<I: Copy>(
    solution: &SnapSolution,
    candidate: &ResizeCandidate<I>,
    actual: Rectangle,
) -> Result<SnapGeometry, SnapError> {
    let input = solution.input();
    if candidate.id != input.candidate || candidate.start.source.parent != input.target {
        return Err(SnapError::Identity);
    }
    let map = candidate.map()?;
    let inverse = map
        .inverse()
        .ok_or(SnapError::Invalid("singular resize map"))?;
    let original = input
        .controls
        .iter()
        .map(|control| geometry::finite_point(inverse.transform_point(control.origin)))
        .collect::<Result<Vec<_>, _>>()?;
    let actual_corners = corners(actual);
    let (x, y) = edges(candidate.start.direction);
    let values = if candidate.start.shape.ratio.is_some() {
        vec![actual.width - (original[1].x() - original[0].x())]
    } else {
        x.map(|low| {
            let i = if low { 0 } else { 1 };
            actual_corners[i].x() - original[i].x()
        })
        .into_iter()
        .chain(y.map(|low| {
            let i = if low { 0 } else { 3 };
            actual_corners[i].y() - original[i].y()
        }))
        .collect()
    };
    let expected = input.geometry(values.clone())?;
    let points = actual_corners
        .into_iter()
        .map(|point| geometry::finite_point(map.transform_point(point)))
        .collect::<Result<Vec<_>, _>>()?;
    for (&point, &expected) in points.iter().zip(expected.points()) {
        let residual = (point - expected).length();
        if !residual.is_finite() {
            return Err(SnapError::Invalid("resize actual residual overflow"));
        }
        if residual > input.settings.tolerance() {
            return Err(SnapError::Rejected(
                "actual resize changes fixed edges or ratio",
            ));
        }
    }
    Ok(SnapGeometry { values, points })
}
