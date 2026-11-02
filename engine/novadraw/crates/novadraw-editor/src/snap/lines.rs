use super::*;
use novadraw::geometry::{Point, Vec2};
use std::marker::PhantomData;

/// Real surface line, with a unit normal and owned typed provenance.
pub struct AlignmentLine<T: 'static> {
    point: Point,
    normal: Vec2,
    marker: PhantomData<fn() -> T>,
}
impl<T> Copy for AlignmentLine<T> {}
impl<T> Clone for AlignmentLine<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> AlignmentLine<T> {
    /// Checks two real endpoints; degenerate and overflowed lines are rejected.
    pub fn new(start: Point, end: Point) -> Result<Self, SnapError> {
        geometry::finite_point(start)?;
        geometry::finite_point(end)?;
        let dx = end.x() - start.x();
        let dy = end.y() - start.y();
        let length = dx.hypot(dy);
        if !length.is_finite() || length <= 0.0 {
            return Err(SnapError::Invalid("invalid alignment line"));
        }
        Ok(Self {
            point: start,
            normal: Vec2::new(-dy / length, dx / length),
            marker: PhantomData,
        })
    }
    /// Ordinary typed feature key, also usable by an external provider.
    pub fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("alignment-line")
    }
    /// Returns a real point on the surface line.
    pub const fn point(&self) -> Point {
        self.point
    }
    /// Returns the unit surface normal.
    pub const fn normal(&self) -> Vec2 {
        self.normal
    }
}
/// Provenance marker for committed sibling geometry.
pub struct GeometryFeature;
/// Provenance marker for independently configured guides.
pub struct GuideFeature;

/// Default committed sibling provider. It retains real edges and their center parallels.
pub struct GeometryProvider;
impl FeatureProvider for GeometryProvider {
    type Payload = ();
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("geometry-provider")
    }
    fn version(&self, _: &()) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn features(
        &self,
        _: &(),
        input: &SnapInput,
        sink: &mut FeatureSink<'_>,
    ) -> Result<(), SnapError> {
        for (order, scene) in input.scene().iter().enumerate() {
            let p = scene.corners();
            if p.len() != 4 {
                return Err(SnapError::Invalid("invalid committed rectangle corners"));
            }
            let base = u64::try_from(order)
                .ok()
                .and_then(|o| o.checked_mul(6))
                .ok_or(SnapError::Invalid("geometry feature identity overflow"))?;
            let center = geometry::finite_point(Point::new(
                p.iter().map(|p| p.x() / 4.0).sum(),
                p.iter().map(|p| p.y() / 4.0).sum(),
            ))?;
            for edge in 0..4 {
                let end = p[(edge + 1) % 4];
                // Zero-sized Figures legitimately contribute fewer edges.
                if p[edge] == end {
                    continue;
                }
                let line = AlignmentLine::<GeometryFeature>::new(p[edge], end)?;
                sink.push_typed(
                    base + edge as u64,
                    vec![p[edge], end],
                    AlignmentLine::<GeometryFeature>::key(),
                    line,
                )?;
            }
            for edge in 0..2 {
                let direction =
                    Vec2::new(p[edge + 1].x() - p[edge].x(), p[edge + 1].y() - p[edge].y());
                if direction == Vec2::ZERO {
                    continue;
                }
                let end = geometry::finite_point(Point::new(
                    center.x() + direction.x(),
                    center.y() + direction.y(),
                ))?;
                let line = AlignmentLine::<GeometryFeature>::new(center, end)?;
                sink.push_typed(
                    base + 4 + edge as u64,
                    vec![center, end],
                    AlignmentLine::<GeometryFeature>::key(),
                    line,
                )?;
            }
        }
        Ok(())
    }
}

/// Application-owned guide identity and endpoints in the target content domain.
pub struct GuideSpec {
    stable: u64,
    start: Point,
    end: Point,
}
impl GuideSpec {
    /// Checks geometric validity; the provider additionally rejects duplicate identities.
    pub fn new(stable: u64, start: Point, end: Point) -> Result<Self, SnapError> {
        AlignmentLine::<GuideFeature>::new(start, end)?;
        Ok(Self { stable, start, end })
    }
}
/// Ordinary configured guide provider; no model attachment is created by this provider.
pub struct GuideProvider;
impl FeatureProvider for GuideProvider {
    type Payload = Vec<GuideSpec>;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("guide-provider")
    }
    fn version(&self, _: &Self::Payload) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn features(
        &self,
        guides: &Self::Payload,
        input: &SnapInput,
        sink: &mut FeatureSink<'_>,
    ) -> Result<(), SnapError> {
        for guide in guides {
            let start =
                geometry::finite_point(input.target_to_surface().transform_point(guide.start))?;
            let end = geometry::finite_point(input.target_to_surface().transform_point(guide.end))?;
            let line = AlignmentLine::<GuideFeature>::new(start, end)?;
            sink.push_typed(
                guide.stable,
                vec![start, end],
                AlignmentLine::<GuideFeature>::key(),
                line,
            )?;
        }
        Ok(())
    }
}

/// Owned linear relation between an anchor sample and a real line.
pub struct LineRelation {
    point: Point,
    normal: Vec2,
    controls: Vec<(usize, f64)>,
}
impl LineRelation {
    /// Declares a finite weighted control sample against any typed line provider.
    /// Control identity and evaluated geometry remain checked on each invocation.
    pub fn new<T>(line: AlignmentLine<T>, controls: Vec<(usize, f64)>) -> Result<Self, SnapError> {
        if controls.is_empty() || controls.iter().any(|(_, weight)| !weight.is_finite()) {
            return Err(SnapError::Invalid("invalid alignment sample"));
        }
        Ok(Self {
            point: line.point,
            normal: line.normal,
            controls,
        })
    }
}
/// Shared open evaluator for geometry, guides and external line providers.
pub struct LineAlignment;
impl EvaluationRule for LineAlignment {
    type Payload = LineRelation;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("line-alignment")
    }
    fn version(&self, _: &LineRelation) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn evaluate(
        &self,
        relation: &LineRelation,
        ctx: &EvaluationContext<'_>,
    ) -> Result<bool, SnapError> {
        let mut x = 0.0;
        let mut y = 0.0;
        for &(index, weight) in &relation.controls {
            let point = ctx
                .geometry
                .points()
                .get(index)
                .ok_or(SnapError::Invalid("invalid alignment control"))?;
            x += point.x() * weight;
            y += point.y() * weight;
        }
        let residual = (x - relation.point.x()) * relation.normal.x()
            + (y - relation.point.y()) * relation.normal.y();
        if !residual.is_finite() {
            return Err(SnapError::Invalid("alignment residual overflow"));
        }
        Ok(residual.abs() <= ctx.input.settings().tolerance())
    }
}

struct IndexedLine<T: 'static> {
    feature: FeatureId,
    line: AlignmentLine<T>,
}
impl<T> Clone for IndexedLine<T> {
    fn clone(&self) -> Self {
        Self {
            feature: self.feature,
            line: self.line,
        }
    }
}
/// Prepared surface axis indexes and general real lines.
pub struct PreparedLines<T: 'static> {
    vertical: Vec<IndexedLine<T>>,
    horizontal: Vec<IndexedLine<T>>,
    general: Vec<IndexedLine<T>>,
}
/// Open line strategy. The marker type chooses typed provider data, never an internal enum.
pub struct LineSnap<T: 'static> {
    admission: RuleBinding<ThresholdCapture>,
    alignment: RuleBinding<LineAlignment>,
    marker: PhantomData<fn() -> T>,
}
/// Geometry and Guide are ordinary instances of the same public line protocol.
pub type Geometry = LineSnap<GeometryFeature>;
/// Guide aligns the unique anchor and moves the batch through its original freedoms.
pub type Guide = LineSnap<GuideFeature>;
impl<T> LineSnap<T> {
    /// Uses the same registry-authorized admission and alignment as third-party strategies.
    pub const fn new(
        admission: RuleBinding<ThresholdCapture>,
        alignment: RuleBinding<LineAlignment>,
    ) -> Self {
        Self {
            admission,
            alignment,
            marker: PhantomData,
        }
    }
}
impl<T: 'static> SnapStrategy for LineSnap<T> {
    type Payload = ();
    type Prepared = PreparedLines<T>;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("line-snap")
    }
    fn version(&self, _: &()) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn prepare(
        &self,
        _: &(),
        _: &SnapInput,
        features: &[SnapFeature],
    ) -> Result<Self::Prepared, SnapError> {
        let mut prepared = PreparedLines {
            vertical: Vec::new(),
            horizontal: Vec::new(),
            general: Vec::new(),
        };
        for feature in features {
            let Some(&line) = feature.payload(AlignmentLine::<T>::key()) else {
                continue;
            };
            let entry = IndexedLine {
                feature: feature.id(),
                line,
            };
            if line.normal.y() == 0.0 {
                prepared.vertical.push(entry);
            } else if line.normal.x() == 0.0 {
                prepared.horizontal.push(entry);
            } else {
                prepared.general.push(entry);
            }
        }
        prepared
            .vertical
            .sort_by(|a, b| a.line.point.x().total_cmp(&b.line.point.x()));
        prepared
            .horizontal
            .sort_by(|a, b| a.line.point.y().total_cmp(&b.line.point.y()));
        Ok(prepared)
    }
    fn reuse_prepared(
        &self,
        _: &(),
        prepared: &Self::Prepared,
        _: &SnapInput,
        features: &[SnapFeature],
    ) -> Result<bool, SnapError> {
        let previous = prepared
            .vertical
            .iter()
            .chain(&prepared.horizontal)
            .chain(&prepared.general)
            .map(|entry| (entry.feature, entry.line))
            .collect::<std::collections::HashMap<_, _>>();
        let mut count = 0;
        for feature in features {
            let Some(line) = feature.payload(AlignmentLine::<T>::key()) else {
                continue;
            };
            count += 1;
            let Some(old) = previous.get(&feature.id()) else {
                return Ok(false);
            };
            if line.point != old.point || line.normal != old.normal {
                return Ok(false);
            }
        }
        Ok(count == previous.len())
    }
    fn query(
        &self,
        _: &(),
        prepared: &Self::Prepared,
        input: &SnapInput,
        sink: &mut CandidateSink<'_>,
    ) -> Result<(), SnapError> {
        let anchor = input.anchor_controls();
        if anchor.is_empty() {
            return Ok(());
        }
        let mut samples = anchor
            .clone()
            .map(|index| vec![(index, 1.0)])
            .collect::<Vec<_>>();
        samples.push(
            anchor
                .clone()
                .map(|index| (index, 1.0 / anchor.len() as f64))
                .collect(),
        );
        for (stable, controls) in samples.into_iter().enumerate() {
            let mut origin = Point::ZERO;
            let mut influence = vec![Vec2::ZERO; input.dofs().len()];
            for &(index, weight) in &controls {
                let control = &input.controls()[index];
                origin = Point::new(
                    origin.x() + control.origin().x() * weight,
                    origin.y() + control.origin().y() * weight,
                );
                for (total, &v) in influence.iter_mut().zip(control.influence()) {
                    *total = Vec2::new(total.x() + v.x() * weight, total.y() + v.y() * weight);
                }
            }
            geometry::finite_point(origin)?;
            let radius = input.settings().radius();
            let window = |entries: &[IndexedLine<T>], axis: fn(Point) -> f64| {
                let low = axis(origin) - radius;
                let high = axis(origin) + radius;
                if !low.is_finite() || !high.is_finite() {
                    return Err(SnapError::Invalid("line query window overflow"));
                }
                let start = entries.partition_point(|entry| axis(entry.line.point) < low);
                let end = entries.partition_point(|entry| axis(entry.line.point) <= high);
                Ok(start..end)
            };
            let vertical = &prepared.vertical[window(&prepared.vertical, Point::x)?];
            let horizontal = &prepared.horizontal[window(&prepared.horizontal, Point::y)?];
            for entry in vertical.iter().chain(horizontal).chain(&prepared.general) {
                let normal = entry.line.normal;
                let coefficients = influence
                    .iter()
                    .map(|v| normal.x() * v.x() + normal.y() * v.y())
                    .collect::<Vec<_>>();
                let norm = coefficients.iter().map(|v| v * v).sum::<f64>();
                let residual = (entry.line.point.x() - origin.x()) * normal.x()
                    + (entry.line.point.y() - origin.y()) * normal.y();
                if !norm.is_finite() || !residual.is_finite() {
                    return Err(SnapError::Invalid("line correction overflow"));
                }
                if norm == 0.0 {
                    continue;
                }
                let values = coefficients
                    .iter()
                    .map(|v| v * residual / norm)
                    .collect::<Vec<_>>();
                let geometry = input.geometry(values.clone())?;
                if (EvaluationContext {
                    input,
                    geometry: &geometry,
                })
                .capture_distance()?
                    > radius
                {
                    continue;
                }
                let mut writes = Vec::new();
                let mut dependencies = Vec::new();
                for (index, &coefficient) in coefficients.iter().enumerate() {
                    if coefficient != 0.0 {
                        writes.push((input.dofs()[index], values[index]));
                        dependencies.push(input.dofs()[index]);
                    }
                }
                let relation = LineRelation::new(entry.line, controls.clone())?;
                sink.push(SnapCandidate::new(
                    entry.feature,
                    stable as u64,
                    writes,
                    dependencies,
                    self.admission.bind(()),
                    self.alignment.bind(relation),
                    vec![FeedbackDeclaration::new(
                        ReferenceLine::key(),
                        ReferenceLine::new(entry.line.point, entry.line.normal)?,
                    )],
                ))?;
            }
        }
        Ok(())
    }
}
