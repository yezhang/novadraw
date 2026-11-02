use super::*;
use novadraw::geometry::{Point, Vec2};

/// Finite target-content grid configuration. `None` disables an axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSpec {
    origin: Point,
    spacing: [Option<f64>; 2],
}
impl GridSpec {
    /// Checks both enabled spacings before the configuration is installed.
    pub fn new(origin: Point, x: Option<f64>, y: Option<f64>) -> Result<Self, SnapError> {
        geometry::finite_point(origin)?;
        if [x, y]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite() || v <= 0.0)
        {
            return Err(SnapError::Invalid("invalid grid spacing"));
        }
        Ok(Self {
            origin,
            spacing: [x, y],
        })
    }
    /// Typed geometry declaration identity, available to external providers too.
    pub fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("grid-spec")
    }
}

/// Ordinary provider for a target-content grid; replacing it retires old grid features.
pub struct GridProvider;
impl FeatureProvider for GridProvider {
    type Payload = GridSpec;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("grid-provider")
    }
    fn version(&self, _: &GridSpec) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn features(
        &self,
        spec: &GridSpec,
        input: &SnapInput,
        sink: &mut FeatureSink<'_>,
    ) -> Result<(), SnapError> {
        if spec.spacing.iter().all(Option::is_none) {
            return Ok(());
        }
        let origin =
            geometry::finite_point(input.target_to_surface().transform_point(spec.origin))?;
        sink.push_typed(0, vec![origin], GridSpec::key(), *spec)
    }
}

/// Owned nearest-grid relation parameters used by admission and final alignment.
#[derive(Clone, Copy)]
pub struct GridProjection {
    spec: GridSpec,
    control: usize,
}
impl GridProjection {
    /// Declares the projection of one original control onto the enabled grid axes.
    pub const fn new(spec: GridSpec, control: usize) -> Self {
        Self { spec, control }
    }
}

fn coordinates(point: Point) -> [f64; 2] {
    [point.x(), point.y()]
}
fn nearest(value: f64, origin: f64, spacing: f64) -> Result<f64, SnapError> {
    let ordinal = (value - origin) / spacing;
    let target = origin + ordinal.round_ties_even() * spacing;
    if !ordinal.is_finite() || !target.is_finite() {
        Err(SnapError::Invalid("grid projection overflow"))
    } else {
        Ok(target)
    }
}

/// Public exact nearest-grid relation. It reads the frozen input on every evaluation.
pub struct NearestGrid;
impl EvaluationRule for NearestGrid {
    type Payload = GridProjection;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("nearest-grid")
    }
    fn version(&self, _: &GridProjection) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn evaluate(
        &self,
        projection: &GridProjection,
        ctx: &EvaluationContext<'_>,
    ) -> Result<bool, SnapError> {
        let inverse = ctx
            .input
            .target_to_surface()
            .inverse()
            .ok_or(SnapError::Invalid("singular grid target mapping"))?;
        let control = ctx
            .input
            .controls()
            .get(projection.control)
            .ok_or(SnapError::Invalid("invalid grid control"))?;
        let actual = ctx
            .geometry
            .points()
            .get(projection.control)
            .ok_or(SnapError::Invalid("invalid grid geometry"))?;
        let original = coordinates(geometry::finite_point(
            inverse.transform_point(control.origin()),
        )?);
        let actual = coordinates(geometry::finite_point(inverse.transform_point(*actual))?);
        let origin = coordinates(projection.spec.origin);
        for axis in 0..2 {
            if let Some(spacing) = projection.spec.spacing[axis] {
                let expected = nearest(original[axis], origin[axis], spacing)?;
                if (actual[axis] - expected).abs() > ctx.input.settings().tolerance() {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

/// Grid capture admission, using the same surface distance rule as external strategies.
pub struct GridCapture;
impl EvaluationRule for GridCapture {
    type Payload = GridProjection;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("grid-capture")
    }
    fn version(&self, _: &GridProjection) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn evaluate(&self, _: &GridProjection, ctx: &EvaluationContext<'_>) -> Result<bool, SnapError> {
        ThresholdCapture.evaluate(&(), ctx)
    }
}

/// Ordinary grid strategy. Admission is explicitly supplied as a registered evaluator.
pub struct Grid<R: EvaluationRule<Payload = GridProjection>> {
    admission: RuleBinding<R>,
    alignment: RuleBinding<NearestGrid>,
}
impl<R: EvaluationRule<Payload = GridProjection>> Grid<R> {
    /// Uses GridCapture for capture or ExactProjection<NearestGrid> for always quantizing.
    /// External evaluators with the same payload use this identical entry point.
    pub const fn new(admission: RuleBinding<R>, alignment: RuleBinding<NearestGrid>) -> Self {
        Self {
            admission,
            alignment,
        }
    }
}

/// Owned checked provider declarations retained by the ordinary strategy bridge.
pub struct PreparedGrid(Vec<(FeatureId, GridSpec)>);

fn correction(
    input: &SnapInput,
    projection: GridProjection,
) -> Result<Option<Vec<f64>>, SnapError> {
    let inverse = input
        .target_to_surface()
        .inverse()
        .ok_or(SnapError::Invalid("singular grid target mapping"))?;
    let control = input
        .controls()
        .get(projection.control)
        .ok_or(SnapError::Invalid("invalid grid control"))?;
    let original = coordinates(geometry::finite_point(
        inverse.transform_point(control.origin()),
    )?);
    let origin = coordinates(projection.spec.origin);
    let mut rows = Vec::new();
    let mut residuals = Vec::new();
    for axis in 0..2 {
        if let Some(spacing) = projection.spec.spacing[axis] {
            let coefficients = control
                .influence()
                .iter()
                .map(|&v| {
                    let mapped = inverse.transform_vector(v);
                    if axis == 0 { mapped.x() } else { mapped.y() }
                })
                .collect::<Vec<_>>();
            if !coefficients.iter().all(|v| v.is_finite()) {
                return Err(SnapError::Invalid("grid influence overflow"));
            }
            residuals.push(nearest(original[axis], origin[axis], spacing)? - original[axis]);
            rows.push(coefficients);
        }
    }
    let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    let multipliers = match rows.as_slice() {
        [] => return Ok(None),
        [a] => {
            let norm = dot(a, a);
            if !norm.is_finite() {
                return Err(SnapError::Invalid("grid influence norm overflow"));
            }
            if norm == 0.0 {
                return Ok(None);
            }
            vec![residuals[0] / norm]
        }
        [a, b] => {
            let aa = dot(a, a);
            let ab = dot(a, b);
            let bb = dot(b, b);
            let determinant = aa * bb - ab * ab;
            if !determinant.is_finite() {
                return Err(SnapError::Invalid("grid influence determinant overflow"));
            }
            if determinant <= 0.0 {
                // A proportional resize can have one freedom shared by both axes.
                // Solve its nonzero row, then test the complete pair of equations below.
                if !aa.is_finite() || !bb.is_finite() || !ab.is_finite() {
                    return Err(SnapError::Invalid("grid influence norm overflow"));
                }
                if aa > 0.0 {
                    vec![residuals[0] / aa, 0.0]
                } else if bb > 0.0 {
                    vec![0.0, residuals[1] / bb]
                } else {
                    return Ok(None);
                }
            } else {
                vec![
                    (bb * residuals[0] - ab * residuals[1]) / determinant,
                    (aa * residuals[1] - ab * residuals[0]) / determinant,
                ]
            }
        }
        _ => unreachable!(),
    };
    let values = (0..input.dofs().len())
        .map(|index| {
            rows.iter()
                .zip(&multipliers)
                .map(|(row, m)| row[index] * m)
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    if !values.iter().all(|v| v.is_finite()) {
        return Err(SnapError::Invalid("grid correction overflow"));
    }
    for (row, residual) in rows.iter().zip(&residuals) {
        let actual = dot(row, &values);
        if !actual.is_finite() {
            return Err(SnapError::Invalid("grid residual overflow"));
        }
        if (actual - residual).abs() > input.settings().tolerance() {
            return Ok(None);
        }
    }
    Ok(Some(values))
}

impl<R: EvaluationRule<Payload = GridProjection>> SnapStrategy for Grid<R> {
    type Payload = ();
    type Prepared = PreparedGrid;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("grid")
    }
    fn version(&self, _: &()) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn prepare(
        &self,
        _: &(),
        _: &SnapInput,
        features: &[SnapFeature],
    ) -> Result<PreparedGrid, SnapError> {
        Ok(PreparedGrid(
            features
                .iter()
                .filter_map(|f| f.payload(GridSpec::key()).map(|spec| (f.id(), *spec)))
                .collect(),
        ))
    }
    fn reuse_prepared(
        &self,
        _: &(),
        prepared: &PreparedGrid,
        _: &SnapInput,
        features: &[SnapFeature],
    ) -> Result<bool, SnapError> {
        let current = features.iter().filter_map(|feature| {
            feature
                .payload(GridSpec::key())
                .map(|spec| (feature.id(), *spec))
        });
        // Provider reordering changes arbitration order, but not this feature-only asset.
        let previous = prepared
            .0
            .iter()
            .copied()
            .collect::<std::collections::HashMap<_, _>>();
        let mut count = 0;
        for (id, spec) in current {
            count += 1;
            if previous.get(&id) != Some(&spec) {
                return Ok(false);
            }
        }
        Ok(count == previous.len())
    }
    fn query(
        &self,
        _: &(),
        prepared: &PreparedGrid,
        input: &SnapInput,
        sink: &mut CandidateSink<'_>,
    ) -> Result<(), SnapError> {
        let control = input.anchor_controls().start;
        if input.anchor_controls().is_empty() {
            return Ok(());
        }
        for &(feature, spec) in &prepared.0 {
            // Independent axes have independent capture distances. A shared influence requires
            // a single coupled candidate, keeping rotated/proportional corrections indivisible.
            let inverse = input
                .target_to_surface()
                .inverse()
                .ok_or(SnapError::Invalid("singular grid target mapping"))?;
            let influences = input.controls()[control].influence();
            let coupled = influences.iter().any(|&v| {
                let v = inverse.transform_vector(v);
                v.x() != 0.0 && v.y() != 0.0
            });
            let specs = if coupled {
                vec![spec]
            } else {
                (0..2)
                    .filter(|&axis| spec.spacing[axis].is_some())
                    .map(|axis| {
                        let mut isolated = spec;
                        isolated.spacing[1 - axis] = None;
                        isolated
                    })
                    .collect()
            };
            for (stable, spec) in specs.into_iter().enumerate() {
                let projection = GridProjection::new(spec, control);
                let Some(values) = correction(input, projection)? else {
                    continue;
                };
                let mut writes = Vec::new();
                let mut dependencies = Vec::new();
                for (index, &value) in values.iter().enumerate() {
                    let v: Vec2 = inverse.transform_vector(influences[index]);
                    if (spec.spacing[0].is_some() && v.x() != 0.0)
                        || (spec.spacing[1].is_some() && v.y() != 0.0)
                    {
                        dependencies.push(input.dofs()[index]);
                        writes.push((input.dofs()[index], value));
                    }
                }
                sink.propose(SnapCandidate::new(
                    feature,
                    stable as u64,
                    writes,
                    dependencies,
                    self.admission.bind(projection),
                    self.alignment.bind(projection),
                    Vec::new(),
                ))?;
            }
        }
        Ok(())
    }
}
