use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

use novadraw_geometry::{Affine2D, Dimension, Point, Rectangle, Vec2};

use crate::{FigureId, FigureTree, ViewportFigure};

/// Coordinate domain used by pure Anchor and Router calculations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CoordinateSpace {
    /// Local border-box coordinates of a Figure.
    FigureLocal(FigureId),
    /// Child content coordinates exposed by a container Figure.
    ChildContent(FigureId),
    /// Logical surface coordinates before physical DPI projection.
    LogicalSurface,
}

/// Stable key selecting geometry exposed by a Figure to an Anchor.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AnchorGeometryKey {
    kind: AnchorGeometryKeyKind,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum AnchorGeometryKeyKind {
    BorderBox,
    Named(Arc<str>),
}

impl AnchorGeometryKey {
    /// Returns the Figure-local border box key.
    pub fn border_box() -> Self {
        Self {
            kind: AnchorGeometryKeyKind::BorderBox,
        }
    }

    /// Returns the conventional Label icon region key.
    pub fn icon() -> Self {
        Self {
            kind: AnchorGeometryKeyKind::Named(Arc::from("icon")),
        }
    }

    /// Creates a non-empty named geometry key.
    pub fn named(value: impl Into<Arc<str>>) -> Result<Self, AnchorGeometryKeyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(AnchorGeometryKeyError);
        }
        Ok(Self {
            kind: AnchorGeometryKeyKind::Named(value),
        })
    }

    /// Returns whether this key selects the Figure border box.
    pub fn is_border_box(&self) -> bool {
        matches!(self.kind, AnchorGeometryKeyKind::BorderBox)
    }

    /// Returns the named region, or `None` for the border box.
    pub fn name(&self) -> Option<&str> {
        match &self.kind {
            AnchorGeometryKeyKind::BorderBox => None,
            AnchorGeometryKeyKind::Named(name) => Some(name),
        }
    }
}

/// Failure to construct a named Anchor geometry key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnchorGeometryKeyError;

impl fmt::Display for AnchorGeometryKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "anchor geometry key must not be empty")
    }
}

impl Error for AnchorGeometryKeyError {}

/// Geometry exposed to an Anchor in the owner's local coordinate space.
#[derive(Clone, Debug, PartialEq)]
pub enum AnchorGeometry {
    /// Rectangular region.
    Rectangle(Rectangle),
    /// Ellipse bounded by the rectangle.
    Ellipse(Rectangle),
    /// Rounded rectangle and its full corner ellipse dimensions.
    RoundedRectangle {
        /// Local bounding rectangle.
        bounds: Rectangle,
        /// Width and height of the full corner ellipse.
        corner: Dimension,
    },
}

impl AnchorGeometry {
    /// Returns the local bounding rectangle.
    pub fn bounds(&self) -> Rectangle {
        match self {
            Self::Rectangle(bounds)
            | Self::Ellipse(bounds)
            | Self::RoundedRectangle { bounds, .. } => *bounds,
        }
    }
}

/// Stable subject used by the Runtime reverse dependency index.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum DependencySubject {
    /// Figure-local geometry.
    FigureGeometry(FigureId),
    /// Named geometry exposed by a Figure.
    NamedAnchorRegion(FigureId, AnchorGeometryKey),
    /// Relative transform between two coordinate spaces.
    RelativeTransform(CoordinateSpace, CoordinateSpace),
    /// Figure attachment or ancestry.
    Topology(FigureId),
}

/// A dependency subject observed at a specific generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyObservation {
    /// Stable dependency identity.
    pub subject: DependencySubject,
    /// Generation observed by the query.
    pub generation: u64,
}

/// Immutable scene data source wrapped by [`TrackedSceneQuery`].
pub trait SceneRead {
    /// Returns whether a Figure is attached to the current public tree.
    fn is_attached(&self, figure: FigureId) -> bool;

    /// Returns the direct parent of a Figure.
    fn parent_id(&self, figure: FigureId) -> Option<FigureId>;

    /// Returns whether a Figure is a Viewport coordinate/clipping boundary.
    fn is_viewport(&self, _figure: FigureId) -> bool {
        false
    }

    /// Returns the Figure-local border box.
    fn border_box(&self, figure: FigureId) -> Result<Rectangle, SceneQueryError>;

    /// Returns Figure-local geometry selected by `key`.
    fn anchor_geometry(
        &self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Result<AnchorGeometry, SceneQueryError>;

    /// Maps a point between explicit coordinate spaces.
    fn map_point(
        &self,
        point: Point,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Point, SceneQueryError>;

    /// Maps a rectangle to the axis-aligned envelope in another space.
    fn map_rect(
        &self,
        rect: Rectangle,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Rectangle, SceneQueryError>;

    /// Maps a geometric normal using the inverse-transpose linear transform.
    fn map_normal(
        &self,
        normal: Vec2,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Vec2, SceneQueryError>;

    /// Returns the current generation of a stable dependency subject.
    fn dependency_generation(&self, subject: &DependencySubject) -> u64;
}

/// Read-only, dependency-tracking scene access used by Anchor and Router strategies.
pub trait SceneQuery {
    /// Returns whether a Figure is attached to the current public tree.
    fn is_attached(&mut self, figure: FigureId) -> Result<bool, SceneQueryError>;

    /// Returns the direct parent of a Figure.
    fn parent_id(&mut self, figure: FigureId) -> Result<Option<FigureId>, SceneQueryError>;

    /// Returns the Figure-local border box.
    fn border_box(&mut self, figure: FigureId) -> Result<Rectangle, SceneQueryError>;

    /// Returns Figure-local geometry selected by `key`.
    fn anchor_geometry(
        &mut self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Result<AnchorGeometry, SceneQueryError>;

    /// Maps a point between explicit coordinate spaces.
    fn map_point(
        &mut self,
        point: Point,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Point, SceneQueryError>;

    /// Maps a rectangle to the axis-aligned envelope in another space.
    fn map_rect(
        &mut self,
        rect: Rectangle,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Rectangle, SceneQueryError>;

    /// Maps a geometric normal using the inverse-transpose linear transform.
    fn map_normal(
        &mut self,
        normal: Vec2,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Vec2, SceneQueryError>;
}

/// One dependency-tracking query session over an immutable scene snapshot.
pub struct TrackedSceneQuery<'a> {
    source: &'a dyn SceneRead,
    observations: Vec<DependencyObservation>,
    indices: HashMap<DependencySubject, usize>,
}

impl<'a> TrackedSceneQuery<'a> {
    /// Starts a new tracking session.
    pub fn new(source: &'a dyn SceneRead) -> Self {
        Self {
            source,
            observations: Vec::new(),
            indices: HashMap::new(),
        }
    }

    /// Returns dependency observations in first-read order.
    pub fn observations(&self) -> &[DependencyObservation] {
        &self.observations
    }

    /// Finishes this session and returns dependency observations.
    pub fn into_observations(self) -> Vec<DependencyObservation> {
        self.observations
    }

    fn observe(&mut self, subject: DependencySubject) -> Result<(), SceneQueryError> {
        let generation = self.source.dependency_generation(&subject);
        if let Some(index) = self.indices.get(&subject).copied() {
            if self.observations[index].generation != generation {
                return Err(SceneQueryError::SceneChangedDuringQuery(subject));
            }
            return Ok(());
        }
        self.indices
            .insert(subject.clone(), self.observations.len());
        self.observations.push(DependencyObservation {
            subject,
            generation,
        });
        Ok(())
    }
}

impl SceneQuery for TrackedSceneQuery<'_> {
    fn is_attached(&mut self, figure: FigureId) -> Result<bool, SceneQueryError> {
        self.observe(DependencySubject::Topology(figure))?;
        Ok(self.source.is_attached(figure))
    }

    fn parent_id(&mut self, figure: FigureId) -> Result<Option<FigureId>, SceneQueryError> {
        self.observe(DependencySubject::Topology(figure))?;
        Ok(self.source.parent_id(figure))
    }

    fn border_box(&mut self, figure: FigureId) -> Result<Rectangle, SceneQueryError> {
        self.observe(DependencySubject::FigureGeometry(figure))?;
        self.source.border_box(figure)
    }

    fn anchor_geometry(
        &mut self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Result<AnchorGeometry, SceneQueryError> {
        let subject = if key.is_border_box() {
            DependencySubject::FigureGeometry(figure)
        } else {
            DependencySubject::NamedAnchorRegion(figure, key.clone())
        };
        self.observe(subject)?;
        self.source.anchor_geometry(figure, key)
    }

    fn map_point(
        &mut self,
        point: Point,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Point, SceneQueryError> {
        self.observe(DependencySubject::RelativeTransform(from, to))?;
        self.source.map_point(point, from, to)
    }

    fn map_rect(
        &mut self,
        rect: Rectangle,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Rectangle, SceneQueryError> {
        self.observe(DependencySubject::RelativeTransform(from, to))?;
        self.source.map_rect(rect, from, to)
    }

    fn map_normal(
        &mut self,
        normal: Vec2,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Vec2, SceneQueryError> {
        self.observe(DependencySubject::RelativeTransform(from, to))?;
        self.source.map_normal(normal, from, to)
    }
}

pub(crate) struct FigureTreeSceneRead<'a> {
    tree: &'a FigureTree,
    anchor_geometries: &'a HashMap<(FigureId, AnchorGeometryKey), AnchorGeometry>,
}

impl<'a> FigureTreeSceneRead<'a> {
    pub(crate) fn new(
        tree: &'a FigureTree,
        anchor_geometries: &'a HashMap<(FigureId, AnchorGeometryKey), AnchorGeometry>,
    ) -> Self {
        Self {
            tree,
            anchor_geometries,
        }
    }

    fn to_surface_transform(&self, space: CoordinateSpace) -> Option<Affine2D> {
        match space {
            CoordinateSpace::LogicalSurface => Some(Affine2D::IDENTITY),
            CoordinateSpace::FigureLocal(figure) => self.tree.local_to_surface_transform(figure),
            CoordinateSpace::ChildContent(figure) => {
                self.tree.child_content_to_surface_transform(figure)
            }
        }
    }

    fn transform(
        &self,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Affine2D, SceneQueryError> {
        let from_surface = self
            .to_surface_transform(from)
            .ok_or_else(|| coordinate_space_error(from))?;
        let to_surface = self
            .to_surface_transform(to)
            .ok_or_else(|| coordinate_space_error(to))?;
        let inverse = to_surface
            .inverse()
            .ok_or_else(|| coordinate_space_error(to))?;
        Ok(inverse * from_surface)
    }
}

impl SceneRead for FigureTreeSceneRead<'_> {
    fn is_attached(&self, figure: FigureId) -> bool {
        self.tree.is_attached(figure)
    }

    fn parent_id(&self, figure: FigureId) -> Option<FigureId> {
        self.tree.parent_id(figure)
    }

    fn is_viewport(&self, figure: FigureId) -> bool {
        self.tree
            .node(figure)
            .is_some_and(|block| block.figure.as_any().is::<ViewportFigure>())
    }

    fn border_box(&self, figure: FigureId) -> Result<Rectangle, SceneQueryError> {
        if !self.tree.is_attached(figure) {
            return Err(SceneQueryError::DetachedFigure(figure));
        }
        let bounds = self
            .tree
            .figure_bounds(figure)
            .ok_or(SceneQueryError::UnknownFigure(figure))?;
        if !bounds.width.is_finite() || !bounds.height.is_finite() {
            return Err(SceneQueryError::NonFiniteGeometry(figure));
        }
        Ok(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))
    }

    fn anchor_geometry(
        &self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Result<AnchorGeometry, SceneQueryError> {
        if key.is_border_box() {
            return self.border_box(figure).map(AnchorGeometry::Rectangle);
        }
        self.anchor_geometries
            .get(&(figure, key.clone()))
            .cloned()
            .ok_or_else(|| SceneQueryError::MissingAnchorGeometry {
                figure,
                key: key.clone(),
            })
    }

    fn map_point(
        &self,
        point: Point,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Point, SceneQueryError> {
        let mapped = self.transform(from, to)?.transform_point(point);
        if !mapped.x().is_finite() || !mapped.y().is_finite() {
            return Err(SceneQueryError::NonFiniteCoordinateMap { from, to });
        }
        Ok(mapped)
    }

    fn map_rect(
        &self,
        rect: Rectangle,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Rectangle, SceneQueryError> {
        let corners = [
            Point::new(rect.x, rect.y),
            Point::new(rect.x + rect.width, rect.y),
            Point::new(rect.x, rect.y + rect.height),
            Point::new(rect.x + rect.width, rect.y + rect.height),
        ];
        let mapped = corners
            .map(|point| self.map_point(point, from, to))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let min_x = mapped
            .iter()
            .map(|point| point.x())
            .fold(f64::INFINITY, f64::min);
        let min_y = mapped
            .iter()
            .map(|point| point.y())
            .fold(f64::INFINITY, f64::min);
        let max_x = mapped
            .iter()
            .map(|point| point.x())
            .fold(f64::NEG_INFINITY, f64::max);
        let max_y = mapped
            .iter()
            .map(|point| point.y())
            .fold(f64::NEG_INFINITY, f64::max);
        Ok(Rectangle::new(min_x, min_y, max_x - min_x, max_y - min_y))
    }

    fn map_normal(
        &self,
        normal: Vec2,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Vec2, SceneQueryError> {
        let [a, b, c, d, _, _] = self.transform(from, to)?.coeffs();
        let determinant = a * d - b * c;
        if !determinant.is_finite() || determinant.abs() <= f64::EPSILON {
            return Err(SceneQueryError::NonInvertibleCoordinateMap { from, to });
        }
        let x = (d * normal.x() - b * normal.y()) / determinant;
        let y = (-c * normal.x() + a * normal.y()) / determinant;
        if !x.is_finite() || !y.is_finite() {
            return Err(SceneQueryError::NonFiniteCoordinateMap { from, to });
        }
        Ok(Vec2::new(x, y))
    }

    fn dependency_generation(&self, subject: &DependencySubject) -> u64 {
        let mut hasher = DefaultHasher::new();
        subject.hash(&mut hasher);
        match subject {
            DependencySubject::FigureGeometry(figure)
            | DependencySubject::NamedAnchorRegion(figure, _) => {
                if let Some(bounds) = self.tree.figure_bounds(*figure) {
                    for value in [bounds.x, bounds.y, bounds.width, bounds.height] {
                        value.to_bits().hash(&mut hasher);
                    }
                }
                if let DependencySubject::NamedAnchorRegion(_, key) = subject
                    && let Some(geometry) = self.anchor_geometries.get(&(*figure, key.clone()))
                {
                    hash_anchor_geometry(geometry, &mut hasher);
                }
            }
            DependencySubject::RelativeTransform(from, to) => {
                if let Ok(transform) = self.transform(*from, *to) {
                    for value in transform.coeffs() {
                        value.to_bits().hash(&mut hasher);
                    }
                }
            }
            DependencySubject::Topology(figure) => {
                self.tree.parent_id(*figure).hash(&mut hasher);
                self.tree.is_attached(*figure).hash(&mut hasher);
                self.tree.child_order(*figure).hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

fn hash_anchor_geometry(geometry: &AnchorGeometry, hasher: &mut DefaultHasher) {
    let bounds = geometry.bounds();
    for value in [bounds.x, bounds.y, bounds.width, bounds.height] {
        value.to_bits().hash(hasher);
    }
    if let AnchorGeometry::RoundedRectangle { corner, .. } = geometry {
        corner.width.to_bits().hash(hasher);
        corner.height.to_bits().hash(hasher);
    }
}

fn coordinate_space_error(space: CoordinateSpace) -> SceneQueryError {
    match space {
        CoordinateSpace::FigureLocal(figure) | CoordinateSpace::ChildContent(figure) => {
            SceneQueryError::NonInvertibleTransform(figure)
        }
        CoordinateSpace::LogicalSurface => SceneQueryError::UnavailableLogicalSurface,
    }
}

/// Failure produced by a read-only scene query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneQueryError {
    /// The Figure identity does not resolve.
    UnknownFigure(FigureId),
    /// The Figure exists but is detached from the public tree.
    DetachedFigure(FigureId),
    /// The requested named or shape geometry is unavailable.
    MissingAnchorGeometry {
        /// Figure expected to provide the geometry.
        figure: FigureId,
        /// Missing geometry key.
        key: AnchorGeometryKey,
    },
    /// A required coordinate transform is not invertible.
    NonInvertibleTransform(FigureId),
    /// Geometry or a mapped result contains a non-finite value.
    NonFiniteGeometry(FigureId),
    /// A dependency changed while one route was being calculated.
    SceneChangedDuringQuery(DependencySubject),
    /// A coordinate-space mapping is singular.
    NonInvertibleCoordinateMap {
        /// Input coordinate space.
        from: CoordinateSpace,
        /// Output coordinate space.
        to: CoordinateSpace,
    },
    /// A coordinate-space mapping produced non-finite values.
    NonFiniteCoordinateMap {
        /// Input coordinate space.
        from: CoordinateSpace,
        /// Output coordinate space.
        to: CoordinateSpace,
    },
    /// Logical surface mapping was unexpectedly unavailable.
    UnavailableLogicalSurface,
}

impl fmt::Display for SceneQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFigure(figure) => write!(formatter, "unknown Figure ID: {figure:?}"),
            Self::DetachedFigure(figure) => write!(formatter, "detached Figure ID: {figure:?}"),
            Self::MissingAnchorGeometry { figure, key } => {
                write!(
                    formatter,
                    "Figure {figure:?} does not expose Anchor geometry {key:?}"
                )
            }
            Self::NonInvertibleTransform(figure) => {
                write!(
                    formatter,
                    "Figure {figure:?} has a non-invertible transform"
                )
            }
            Self::NonFiniteGeometry(figure) => {
                write!(formatter, "Figure {figure:?} exposes non-finite geometry")
            }
            Self::SceneChangedDuringQuery(subject) => {
                write!(
                    formatter,
                    "scene dependency changed during query: {subject:?}"
                )
            }
            Self::NonInvertibleCoordinateMap { from, to } => {
                write!(
                    formatter,
                    "non-invertible coordinate map: {from:?} -> {to:?}"
                )
            }
            Self::NonFiniteCoordinateMap { from, to } => {
                write!(formatter, "non-finite coordinate map: {from:?} -> {to:?}")
            }
            Self::UnavailableLogicalSurface => {
                write!(formatter, "logical surface coordinate space is unavailable")
            }
        }
    }
}

impl Error for SceneQueryError {}
