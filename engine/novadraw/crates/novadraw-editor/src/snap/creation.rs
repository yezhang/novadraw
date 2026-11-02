use super::*;
use crate::{
    ContributionLimit, EditPartFactory, GraphicalViewer, ModelAdapter, RequestKey, RequestMetadata,
    RequestProtocol, ToolContext, ToolError,
    container::{
        CheckedCreationPlan, ContainerCreationTemplate, CreationCandidate, CreationPlan,
        CreationPlanHeader,
    },
};
use novadraw::{
    geometry::{Point, Rectangle, Vec2},
    tree::FigureId,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Target-parent horizontal freedom of a creation template.
pub struct CreationHorizontal;
/// Target-parent vertical freedom of a creation template.
pub struct CreationVertical;

/// Ordinary target policy contribution for creation/drop legality before alignment.
pub struct CreationLayoutConstraints;
impl<A: ModelAdapter + 'static> RequestProtocol<A> for CreationLayoutConstraints {
    type Payload = CreationCandidate<A::ModelId>;
    type Contribution = Vec<RuleCall>;
    fn key() -> RequestKey<Self> {
        RequestKey::new("snap-creation-layout-constraints")
    }
}

/// E03 creation authorization bound to actual model geometry and retained snap facts.
pub struct SnappedCreationPlan<A: ModelAdapter, T: ContainerCreationTemplate<A>> {
    checked: CheckedCreationPlan<A, T>,
    solution: SnapSolution,
    geometry: SnapGeometry,
}
impl<A: ModelAdapter, T: ContainerCreationTemplate<A>> SnappedCreationPlan<A, T> {
    /// Returns the complete checked parent-domain plan.
    pub const fn plan(&self) -> &CheckedCreationPlan<A, T> {
        &self.checked
    }
    /// Returns actual evaluated geometry after the target's plan.
    pub const fn geometry(&self) -> &SnapGeometry {
        &self.geometry
    }
    /// Returns feedback only for retained facts.
    pub fn feedback(&self) -> impl Iterator<Item = &FeedbackDeclaration> {
        self.solution.feedback()
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

fn corrected_bounds<I: Copy>(
    candidate: &CreationCandidate<I>,
    values: &[f64],
) -> Result<Rectangle, SnapError> {
    let original = candidate.bounds();
    let mut points = corners(original);
    for (index, point) in points.iter_mut().enumerate() {
        let left = index == 0 || index == 3;
        let top = index < 2;
        let x = candidate.fixed_origin.is_none() || left == candidate.moving_low.0;
        let y = candidate.fixed_origin.is_none() || top == candidate.moving_low.1;
        *point = geometry::finite_point(Point::new(
            point.x() + if x { values[0] } else { 0.0 },
            point.y() + if y { values[1] } else { 0.0 },
        ))?;
    }
    let width = points[1].x() - points[0].x();
    let height = points[3].y() - points[0].y();
    if !width.is_finite() || !height.is_finite() || width < 0.0 || height < 0.0 {
        return Err(SnapError::Rejected(
            "creation correction crosses fixed origin",
        ));
    }
    Ok(Rectangle::new(points[0].x(), points[0].y(), width, height))
}

impl<A: ModelAdapter, F: EditPartFactory<A>> GraphicalViewer<A, F> {
    /// Creates input from an E03 creation candidate. Drop/click translates the complete template;
    /// drag creation changes only the two moving edges and preserves its fixed press.
    pub fn snap_creation_input(
        &mut self,
        candidate: &CreationCandidate<A::ModelId>,
        settings: SnapSettings,
        layouts: Vec<RuleCall>,
    ) -> Result<SnapInput, SnapError> {
        self.snap_ready()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            if candidate.namespace() != self.namespace()
                || candidate.revision() != self.model().revision()
                || candidate.revision() != self.applied_revision()
                || candidate.generation() != self.container_policy_generation(candidate.target())?
            {
                return Err(SnapError::Invalidated);
            }
            if self.parts().get(candidate.target()).is_none_or(|node| {
                !node.is_active()
                    || node.model_id() != Some(candidate.target_model())
                    || node.kind() != crate::PartKind::Containment
            }) {
                return Err(SnapError::Identity);
            }
            let target_map =
                geometry::finite_map(self.child_content_to_surface(candidate.target())?)?;
            if target_map != candidate.target_to_surface() {
                return Err(SnapError::Invalidated);
            }
            let (siblings, scene) = self.snap_scene(candidate.target(), &[], settings)?;
            let coefficients = target_map.coeffs();
            let x_basis = Vec2::new(coefficients[0], coefficients[1]);
            let y_basis = Vec2::new(coefficients[2], coefficients[3]);
            let controls = corners(candidate.bounds())
                .into_iter()
                .enumerate()
                .map(|(index, point)| {
                    let left = index == 0 || index == 3;
                    let top = index < 2;
                    Ok(ControlPoint {
                        origin: geometry::finite_point(target_map.transform_point(point))?,
                        influence: vec![
                            if candidate.fixed_origin.is_none() || left == candidate.moving_low.0 {
                                x_basis
                            } else {
                                Vec2::ZERO
                            },
                            if candidate.fixed_origin.is_none() || top == candidate.moving_low.1 {
                                y_basis
                            } else {
                                Vec2::ZERO
                            },
                        ],
                    })
                })
                .collect::<Result<Vec<_>, SnapError>>()?;
            for rule in &layouts {
                self.snapping.check_rule(rule)?;
            }
            let input = SnapInput {
                namespace: self.namespace(),
                candidate: candidate.id(),
                revision: candidate.revision(),
                gesture: geometry::GestureIdentity::of::<CreationHorizontal>(
                    candidate.operation.as_u128(),
                ),
                target: candidate.target(),
                sources: vec![],
                anchor: self.contents(),
                generations: vec![(candidate.target(), candidate.generation())],
                source_maps: vec![],
                target_map,
                pointer: geometry::finite_point(target_map.transform_point(candidate.pointer()))?,
                dofs: vec![
                    DofKey::of::<CreationHorizontal>(),
                    DofKey::of::<CreationVertical>(),
                ],
                controls,
                anchor_controls: if candidate.fixed_origin.is_some() {
                    let index = match candidate.moving_low {
                        (true, true) => 0,
                        (false, true) => 1,
                        (false, false) => 2,
                        (true, false) => 3,
                    };
                    index..index + 1
                } else {
                    0..4
                },
                scene,
                siblings,
                viewport: self.snap_viewport()?,
                settings,
                layouts,
            };
            self.validate_snap_input(&input)?;
            Ok(input)
        }));
        self.finish_snap_boundary(result, "snap.creation.input")
    }
}

fn actual_geometry<A: ModelAdapter, T: ContainerCreationTemplate<A>>(
    checked: &CheckedCreationPlan<A, T>,
    solution: &SnapSolution,
) -> Result<SnapGeometry, SnapError> {
    let candidate = checked.header().candidate();
    let input = solution.input();
    if candidate.id() != input.candidate() || candidate.target() != input.target() {
        return Err(SnapError::Identity);
    }
    let actual = checked.actual_bounds().ok_or(SnapError::Rejected(
        "snapping requires actual creation bounds",
    ))?;
    let inverse = input
        .target_to_surface()
        .inverse()
        .ok_or(SnapError::Invalid("singular creation target"))?;
    let original = input
        .controls()
        .iter()
        .map(|control| geometry::finite_point(inverse.transform_point(control.origin())))
        .collect::<Result<Vec<_>, _>>()?;
    let actual_corners = corners(actual);
    let moving = (
        if candidate.moving_low.0 { 0 } else { 1 },
        if candidate.moving_low.1 { 0 } else { 3 },
    );
    let values = vec![
        actual_corners[moving.0].x() - original[moving.0].x(),
        actual_corners[moving.1].y() - original[moving.1].y(),
    ];
    let expected = input.geometry(values.clone())?;
    let points = actual_corners
        .into_iter()
        .map(|point| geometry::finite_point(input.target_to_surface().transform_point(point)))
        .collect::<Result<Vec<_>, _>>()?;
    for (&point, &expected) in points.iter().zip(expected.points()) {
        let distance = (point - expected).length();
        if !distance.is_finite() {
            return Err(SnapError::Invalid("creation residual overflow"));
        }
        if distance > input.settings().tolerance() {
            return Err(SnapError::Rejected(
                "actual creation changes fixed template geometry",
            ));
        }
    }
    Ok(SnapGeometry { values, points })
}

impl<A: ModelAdapter + 'static> ToolContext<'_, A> {
    /// Probes parent targets for a drop through the same checked snapping and creation protocol.
    #[allow(
        clippy::too_many_arguments,
        reason = "keeps explicit parent-domain drop input and independently owned session"
    )]
    pub fn resolve_snapped_container_drop<T: ContainerCreationTemplate<A>>(
        &mut self,
        session: &mut SnapSession,
        template: &T,
        pointer: Point,
        grab_offset: Point,
        modifiers: crate::RequestModifiers,
        revision: crate::InteractionRevision,
        settings: SnapSettings,
    ) -> Result<Option<SnappedCreationPlan<A, T>>, ToolError> {
        let mut target = self.container_target_at(pointer, &[])?;
        let operation = uuid::Uuid::new_v4();
        for _ in 0..crate::container::MAX_CONTAINMENT_DEPTH {
            let mut candidate = self.container_drop_candidate(
                target,
                pointer,
                template.default_size(),
                grab_offset,
                modifiers,
                revision,
            )?;
            candidate.operation = operation;
            if let Some(plan) =
                self.resolve_snapped_container_creation(session, template, &candidate, settings)?
            {
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
        session.cancel();
        Err(SnapError::Invalid("creation target ancestry exceeds supported depth").into())
    }

    /// Solves once, resolves a fresh E03 creation plan, then evaluates its actual model bounds.
    pub fn resolve_snapped_container_creation<T: ContainerCreationTemplate<A>>(
        &mut self,
        session: &mut SnapSession,
        template: &T,
        candidate: &CreationCandidate<A::ModelId>,
        settings: SnapSettings,
    ) -> Result<Option<SnappedCreationPlan<A, T>>, ToolError> {
        let result = (|| {
            self.validate_creation_candidate(candidate)?;
            let metadata = RequestMetadata::new(candidate.interaction_revision(), vec![]);
            let mut contributions = self
                .aggregate_at::<CreationLayoutConstraints>(
                    candidate.target(),
                    &metadata,
                    candidate,
                    ContributionLimit::AtMostOne,
                )?
                .into_contributions();
            let layouts = contributions.pop().unwrap_or_default();
            if layouts.len() > settings.budget().sources.get() {
                return Err(SnapError::Budget("layout constraint budget exceeded").into());
            }
            let input = self.snap_creation_input(candidate, settings, layouts)?;
            let solution = self.solve_snapping(session, input)?;
            let bounds = corrected_bounds(candidate, solution.geometry().values())?;
            let corrected = self.corrected_creation_candidate(candidate, bounds)?;
            let Some(checked) = self.resolve_container_creation(template, &corrected)? else {
                return Ok(None);
            };
            let actual = actual_geometry(&checked, &solution)?;
            let geometry = self.validate_snapping_plan_geometry(session, &solution, actual)?;
            let plan = SnappedCreationPlan {
                checked,
                solution,
                geometry,
            };
            self.recheck_snapped_creation(session, &plan)?;
            Ok(Some(plan))
        })();
        if !matches!(result, Ok(Some(_))) {
            session.latest_solution = None;
        }
        result
    }

    fn recheck_snapped_creation<T: ContainerCreationTemplate<A>>(
        &mut self,
        session: &mut SnapSession,
        plan: &SnappedCreationPlan<A, T>,
    ) -> Result<(), ToolError> {
        let result = (|| {
            self.validate_creation_candidate(plan.checked.header().candidate())?;
            if plan.checked.plan.header()
                != &CreationPlanHeader::from_candidate(plan.checked.header().candidate())
                || plan.checked.plan.actual_bounds()? != plan.checked.actual_bounds()
            {
                return Err(SnapError::Invalidated.into());
            }
            let actual = actual_geometry(&plan.checked, &plan.solution)?;
            self.validate_snapping_plan_geometry(session, &plan.solution, actual)?;
            Ok(())
        })();
        if result.is_err() {
            session.latest_solution = None;
        }
        result
    }

    /// Renders and attaches all plan and snap visuals after complete dependency validation.
    pub fn show_snapped_container_creation_feedback<T: ContainerCreationTemplate<A>>(
        &mut self,
        session: &mut SnapSession,
        plan: &SnappedCreationPlan<A, T>,
    ) -> Result<Vec<FigureId>, ToolError> {
        let result = (|| {
            self.recheck_snapped_creation(session, plan)?;
            let declarations = plan.feedback().cloned().collect::<Vec<_>>();
            let mut visuals = self.render_snap_feedback(
                &declarations,
                plan.geometry(),
                plan.solution.input().settings().budget().feedback.get(),
            )?;
            visuals.append(&mut plan.checked.plan.feedback()?);
            if visuals.len() > plan.solution.input().settings().budget().feedback.get() {
                return Err(SnapError::Budget("combined feedback budget exceeded").into());
            }
            self.recheck_snapped_creation(session, plan)?;
            self.attach_feedback(plan.checked.header().candidate().target(), visuals)
        })();
        if result.is_err() {
            session.latest_solution = None;
        }
        result
    }

    /// Performs the final fact gate after cleanup and stages at most one guarded command.
    pub fn submit_snapped_container_creation<T: ContainerCreationTemplate<A>>(
        &mut self,
        session: &mut SnapSession,
        plan: SnappedCreationPlan<A, T>,
    ) -> Result<(), ToolError> {
        let result = (|| {
            self.recheck_snapped_creation(session, &plan)?;
            if let Some(command) = plan.checked.plan.build_command(self.model())? {
                self.submit_command(crate::container::guard_command(
                    command,
                    plan.checked.header.candidate().revision(),
                ))?;
            }
            Ok(())
        })();
        session.cancel();
        result
    }
}
