use super::{RuleCall, SnapError};
use crate::{
    EditPartId, EditorNamespace, ModelRevision,
    container::{CandidateId, PolicyGeneration},
};
use novadraw::geometry::{Affine2D, Point, Rectangle, Vec2};
use std::{any::TypeId, num::NonZeroUsize};

/// Typed identity of an operation's scalar degree of freedom.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DofKey(TypeId);
impl DofKey {
    /// Uses the marker type as identity; strings never dispatch geometric behavior.
    pub fn of<T: 'static>() -> Self {
        Self(TypeId::of::<T>())
    }
}
/// Surface translation in x, installed by the checked move-input builder.
pub struct SurfaceTranslationX;
/// Surface translation in y, installed by the checked move-input builder.
pub struct SurfaceTranslationY;

/// Private typed operation identity; an event revision does not identify an entire gesture.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct GestureIdentity(TypeId, u128);
impl GestureIdentity {
    pub(crate) fn of<T: 'static>(identity: u128) -> Self {
        Self(TypeId::of::<T>(), identity)
    }
}

/// Surface control point with the linear influence of each operation degree of freedom.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlPoint {
    pub(crate) origin: Point,
    pub(crate) influence: Vec<Vec2>,
}
impl ControlPoint {
    /// Returns the unsnapped surface position.
    pub const fn origin(&self) -> Point {
        self.origin
    }
    /// Returns influences in the input's degree-of-freedom order.
    pub fn influence(&self) -> &[Vec2] {
        &self.influence
    }
}

/// Explicit finite limits shared by built-in and external contributions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapBudget {
    /// Maximum registered source contributions prepared for a session.
    pub sources: NonZeroUsize,
    /// Maximum accepted indexed features.
    pub features: NonZeroUsize,
    /// Maximum candidate contributions in one query.
    pub candidates: NonZeroUsize,
    /// Maximum feedback contributions in one accepted solution.
    pub feedback: NonZeroUsize,
}

/// Checked numeric settings. There are no implicit resource or geometric limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapSettings {
    radius: f64,
    tolerance: f64,
    budget: SnapBudget,
}
impl SnapSettings {
    /// Validates finite, nonnegative capture radius and residual tolerance.
    pub fn new(radius: f64, tolerance: f64, budget: SnapBudget) -> Result<Self, SnapError> {
        if !radius.is_finite() || radius < 0.0 || !tolerance.is_finite() || tolerance < 0.0 {
            return Err(SnapError::Invalid("invalid snapping radius or tolerance"));
        }
        Ok(Self {
            radius,
            tolerance,
            budget,
        })
    }
    /// Returns logical surface capture radius.
    pub const fn radius(self) -> f64 {
        self.radius
    }
    /// Returns geometric residual tolerance, without enlarging capture radius.
    pub const fn tolerance(self) -> f64 {
        self.tolerance
    }
    /// Returns explicit resource limits.
    pub const fn budget(self) -> SnapBudget {
        self.budget
    }
}

/// Checked snapshot of one visible sibling's committed primary-Figure geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneFeature {
    pub(crate) part: EditPartId,
    pub(crate) corners: Vec<Point>,
    pub(crate) mapping: Affine2D,
}
impl SceneFeature {
    /// Returns the live source identity.
    pub const fn part(&self) -> EditPartId {
        self.part
    }
    /// Returns real ordered surface corners, never an AABB approximation.
    pub fn corners(&self) -> &[Point] {
        &self.corners
    }
    /// Returns the complete local-to-surface mapping used for the corners.
    pub const fn mapping(&self) -> Affine2D {
        self.mapping
    }
}

/// Framework-created input for exactly one unsnapped operation computation.
///
/// ```compile_fail
/// use novadraw_editor::snap::SnapInput;
/// fn forge(input: &mut SnapInput) { input.controls.clear(); }
/// ```
pub struct SnapInput {
    pub(crate) namespace: EditorNamespace,
    pub(crate) candidate: CandidateId,
    pub(crate) revision: ModelRevision,
    pub(crate) gesture: GestureIdentity,
    pub(crate) target: EditPartId,
    pub(crate) sources: Vec<EditPartId>,
    pub(crate) anchor: EditPartId,
    pub(crate) generations: Vec<(EditPartId, PolicyGeneration)>,
    pub(crate) source_maps: Vec<(EditPartId, Affine2D)>,
    pub(crate) target_map: Affine2D,
    pub(crate) pointer: Point,
    pub(crate) dofs: Vec<DofKey>,
    pub(crate) controls: Vec<ControlPoint>,
    pub(crate) anchor_controls: std::ops::Range<usize>,
    pub(crate) scene: Vec<SceneFeature>,
    pub(crate) siblings: Vec<EditPartId>,
    pub(crate) viewport: (Point, f64, Rectangle),
    pub(crate) settings: SnapSettings,
    pub(crate) layouts: Vec<RuleCall>,
}
impl SnapInput {
    /// Returns the unique unsnapped computation identity.
    pub const fn candidate(&self) -> CandidateId {
        self.candidate
    }
    /// Returns the target child-content owner.
    pub const fn target(&self) -> EditPartId {
        self.target
    }
    /// Returns the unique anchor part.
    pub const fn anchor(&self) -> EditPartId {
        self.anchor
    }
    /// Returns the ordered operation set.
    pub fn sources(&self) -> &[EditPartId] {
        &self.sources
    }
    /// Returns the latest pointer in logical surface units.
    pub const fn pointer(&self) -> Point {
        self.pointer
    }
    /// Returns the complete target child-content-to-surface map.
    pub const fn target_to_surface(&self) -> Affine2D {
        self.target_map
    }
    /// Returns scalar freedoms in deterministic operation order.
    pub fn dofs(&self) -> &[DofKey] {
        &self.dofs
    }
    /// Returns the frozen unsnapped control geometry.
    pub fn controls(&self) -> &[ControlPoint] {
        &self.controls
    }
    /// Returns the unique anchor's control points within the full batch.
    pub fn anchor_controls(&self) -> std::ops::Range<usize> {
        self.anchor_controls.clone()
    }
    /// Returns the complete participating sibling snapshot.
    pub fn scene(&self) -> &[SceneFeature] {
        &self.scene
    }
    /// Returns checked settings.
    pub const fn settings(&self) -> SnapSettings {
        self.settings
    }

    pub(crate) fn geometry(&self, values: Vec<f64>) -> Result<SnapGeometry, SnapError> {
        if values.len() != self.dofs.len() || !values.iter().all(|v| v.is_finite()) {
            return Err(SnapError::Invalid("invalid operation values"));
        }
        let points = self
            .controls
            .iter()
            .map(|control| {
                if control.influence.len() != values.len() {
                    return Err(SnapError::Invalid("invalid control-point influence"));
                }
                let mut x = control.origin.x();
                let mut y = control.origin.y();
                for (influence, value) in control.influence.iter().zip(&values) {
                    x += influence.x() * value;
                    y += influence.y() * value;
                }
                finite_point(Point::new(x, y))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SnapGeometry { values, points })
    }
}

/// Finite evaluated geometry, tied by its owner to one checked input.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapGeometry {
    pub(crate) values: Vec<f64>,
    pub(crate) points: Vec<Point>,
}
impl SnapGeometry {
    /// Returns evaluated scalar corrections in input order.
    pub fn values(&self) -> &[f64] {
        &self.values
    }
    /// Returns real evaluated surface control points.
    pub fn points(&self) -> &[Point] {
        &self.points
    }
}

/// Shared read-only context for admission, layout and alignment evaluators.
pub struct EvaluationContext<'a> {
    /// Frozen original input; it is never replaced by the last snapped geometry.
    pub input: &'a SnapInput,
    /// Candidate or composed geometry being checked.
    pub geometry: &'a SnapGeometry,
}
impl EvaluationContext<'_> {
    /// Reads one scalar correction by typed identity.
    pub fn value(&self, key: DofKey) -> Option<f64> {
        self.input
            .dofs
            .iter()
            .position(|k| *k == key)
            .map(|i| self.geometry.values[i])
    }
    /// Maximum changed-control-point distance from the frozen original input.
    pub fn capture_distance(&self) -> Result<f64, SnapError> {
        self.geometry
            .points
            .iter()
            .zip(&self.input.controls)
            .try_fold(0.0_f64, |maximum, (p, c)| {
                let dx = p.x() - c.origin.x();
                let dy = p.y() - c.origin.y();
                let distance = dx.hypot(dy);
                if distance.is_finite() {
                    Ok(maximum.max(distance))
                } else {
                    Err(SnapError::Invalid("capture distance overflow"))
                }
            })
    }
}

pub(crate) fn finite_point(point: Point) -> Result<Point, SnapError> {
    if point.x().is_finite() && point.y().is_finite() {
        Ok(point)
    } else {
        Err(SnapError::Invalid("non-finite snapping geometry"))
    }
}
pub(crate) fn finite_map(map: Affine2D) -> Result<Affine2D, SnapError> {
    if map.coeffs().into_iter().all(f64::is_finite) {
        Ok(map)
    } else {
        Err(SnapError::Invalid("non-finite snapping map"))
    }
}
