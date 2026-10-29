use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::geometry::{Affine2D, Dimension, Point, Rectangle, Vec2};

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
    /// Runtime-owned immutable prepared result.
    PreparedScene(FigureId),
    /// Figure metrics, constraints and layout measurement generation.
    FigureMeasurement(FigureId),
    /// Source inputs for a constrained measurement, excluding derived cache invalidation.
    /// The preparation result separately retains constraints and text resource authority.
    FigureMeasurementSource(FigureId),
    /// Figure-local geometry.
    FigureGeometry(FigureId),
    /// Effective Figure visibility.
    FigureVisibility(FigureId),
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

/// Immutable scene data source wrapped by [`TrackedRouteSceneQuery`].
pub trait RouteSceneRead {
    /// Returns a checked immutable preparation result from this query's scene.
    fn prepared_scene(
        &self,
        owner: FigureId,
    ) -> Result<crate::figure::PreparedSceneSnapshot, RouteSceneQueryError> {
        Err(RouteSceneQueryError::UnsupportedPreparedSceneQuery(owner))
    }

    /// Returns whether a Figure is attached to the current public tree.
    fn is_attached(&self, figure: FigureId) -> Result<bool, RouteSceneQueryError>;

    /// Returns the direct parent of a Figure.
    fn parent_id(&self, figure: FigureId) -> Result<Option<FigureId>, RouteSceneQueryError>;

    /// Returns effective visibility through the Figure ancestor chain.
    fn is_visible(&self, figure: FigureId) -> Result<bool, RouteSceneQueryError> {
        self.is_attached(figure)
    }

    /// Returns whether a Figure is a Viewport coordinate/clipping boundary.
    fn is_viewport(&self, figure: FigureId) -> Result<bool, RouteSceneQueryError>;

    /// Returns the Figure-local border box.
    fn border_box(&self, figure: FigureId) -> Result<Rectangle, RouteSceneQueryError>;

    /// Returns Figure-local geometry selected by `key`.
    fn anchor_geometry(
        &self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Result<AnchorGeometry, RouteSceneQueryError>;

    /// Maps a point between explicit coordinate spaces.
    fn map_point(
        &self,
        point: Point,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Point, RouteSceneQueryError>;

    /// Maps a rectangle to the axis-aligned envelope in another space.
    fn map_rect(
        &self,
        rect: Rectangle,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Rectangle, RouteSceneQueryError>;

    /// Maps a geometric normal using the inverse-transpose linear transform.
    fn map_normal(
        &self,
        normal: Vec2,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Vec2, RouteSceneQueryError>;

    /// Returns the current generation of a stable dependency subject.
    fn dependency_generation(&self, subject: &DependencySubject) -> u64;
}

/// Read-only, dependency-tracking scene access used by Anchor and Router strategies.
pub trait RouteSceneQuery {
    /// Returns a checked immutable preparation result and observes its identity.
    fn prepared_scene(
        &mut self,
        owner: FigureId,
    ) -> Result<crate::figure::PreparedSceneSnapshot, RouteSceneQueryError> {
        Err(RouteSceneQueryError::UnsupportedPreparedSceneQuery(owner))
    }

    /// Returns whether a Figure is attached to the current public tree.
    fn is_attached(&mut self, figure: FigureId) -> Result<bool, RouteSceneQueryError>;

    /// Returns the direct parent of a Figure.
    fn parent_id(&mut self, figure: FigureId) -> Result<Option<FigureId>, RouteSceneQueryError>;

    /// Returns effective visibility through the Figure ancestor chain.
    fn is_visible(&mut self, figure: FigureId) -> Result<bool, RouteSceneQueryError> {
        self.is_attached(figure)
    }

    /// Returns the Figure-local border box.
    fn border_box(&mut self, figure: FigureId) -> Result<Rectangle, RouteSceneQueryError>;

    /// Returns Figure-local geometry selected by `key`.
    fn anchor_geometry(
        &mut self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Result<AnchorGeometry, RouteSceneQueryError>;

    /// Maps a point between explicit coordinate spaces.
    fn map_point(
        &mut self,
        point: Point,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Point, RouteSceneQueryError>;

    /// Maps a rectangle to the axis-aligned envelope in another space.
    fn map_rect(
        &mut self,
        rect: Rectangle,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Rectangle, RouteSceneQueryError>;

    /// Maps a geometric normal using the inverse-transpose linear transform.
    fn map_normal(
        &mut self,
        normal: Vec2,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Vec2, RouteSceneQueryError>;
}

/// One dependency-tracking query session over an immutable scene snapshot.
pub struct TrackedRouteSceneQuery<'a> {
    source: &'a dyn RouteSceneRead,
    observations: Vec<DependencyObservation>,
    indices: HashMap<DependencySubject, usize>,
}

impl<'a> TrackedRouteSceneQuery<'a> {
    /// Starts a new tracking session.
    pub fn new(source: &'a dyn RouteSceneRead) -> Self {
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

    fn observe(&mut self, subject: DependencySubject) -> Result<(), RouteSceneQueryError> {
        let generation = self.source.dependency_generation(&subject);
        if let Some(index) = self.indices.get(&subject).copied() {
            if self.observations[index].generation != generation {
                return Err(RouteSceneQueryError::SceneChangedDuringQuery(subject));
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

impl RouteSceneQuery for TrackedRouteSceneQuery<'_> {
    fn prepared_scene(
        &mut self,
        owner: FigureId,
    ) -> Result<crate::figure::PreparedSceneSnapshot, RouteSceneQueryError> {
        self.observe(DependencySubject::PreparedScene(owner))?;
        let snapshot = self.source.prepared_scene(owner)?;
        self.observe(DependencySubject::PreparedScene(owner))?;
        Ok(snapshot)
    }

    fn is_attached(&mut self, figure: FigureId) -> Result<bool, RouteSceneQueryError> {
        self.observe(DependencySubject::Topology(figure))?;
        self.source.is_attached(figure)
    }

    fn parent_id(&mut self, figure: FigureId) -> Result<Option<FigureId>, RouteSceneQueryError> {
        self.observe(DependencySubject::Topology(figure))?;
        self.source.parent_id(figure)
    }

    fn is_visible(&mut self, figure: FigureId) -> Result<bool, RouteSceneQueryError> {
        self.observe(DependencySubject::FigureVisibility(figure))?;
        self.source.is_visible(figure)
    }

    fn border_box(&mut self, figure: FigureId) -> Result<Rectangle, RouteSceneQueryError> {
        self.observe(DependencySubject::FigureGeometry(figure))?;
        self.source.border_box(figure)
    }

    fn anchor_geometry(
        &mut self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Result<AnchorGeometry, RouteSceneQueryError> {
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
    ) -> Result<Point, RouteSceneQueryError> {
        self.observe(DependencySubject::RelativeTransform(from, to))?;
        self.source.map_point(point, from, to)
    }

    fn map_rect(
        &mut self,
        rect: Rectangle,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Rectangle, RouteSceneQueryError> {
        self.observe(DependencySubject::RelativeTransform(from, to))?;
        self.source.map_rect(rect, from, to)
    }

    fn map_normal(
        &mut self,
        normal: Vec2,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Vec2, RouteSceneQueryError> {
        self.observe(DependencySubject::RelativeTransform(from, to))?;
        self.source.map_normal(normal, from, to)
    }
}

pub(crate) struct FigureTreeRouteSceneRead<'a> {
    tree: &'a FigureTree,
    anchor_geometries: &'a HashMap<(FigureId, AnchorGeometryKey), AnchorGeometry>,
    budget: crate::figure::PreparationBudgetHandle,
}

impl<'a> FigureTreeRouteSceneRead<'a> {
    pub(crate) fn new(
        tree: &'a FigureTree,
        anchor_geometries: &'a HashMap<(FigureId, AnchorGeometryKey), AnchorGeometry>,
    ) -> Self {
        Self::with_budget(
            tree,
            anchor_geometries,
            crate::figure::PreparationBudgetHandle::default(),
        )
    }

    pub(crate) fn with_budget(
        tree: &'a FigureTree,
        anchor_geometries: &'a HashMap<(FigureId, AnchorGeometryKey), AnchorGeometry>,
        budget: crate::figure::PreparationBudgetHandle,
    ) -> Self {
        Self {
            tree,
            anchor_geometries,
            budget,
        }
    }

    fn validate_figure(&self, figure: FigureId) -> Result<(), RouteSceneQueryError> {
        self.tree
            .validate_identity(figure)
            .map_err(RouteSceneQueryError::from)
    }

    fn validate_space(&self, space: CoordinateSpace) -> Result<(), RouteSceneQueryError> {
        match space {
            CoordinateSpace::LogicalSurface => Ok(()),
            CoordinateSpace::FigureLocal(figure) | CoordinateSpace::ChildContent(figure) => {
                self.validate_figure(figure)
            }
        }
    }

    fn to_surface_transform(
        &self,
        space: CoordinateSpace,
    ) -> Result<Affine2D, RouteSceneQueryError> {
        match space {
            CoordinateSpace::LogicalSurface => Ok(Affine2D::IDENTITY),
            CoordinateSpace::FigureLocal(figure) => self
                .tree
                .local_to_surface_transform(figure)
                .map_err(Into::into),
            CoordinateSpace::ChildContent(figure) => self
                .tree
                .child_content_to_surface_transform(figure)
                .map_err(Into::into),
        }
    }

    fn transform(
        &self,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Affine2D, RouteSceneQueryError> {
        self.validate_space(from)?;
        self.validate_space(to)?;
        let from_surface = self.to_surface_transform(from)?;
        let to_surface = self.to_surface_transform(to)?;
        let inverse = match to {
            CoordinateSpace::LogicalSurface => Affine2D::IDENTITY,
            CoordinateSpace::FigureLocal(figure) | CoordinateSpace::ChildContent(figure) => {
                crate::graph::checked_coordinate_inverse(figure, to_surface)?
                    .ok_or_else(|| coordinate_space_error(to))?
            }
        };
        let transform = inverse * from_surface;
        if !transform.coeffs().into_iter().all(f64::is_finite) {
            return Err(RouteSceneQueryError::NonFiniteCoordinateMap { from, to });
        }
        Ok(transform)
    }
}

impl RouteSceneRead for FigureTreeRouteSceneRead<'_> {
    fn prepared_scene(
        &self,
        owner: FigureId,
    ) -> Result<crate::figure::PreparedSceneSnapshot, RouteSceneQueryError> {
        self.tree
            .checked_prepared_scene_with_budget(owner, &self.budget)
    }

    fn is_attached(&self, figure: FigureId) -> Result<bool, RouteSceneQueryError> {
        self.validate_figure(figure)?;
        self.tree.is_attached(figure).map_err(Into::into)
    }

    fn parent_id(&self, figure: FigureId) -> Result<Option<FigureId>, RouteSceneQueryError> {
        self.validate_figure(figure)?;
        self.tree.parent_id(figure).map_err(Into::into)
    }

    fn is_visible(&self, figure: FigureId) -> Result<bool, RouteSceneQueryError> {
        self.validate_figure(figure)?;
        self.tree.is_effectively_visible(figure).map_err(Into::into)
    }

    fn is_viewport(&self, figure: FigureId) -> Result<bool, RouteSceneQueryError> {
        self.validate_figure(figure)?;
        Ok(self
            .tree
            .node(figure)?
            .figure
            .as_any()
            .is::<ViewportFigure>())
    }

    fn border_box(&self, figure: FigureId) -> Result<Rectangle, RouteSceneQueryError> {
        self.validate_figure(figure)?;
        if !self.tree.is_attached(figure)? {
            return Err(RouteSceneQueryError::DetachedFigure(figure));
        }
        let bounds = self.tree.figure_bounds(figure)?;
        if !bounds.width.is_finite() || !bounds.height.is_finite() {
            return Err(RouteSceneQueryError::NonFiniteGeometry(figure));
        }
        Ok(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))
    }

    fn anchor_geometry(
        &self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Result<AnchorGeometry, RouteSceneQueryError> {
        self.validate_figure(figure)?;
        if !self.tree.is_attached(figure)? {
            return Err(RouteSceneQueryError::DetachedFigure(figure));
        }
        if key.is_border_box() {
            return self.border_box(figure).map(AnchorGeometry::Rectangle);
        }
        self.anchor_geometries
            .get(&(figure, key.clone()))
            .cloned()
            .ok_or_else(|| RouteSceneQueryError::MissingAnchorGeometry {
                figure,
                key: key.clone(),
            })
    }

    fn map_point(
        &self,
        point: Point,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Point, RouteSceneQueryError> {
        let mapped = self.transform(from, to)?.transform_point(point);
        if !mapped.x().is_finite() || !mapped.y().is_finite() {
            return Err(RouteSceneQueryError::NonFiniteCoordinateMap { from, to });
        }
        Ok(mapped)
    }

    fn map_rect(
        &self,
        rect: Rectangle,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Rectangle, RouteSceneQueryError> {
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
        let width = max_x - min_x;
        let height = max_y - min_y;
        if !width.is_finite() || !height.is_finite() {
            return Err(RouteSceneQueryError::NonFiniteCoordinateMap { from, to });
        }
        Ok(Rectangle::new(min_x, min_y, width, height))
    }

    fn map_normal(
        &self,
        normal: Vec2,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Vec2, RouteSceneQueryError> {
        let transform = self.transform(from, to)?;
        let figure = match (from, to) {
            (CoordinateSpace::FigureLocal(figure) | CoordinateSpace::ChildContent(figure), _)
            | (_, CoordinateSpace::FigureLocal(figure) | CoordinateSpace::ChildContent(figure)) => {
                Some(figure)
            }
            _ => None,
        };
        let inverse = match figure {
            Some(figure) => crate::graph::checked_coordinate_inverse(figure, transform)?
                .ok_or(RouteSceneQueryError::NonInvertibleCoordinateMap { from, to })?,
            None => Affine2D::IDENTITY,
        };
        let [a, b, c, d, _, _] = inverse.coeffs();
        let x = a * normal.x() + b * normal.y();
        let y = c * normal.x() + d * normal.y();
        if !x.is_finite() || !y.is_finite() {
            return Err(RouteSceneQueryError::NonFiniteCoordinateMap { from, to });
        }
        Ok(Vec2::new(x, y))
    }

    fn dependency_generation(&self, subject: &DependencySubject) -> u64 {
        if self
            .budget
            .consume("route dependency generation", 1)
            .is_err()
        {
            return 0;
        }
        let mut hasher = DefaultHasher::new();
        subject.hash(&mut hasher);
        let figure_for = |space| match space {
            CoordinateSpace::FigureLocal(figure) | CoordinateSpace::ChildContent(figure) => {
                Some(figure)
            }
            CoordinateSpace::LogicalSurface => None,
        };
        let figures = match subject {
            DependencySubject::PreparedScene(figure)
            | DependencySubject::FigureMeasurement(figure)
            | DependencySubject::FigureMeasurementSource(figure)
            | DependencySubject::FigureGeometry(figure)
            | DependencySubject::FigureVisibility(figure)
            | DependencySubject::Topology(figure)
            | DependencySubject::NamedAnchorRegion(figure, _) => [Some(*figure), None],
            DependencySubject::RelativeTransform(from, to) => [figure_for(*from), figure_for(*to)],
        };
        for figure in figures.into_iter().flatten() {
            self.tree.contains(figure).hash(&mut hasher);
            if matches!(
                subject,
                DependencySubject::FigureMeasurementSource(_)
                    | DependencySubject::FigureVisibility(_)
                    | DependencySubject::RelativeTransform(..)
                    | DependencySubject::Topology(_)
            ) {
                let mut cursor = Some(figure);
                while let Some(id) = cursor {
                    if self.budget.consume("route dependency ancestry", 1).is_err() {
                        return 0;
                    }
                    cursor = self.tree.node(id).ok().and_then(|node| node.parent);
                }
            }
        }
        match subject {
            DependencySubject::PreparedScene(figure) => {
                self.tree
                    .source_checked_prepared_scene_with_budget(*figure, &self.budget)
                    .ok()
                    .map(|snapshot| snapshot.identity())
                    .hash(&mut hasher);
            }
            DependencySubject::FigureMeasurement(figure) => {
                if let Ok(node) = self.tree.node(*figure) {
                    node.layout.generation().hash(&mut hasher);
                    for value in [
                        node.insets().top,
                        node.insets().right,
                        node.insets().bottom,
                        node.insets().left,
                    ] {
                        value.to_bits().hash(&mut hasher);
                    }
                }
            }
            DependencySubject::FigureMeasurementSource(figure) => {
                if let Ok(node) = self.tree.node(*figure) {
                    node.component_revision.hash(&mut hasher);
                    node.resource_inputs.hash(&mut hasher);
                    node.layout.source_generation.hash(&mut hasher);
                    node.preparation()
                        .map(|provider| provider.revision())
                        .hash(&mut hasher);
                    node.parent.hash(&mut hasher);
                    node.children.hash(&mut hasher);
                    for size in [node.preferred_size, node.minimum_size, node.maximum_size] {
                        size.is_some().hash(&mut hasher);
                        if let Some(size) = size {
                            size.width.to_bits().hash(&mut hasher);
                            size.height.to_bits().hash(&mut hasher);
                        }
                    }
                    for value in [
                        node.source_bounds.x,
                        node.source_bounds.y,
                        node.source_bounds.width,
                        node.source_bounds.height,
                        node.insets().top,
                        node.insets().right,
                        node.insets().bottom,
                        node.insets().left,
                    ] {
                        value.to_bits().hash(&mut hasher);
                    }
                    if let Ok(style) = self.tree.resolved_style(*figure) {
                        style.font.hash(&mut hasher);
                        for value in style
                            .foreground
                            .to_rgba_array()
                            .into_iter()
                            .chain(style.background.to_rgba_array())
                        {
                            value.to_bits().hash(&mut hasher);
                        }
                        style.alpha.get().to_bits().hash(&mut hasher);
                        std::mem::discriminant(&style.cursor).hash(&mut hasher);
                        style.tooltip.hash(&mut hasher);
                    }
                }
            }
            DependencySubject::FigureGeometry(figure)
            | DependencySubject::NamedAnchorRegion(figure, _) => {
                if let Ok(bounds) = self.tree.figure_bounds(*figure) {
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
            DependencySubject::FigureVisibility(figure) => {
                self.tree.is_effectively_visible(*figure).hash(&mut hasher);
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

fn coordinate_space_error(space: CoordinateSpace) -> RouteSceneQueryError {
    match space {
        CoordinateSpace::FigureLocal(figure) | CoordinateSpace::ChildContent(figure) => {
            RouteSceneQueryError::NonInvertibleTransform(figure)
        }
        CoordinateSpace::LogicalSurface => RouteSceneQueryError::UnavailableLogicalSurface,
    }
}

/// Failure produced by a read-only scene query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteSceneQueryError {
    /// Reachability or source validation exhausted its preparation session.
    PreparationBudget(crate::figure::PreparationBudgetError),
    /// The query implementation does not expose preparation snapshots.
    UnsupportedPreparedSceneQuery(FigureId),
    /// Preparation is absent, retired or does not match the current source prefix.
    UnavailablePreparedScene(FigureId),
    /// The Figure identity belongs to another tree.
    ForeignFigure(FigureId),
    /// A checked coordinate calculation failed.
    Coordinate(crate::tree::CoordinateQueryError),
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

impl fmt::Display for RouteSceneQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreparationBudget(error) => error.fmt(formatter),
            Self::UnsupportedPreparedSceneQuery(owner) => {
                write!(formatter, "prepared scene query unsupported for {owner:?}")
            }
            Self::UnavailablePreparedScene(owner) => {
                write!(
                    formatter,
                    "prepared scene unavailable or stale for {owner:?}"
                )
            }
            Self::ForeignFigure(figure) => write!(formatter, "foreign Figure ID: {figure:?}"),
            Self::Coordinate(error) => error.fmt(formatter),
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

impl Error for RouteSceneQueryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PreparationBudget(error) => Some(error),
            Self::Coordinate(error) => Some(error),
            _ => None,
        }
    }
}

impl From<crate::tree::TreeQueryError> for RouteSceneQueryError {
    fn from(error: crate::tree::TreeQueryError) -> Self {
        match error {
            crate::tree::TreeQueryError::ForeignFigure(figure) => Self::ForeignFigure(figure),
            crate::tree::TreeQueryError::UnknownFigure(figure) => Self::UnknownFigure(figure),
        }
    }
}

impl From<crate::tree::CoordinateQueryError> for RouteSceneQueryError {
    fn from(error: crate::tree::CoordinateQueryError) -> Self {
        match error {
            crate::tree::CoordinateQueryError::Identity(error) => error.into(),
            error => Self::Coordinate(error),
        }
    }
}

#[cfg(test)]
mod s07_query_failure_tests {
    use super::*;
    use crate::figure::RectangleFigure;
    use crate::runtime::Runtime;

    #[test]
    fn adapter_rejects_identity_before_missing_anchor_or_detachment() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::from_bounds(Rectangle::ZERO)))
            .unwrap();
        let retired = tree
            .builder()
            .add_child(
                root,
                Box::new(RectangleFigure::from_bounds(Rectangle::ZERO)),
            )
            .unwrap();
        let mut other = FigureTree::new();
        let foreign = other
            .builder()
            .set_contents(Box::new(RectangleFigure::from_bounds(Rectangle::ZERO)))
            .unwrap();
        let mut runtime = Runtime::new(tree);
        runtime.figure(root).unwrap().remove(retired).unwrap();
        let geometries = HashMap::new();
        let adapter = FigureTreeRouteSceneRead::new(runtime.tree(), &geometries);
        for (id, error) in [
            (retired, RouteSceneQueryError::UnknownFigure(retired)),
            (foreign, RouteSceneQueryError::ForeignFigure(foreign)),
        ] {
            assert_eq!(adapter.is_attached(id), Err(error.clone()));
            assert_eq!(adapter.parent_id(id), Err(error.clone()));
            assert_eq!(adapter.is_visible(id), Err(error.clone()));
            assert_eq!(adapter.is_viewport(id), Err(error.clone()));
            assert_eq!(adapter.border_box(id), Err(error.clone()));
            assert_eq!(
                adapter.anchor_geometry(id, &AnchorGeometryKey::icon()),
                Err(error.clone())
            );
            assert_eq!(
                adapter.map_point(
                    Point::ORIGIN,
                    CoordinateSpace::FigureLocal(root),
                    CoordinateSpace::FigureLocal(id)
                ),
                Err(error)
            );
        }
        assert_eq!(adapter.is_attached(root), Ok(true));
        assert_eq!(adapter.is_viewport(root), Ok(false));
    }

    #[test]
    fn adapter_dependency_fingerprint_changes_when_a_subject_is_deleted() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::from_bounds(Rectangle::ZERO)))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(RectangleFigure::from_bounds(Rectangle::ZERO)),
            )
            .unwrap();
        let mut runtime = Runtime::new(tree);
        runtime.figure(child).unwrap().set_visible(false).unwrap();
        let geometries = HashMap::new();
        let subjects = [
            DependencySubject::FigureGeometry(child),
            DependencySubject::FigureVisibility(child),
            DependencySubject::Topology(child),
            DependencySubject::RelativeTransform(
                CoordinateSpace::FigureLocal(child),
                CoordinateSpace::LogicalSurface,
            ),
        ];
        let before = {
            let adapter = FigureTreeRouteSceneRead::new(runtime.tree(), &geometries);
            subjects
                .each_ref()
                .map(|subject| adapter.dependency_generation(subject))
        };
        runtime.figure(root).unwrap().remove(child).unwrap();
        let adapter = FigureTreeRouteSceneRead::new(runtime.tree(), &geometries);
        for (subject, before) in subjects.iter().zip(before) {
            assert_ne!(adapter.dependency_generation(subject), before);
        }
        let mut query = TrackedRouteSceneQuery::new(&adapter);
        assert_eq!(
            query.border_box(child),
            Err(RouteSceneQueryError::UnknownFigure(child))
        );
        assert_eq!(query.into_observations().len(), 1);
    }
}
