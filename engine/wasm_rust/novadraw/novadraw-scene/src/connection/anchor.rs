use std::{error::Error, fmt, sync::Arc};

use novadraw_geometry::{Dimension, Point, Rectangle, Vector};

use super::{AnchorGeometry, AnchorGeometryKey, CoordinateSpace, SceneQuery, SceneQueryError};
use crate::FigureId;

const GEOMETRY_EPSILON: f64 = 1.0e-9;
const ROUNDED_INTERSECTION_STEPS: usize = 64;

/// Stable semantic identity used by routers that group equivalent endpoints.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AnchorSemanticKey {
    owner: Option<FigureId>,
    kind: Arc<str>,
    parameter_bits: Vec<u64>,
}

/// Effective identity used to group equivalent Connection endpoints.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AnchorGroupKey {
    /// Value identity supplied by the Anchor strategy.
    Semantic(AnchorSemanticKey),
    /// Runtime identity used when the strategy has no value identity.
    Instance(super::AnchorId),
}

impl AnchorSemanticKey {
    /// Creates a semantic key for equivalent Anchor instances.
    pub fn new(
        owner: Option<FigureId>,
        kind: impl Into<Arc<str>>,
        parameter_bits: impl Into<Vec<u64>>,
    ) -> Result<Self, AnchorSemanticKeyError> {
        let kind = kind.into();
        if kind.is_empty() {
            return Err(AnchorSemanticKeyError);
        }
        Ok(Self {
            owner,
            kind,
            parameter_bits: parameter_bits.into(),
        })
    }

    fn builtin(owner: Option<FigureId>, kind: &'static str, parameter_bits: Vec<u64>) -> Self {
        Self {
            owner,
            kind: Arc::from(kind),
            parameter_bits,
        }
    }
}

/// Failure to construct an Anchor semantic key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnchorSemanticKeyError;

impl fmt::Display for AnchorSemanticKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "anchor semantic kind must not be empty")
    }
}

impl Error for AnchorSemanticKeyError {}

/// A resolved endpoint in the requested coordinate space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchorSite {
    /// Endpoint location.
    pub point: Point,
    /// Optional outward geometric normal.
    pub outward_normal: Option<Vector>,
}

/// Pure endpoint strategy used by a ConnectionRouter.
pub trait ConnectionAnchor {
    /// Returns the primary owner used for diagnostics and lifecycle checks.
    fn owner(&self) -> Option<FigureId>;

    /// Returns semantic grouping identity when separate instances are equivalent.
    fn semantic_group_key(&self) -> Option<AnchorSemanticKey> {
        None
    }

    /// Resolves the anchor reference point into `output`.
    fn reference_point(
        &self,
        scene: &mut dyn SceneQuery,
        output: CoordinateSpace,
    ) -> Result<Point, AnchorError>;

    /// Resolves the endpoint using a point expressed in `reference_space`.
    fn location(
        &self,
        scene: &mut dyn SceneQuery,
        reference: Point,
        reference_space: CoordinateSpace,
        output: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError>;
}

/// Failure produced while resolving an Anchor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnchorError {
    /// Scene lookup or coordinate conversion failed.
    Scene(SceneQueryError),
    /// An owner-backed Anchor has no owner.
    MissingOwner,
    /// Owner geometry is empty, negative, or incompatible with this Anchor.
    InvalidGeometry(FigureId),
    /// An input or result contains a non-finite coordinate.
    NonFiniteResult,
}

impl fmt::Display for AnchorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scene(error) => write!(formatter, "{error}"),
            Self::MissingOwner => write!(formatter, "anchor requires an owner"),
            Self::InvalidGeometry(owner) => {
                write!(formatter, "invalid Anchor geometry for Figure {owner:?}")
            }
            Self::NonFiniteResult => {
                write!(formatter, "anchor calculation produced non-finite geometry")
            }
        }
    }
}

impl Error for AnchorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scene(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SceneQueryError> for AnchorError {
    fn from(value: SceneQueryError) -> Self {
        Self::Scene(value)
    }
}

/// Owner-independent Anchor fixed in an explicit coordinate space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XYAnchor {
    point: Point,
    space: CoordinateSpace,
}

impl XYAnchor {
    /// Creates a fixed Anchor.
    pub fn new(point: Point, space: CoordinateSpace) -> Self {
        Self { point, space }
    }
}

impl ConnectionAnchor for XYAnchor {
    fn owner(&self) -> Option<FigureId> {
        None
    }

    fn reference_point(
        &self,
        scene: &mut dyn SceneQuery,
        output: CoordinateSpace,
    ) -> Result<Point, AnchorError> {
        map_finite_point(scene, self.point, self.space, output)
    }

    fn location(
        &self,
        scene: &mut dyn SceneQuery,
        _reference: Point,
        _reference_space: CoordinateSpace,
        output: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError> {
        Ok(AnchorSite {
            point: self.reference_point(scene, output)?,
            outward_normal: None,
        })
    }
}

/// Rectangle-boundary Anchor equivalent to Draw2D ChopboxAnchor.
#[derive(Clone, Debug, PartialEq)]
pub struct ChopboxAnchor {
    owner: FigureId,
    geometry_key: AnchorGeometryKey,
}

impl ChopboxAnchor {
    /// Creates an Anchor on the owner's border box.
    pub fn new(owner: FigureId) -> Self {
        Self {
            owner,
            geometry_key: AnchorGeometryKey::border_box(),
        }
    }

    /// Creates an Anchor on a named owner-local region.
    pub fn with_geometry(owner: FigureId, geometry_key: AnchorGeometryKey) -> Self {
        Self {
            owner,
            geometry_key,
        }
    }
}

impl ConnectionAnchor for ChopboxAnchor {
    fn owner(&self) -> Option<FigureId> {
        Some(self.owner)
    }

    fn semantic_group_key(&self) -> Option<AnchorSemanticKey> {
        Some(geometry_semantic_key(
            self.owner,
            "chopbox",
            &self.geometry_key,
            Vec::new(),
        ))
    }

    fn reference_point(
        &self,
        scene: &mut dyn SceneQuery,
        output: CoordinateSpace,
    ) -> Result<Point, AnchorError> {
        let bounds = anchor_bounds(scene, self.owner, &self.geometry_key)?;
        map_finite_point(
            scene,
            bounds.center(),
            CoordinateSpace::FigureLocal(self.owner),
            output,
        )
    }

    fn location(
        &self,
        scene: &mut dyn SceneQuery,
        reference: Point,
        reference_space: CoordinateSpace,
        output: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError> {
        let bounds = anchor_bounds(scene, self.owner, &self.geometry_key)?;
        let local_reference = map_finite_point(
            scene,
            reference,
            reference_space,
            CoordinateSpace::FigureLocal(self.owner),
        )?;
        let site = rectangle_boundary_site(bounds, local_reference);
        map_site(
            scene,
            self.owner,
            site.point,
            site.outward_normal,
            CoordinateSpace::FigureLocal(self.owner),
            output,
        )
    }
}

/// Ellipse-boundary Anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct EllipseAnchor {
    owner: FigureId,
    geometry_key: AnchorGeometryKey,
}

impl EllipseAnchor {
    /// Creates an Anchor on the ellipse defined by the owner's border box.
    pub fn new(owner: FigureId) -> Self {
        Self {
            owner,
            geometry_key: AnchorGeometryKey::border_box(),
        }
    }

    /// Creates an Anchor on an ellipse defined by a named owner-local region.
    pub fn with_geometry(owner: FigureId, geometry_key: AnchorGeometryKey) -> Self {
        Self {
            owner,
            geometry_key,
        }
    }
}

impl ConnectionAnchor for EllipseAnchor {
    fn owner(&self) -> Option<FigureId> {
        Some(self.owner)
    }

    fn semantic_group_key(&self) -> Option<AnchorSemanticKey> {
        Some(geometry_semantic_key(
            self.owner,
            "ellipse",
            &self.geometry_key,
            Vec::new(),
        ))
    }

    fn reference_point(
        &self,
        scene: &mut dyn SceneQuery,
        output: CoordinateSpace,
    ) -> Result<Point, AnchorError> {
        let bounds = anchor_bounds(scene, self.owner, &self.geometry_key)?;
        map_finite_point(
            scene,
            bounds.center(),
            CoordinateSpace::FigureLocal(self.owner),
            output,
        )
    }

    fn location(
        &self,
        scene: &mut dyn SceneQuery,
        reference: Point,
        reference_space: CoordinateSpace,
        output: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError> {
        let bounds = anchor_bounds(scene, self.owner, &self.geometry_key)?;
        let local_reference = map_finite_point(
            scene,
            reference,
            reference_space,
            CoordinateSpace::FigureLocal(self.owner),
        )?;
        let (point, normal) = ellipse_intersection(bounds, local_reference);
        map_site(
            scene,
            self.owner,
            point,
            normal,
            CoordinateSpace::FigureLocal(self.owner),
            output,
        )
    }
}

/// Rounded-rectangle-boundary Anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct RoundedRectangleAnchor {
    owner: FigureId,
    geometry_key: AnchorGeometryKey,
    corner_override: Option<Dimension>,
}

impl RoundedRectangleAnchor {
    /// Creates an Anchor using rounded geometry exposed by the owner.
    pub fn new(owner: FigureId) -> Self {
        Self {
            owner,
            geometry_key: AnchorGeometryKey::border_box(),
            corner_override: None,
        }
    }

    /// Creates an Anchor with explicit full corner ellipse dimensions.
    pub fn with_corner_dimensions(owner: FigureId, corner: Dimension) -> Self {
        Self {
            owner,
            geometry_key: AnchorGeometryKey::border_box(),
            corner_override: Some(corner),
        }
    }

    /// Selects a named owner-local region.
    pub fn with_geometry(mut self, geometry_key: AnchorGeometryKey) -> Self {
        self.geometry_key = geometry_key;
        self
    }
}

impl ConnectionAnchor for RoundedRectangleAnchor {
    fn owner(&self) -> Option<FigureId> {
        Some(self.owner)
    }

    fn semantic_group_key(&self) -> Option<AnchorSemanticKey> {
        let mut parameters = Vec::new();
        if let Some(corner) = self.corner_override {
            parameters.extend([corner.width.to_bits(), corner.height.to_bits()]);
        }
        Some(geometry_semantic_key(
            self.owner,
            "rounded_rectangle",
            &self.geometry_key,
            parameters,
        ))
    }

    fn reference_point(
        &self,
        scene: &mut dyn SceneQuery,
        output: CoordinateSpace,
    ) -> Result<Point, AnchorError> {
        let (bounds, _) = self.geometry(scene)?;
        map_finite_point(
            scene,
            bounds.center(),
            CoordinateSpace::FigureLocal(self.owner),
            output,
        )
    }

    fn location(
        &self,
        scene: &mut dyn SceneQuery,
        reference: Point,
        reference_space: CoordinateSpace,
        output: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError> {
        let (bounds, corner) = self.geometry(scene)?;
        let local_reference = map_finite_point(
            scene,
            reference,
            reference_space,
            CoordinateSpace::FigureLocal(self.owner),
        )?;
        let (point, normal) = rounded_rectangle_intersection(bounds, corner, local_reference);
        map_site(
            scene,
            self.owner,
            point,
            normal,
            CoordinateSpace::FigureLocal(self.owner),
            output,
        )
    }
}

impl RoundedRectangleAnchor {
    fn geometry(&self, scene: &mut dyn SceneQuery) -> Result<(Rectangle, Dimension), AnchorError> {
        let geometry = scene.anchor_geometry(self.owner, &self.geometry_key)?;
        let bounds = geometry.bounds();
        validate_bounds(self.owner, bounds)?;
        let corner = self.corner_override.or(match geometry {
            AnchorGeometry::RoundedRectangle { corner, .. } => Some(corner),
            _ => None,
        });
        let Some(corner) = corner else {
            return Err(AnchorError::InvalidGeometry(self.owner));
        };
        if !corner.width.is_finite()
            || !corner.height.is_finite()
            || corner.width < 0.0
            || corner.height < 0.0
        {
            return Err(AnchorError::InvalidGeometry(self.owner));
        }
        Ok((bounds, corner))
    }
}

/// Chopbox-style Anchor targeting a named `icon` region.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelAnchor {
    inner: ChopboxAnchor,
}

impl LabelAnchor {
    /// Creates a Label Anchor that reads the owner's named icon region.
    pub fn new(owner: FigureId) -> Self {
        Self {
            inner: ChopboxAnchor::with_geometry(owner, AnchorGeometryKey::icon()),
        }
    }
}

impl ConnectionAnchor for LabelAnchor {
    fn owner(&self) -> Option<FigureId> {
        self.inner.owner()
    }

    fn semantic_group_key(&self) -> Option<AnchorSemanticKey> {
        Some(AnchorSemanticKey::builtin(
            self.owner(),
            "label",
            Vec::new(),
        ))
    }

    fn reference_point(
        &self,
        scene: &mut dyn SceneQuery,
        output: CoordinateSpace,
    ) -> Result<Point, AnchorError> {
        self.inner.reference_point(scene, output)
    }

    fn location(
        &self,
        scene: &mut dyn SceneQuery,
        reference: Point,
        reference_space: CoordinateSpace,
        output: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError> {
        self.inner
            .location(scene, reference, reference_space, output)
    }
}

fn anchor_bounds(
    scene: &mut dyn SceneQuery,
    owner: FigureId,
    key: &AnchorGeometryKey,
) -> Result<Rectangle, AnchorError> {
    let bounds = scene.anchor_geometry(owner, key)?.bounds();
    validate_bounds(owner, bounds)?;
    Ok(bounds)
}

fn validate_bounds(owner: FigureId, bounds: Rectangle) -> Result<(), AnchorError> {
    if !bounds.x.is_finite()
        || !bounds.y.is_finite()
        || !bounds.width.is_finite()
        || !bounds.height.is_finite()
    {
        return Err(AnchorError::NonFiniteResult);
    }
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Err(AnchorError::InvalidGeometry(owner));
    }
    Ok(())
}

fn map_finite_point(
    scene: &mut dyn SceneQuery,
    point: Point,
    from: CoordinateSpace,
    to: CoordinateSpace,
) -> Result<Point, AnchorError> {
    if !point.x().is_finite() || !point.y().is_finite() {
        return Err(AnchorError::NonFiniteResult);
    }
    let point = scene.map_point(point, from, to)?;
    if !point.x().is_finite() || !point.y().is_finite() {
        return Err(AnchorError::NonFiniteResult);
    }
    Ok(point)
}

fn map_site(
    scene: &mut dyn SceneQuery,
    _owner: FigureId,
    point: Point,
    normal: Option<Vector>,
    from: CoordinateSpace,
    to: CoordinateSpace,
) -> Result<AnchorSite, AnchorError> {
    let point = map_finite_point(scene, point, from, to)?;
    let outward_normal = match normal {
        Some(normal) => {
            let mapped = scene.map_normal(normal, from, to)?;
            normalize(mapped).ok_or(AnchorError::NonFiniteResult)?
        }
        None => {
            return Ok(AnchorSite {
                point,
                outward_normal: None,
            });
        }
    };
    Ok(AnchorSite {
        point,
        outward_normal: Some(outward_normal),
    })
}

/// Resolves the point where a ray from a rectangle's center toward `reference` meets its border.
///
/// When `reference` equals the center, the center is returned without an outward normal.
pub fn rectangle_boundary_site(bounds: Rectangle, reference: Point) -> AnchorSite {
    let center = bounds.center();
    let delta = reference - center;
    if delta.length_squared() <= GEOMETRY_EPSILON * GEOMETRY_EPSILON {
        return AnchorSite {
            point: center,
            outward_normal: None,
        };
    }

    let x_scale = if delta.x().abs() <= GEOMETRY_EPSILON {
        f64::INFINITY
    } else {
        bounds.width / (2.0 * delta.x().abs())
    };
    let y_scale = if delta.y().abs() <= GEOMETRY_EPSILON {
        f64::INFINITY
    } else {
        bounds.height / (2.0 * delta.y().abs())
    };
    let scale = x_scale.min(y_scale);
    let point = center + delta * scale;
    let normal = if x_scale < y_scale {
        Vector::new(delta.x().signum(), 0.0)
    } else {
        Vector::new(0.0, delta.y().signum())
    };
    AnchorSite {
        point,
        outward_normal: Some(normal),
    }
}

fn ellipse_intersection(bounds: Rectangle, reference: Point) -> (Point, Option<Vector>) {
    let center = bounds.center();
    let delta = reference - center;
    if delta.length_squared() <= GEOMETRY_EPSILON * GEOMETRY_EPSILON {
        return (center, None);
    }
    let radius_x = bounds.width / 2.0;
    let radius_y = bounds.height / 2.0;
    let divisor = ((delta.x() / radius_x).powi(2) + (delta.y() / radius_y).powi(2)).sqrt();
    let point = center + delta / divisor;
    let normal = Vector::new(
        (point.x() - center.x()) / radius_x.powi(2),
        (point.y() - center.y()) / radius_y.powi(2),
    );
    (point, normalize(normal))
}

fn rounded_rectangle_intersection(
    bounds: Rectangle,
    corner: Dimension,
    reference: Point,
) -> (Point, Option<Vector>) {
    let center = bounds.center();
    let delta = reference - center;
    if delta.length_squared() <= GEOMETRY_EPSILON * GEOMETRY_EPSILON {
        return (center, None);
    }

    let radius_x = (corner.width / 2.0).clamp(0.0, bounds.width / 2.0);
    let radius_y = (corner.height / 2.0).clamp(0.0, bounds.height / 2.0);
    if radius_x <= GEOMETRY_EPSILON || radius_y <= GEOMETRY_EPSILON {
        let site = rectangle_boundary_site(bounds, reference);
        return (site.point, site.outward_normal);
    }

    let box_point = rectangle_boundary_site(bounds, reference).point;
    if point_in_rounded_rectangle(box_point, bounds, radius_x, radius_y) {
        return (
            box_point,
            rounded_rectangle_normal(box_point, bounds, radius_x, radius_y),
        );
    }

    let mut inside = 0.0;
    let mut outside = 1.0;
    for _ in 0..ROUNDED_INTERSECTION_STEPS {
        let candidate = center + (box_point - center) * ((inside + outside) / 2.0);
        if point_in_rounded_rectangle(candidate, bounds, radius_x, radius_y) {
            inside = (inside + outside) / 2.0;
        } else {
            outside = (inside + outside) / 2.0;
        }
    }
    let point = center + (box_point - center) * inside;
    (
        point,
        rounded_rectangle_normal(point, bounds, radius_x, radius_y),
    )
}

fn point_in_rounded_rectangle(
    point: Point,
    bounds: Rectangle,
    radius_x: f64,
    radius_y: f64,
) -> bool {
    if !bounds.contains(point) {
        return false;
    }
    let nearest_x = point
        .x()
        .clamp(bounds.x + radius_x, bounds.x + bounds.width - radius_x);
    let nearest_y = point
        .y()
        .clamp(bounds.y + radius_y, bounds.y + bounds.height - radius_y);
    let dx = (point.x() - nearest_x) / radius_x;
    let dy = (point.y() - nearest_y) / radius_y;
    dx * dx + dy * dy <= 1.0 + GEOMETRY_EPSILON
}

fn rounded_rectangle_normal(
    point: Point,
    bounds: Rectangle,
    radius_x: f64,
    radius_y: f64,
) -> Option<Vector> {
    let corner_x = point
        .x()
        .clamp(bounds.x + radius_x, bounds.x + bounds.width - radius_x);
    let corner_y = point
        .y()
        .clamp(bounds.y + radius_y, bounds.y + bounds.height - radius_y);
    let dx = point.x() - corner_x;
    let dy = point.y() - corner_y;
    if dx.abs() > GEOMETRY_EPSILON && dy.abs() > GEOMETRY_EPSILON {
        return normalize(Vector::new(dx / radius_x.powi(2), dy / radius_y.powi(2)));
    }
    let center = bounds.center();
    if (point.x() - bounds.x).abs() <= GEOMETRY_EPSILON {
        Some(-Vector::X)
    } else if (point.x() - (bounds.x + bounds.width)).abs() <= GEOMETRY_EPSILON {
        Some(Vector::X)
    } else if (point.y() - bounds.y).abs() <= GEOMETRY_EPSILON {
        Some(-Vector::Y)
    } else if (point.y() - (bounds.y + bounds.height)).abs() <= GEOMETRY_EPSILON {
        Some(Vector::Y)
    } else {
        normalize(point - center)
    }
}

fn normalize(vector: Vector) -> Option<Vector> {
    let length = vector.length();
    if !length.is_finite() || length <= GEOMETRY_EPSILON {
        None
    } else {
        Some(vector / length)
    }
}

fn geometry_semantic_key(
    owner: FigureId,
    kind: &'static str,
    geometry_key: &AnchorGeometryKey,
    mut parameters: Vec<u64>,
) -> AnchorSemanticKey {
    if let Some(name) = geometry_key.name() {
        parameters.push(1);
        parameters.extend(name.as_bytes().iter().map(|byte| u64::from(*byte)));
    } else {
        parameters.push(0);
    }
    AnchorSemanticKey::builtin(Some(owner), kind, parameters)
}
