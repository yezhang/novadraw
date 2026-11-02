use super::*;
use crate::{
    ContributionLimit, ModelAdapter, RequestKey, RequestMetadata, RequestProtocol, ToolContext,
    ToolError,
    container::{CheckedMovePlan, ContainerMoveProtocol, MoveCandidate, MovePlan, MovePlanHeader},
};
use novadraw::{geometry::Affine2D, tree::FigureId};

/// Open target-policy request for read-only allowed-position constraints.
/// An unsupported request leaves legality to the actual E03 plan, never to an inferred layout.
pub struct MoveLayoutConstraints;
impl<A: ModelAdapter + 'static> RequestProtocol<A> for MoveLayoutConstraints {
    type Payload = MoveCandidate<A::ModelId>;
    type Contribution = Vec<RuleCall>;
    fn key() -> RequestKey<Self> {
        RequestKey::new("snap-move-layout-constraints")
    }
}

/// A complete E03 plan bound to checked snapping facts and its actual model geometry.
/// This cannot be constructed from a tentative solution by an external consumer.
pub struct SnappedMovePlan<A: ModelAdapter, P: ContainerMoveProtocol<A>> {
    checked: CheckedMovePlan<A, P>,
    solution: SnapSolution,
    geometry: SnapGeometry,
}
impl<A: ModelAdapter, P: ContainerMoveProtocol<A>> SnappedMovePlan<A, P> {
    /// Returns actual evaluated geometry after the target layout's complete plan.
    pub const fn geometry(&self) -> &SnapGeometry {
        &self.geometry
    }
    /// Returns the complete E03 authorization, for read-only diagnostics.
    pub const fn plan(&self) -> &CheckedMovePlan<A, P> {
        &self.checked
    }
    /// Returns only retained facts' feedback after actual-plan validation.
    pub fn feedback(&self) -> impl Iterator<Item = &FeedbackDeclaration> {
        self.solution.feedback()
    }
}

fn move_geometry<A: ModelAdapter, P: ContainerMoveProtocol<A>>(
    checked: &CheckedMovePlan<A, P>,
    solution: &SnapSolution,
) -> Result<SnapGeometry, SnapError> {
    let input = solution.input();
    if input.candidate() != checked.header().candidate()
        || input.target() != checked.header().target()
        || input
            .sources()
            .iter()
            .copied()
            .ne(checked.header().sources().iter().map(|s| s.part()))
    {
        return Err(SnapError::Identity);
    }
    let actual = checked.actual_geometry().ok_or(SnapError::Rejected(
        "snapping requires actual plan geometry",
    ))?;
    let mut points = Vec::new();
    for (geometry, source) in actual.iter().zip(checked.candidate.geometry()) {
        if geometry.corners().len() != source.corners().len() {
            return Err(SnapError::Rejected(
                "actual plan changed operation topology",
            ));
        }
        for &point in geometry.corners() {
            points.push(geometry::finite_point(
                input.target_to_surface().transform_point(point),
            )?);
        }
    }
    if points.len() != input.controls().len() || points.is_empty() {
        return Err(SnapError::Rejected(
            "actual plan does not cover operation controls",
        ));
    }
    let delta = points[0] - input.controls()[0].origin();
    let values = vec![delta.x(), delta.y()];
    let expected = input.geometry(values.clone())?;
    for (&actual, &expected) in points.iter().zip(expected.points()) {
        let distance = (actual - expected).length();
        if !distance.is_finite() {
            return Err(SnapError::Invalid("actual plan residual overflow"));
        }
        if distance > input.settings().tolerance() {
            return Err(SnapError::Rejected(
                "actual plan is not one shared batch translation",
            ));
        }
    }
    Ok(SnapGeometry { values, points })
}

impl<A: ModelAdapter + 'static> ToolContext<'_, A> {
    /// Queries target constraints, solves once, resolves fresh E03 permissions and checks
    /// actual plan geometry. A legal absent target plan permits the caller's ancestor probing.
    /// A failed adopted fact rejects this flow; no fallback or second solve is performed.
    pub fn resolve_snapped_container_move<P: ContainerMoveProtocol<A>>(
        &mut self,
        session: &mut SnapSession,
        candidate: &MoveCandidate<A::ModelId>,
        settings: SnapSettings,
    ) -> Result<Option<SnappedMovePlan<A, P>>, ToolError> {
        let result = (|| {
            let metadata = RequestMetadata::new(
                candidate.interaction_revision(),
                candidate
                    .batch()
                    .sources()
                    .iter()
                    .map(|s| s.part())
                    .collect::<Vec<_>>(),
            );
            let mut contributions = self
                .aggregate_at::<MoveLayoutConstraints>(
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
            let input = self.snap_move_input(candidate, settings, layouts)?;
            let solution = self.solve_snapping(session, input)?;
            let values = solution.geometry().values();
            let translation = Affine2D::from_translation(values[0], values[1]);
            let target_inverse = candidate
                .target_to_surface()
                .inverse()
                .ok_or(SnapError::Invalid("singular snapped move target"))?;
            let parent_translation =
                geometry::finite_map(target_inverse * translation * candidate.target_to_surface())?;
            let mut corrected = candidate.clone();
            corrected.geometry = candidate
                .geometry()
                .iter()
                .map(|geometry| geometry.mapped(parent_translation))
                .collect::<Result<Vec<_>, _>>()?
                .into();
            corrected.translation = geometry::finite_map(translation * candidate.translation())?;
            // Preserve raw pointer/targeting and the unique original computation identity.
            let Some(checked) = self.resolve_container_move::<P>(&corrected)? else {
                return Ok(None);
            };
            let actual = move_geometry(&checked, &solution)?;
            let geometry = self.validate_snapping_plan_geometry(session, &solution, actual)?;
            let plan = SnappedMovePlan {
                checked,
                solution,
                geometry,
            };
            self.recheck_snapped_move(session, &plan)?;
            Ok(Some(plan))
        })();
        if !matches!(result, Ok(Some(_))) {
            session.latest_solution = None;
        }
        result
    }

    fn recheck_snapped_move<P: ContainerMoveProtocol<A>>(
        &mut self,
        session: &mut SnapSession,
        plan: &SnappedMovePlan<A, P>,
    ) -> Result<(), ToolError> {
        let result = (|| {
            self.validate_move_candidate(&plan.checked.candidate)?;
            if plan.checked.plan.header()
                != &MovePlanHeader::from_candidate(&plan.checked.candidate)
                || plan.checked.plan.actual_geometry()?.as_deref() != plan.checked.actual_geometry()
            {
                return Err(SnapError::Invalidated.into());
            }
            let actual = move_geometry(&plan.checked, &plan.solution)?;
            self.validate_snapping_plan_geometry(session, &plan.solution, actual)?;
            Ok(())
        })();
        if result.is_err() {
            session.latest_solution = None;
        }
        result
    }

    /// Publishes an actual-plan preview after rechecking all retained dependencies.
    /// All snap declarations are rendered through the Viewer-owned typed registry.
    pub fn show_snapped_container_move_feedback<P: ContainerMoveProtocol<A>>(
        &mut self,
        session: &mut SnapSession,
        plan: &SnappedMovePlan<A, P>,
    ) -> Result<Vec<FigureId>, ToolError> {
        let result = (|| {
            self.recheck_snapped_move(session, plan)?;
            let declarations = plan.feedback().cloned().collect::<Vec<_>>();
            let mut snap_visuals = self.render_snap_feedback(
                &declarations,
                plan.geometry(),
                plan.solution.input().settings().budget().feedback.get(),
            )?;
            let mut visuals = plan.checked.plan.feedback()?;
            visuals.append(&mut snap_visuals);
            if visuals.len() > plan.solution.input().settings().budget().feedback.get() {
                return Err(SnapError::Budget("combined feedback budget exceeded").into());
            }
            self.recheck_snapped_move(session, plan)?;
            self.attach_feedback(plan.checked.header().target(), visuals)
        })();
        if result.is_err() {
            session.latest_solution = None;
        }
        result
    }

    /// Rechecks after successful caller cleanup, then constructs and stages one model command.
    /// Failed validation cannot retry from a new baseline or submit an unsnapped fallback.
    pub fn submit_snapped_container_move<P: ContainerMoveProtocol<A>>(
        &mut self,
        session: &mut SnapSession,
        plan: SnappedMovePlan<A, P>,
    ) -> Result<(), ToolError> {
        let result = (|| {
            self.recheck_snapped_move(session, &plan)?;
            // Final fact validation above is the last owner callback before command construction.
            // Do not introduce another synchronization/qualification pass between these two gates.
            if let Some(command) = plan.checked.plan.build_command(self.model())? {
                self.submit_command(crate::container::guard_command(
                    command,
                    plan.checked.header.revision(),
                ))?;
            }
            Ok(())
        })();
        session.cancel();
        result
    }
}
